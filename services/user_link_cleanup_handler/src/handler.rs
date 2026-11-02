use aws_lambda_events::eventbridge::EventBridgeEvent;
use lambda_runtime::{Error, LambdaEvent};

pub async fn handler(db: sqlx::PgPool, _event: LambdaEvent<EventBridgeEvent>) -> Result<(), Error> {
    macro_db_client::in_progress_user_link::delete_day_old_in_progress_user_links(&db).await?;

    macro_db_client::in_progress_email_link::delete_day_old_in_progress_email_links(&db).await?;

    Ok(())
}
