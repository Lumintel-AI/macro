//! Startup configuration. `DATABASE_URL` comes from Doppler (`APP_SECRETS_JSON`
//! in ECS); `WHATSAPP_SESSION_KMS_KEY_ID` optionally overrides the KMS key
//! selected by `ENVIRONMENT`; `main.rs` reads the test-only
//! overrides from the process environment.

use database_env_vars::DatabaseUrl;
use macro_env::Environment;

#[cfg(test)]
mod test;

/// Settings supplied by the deployment.
#[derive(macro_config::MacroConfig)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct Config {
    /// MacroDB URL. It must reach the database instance, not RDS Proxy: the
    /// advisory-lock connection and `LISTEN` open from it too.
    pub database_url: DatabaseUrl,
    /// Optional KMS key ID, key ARN, alias name or alias ARN for session data keys.
    /// Unset keeps the alias selected by `ENVIRONMENT`.
    pub whatsapp_session_kms_key_id: Option<String>,
}

impl Config {
    /// The key passed to envelope encryption for both generation and decryption.
    pub fn session_kms_key(&self, environment: Environment) -> &str {
        self.whatsapp_session_kms_key_id
            .as_deref()
            .unwrap_or_else(|| SessionKmsKey::for_environment(environment))
    }
}

/// The default KMS key that wraps each account's data key:
/// `alias/whatsapp-session-service-<environment>`.
/// `infra/stacks/whatsapp-session-service` creates the dev and prod aliases;
/// `xtask_local` creates the local one in LocalStack.
pub struct SessionKmsKey;

impl SessionKmsKey {
    /// Alias in [`Environment::Local`].
    pub const LOCAL: &'static str = "alias/whatsapp-session-service-local";
    /// Alias in [`Environment::Develop`].
    pub const DEV: &'static str = "alias/whatsapp-session-service-dev";
    /// Alias in [`Environment::Production`].
    pub const PROD: &'static str = "alias/whatsapp-session-service-prod";

    /// The alias for `environment`.
    pub const fn for_environment(environment: Environment) -> &'static str {
        match environment {
            Environment::Local => Self::LOCAL,
            Environment::Develop => Self::DEV,
            Environment::Production => Self::PROD,
        }
    }
}
