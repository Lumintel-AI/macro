use super::*;

#[test]
fn configured_origin_is_exact_and_keeps_defaults() {
    let config = CorsConfig {
        frontend_origin: Some("https://app.example.com".to_owned()),
    };
    assert!(config.is_origin_allowed("https://app.example.com"));
    for origin in [
        "https://unrelated.example.com",
        "https://app.example.com.attacker.test",
        "https://sub.app.example.com",
        "http://app.example.com",
        "https://app.example.com:8443",
        "https://app.example.com/",
    ] {
        assert!(!config.is_origin_allowed(origin));
    }
    for origin in ALLOWED_ORIGINS {
        assert!(config.is_origin_allowed(origin));
    }
}

#[test]
fn unset_origin_preserves_upstream_rules() {
    let config = CorsConfig::default();
    for origin in ALLOWED_ORIGINS {
        assert!(config.is_origin_allowed(origin));
    }
    assert!(config.is_origin_allowed("http://persona.localhost:20010"));
    assert!(config.is_origin_allowed("https://branch.preview.macro.com"));
    assert!(!config.is_origin_allowed("https://app.example.com"));
    assert!(!config.is_origin_allowed("https://unrelated.example.com"));
}

#[test]
fn empty_wildcard_and_opaque_origins_never_grant_credentials() {
    for origin in ["", "*", "null"] {
        let config = CorsConfig {
            frontend_origin: Some(origin.to_owned()),
        };
        assert!(!config.is_origin_allowed(origin));
        assert!(!config.is_origin_allowed("https://unrelated.example.com"));
        assert!(config.is_origin_allowed("https://macro.com"));
    }
}
