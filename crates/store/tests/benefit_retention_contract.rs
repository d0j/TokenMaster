use rusqlite::{Connection, params};
use tempfile::TempDir;
use tokenmaster_benefits::benefit_scope_id;
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, QuotaAccountId,
    UsageProviderId,
};
use tokenmaster_store::{
    DEFAULT_BENEFIT_CHANGES_PER_SCOPE, DEFAULT_BENEFIT_DELIVERIES_PER_SCOPE,
    MAX_BENEFIT_MAINTENANCE_PAGE_SIZE, StoreErrorCode, UsageStore,
};

const OBSERVED_AT_MS: i64 = 1_800_000_000_000;

fn opaque_id(value: u64) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    bytes
}

fn scope() -> BenefitScope {
    BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new("acct_private").expect("account"),
        None,
    )
}

fn lot(id: u64, quantity: u64, state: BenefitState) -> BenefitLotObservation {
    BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes(opaque_id(id)),
        kind: BenefitKind::BankedRateLimitReset,
        quantity,
        state,
        target: BenefitTarget::Provider,
        granted_at_ms: None,
        expiry: BenefitExpiry::unknown(),
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset").expect("label"),
    })
    .expect("lot")
}

fn observation(
    id: u64,
    observed_at_ms: i64,
    lots: Vec<BenefitLotObservation>,
) -> BenefitInventoryObservation {
    BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope: scope(),
        observation_id: BenefitObservationId::from_bytes(opaque_id(id)),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 1_000,
        stale_after_ms: observed_at_ms + 2_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots,
    })
    .expect("observation")
}

fn scalar(path: &std::path::Path, sql: &str) -> i64 {
    Connection::open(path)
        .expect("inspect store")
        .query_row(sql, [], |row| row.get(0))
        .expect("scalar")
}

#[test]
fn maintenance_is_bounded_and_plateaus_change_history_without_losing_current() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-change-retention.sqlite3");
    let mut store = UsageStore::open(&path).expect("store");
    for revision in 1_u64..=600 {
        store
            .apply_benefit_observation(&observation(
                revision,
                OBSERVED_AT_MS + i64::try_from(revision).expect("time"),
                vec![lot(1, revision, BenefitState::Available)],
            ))
            .expect("changed observation");
    }

    let zero = store
        .maintain_benefit_history_page(&scope(), 0)
        .expect_err("zero page must fail");
    assert_eq!(zero.code(), StoreErrorCode::InvalidValue);
    let oversized = store
        .maintain_benefit_history_page(&scope(), MAX_BENEFIT_MAINTENANCE_PAGE_SIZE + 1)
        .expect_err("oversized page must fail");
    assert_eq!(oversized.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(
        oversized.limit(),
        Some(u64::from(MAX_BENEFIT_MAINTENANCE_PAGE_SIZE))
    );

    let maintained = store
        .maintain_benefit_history_page(&scope(), MAX_BENEFIT_MAINTENANCE_PAGE_SIZE)
        .expect("maintenance");
    assert_eq!(maintained.deleted_changes(), 88);
    assert!(maintained.deleted_lot_revisions() <= 168);
    assert_eq!(
        maintained.remaining_changes(),
        DEFAULT_BENEFIT_CHANGES_PER_SCOPE
    );
    assert!(maintained.total_deleted() <= u64::from(MAX_BENEFIT_MAINTENANCE_PAGE_SIZE));
    drop(store);

    assert_eq!(
        scalar(&path, "SELECT count(*) FROM benefit_change"),
        i64::try_from(DEFAULT_BENEFIT_CHANGES_PER_SCOPE).expect("count")
    );
    assert_eq!(
        scalar(
            &path,
            "SELECT lot_revision FROM benefit_lot_current WHERE lot_id = x'0000000000000000000000000000000000000000000000000000000000000001'"
        ),
        600
    );
    assert_eq!(
        scalar(&path, "SELECT min(sequence) FROM benefit_change"),
        89
    );
}

