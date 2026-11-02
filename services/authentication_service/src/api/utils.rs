use crate::api::context::ApiContext;
use cookie::{Cookie, SameSite};
use email::domain::ports::{FirstInboxProvisionOutcome, FirstInboxProvisioner};
use macro_auth::constant::{MACRO_ACCESS_TOKEN_COOKIE, MACRO_REFRESH_TOKEN_COOKIE};
use macro_env::Environment;
use macro_env_var::maybe_env_vars;
use rand::{Rng, seq::SliceRandom};
use url::Url;

maybe_env_vars! {
    struct FrontendPort;
    struct FrontendOrigin;
}

/// Generates a random 25 character session code
pub fn generate_session_code() -> String {
    const CHARSET_LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    const CHARSET_UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const CHARSET_NUMBERS: &[u8] = b"0123456789";

    let mut rng = rand::rng();
    let mut code = String::with_capacity(25);

    // Ensure at least one character from each set
    code.push(CHARSET_LOWER[rng.random_range(0..CHARSET_LOWER.len())] as char);
    code.push(CHARSET_UPPER[rng.random_range(0..CHARSET_UPPER.len())] as char);
    code.push(CHARSET_NUMBERS[rng.random_range(0..CHARSET_NUMBERS.len())] as char);

    // Combine all charsets for remaining characters
    let combined_charset: Vec<u8> = CHARSET_LOWER
        .iter()
        .chain(CHARSET_UPPER)
        .chain(CHARSET_NUMBERS)
        .copied()
        .collect();

    // Fill the remaining 22 characters
    for _ in 0..22 {
        let idx = rng.random_range(0..combined_charset.len());
        code.push(combined_charset[idx] as char);
    }

    // Shuffle the entire code to avoid predictable character positions
    let mut code_chars: Vec<char> = code.chars().collect();
    code_chars.shuffle(&mut rng);

    code_chars.into_iter().collect()
}

pub(crate) struct FrontendConfig {
    redirect_url: Url,
    cookie_domain: Option<String>,
    pub(crate) custom_origin: bool,
}

pub(crate) static FRONTEND_CONFIG: std::sync::LazyLock<FrontendConfig> =
    std::sync::LazyLock::new(|| {
        let environment = Environment::new_or_prod();
        let app_url = macro_service_urls::AppServiceUrl::unwrap_new();
        let origin = FrontendOrigin::new();
        let port = FrontendPort::new();
        resolve_frontend(
            environment,
            app_url.as_str(),
            origin.as_ref().and_then(FrontendOrigin::value),
            port.as_ref().and_then(FrontendPort::value),
        )
    });

fn resolve_frontend(
    environment: Environment,
    app_url: &str,
    frontend_origin: Option<&str>,
    port: Option<&str>,
) -> FrontendConfig {
    let default = macro_service_urls::AppServiceUrl::default_for_environment(environment);
    let custom_origin = frontend_origin.is_some() || app_url != default.as_str();
    let origin = frontend_origin.unwrap_or(app_url);
    let origin = if matches!(environment, Environment::Local) && !custom_origin {
        format!("http://localhost:{}", port.unwrap_or("3000"))
    } else {
        origin.to_owned()
    };
    let mut redirect_url = Url::parse(&origin).expect("frontend origin must be a valid URL");
    assert!(
        matches!(redirect_url.scheme(), "http" | "https")
            && redirect_url.host_str().is_some()
            && redirect_url.username().is_empty()
            && redirect_url.password().is_none()
            && redirect_url.path() == "/"
            && redirect_url.query().is_none()
            && redirect_url.fragment().is_none(),
        "frontend origin must be an HTTP(S) origin without credentials, path, query or fragment"
    );
    let cookie_domain = match environment {
        Environment::Local => None,
        _ if !custom_origin => Some("macro.com".to_owned()),
        _ => match redirect_url.host() {
            Some(url::Host::Domain(domain)) if domain != "localhost" => Some(domain.to_owned()),
            _ => None,
        },
    };
    if !matches!(environment, Environment::Local) {
        redirect_url.set_path("/app");
    }
    FrontendConfig {
        redirect_url,
        cookie_domain,
        custom_origin,
    }
}

