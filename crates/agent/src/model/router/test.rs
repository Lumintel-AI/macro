use super::*;

#[test]
fn unconfigured_providers_fail_without_falling_back() {
    let router: ModelRouter = ModelRouter {
        anthropic: None,
        openai: None,
        gemini: None,
        openai_compatible: HashMap::new(),
    };
    for model in [
        "anthropic/claude-sonnet-5-5",
        "openai/gpt-5.5",
        "google/gemini-3-pro",
        "smart",
    ] {
        assert!(matches!(
            router.route_or_default(model),
            Err(AgentError::ProviderUnavailable(_))
        ));
    }
}

#[test]
fn one_configured_provider_does_not_require_the_others() {
    let router: ModelRouter = ModelRouter {
        anthropic: None,
        openai: Some(Arc::new(
            openai::Client::builder()
                .api_key("test-key")
                .build()
                .unwrap(),
        )),
        gemini: None,
        openai_compatible: HashMap::new(),
    };
    assert!(matches!(
        router.route_or_default("openai/gpt-5.5"),
        Ok(RoutedModel::OpenAiResponses(_))
    ));
    assert!(matches!(
        router.route_or_default("anthropic/claude-sonnet-5-5"),
        Err(AgentError::ProviderUnavailable(_))
    ));
}
