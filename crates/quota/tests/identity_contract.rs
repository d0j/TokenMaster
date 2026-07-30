use std::fmt::Write;

use tokenmaster_domain::{
    QuotaAccountId, QuotaObservationId, QuotaScope, QuotaWindowId, QuotaWindowKey,
    QuotaWorkspaceId, UsageProviderId,
};
use tokenmaster_quota::{quota_epoch_id, quota_scope_id};

fn window_key(workspace: Option<&str>) -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new("personal").expect("account"),
            workspace.map(|value| QuotaWorkspaceId::new(value).expect("workspace")),
        ),
        QuotaWindowId::new("weekly").expect("window"),
    )
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("write to string");
    }
    output
}

#[test]
fn scope_and_epoch_hash_vectors_are_versioned_and_architecture_independent() {
    let key = window_key(Some("default"));
    let scope_id = quota_scope_id(key.scope());
    let epoch_id = quota_epoch_id(&key, 1, QuotaObservationId::from_bytes([1; 32]));

    assert_eq!(
        lower_hex(scope_id.as_bytes()),
        "b2864d96a659570dde98520ad317286e19c715cc271c94f5d42b05c6d3b520d2"
    );
    assert_eq!(
        lower_hex(epoch_id.as_bytes()),
        "aa05827fb30b49c76523b7c11cdf0f2f7f43c154b2765aa59173f46674f365ac"
    );
    assert_eq!(format!("{scope_id:?}"), "QuotaScopeId([redacted])");
    assert_eq!(format!("{epoch_id:?}"), "QuotaEpochId([redacted])");
}

#[test]
fn optional_workspace_revision_and_first_observation_change_identity() {
    let with_workspace = window_key(Some("default"));
    let without_workspace = window_key(None);
    let first = QuotaObservationId::from_bytes([1; 32]);
    let second = QuotaObservationId::from_bytes([2; 32]);

    assert_ne!(
        quota_scope_id(with_workspace.scope()),
        quota_scope_id(without_workspace.scope())
    );
    assert_ne!(
        quota_epoch_id(&with_workspace, 1, first),
        quota_epoch_id(&with_workspace, 2, first)
    );
    assert_ne!(
        quota_epoch_id(&with_workspace, 1, first),
        quota_epoch_id(&with_workspace, 1, second)
    );
}