/// Returns the configured frontend destination after login.
pub fn default_redirect_url() -> Url {
    FRONTEND_CONFIG.redirect_url.clone()
}

fn domain() -> Option<String> {
    FRONTEND_CONFIG.cookie_domain.clone()
}

#[cfg(test)]
mod test;

fn same_site() -> SameSite {
    match Environment::new_or_prod() {
        Environment::Production => SameSite::Strict,
        Environment::Local | Environment::Develop => SameSite::None,
    }
}

pub fn create_access_token_cookie(token: &str) -> Cookie<'static> {
    let same_site = same_site();
    let domain = domain();
    let access_token_cookie_name = match Environment::new_or_prod() {
        Environment::Production => MACRO_ACCESS_TOKEN_COOKIE.to_string(),
        Environment::Develop => format!("dev-{MACRO_ACCESS_TOKEN_COOKIE}"),
        Environment::Local => format!("local-{MACRO_ACCESS_TOKEN_COOKIE}"),
    };

    let mut cookie = Cookie::new(
        access_token_cookie_name,
        token.to_owned(), // Convert the borrowed str to an owned String
    );
    cookie.set_secure(true);
    cookie.set_http_only(true);
    cookie.set_same_site(same_site);
    if let Some(domain) = domain {
        cookie.set_domain(domain);
    }
    cookie.set_path("/");
    cookie.set_expires(Some(
        time::OffsetDateTime::now_utc() + time::Duration::days(365),
    ));
    cookie
}

pub fn create_refresh_token_cookie(token: &str) -> Cookie<'static> {
    let same_site = same_site();
    let domain = domain();
    let refresh_token_cookie_name = match Environment::new_or_prod() {
        Environment::Production => MACRO_REFRESH_TOKEN_COOKIE.to_string(),
        Environment::Develop => format!("dev-{MACRO_REFRESH_TOKEN_COOKIE}"),
        Environment::Local => format!("local-{MACRO_REFRESH_TOKEN_COOKIE}"),
    };
    let mut cookie = Cookie::new(
        refresh_token_cookie_name,
        token.to_owned(), // Convert the borrowed str to an owned String
    );
    cookie.set_secure(true);
    cookie.set_http_only(true);
    cookie.set_same_site(same_site);
    if let Some(domain) = domain {
        cookie.set_domain(domain);
    }
    cookie.set_path("/");
    cookie.set_expires(Some(
        time::OffsetDateTime::now_utc() + time::Duration::days(365),
    ));
    cookie
}

/// If this account was created during the auth flow that is completing (the
/// create-user webhook marks it in the cache), appends `signed_up=true` to the
/// redirect URL so the app can attribute the session as a signup for
/// analytics. Best-effort: never fails the login.
pub async fn append_signed_up_param_if_new_user(
    macro_cache_client: &macro_cache_client::MacroCache,
    email: &str,
    redirect_url: &mut Url,
) {
    match macro_cache_client.take_user_just_signed_up(email).await {
        Ok(true) => {
            redirect_url
                .query_pairs_mut()
                .append_pair("signed_up", "true");
        }
        Ok(false) => {}
        Err(e) => {
            tracing::error!(error=?e, "unable to check just-signed-up marker");
        }
    }
}

/// Provisions the user's primary inbox as a side effect of authentication,
/// the moment a fresh access token exists. Fire-and-forget so the login
/// response is never delayed. The email service arbitrates: a login without a
/// usable Gmail grant is an expected no-op, and init is idempotent and recurs
/// on every login, so a lost attempt only delays provisioning until the next one.
pub fn spawn_first_inbox_provision(ctx: &ApiContext, access_token: &str) {
    let email_service_client = ctx.email_service_client.clone();
    let access_token = access_token.to_string();

    tokio::spawn(async move {
        match email_service_client
            .provision_first_inbox(&access_token)
            .await
        {
            Ok(FirstInboxProvisionOutcome::Provisioned) => {
                tracing::info!("first-inbox provision: inbox initialized on login");
            }
            Ok(FirstInboxProvisionOutcome::Skipped) => {}
            Err(e) => {
                tracing::warn!(error=?e, "first-inbox provision: init failed");
            }
        }
    });
}
