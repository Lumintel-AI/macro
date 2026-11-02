use anyhow::{Context, Result};
use aws_lambda_events::{event::sqs::SqsEvent, sqs::SqsMessage};
use aws_sdk_s3::{Client as S3Client, primitives::ByteStream};
use connection_gateway_client::client::ConnectionGatewayClient;
use document_storage_service_client::DocumentStorageServiceClient;
use dynamodb_client::DynamodbClient;
use futures::{
    TryFutureExt,
    future::{join_all, try_join_all},
};
use lambda_runtime::{
    Error, LambdaEvent,
    tracing::{self},
};
use model::{
    document::{FileType, FileTypeExt},
    folder::{FileSystemNodeWithIds, FolderItem, S3Destination, S3DestinationMap},
};
use model_entity::EntityType;
use models_bulk_upload::{UploadDocumentStatus, UploadFolderStatus, UploadFolderStatusUpdate};
use sha2::{Digest, Sha256};
use sqs_client::upload_extractor::UploadExtractQueueMessage;
use std::ffi::OsStr;
use std::path::Path;
use std::{fs, str::FromStr};
use std::{path::PathBuf, sync::Arc};
use tempfile::tempdir;
use tokio::fs::File;
use tokio_stream::StreamExt;
use tokio_util::io::ReaderStream;
use zip::read::root_dir_common_filter;

use crate::LocalZipPath;

fn get_upload_message_from_sqs_record(record: SqsMessage) -> Result<UploadExtractQueueMessage> {
    let body = record.body.as_deref().unwrap_or_default();
    let message = serde_json::from_str::<UploadExtractQueueMessage>(body)?;
    Ok(message)
}

#[tracing::instrument(skip_all)]
pub(crate) async fn handler(
    s3_client: Arc<S3Client>,
    dss_client: Arc<DocumentStorageServiceClient>,
    dynamodb_client: Arc<DynamodbClient>,
    conn_gateway_client: Arc<ConnectionGatewayClient>,
    upload_bucket_name: &str,
    event: LambdaEvent<SqsEvent>,
) -> Result<(), Error> {
    tracing::debug!("Processing SQS event");

    for record in event.payload.records {
        let upload_request = match get_upload_message_from_sqs_record(record) {
            Ok(req) => req,
            Err(err) => {
                tracing::error!("Failed to get upload request from SQS record: {:?}", err);
                continue;
            }
        };

        tracing::debug!("Processing upload request {:?}", upload_request);

        let request_id = upload_request.upload_request_id();
        let user_id = upload_request.user_id().as_str();
        let zip_name = upload_request.name();
        let parent_id = match &upload_request {
            UploadExtractQueueMessage::ExtractZip(msg) => msg.parent_id.clone(),
        };

        let upload_status = match process_zipped_s3_object(
            &s3_client,
            &dss_client,
            &dynamodb_client,
            upload_request.key(),
            upload_bucket_name,
            request_id,
            user_id,
            zip_name,
            parent_id.as_deref(),
        )
        .await
        {
            Ok((upload_status, root_project_id)) => {
                let pid = root_project_id.as_str();
                let update_dynamodb_fut = dynamodb_client
                    .bulk_upload
                    .update_bulk_upload_request_status(
                        request_id,
                        upload_status.clone(),
                        None,
                        Some(pid),
                    );

                // A partial upload still has a usable tree; leaving it pending would hide
                // every folder because of one bad file.
                let (update_dynamodb_res, update_dss_res) = match &upload_status {
                    UploadFolderStatus::Completed | UploadFolderStatus::PartiallyCompleted => {
                        tokio::join!(
                            update_dynamodb_fut,
                            dss_client
                                .mark_projects_uploaded(user_id, pid)
                                .map_ok(|_| ())
                        )
                    }
                    _ => tokio::join!(update_dynamodb_fut, futures::future::ok(())),
                };

                update_dynamodb_res
                    .inspect_err(|e| tracing::error!("Failed to update dynamo status: {:?}", e))
                    .ok();

                match (update_dss_res, &upload_status) {
                    (Ok(_), UploadFolderStatus::Completed) => UploadFolderStatusUpdate::Completed {
                        request_id: request_id.clone(),
                        project_id: root_project_id.clone(),
                    },
                    (Ok(_), UploadFolderStatus::PartiallyCompleted) => {
                        UploadFolderStatusUpdate::PartiallyCompleted {
                            request_id: request_id.clone(),
                            project_id: root_project_id.clone(),
                        }
                    }
                    (Ok(_), _) => UploadFolderStatusUpdate::Failed {
                        request_id: request_id.clone(),
                    },
                    (Err(e), _) => {
                        tracing::error!("Failed to update dss status: {:?}", e);
                        UploadFolderStatusUpdate::Unknown {
                            request_id: request_id.clone(),
                        }
                    }
                }
            }
            Err(err) => {
                tracing::error!(
                    "Failed to process upload request {:?}: {:?}",
                    upload_request,
                    err
                );
                dynamodb_client
                    .bulk_upload
                    .update_bulk_upload_request_status(
                        request_id,
                        UploadFolderStatus::Failed,
                        Some(&err.to_string()),
                        None,
                    )
                    .await
                    .inspect_err(|e| tracing::error!("Failed to update dynamo status: {:?}", e))
                    .ok();

                UploadFolderStatusUpdate::Failed {
                    request_id: request_id.clone(),
                }
            }
        };

        update_bulk_upload_request_state(conn_gateway_client.as_ref(), user_id, upload_status)
            .await;
    }

    Ok(())
}

