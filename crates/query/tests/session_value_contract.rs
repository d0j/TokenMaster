use tokenmaster_domain::{UsageProfileId, UsageProviderId};
use tokenmaster_query::{PageSize, QueryErrorCode, QueryScope, UsageSessionPageRequest};

fn scope(provider: &str, profile: &str) -> QueryScope {
    QueryScope::new(
        UsageProviderId::new(provider).expect("provider"),
        UsageProfileId::new(profile).expect("profile"),
    )
}

#[test]
fn session_page_requests_are_bounded_canonical_and_duplicate_free() {
    let request = UsageSessionPageRequest::first(
        PageSize::new(256).expect("maximum page"),
        vec![scope("z-provider", "default"), scope("a-provider", "work")],
    )
    .expect("request");
    assert_eq!(request.page_size().get(), 256);
    assert_eq!(request.scopes()[0].provider_id().as_str(), "a-provider");
    assert!(!request.is_continuation());

    let duplicate = UsageSessionPageRequest::first(
        PageSize::new(1).expect("page"),
        vec![scope("codex", "default"), scope("codex", "default")],
    )
    .expect_err("duplicate scope");
    assert_eq!(duplicate.code(), QueryErrorCode::InvalidValue);

    let too_many = UsageSessionPageRequest::first(
        PageSize::new(1).expect("page"),
        (0..=32)
            .map(|index| scope("codex", &format!("profile-{index}")))
            .collect(),
    )
    .expect_err("scope cap");
    assert_eq!(too_many.code(), QueryErrorCode::CapacityExceeded);
}
