mod handler;
use handler::S3CreateObjectDetail;
use handler::handler;
use std::sync::Arc;

use anyhow::Context;
use aws_lambda_events::event::eventbridge::EventBridgeEvent;
use dynamodb_client::DynamodbClient;
use lambda_runtime::{
    Error, LambdaEvent, run, service_fn,
    tracing::{self},
};
use macro_entrypoint::MacroEntrypoint;
use macro_env_var::env_vars;

env_vars! {
    pub struct DynamodbTable;
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    MacroEntrypoint::default().init();
    tracing::info!("initiating lambda");

    let dynamo_table_name = DynamodbTable::new().context("DYNAMODB_TABLE must be set")?;
    let upload_extract_queue = macro_queues::UploadExtractorQueue::new();

    tracing::trace!("initialized env vars");

    let config = macro_aws_config::get_macro_aws_config().await;
    let dynamodb_client = DynamodbClient::new(&config, Some(dynamo_table_name.to_string()));

    tracing::trace!("initialized dynamodb client");

    let sqs_client = sqs_client::SQS::new(aws_sdk_sqs::Client::new(&config))
        .upload_extractor_queue(&upload_extract_queue);

    tracing::trace!("initialized sqs client");

    // Shared references
    let shared_sqs_client = Arc::new(sqs_client);
    let shared_dynamodb_client = Arc::new(dynamodb_client);

    let func = service_fn(
        move |event: LambdaEvent<EventBridgeEvent<S3CreateObjectDetail>>| {
            let sqs_client = shared_sqs_client.clone();
            let dynamodb_client = shared_dynamodb_client.clone();
            async move { handler(dynamodb_client, sqs_client, event).await }
        },
    );

    run(func).await
}