#[tracing::instrument(skip_all)]
async fn update_bulk_upload_request_state(
    client: &ConnectionGatewayClient,
    user_id: &str,
    message: UploadFolderStatusUpdate,
) -> () {
    if cfg!(feature = "local") {
        tracing::info!("bypassing connection gateway");
    } else {
        let entities = vec![EntityType::User.with_entity_str(user_id)];

        match serde_json::to_value(message) {
            Ok(message) => {
                client
                    .batch_send_message("bulk_upload".to_string(), message, entities)
                    .await
                    .inspect_err(|e| {
                        tracing::error!(error = ?e, "failed to send message to connection gateway");
                    })
                    .ok();
            }
            Err(e) => {
                tracing::error!(error = ?e, "failed to serialize message");
            }
        }
    }
}

#[tracing::instrument(skip_all, fields(request_id=request_id))]
#[expect(clippy::too_many_arguments, reason = "too annoying to fix")]
async fn process_zipped_s3_object(
    s3_client: &S3Client,
    dss_client: &DocumentStorageServiceClient,
    dynamodb_client: &DynamodbClient,
    object_key: &str,
    staging_bucket: &str,
    request_id: &str,
    user_id: &str,
    zip_name: Option<&str>,
    parent_id: Option<&str>,
) -> Result<(UploadFolderStatus, String), Error> {
    tracing::debug!("Processing object: {}", object_key);

    dynamodb_client
        .bulk_upload
        .update_bulk_upload_request_status(request_id, UploadFolderStatus::Processing, None, None)
        .await?;

    // Create a temporary directory to extract files
    // automatically cleaned up when the process exits
    let temp_dir = tempdir()?;
    let zip_path = temp_dir.path().join("upload.zip");

    tracing::debug!("Downloading zip file to {:?}", zip_path);

    if cfg!(feature = "local") {
        tracing::info!("using local zip file, remove local feature to download s3 object");
        // Update this path to your zip file location
        let file_path = LocalZipPath::new().context("LOCAL_ZIP_PATH must be set")?;
        tracing::info!("Loading local zip file from: {}", file_path.as_ref());
        let file = File::open(file_path.as_ref()).await?;
        let mut body = tokio::io::BufReader::new(file);

        // Write the file to disk
        tokio::io::copy(&mut body, &mut tokio::fs::File::create(&zip_path).await?).await?;
    } else {
        let obj = s3_client
            .get_object()
            .bucket(staging_bucket)
            .key(object_key)
            .send()
            .await?;

        let mut body = obj.body.into_async_read();

        // Write the file to disk
        tokio::io::copy(&mut body, &mut tokio::fs::File::create(&zip_path).await?).await?;
    }

    // Extract zip file
    let extract_dir = temp_dir.path().join("extracted");
    fs::create_dir_all(&extract_dir)?;

    tracing::debug!("Extracting zip file to {:?}", extract_dir);
    let root_folder_name = extract_zip_file(&zip_path, &extract_dir, zip_name)?;

    // Build file system structure
    tracing::debug!(
        "Building file system structure for folder: {}",
        root_folder_name
    );
    let ExtractedTree {
        items: folder_items,
        folders,
    } = build_file_system(&extract_dir).await?;
    tracing::debug!("Upload file system: {:?} {:?}", folder_items, folders);

    let upload_result = dss_client
        .upload_unnested_folder(
            user_id.to_string(),
            root_folder_name,
            folder_items,
            folders,
            request_id.to_string(),
            parent_id.map(|s| s.to_string()),
        )
        .await?;

    let file_system = upload_result.file_system;
    let root_project_id = file_system
        .get_project_id()
        .ok_or_else(|| anyhow::anyhow!("No project found for root folder"))?;

    tracing::debug!("returned file system: {:?}", file_system);
    tracing::debug!("root project id: {:?}", root_project_id);

    let destination_map = upload_result.destination_map;

    let request_status = move_files_to_cloud_storage_bucket(
        s3_client,
        dynamodb_client,
        request_id,
        &extract_dir,
        &file_system,
        &destination_map,
    )
    .await?;
    tracing::debug!("Request status: {:?}", request_status);

    return Ok((request_status, root_project_id.clone()));
}

