use crate::service;
use aws_lambda_events::eventbridge::EventBridgeEvent;
use lambda_runtime::{Error, LambdaEvent};
use std::sync::Arc;

#[tracing::instrument(skip(db, sqs_client, event))]
pub(crate) async fn handler(
    db: Arc<service::db::DB>,
    sqs_client: Arc<sqs_client::SQS>,
    event: LambdaEvent<EventBridgeEvent<serde_json::Value>>,
) -> Result<(), Error> {
    tracing::trace!("processing event {:?}", event.payload.id);

    let organizations = db.get_organization_retention().await?;

    sqs_client
        .bulk_enqueue_organization_retention(organizations)
        .await?;

    Ok(())
}
