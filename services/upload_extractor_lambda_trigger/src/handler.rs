use aws_lambda_events::eventbridge::EventBridgeEvent;
use dynamodb_client::DynamodbClient;
use lambda_runtime::{Error, LambdaEvent};
use models_bulk_upload::UploadFolderStatus;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// see: https://docs.aws.amazon.com/AmazonS3/latest/userguide/ev-events.html
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct S3CreateObjectDetail {
    bucket: Bucket,
    object: Object,
}

#[derive(Debug, Serialize, Deserialize)]
struct Bucket {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Object {
    key: String,
}

#[tracing::instrument(skip_all)]
pub(crate) async fn handler(
    dynamodb_client: Arc<DynamodbClient>,
    sqs_client: Arc<sqs_client::SQS>,
    event: LambdaEvent<EventBridgeEvent<S3CreateObjectDetail>>,
) -> Result<(), Error> {
    let key = event.payload.detail.object.key;

    let staging_key = s3_key::BulkUploadStagingKey::from_s3_key(&key).map_err(|e| {
        tracing::warn!(error=?e, key=%key, "skipping non-extract key");
        Error::from("key is not a bulk upload staging key")
    })?;

    let upload_request_id = staging_key.request_id.as_str();

    tracing::info!("Processing request: {}", upload_request_id);

    let bulk_upload_request = dynamodb_client
        .bulk_upload
        .get_bulk_upload_request(upload_request_id)
        .await
        .inspect_err(|e| tracing::error!("Failed to get request info: {:?}", e))?;

    tracing::info!(
        "Received request info from dynamodb {:?}",
        bulk_upload_request
    );

    dynamodb_client
        .bulk_upload
        .update_bulk_upload_request_status(
            &bulk_upload_request.request_id,
            UploadFolderStatus::Uploaded,
            None,
            None,
        )
        .await
        .inspect_err(|e| tracing::error!("Failed to update status: {:?}", e))?;

    tracing::info!("Updated upload status for request {}", upload_request_id);

    sqs_client
        .enqueue_upload_extractor_unzip(
            upload_request_id,
            &staging_key.to_key(),
            bulk_upload_request.user_id.as_str(),
            &bulk_upload_request.name,
            &bulk_upload_request.parent_id,
        )
        .await
        .inspect_err(|e| tracing::error!("Failed to enqueue job: {:?}", e))?;

    tracing::info!("Finished processing request {}", upload_request_id);

    Ok(())
}
