use lambda_runtime::tracing::{self};
use sqlx::{Pool, Postgres};

use crate::models::{DocxUploadJobData, DocxUploadJobResult};

/// Sends failed response to job if present
#[tracing::instrument(skip(db, job_updates), err)]
pub async fn handle_docx_unzip_failure(
    db: Pool<Postgres>,
    job_updates: &impl lambda_client::job_update::JobUpdateSender,
    document_id: &str,
) -> anyhow::Result<()> {
    let result = macro_db_client::docx_unzip::get_job_for_docx_upload(&db, document_id).await?;

    if let Some((job_id, job_type)) = result {
        tracing::trace!(document_id=?document_id, "docx unzip failed");
        job_updates
            .send(
                document_id,
                &DocxUploadJobResult {
                    job_id: job_id.into(),
                    status: "Failed".into(),
                    job_type: job_type.into(),
                    data: DocxUploadJobData {
                        error: true,
                        message: Some("error unzipping document".to_string()),
                        data: None,
                    },
                },
            )
            .await?;
    }

    Ok(())
}
