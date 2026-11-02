mod config;
mod context;
mod handler;

use anyhow::Context;
use aws_lambda_events::event::eventbridge::EventBridgeEvent;
use config::Config;
use handler::handler;
use lambda_runtime::{Error, LambdaEvent};
use macro_entrypoint::MacroEntrypoint;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let telemetry = MacroEntrypoint::default().init();

    let config = Config::from_env().context("all necessary env vars should be available")?;

    // should only need a single connection to fetch the list of emails
    let db = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(1)
        .connect(&config.database_url)
        .await
        .context("could not connect to db")?;

    let link_manager_queue = macro_queues::LinkManagerQueue::new();
    let sqs_client = sqs_client::SQS::new(aws_sdk_sqs::Client::new(
        &macro_aws_config::get_macro_aws_config().await,
    ))
    .email_link_manager_queue(&link_manager_queue);

    let ctx = context::Context {
        db,
        sqs_client: Arc::new(sqs_client),
        config,
    };

    let func = move |event: LambdaEvent<EventBridgeEvent>| {
        let ctx = ctx.clone();

        async move { handler(ctx, event).await }
    };

    let result =
        sqs_worker::standalone::run_once(func, sqs_worker::standalone::scheduled_event()).await;
    telemetry.shutdown();
    result
}