#[tracing::instrument(skip_all, fields(request_id=request_id))]
async fn move_files_to_cloud_storage_bucket(
    s3_client: &S3Client,
    dynamodb_client: &DynamodbClient,
    request_id: &str,
    extract_dir: &Path,
    file_system: &FileSystemNodeWithIds,
    destination_map: &S3DestinationMap,
) -> Result<UploadFolderStatus, Error> {
    let folder_items = file_system.get_folder_items();

    let document_ids: Vec<String> = folder_items.keys().cloned().collect();
    let document_count = document_ids.len();
    if document_count == 0 {
        tracing::error!("No documents found in file system {}", request_id);
        return Ok(UploadFolderStatus::Completed);
    }

    dynamodb_client
        .bulk_upload
        .bulk_update_file_status(
            request_id,
            document_ids.clone(),
            UploadDocumentStatus::Pending,
        )
        .await?;

    let mut upload_tasks = Vec::new();

    for (document_id, item) in folder_items {
        let destination = match destination_map.get(&document_id) {
            Some(dest) => dest.clone(),
            None => {
                tracing::error!("No destination found for {}", document_id);
                continue;
            }
        };

        let s3_client = s3_client.clone();
        let extract_dir = extract_dir.to_path_buf();

        let fut = async move {
            match destination {
                S3Destination::External(_) => {
                    let error_message = format!(
                        "External destination not implemented for file {}",
                        document_id
                    );
                    (document_id, false, Some(error_message))
                }
                S3Destination::Internal(s3_info) => {
                    let path = get_file_path_for_item(&extract_dir, &item);

                    if !path.is_file() {
                        let error_message = format!("Not a file: {}", path.display());
                        return (document_id, false, Some(error_message));
                    }

                    let byte_stream = match ByteStream::from_path(&path).await {
                        Ok(bs) => bs,
                        Err(e) => {
                            let error_message = format!(
                                "Failed to read file for upload: {:?} {} {} {} {:?}",
                                e,
                                document_id,
                                s3_info.bucket,
                                s3_info.key,
                                path.display()
                            );
                            return (document_id, false, Some(error_message));
                        }
                    };

                    match s3_client
                        .put_object()
                        .bucket(&s3_info.bucket)
                        .key(&s3_info.key)
                        .body(byte_stream)
                        .send()
                        .await
                    {
                        Ok(_) => (document_id, true, None),
                        Err(e) => {
                            let error_message =
                                format!("Failed to upload {}: {:?}", document_id, e);
                            (document_id, false, Some(error_message))
                        }
                    }
                }
            }
        };

        upload_tasks.push(fut);
    }

    let results = join_all(upload_tasks).await;

    let mut success_ids = Vec::new();
    let mut failed_ids = Vec::new();

    for result in results {
        let (document_id, success, error_message) = result;
        match success {
            true => success_ids.push(document_id),
            false => {
                match error_message {
                    Some(error_message) => tracing::error!(error_message),
                    None => tracing::error!("Failed to upload {}", document_id),
                }
                failed_ids.push(document_id)
            }
        };
    }

    let success_count = success_ids.len();
    let failed_count = failed_ids.len();

    match (success_count == 0, failed_count == 0) {
        (true, true) => Ok(UploadFolderStatus::Completed),
        (false, true) => {
            dynamodb_client
                .bulk_upload
                .bulk_update_file_status(request_id, success_ids, UploadDocumentStatus::Completed)
                .await?;
            Ok(UploadFolderStatus::Completed)
        }
        (true, false) => {
            dynamodb_client
                .bulk_upload
                .bulk_update_file_status(request_id, failed_ids, UploadDocumentStatus::Failed)
                .await?;
            Ok(UploadFolderStatus::Failed)
        }
        (false, false) => {
            let (success_res, failed_res) = tokio::join!(
                dynamodb_client.bulk_upload.bulk_update_file_status(
                    request_id,
                    success_ids,
                    UploadDocumentStatus::Completed
                ),
                dynamodb_client.bulk_upload.bulk_update_file_status(
                    request_id,
                    failed_ids,
                    UploadDocumentStatus::Failed
                )
            );

            if let Err(e) = success_res {
                tracing::error!("Failed to update completed statuses: {:?}", e);
            }
            if let Err(e) = failed_res {
                tracing::error!("Failed to update failed statuses: {:?}", e);
            }

            Ok(UploadFolderStatus::PartiallyCompleted)
        }
    }
}

