use macro_env_var::maybe_env_vars;
use reqwest::header::HeaderMap;

const ANTHROPIC_ROUTER_BASE_URL: &str = "https://api.anthropic.com";

maybe_env_vars! {
    struct AnthropicApiKey;
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub api_base: String,
    pub headers: HeaderMap,
}

impl Config {
    /// Load optional configuration through the standard environment lookup.
    pub fn try_from_env() -> Result<Option<Self>, reqwest::header::InvalidHeaderValue> {
        let key = AnthropicApiKey::new();
        Self::from_optional_api_key(key.as_ref().and_then(|key| key.value()))
    }

    /// Build authenticated configuration only when a key was supplied at startup.
    pub fn from_optional_api_key(
        api_key: Option<&str>,
    ) -> Result<Option<Self>, reqwest::header::InvalidHeaderValue> {
        let Some(api_key) = api_key.filter(|key| !key.trim().is_empty()) else {
            return Ok(None);
        };
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", api_key.parse()?);
        headers.insert(
            "anthropic-version",
            reqwest::header::HeaderValue::from_static("2023-06-01"),
        );
        Ok(Some(Self {
            api_base: ANTHROPIC_ROUTER_BASE_URL.into(),
            headers,
        }))
    }

    pub fn dangrously_try_from_env() -> Self {
        let api_key = AnthropicApiKey::new()
            .and_then(|key| key.value().map(str::to_owned))
            .expect("api key");
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", api_key.parse().expect("good config"));
        headers.insert(
            "anthropic-version",
            "2023-06-01".parse().expect("good version"),
        );
        Self {
            api_base: ANTHROPIC_ROUTER_BASE_URL.into(),
            headers,
        }
    }
}
