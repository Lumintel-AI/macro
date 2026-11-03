use super::*;

#[test]
fn unconfigured_linking_is_refused_before_starting_oauth() {
    let configured = service(
        StubGithubRepo::linked(test_link(&test_user_id())),
        StubGithubOauth::new(true),
        StubAuth::new("test-token"),
    );
    assert!(configured.ensure_configured().is_ok());
    let service = GithubLinkServiceImpl::new(
        StubGithubRepo::linked(test_link(&test_user_id())),
        StubGithubOauth::new(true),
        StubAuth::new("test-token"),
        GithubPullRequestServiceImpl::new(StubForeignEntityService::default(), NoPullRequestRows),
        None,
    );
    assert!(matches!(
        service.ensure_configured(),
        Err(GithubError::Unavailable)
    ));
    assert!(matches!(
        service.construct_oauth_url("https://example.com/callback", "state"),
        Err(GithubError::Unavailable)
    ));
}
