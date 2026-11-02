//! Document-job status delivery through Lambda or the connection gateway.

use anyhow::{Context, bail};
use connection_gateway_client::ConnectionGatewayClient;
use macro_env_var::maybe_env_vars;
use std::sync::Arc;

use crate::Lambda;

maybe_env_vars! {
    struct JobUpdateTransport;
    struct WebSocketResponseLambda;
    struct InternalApiKey;
}

/// The delivery capability conversion and unzip jobs require.
pub trait JobUpdateSender: Send + Sync {
    /// Deliver the original job result to its document's subscribers.
    fn send<T: serde::Serialize + std::fmt::Debug + Sync>(
        &self,
        document_id: &str,
        result: &T,
    ) -> impl Future<Output = anyhow::Result<()>> + Send;
}

/// Validated callback configuration, loaded once at startup.
#[derive(Clone, Debug)]
pub enum JobUpdateConfig {
    /// The upstream asynchronous callback.
    Lambda { function_name: String },
    /// The owned connection gateway, authenticated with the fleet key.
    Gateway(Arc<ConnectionGatewayClient>),
}

impl JobUpdateConfig {
    /// Select `lambda` by default, or `connection_gateway` without any Lambda ARN.
    pub fn from_env() -> anyhow::Result<Self> {
        let transport = JobUpdateTransport::new();
        match transport
            .as_ref()
            .and_then(|value| value.value())
            .unwrap_or("lambda")
        {
            "lambda" => {
                let function = WebSocketResponseLambda::new();
                let function_name = function
                    .as_ref()
                    .and_then(|value| value.value())
                    .filter(|value| !value.trim().is_empty())
                    .context(
                        "WEB_SOCKET_RESPONSE_LAMBDA is required for JOB_UPDATE_TRANSPORT=lambda",
                    )?;
                Ok(Self::Lambda {
                    function_name: function_name.to_owned(),
                })
            }
            "connection_gateway" => {
                let key = InternalApiKey::new();
                let key = key
                    .as_ref()
                    .and_then(|value| value.value())
                    .filter(|value| !value.trim().is_empty())
                    .context(
                        "INTERNAL_API_KEY is required for JOB_UPDATE_TRANSPORT=connection_gateway",
                    )?;
                // Validate the header before the legacy client's constructor.
                anyhow::ensure!(
                    !key.bytes().any(|byte| byte < 32 || byte >= 127),
                    "INTERNAL_API_KEY must be an HTTP header value"
                );
                Ok(Self::Gateway(Arc::new(ConnectionGatewayClient::new(
                    key.to_owned(),
                    macro_service_urls::ConnectionGatewayUrl::new()?.to_string(),
                ))))
            }
            _ => bail!("JOB_UPDATE_TRANSPORT must be lambda or connection_gateway"),
        }
    }

    /// Bind the existing Lambda client only on the upstream delivery path.
    pub fn sender(&self, lambda: &Lambda) -> JobUpdateDelivery {
        match self {
            Self::Lambda { function_name } => JobUpdateDelivery::Lambda {
                client: lambda.clone(),
                function_name: function_name.clone(),
            },
            Self::Gateway(client) => JobUpdateDelivery::Gateway(client.clone()),
        }
    }
}

/// Outbound adapters for the job-update port.
#[derive(Clone)]
pub enum JobUpdateDelivery {
    /// Preserve upstream asynchronous invocation semantics.
    Lambda {
        client: Lambda,
        function_name: String,
    },
    /// POST status through the connection gateway.
    Gateway(Arc<ConnectionGatewayClient>),
}

impl JobUpdateSender for JobUpdateDelivery {
    #[tracing::instrument(
        name = "document_job.deliver_status",
        skip_all,
        fields(document_id),
        err
    )]
    async fn send<T: serde::Serialize + std::fmt::Debug + Sync>(
        &self,
        document_id: &str,
        result: &T,
    ) -> anyhow::Result<()> {
        match self {
            Self::Lambda {
                client,
                function_name,
            } => client.invoke_event(function_name, result).await,
            Self::Gateway(client) => {
                client
                    .send_job_update(document_id, serde_json::to_value(result)?)
                    .await?;
                Ok(())
            }
        }
    }
}
