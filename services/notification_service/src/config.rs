//! Configuration for the notification service, loaded via the standard
//! `macro_config` pattern so it gets a `doppler_config` validation binary.
//!
//! Required env vars are declared here as typed fields. The `doppler_config`
//! binary loads this `Config` from Doppler for both the dev and prod
//! environments, surfacing any missing or mistyped values at CI time.

use anyhow::Context;
use database_env_vars::{DatabaseUrl, RedisUri};
use macro_auth::InternalApiKey;
use macro_env::Environment;
use macro_env_var::{env_var, env_vars, maybe_env_vars};

env_vars! {
    #[derive(Debug, Clone)]
    pub(crate) struct SenderBaseAddress;
    #[derive(Debug, Clone)]
    pub(crate) struct LastOnlineRedisUri;
    /// Comma-separated Kafka bootstrap servers for WebSocket notification delivery.
    #[derive(Debug, Clone)]
    pub(crate) struct KafkaBrokers;
}

maybe_env_vars! {
    #[derive(Debug, Clone)]
    pub(crate) struct AppleBundleId;
    #[derive(Debug, Clone)]
    pub(crate) struct SnsApnsPlatformArn;
    #[derive(Debug, Clone)]
    pub(crate) struct SnsFcmPlatformArn;

    #[derive(Debug, Clone)]
    pub(crate) struct SnsApnsVoipPlatformArn;
}

env_var!(
    #[derive(Debug, Clone)]
    pub(crate) struct UrlSigningHmac;
);

/// The configuration parameters for the application.
///
/// These are loaded from `APP_SECRETS_JSON` when present, otherwise from environment variables.
#[derive(macro_config::MacroConfig)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct Config {
    /// The connection URL for the Postgres database this application should use.
    pub(crate) database_url: DatabaseUrl,

    /// Internal API key.
    pub(crate) internal_api_key: InternalApiKey,

    /// Secret name/value for digest unsubscribe URL signing.
    pub(crate) url_signing_hmac: UrlSigningHmac,

    /// The port to listen for HTTP requests on.
    #[macro_config_default(8080)]
    pub(crate) port: usize,

    /// The environment we are in.
    #[macro_config_default(Environment::new_or_prod())]
    pub(crate) environment: Environment,

    /// The notification queue max messages per poll.
    #[macro_config_default(9)]
    pub(crate) notification_queue_max_messages: i32,

    /// The notification queue wait time seconds.
    #[macro_config_default(4)]
    pub(crate) notification_queue_wait_time_seconds: i32,

    /// Redis used by notification-service for digest batching, rate limiting, etc.
    pub(crate) redis_uri: RedisUri,

    /// Redis used by connection-gateway for last-online state.
    pub(crate) last_online_redis_uri: LastOnlineRedisUri,

    /// Comma-separated Kafka bootstrap servers for WebSocket notification delivery.
    pub(crate) kafka_brokers: KafkaBrokers,

    /// Apple app bundle id. Omit all four mobile settings to disable mobile push.
    pub(crate) apple_bundle_id: AppleBundleId,

    /// The SNS iOS platform ARN.
    pub(crate) sns_apns_platform_arn: SnsApnsPlatformArn,

    /// The SNS Android platform ARN.
    pub(crate) sns_fcm_platform_arn: SnsFcmPlatformArn,

    /// The SNS iOS VoIP platform ARN (APNS_VOIP). Optional when mobile push is unconfigured.
    pub(crate) sns_apns_voip_platform_arn: SnsApnsVoipPlatformArn,
}

/// Mobile push settings validated together before connecting to external services.
pub(crate) struct MobilePushConfig {
    pub(crate) bundle_id: String,
    pub(crate) platforms: notification::domain::service::PlatformArnConfig,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let config = macro_config::ConfigLoader::load::<Config>()
            .context("failed to load notification service config")?;
        config.mobile_push_config()?;
        Ok(config)
    }

    pub(crate) fn mobile_push_config(&self) -> anyhow::Result<Option<MobilePushConfig>> {
        let settings = [
            ("APPLE_BUNDLE_ID", self.apple_bundle_id.value()),
            ("SNS_APNS_PLATFORM_ARN", self.sns_apns_platform_arn.value()),
            ("SNS_FCM_PLATFORM_ARN", self.sns_fcm_platform_arn.value()),
            (
                "SNS_APNS_VOIP_PLATFORM_ARN",
                self.sns_apns_voip_platform_arn.value(),
            ),
        ];
        let values = settings.map(|(_, value)| value.filter(|value| !value.trim().is_empty()));
        if values.iter().all(Option::is_none) {
            return Ok(None);
        }
        match values {
            [Some(bundle_id), Some(apns), Some(fcm), Some(voip)] => Ok(Some(MobilePushConfig {
                bundle_id: bundle_id.to_owned(),
                platforms: notification::domain::service::PlatformArnConfig {
                    apns_platform_arn: apns.to_owned(),
                    fcm_platform_arn: fcm.to_owned(),
                    apns_voip_platform_arn: voip.to_owned(),
                },
            })),
            _ => {
                let missing = settings
                    .into_iter()
                    .zip(values)
                    .filter_map(|((name, _), value)| value.is_none().then_some(name))
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::bail!(
                    "incomplete mobile push configuration; set all four mobile settings or omit them all; missing {missing}"
                )
            }
        }
    }
}

#[cfg(test)]
mod test;
