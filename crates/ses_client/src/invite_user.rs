pub(crate) static INVITE_USER_SUBJECT: &str = "Invitation to Macro";

/// Builds the user invite message
pub(crate) fn build_user_invite_message(org_name: &str, environment: &str) -> String {
    let prefix = match environment {
        "prod" => "".to_string(),
        _ => format!("{}.", environment),
    };

    let result = include_str!("../templates/invite_user.html");

    let app_url = macro_service_urls::AppServiceUrl::unwrap_new();
    let default = macro_service_urls::AppServiceUrl::default_for_environment(
        macro_service_urls::macro_env::Environment::new_or_prod(),
    );
    let login_url = if app_url.as_str() != default.as_str() {
        format!("{}/app/?login=true", app_url.as_str().trim_end_matches('/'))
    } else {
        format!("https://{prefix}macro.com/app/?login=true")
    };
    let result = result.replace(
        "{LOGIN_URL}",
        &html_escape::encode_double_quoted_attribute(&login_url),
    );
    result.replace("{ORG_NAME}", &html_escape::encode_text(org_name))
}

#[cfg(test)]
mod test;
