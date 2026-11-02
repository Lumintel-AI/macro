use super::build_user_invite_message;

#[test]
fn escapes_organization_html() {
    let org_name = "R&D <img src=x onerror=alert(1)> &lt;Partners&gt;";
    let result = build_user_invite_message(org_name, "prod");

    assert!(result.contains(
        "<strong>R&amp;D &lt;img src=x onerror=alert(1)&gt; &amp;lt;Partners&amp;gt;</strong>"
    ));
    assert!(!result.contains(org_name));
}

#[test]
fn logo_keeps_the_png_aspect_ratio() {
    let result = build_user_invite_message("Acme", "prod");
    let result = result.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(result.contains(
        r#"src="https://static-file-service.macro.com/file/b911aba9-94a6-435c-aa16-8404739f13b0" width="54" height="36""#
    ));
}

#[test]
fn preserves_organization_text_and_environment_links() {
    for (environment, prefix) in [("prod", ""), ("staging", "staging."), ("dev", "dev.")] {
        let result = build_user_invite_message("Acme's \"Team\" – 東京", environment);

        assert!(result.contains("<strong>Acme's \"Team\" – 東京</strong>"));
        assert!(result.contains(&format!(
            "href=\"https://{prefix}macro.com/app/?login=true\""
        )));
        assert!(!result.contains("{ORG_NAME}"));
        assert!(!result.contains("{PREFIX}"));
    }
}

#[test]
fn organization_invite_uses_configured_app_origin() {
    let origin = "https://app.example.com:8443";
    let app_url = macro_service_urls::AppServiceUrl::unwrap_new();
    if app_url.as_str() != origin {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "invite_user::test::organization_invite_uses_configured_app_origin",
            ])
            .env("OVERRIDE_APP_SERVICE_URL", origin)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    let html = build_user_invite_message("Acme", "prod");
    assert!(html.contains("href=\"https://app.example.com:8443/app/?login=true\""));
    assert!(!html.contains("{LOGIN_URL}"));
}
