//! Status messages for document processing jobs.

use super::ConnectionGatewayClient;
use connection_gateway_models::MessageReceipt;
use model_entity::EntityType;

impl ConnectionGatewayClient {
    /// POST the job result unchanged to the document's subscribers.
    #[tracing::instrument(name = "connection_gateway.job_update", skip(self, result), err)]
    pub async fn send_job_update(
        &self,
        document_id: &str,
        result: serde_json::Value,
    ) -> anyhow::Result<Vec<MessageReceipt>> {
        anyhow::ensure!(
            !document_id.is_empty(),
            "a job update requires a document id"
        );
        self.send_message(
            EntityType::Document.with_entity_str(document_id),
            "job_update".to_owned(),
            result,
        )
        .await
    }
}

#[cfg(test)]
mod test;
