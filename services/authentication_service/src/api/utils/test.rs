use super::*;

#[test]
fn hosted_frontend_controls_redirect_and_cookie_domain() {
    let config = resolve_frontend(
        Environment::Production,
        "https://app.example.com:8443",
        None,
        None,
    );
    assert_eq!(
        config.redirect_url.as_str(),
        "https://app.example.com:8443/app"
    );
    assert_eq!(config.cookie_domain.as_deref(), Some("app.example.com"));
    assert!(config.custom_origin);
    let config = resolve_frontend(
        Environment::Develop,
        "https://dev.macro.com",
        Some("https://login.example.com"),
        None,
    );
    assert_eq!(
        config.redirect_url.as_str(),
        "https://login.example.com/app"
    );
    assert_eq!(config.cookie_domain.as_deref(), Some("login.example.com"));
}

#[test]
fn unconfigured_frontend_keeps_upstream_behavior() {
    for (environment, app, redirect) in [
        (
            Environment::Production,
            "https://macro.com",
            "https://macro.com/app",
        ),
        (
            Environment::Develop,
            "https://dev.macro.com",
            "https://dev.macro.com/app",
        ),
        (
            Environment::Local,
            "http://localhost:3000",
            "http://localhost:4321/",
        ),
    ] {
        let config = resolve_frontend(environment, app, None, Some("4321"));
        assert_eq!(config.redirect_url.as_str(), redirect);
        assert!(!config.custom_origin);
        assert_eq!(
            config.cookie_domain.as_deref(),
            if matches!(environment, Environment::Local) {
                None
            } else {
                Some("macro.com")
            }
        );
    }
}

#[test]
fn local_proxy_origin_keeps_host_only_cookies() {
    let config = resolve_frontend(
        Environment::Local,
        "http://localhost:3000",
        Some("https://proxy.example.com"),
        None,
    );
    assert_eq!(config.redirect_url.as_str(), "https://proxy.example.com/");
    assert!(config.cookie_domain.is_none());
}

#[test]
#[should_panic(expected = "frontend origin must be an HTTP(S) origin")]
fn frontend_origin_rejects_credentials() {
    resolve_frontend(
        Environment::Production,
        "https://user:password@app.example.com",
        None,
        None,
    );
}