// returns the root folder name
fn extract_zip_file(
    zip_path: &Path,
    extract_dir: &Path,
    zip_name: Option<&str>,
) -> Result<String, Error> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let extract_to: PathBuf;
    let root_dir_name = match archive
        .root_dir(root_dir_common_filter)?
        .and_then(|root| root.file_name().map(|f| f.to_string_lossy().to_string()))
    {
        Some(root_dir_name) => {
            extract_to = extract_dir.join(&root_dir_name);
            root_dir_name
        }
        None => {
            tracing::error!("No root directory found, using provided root folder name");
            let extract_folder = match zip_name {
                Some(zip_name) => zip_name.to_string(),
                None => {
                    let default_extract_folder = "Folder Upload".to_string();
                    tracing::error!(
                        "No zip name provided, using default extract folder {}",
                        default_extract_folder
                    );
                    default_extract_folder
                }
            };
            extract_to = extract_dir.join(&extract_folder);
            extract_folder
        }
    };
    fs::create_dir_all(&extract_to)?;

    tracing::debug!("Extracting to: {:?}", extract_to);
    archive.extract_unwrapped_root_dir(&extract_to, root_dir_common_filter)?;

    Ok(root_dir_name)
}

/// Files and folders collected from an extracted upload.
#[derive(Debug, Default)]
struct ExtractedTree {
    items: Vec<FolderItem>,
    /// Every kept folder, as a path relative to the extract dir, so empty
    /// folders still become projects.
    folders: Vec<String>,
}

/// OS and editor clutter that should never become a document or project.
fn is_ignored_name(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    let lower = name.to_ascii_lowercase();
    // Hidden files/folders, including `.DS_Store`, `._*` and `.~lock.*#`.
    name.starts_with('.')
        || name == "__MACOSX"
        // Office owner/lock files written next to open documents.
        || name.starts_with("~$")
        || lower == "thumbs.db"
        || lower == "ehthumbs.db"
        || lower == "desktop.ini"
        // macOS custom folder icon file.
        || name == "Icon\r"
}

/// Whether a path under the extract dir should be skipped. The first component
/// is the root folder the user picked, which is kept whatever its name.
fn is_ignored_entry(relative_path: &Path) -> bool {
    relative_path.iter().skip(1).any(is_ignored_name)
}

async fn build_file_system(extract_dir: &Path) -> Result<ExtractedTree, Error> {
    let mut tasks = Vec::new();
    let mut folders = Vec::new();

    let walker = walkdir::WalkDir::new(extract_dir)
        .min_depth(1)
        .sort_by_file_name()
        .into_iter()
        // Pruning here also skips everything inside an ignored folder.
        .filter_entry(|entry| {
            entry
                .path()
                .strip_prefix(extract_dir)
                .map(|relative| !is_ignored_entry(relative))
                .unwrap_or(false)
        });

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path().to_path_buf();
        let relative = path
            .strip_prefix(extract_dir)
            .map_err(|e| Error::from(format!("strip_prefix failed: {:?}", e)))?
            .to_path_buf();

        if entry.file_type().is_dir() {
            let folder = relative
                .to_str()
                .ok_or_else(|| Error::from(format!("Invalid folder name: {:?}", relative)))?;
            folders.push(folder.to_string());
            continue;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let fut = async move {
            let full_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| Error::from(format!("Invalid file name: {:?}", path)))?;

            let (file_name, file_type) = match FileType::split_suffix_match(full_name) {
                Some((file, extension)) => {
                    let file_type = FileType::from_str(extension).ok();
                    (file, file_type)
                }
                None => (full_name, None),
            };

            let relative_path_without_filename = relative
                .parent()
                .ok_or_else(|| Error::from(format!("No parent for path: {:?}", path)))?
                .to_string_lossy()
                .to_string();

            let sha = compute_sha256(&path).await?;

            Ok::<_, Error>(FolderItem {
                name: file_name.to_string(),
                full_name: full_name.to_string(),
                file_type,
                relative_path: relative_path_without_filename,
                sha,
            })
        };

        tasks.push(fut);
    }

    // exits immediately if any task fails
    let items = try_join_all(tasks).await?;
    Ok(ExtractedTree { items, folders })
}

async fn compute_sha256(path: &Path) -> Result<String, Error> {
    let file = File::open(path).await?;
    let mut stream = ReaderStream::new(file);
    let mut hasher = Sha256::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        hasher.update(&bytes);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result)) // hex string
}

/// `full_name` is the name on disk; rebuilding it from the parsed type would
/// lowercase extensions like `.DOCX` and miss the file.
fn get_file_path_for_item(extract_dir: &Path, item: &FolderItem) -> PathBuf {
    extract_dir.join(&item.relative_path).join(&item.full_name)
}

#[cfg(test)]
mod test;
