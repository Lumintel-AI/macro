mod handler;
use handler::handler;
use std::sync::Arc;

mod service;

use anyhow::Context;
use aws_lambda_events::event::eventbridge::EventBridgeEvent;
use lambda_runtime::{
    Error, LambdaEvent,
    tracing::{self},
};
use macro_entrypoint::MacroEntrypoint;
use macro_env_var::env_vars;
use sqlx::postgres::PgPoolOptions;

env_vars! {
    struct DatabaseUrl;
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let telemetry = MacroEntrypoint::default().init();

    tracing::info!("initiating lambda");

    let database_url = DatabaseUrl::new().context("DATABASE_URL must be set")?;
    let db = service::db::DB::new(
        PgPoolOptions::new()
            .min_connections(1)
            .max_connections(1) // we only ever need one connection per lambda
            .connect(database_url.as_ref())
            .await
            .context("could not connect to db")?,
    );

    tracing::trace!("initialized db client");

    let organization_retention_queue = macro_queues::OrganizationRetentionQueue::new();
    let sqs_client = sqs_client::SQS::new(aws_sdk_sqs::Client::new(
        &macro_aws_config::get_macro_aws_config().await,
    ))
    .organization_retention_queue(organization_retention_queue.as_ref());

    tracing::trace!("initialized ecs client");

    // Shared references
    let shared_sqs_client = Arc::new(sqs_client);
    let shared_db = Arc::new(db);

    let func = move |event: LambdaEvent<EventBridgeEvent<serde_json::Value>>| {
        let sqs_client = shared_sqs_client.clone();
        let db = shared_db.clone();
        async move { handler(db, sqs_client, event).await }
    };

    let result =
        sqs_worker::standalone::run_once(func, sqs_worker::standalone::scheduled_event()).await;
    telemetry.shutdown();
    result
}