#[test]
fn latest_terminal_cursor_is_protected_and_supports_reappearance_after_maintenance() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-terminal-retention.sqlite3");
    let mut store = UsageStore::open(&path).expect("store");
    for id in 1_u64..=260 {
        store
            .apply_benefit_observation(&observation(
                id * 2 - 1,
                OBSERVED_AT_MS + i64::try_from(id * 2 - 1).expect("time"),
                vec![lot(id, 1, BenefitState::Activated)],
            ))
            .expect("terminal lot");
        store
            .apply_benefit_observation(&observation(
                id * 2,
                OBSERVED_AT_MS + i64::try_from(id * 2).expect("time"),
                Vec::new(),
            ))
            .expect("terminal retirement");
    }

    let maintained = store
        .maintain_benefit_history_page(&scope(), MAX_BENEFIT_MAINTENANCE_PAGE_SIZE)
        .expect("maintenance");
    assert_eq!(maintained.deleted_changes(), 8);
    assert_eq!(
        maintained.remaining_changes(),
        DEFAULT_BENEFIT_CHANGES_PER_SCOPE
    );
    drop(store);

    let mut reopened = UsageStore::open(&path).expect("reopen");
    reopened
        .apply_benefit_observation(&observation(
            10_000,
            OBSERVED_AT_MS + 10_000,
            vec![lot(1, 1, BenefitState::Available)],
        ))
        .expect("retired lot reappears");
    drop(reopened);
    assert_eq!(
        scalar(
            &path,
            "SELECT lot_revision FROM benefit_lot_current WHERE lot_id = x'0000000000000000000000000000000000000000000000000000000000000001'"
        ),
        3
    );
}

#[test]
fn delivery_retention_prunes_only_noncurrent_revision_receipts() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-delivery-retention.sqlite3");
    {
        let mut store = UsageStore::open(&path).expect("store");
        store
            .apply_benefit_observation(&observation(
                1,
                OBSERVED_AT_MS,
                vec![lot(1, 1, BenefitState::Available)],
            ))
            .expect("revision one");
        store
            .apply_benefit_observation(&observation(
                2,
                OBSERVED_AT_MS + 1,
                vec![lot(1, 2, BenefitState::Available)],
            ))
            .expect("revision two");
    }

    let connection = Connection::open(&path).expect("seed deliveries");
    let scope_id = benefit_scope_id(&scope());
    for receipt in 1_u64..=300 {
        connection
            .execute(
                "INSERT INTO benefit_reminder_delivery(
                   delivery_id, scope_id, lot_id, lot_revision, threshold_seconds,
                   channel, due_at_ms, expiry_at_ms, delivered_at_ms
                 ) VALUES (?1, ?2, ?3, 1, 3600, 'in_app', 1, 2, ?4)",
                params![
                    opaque_id(receipt).as_slice(),
                    scope_id.as_bytes().as_slice(),
                    opaque_id(1).as_slice(),
                    i64::try_from(receipt).expect("delivery time"),
                ],
            )
            .expect("old delivery");
    }
    for receipt in 301_u64..=305 {
        connection
            .execute(
                "INSERT INTO benefit_reminder_delivery(
                   delivery_id, scope_id, lot_id, lot_revision, threshold_seconds,
                   channel, due_at_ms, expiry_at_ms, delivered_at_ms
                 ) VALUES (?1, ?2, ?3, 2, 3600, 'in_app', 1, 2, ?4)",
                params![
                    opaque_id(receipt).as_slice(),
                    scope_id.as_bytes().as_slice(),
                    opaque_id(1).as_slice(),
                    i64::try_from(receipt).expect("delivery time"),
                ],
            )
            .expect("current delivery");
    }
    connection
        .execute(
            "UPDATE benefit_state SET retained_delivery_count = 305 WHERE singleton_id = 1",
            [],
        )
        .expect("delivery count");
    drop(connection);

    let mut store = UsageStore::open(&path).expect("validated store");
    let maintained = store
        .maintain_benefit_history_page(&scope(), MAX_BENEFIT_MAINTENANCE_PAGE_SIZE)
        .expect("delivery maintenance");
    assert_eq!(maintained.deleted_deliveries(), 49);
    assert_eq!(
        maintained.remaining_deliveries(),
        DEFAULT_BENEFIT_DELIVERIES_PER_SCOPE
    );
    drop(store);
    assert_eq!(
        scalar(
            &path,
            "SELECT count(*) FROM benefit_reminder_delivery WHERE lot_revision = 2"
        ),
        5
    );
}
