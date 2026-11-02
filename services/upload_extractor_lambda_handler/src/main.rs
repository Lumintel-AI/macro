mod handler;
use anyhow::Context;
use aws_lambda_events::sqs::SqsEvent;
use aws_sdk_s3::Client as S3Client;
use connection_gateway_client::client::ConnectionGatewayClient;
use document_storage_service_client::DocumentStorageServiceClient;
use dynamodb_client::DynamodbClient;
use handler::handler;
use lambda_runtime::{Error, LambdaEvent, run, service_fn};
use macro_entrypoint::MacroEntrypoint;
use macro_env_var::env_vars;
use macro_service_urls::ConnectionGatewayUrl;
use std::sync::Arc;

env_vars! {
    struct LocalZipPath;
    struct UploadBucketName;
    struct InternalApiSecretKey;
    struct DssUrl;
    struct DynamodbTable;
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    MacroEntrypoint::default().init();
    tracing::trace!("initiating lambda");

    let upload_staging_bucket = UploadBucketName::new()
        .context("UPLOAD_BUCKET_NAME must be set")?
        .to_string();
    let internal_api_secret_key = InternalApiSecretKey::new()
        .context("INTERNAL_API_SECRET_KEY must be set")?
        .to_string();
    let dss_url = DssUrl::new().context("DSS_URL must be set")?.to_string();
    let dynamo_table_name = DynamodbTable::new()
        .context("DYNAMODB_TABLE must be set")?
        .to_string();
    let connection_gateway_url = ConnectionGatewayUrl::new()?.to_string();

    let config = macro_aws_config::get_macro_aws_config().await;
    let s3_client = S3Client::new(&config);

    let dss_client = DocumentStorageServiceClient::new(internal_api_secret_key.clone(), dss_url);

    let dynamodb_client = DynamodbClient::new(&config, Some(dynamo_table_name));

    let conn_gateway_client =
        ConnectionGatewayClient::new(internal_api_secret_key, connection_gateway_url);

    let shared_s3_client = Arc::new(s3_client);
    let shared_dss_client = Arc::new(dss_client);
    let shared_dynamodb_client = Arc::new(dynamodb_client);
    let shared_conn_gateway_client = Arc::new(conn_gateway_client);
    let upload_staging_bucket_str = upload_staging_bucket.as_str();

    let func = service_fn(move |event: LambdaEvent<SqsEvent>| {
        handler(
            shared_s3_client.clone(),
            shared_dss_client.clone(),
            shared_dynamodb_client.clone(),
            shared_conn_gateway_client.clone(),
            upload_staging_bucket_str,
            event,
        )
    });

    run(func).await
}
