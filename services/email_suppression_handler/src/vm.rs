mod handler;
mod model;

use handler::handler;

use anyhow::Context;
use aws_lambda_events::sns::SnsEvent;
use lambda_runtime::{Error, LambdaEvent, tracing};
use macro_entrypoint::MacroEntrypoint;
use macro_env_var::env_vars;
use sqlx::postgres::PgPoolOptions;

env_vars! {
    struct DatabaseUrl;
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let telemetry = MacroEntrypoint::default().init();

    tracing::trace!("initiating lambda");

    tracing::trace!("initialized config");
    let database_url = DatabaseUrl::new().context("DATABASE_URL must be provided")?;

    let db = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(1) // we only ever need one connection per lambda
        .connect(database_url.as_ref())
        .await
        .context("could not connect to db")?;

    tracing::trace!("initialized db client");

    let func = move |event: LambdaEvent<SnsEvent>| {
        let db = db.clone();
        async move { handler(db, event).await }
    };

    let result =
        sqs_worker::standalone::run_queue(func, sqs_worker::standalone::EventFormat::Sns).await;
    telemetry.shutdown();
    result
}
