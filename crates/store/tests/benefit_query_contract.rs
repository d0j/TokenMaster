use std::time::Duration;

use rusqlite::Connection;
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
    BenefitOverviewQuery, MAX_BENEFIT_OVERVIEW_LOTS, MAX_BENEFIT_OVERVIEW_SCOPES, StoreErrorCode,
    UsageReadStore, UsageStore,
};

const OBSERVED_AT_MS: i64 = 1_800_000_000_000;

fn opaque(value: u64) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes[..8].copy_from_slice(&value.to_be_bytes());
    bytes
}

fn scope(index: usize) -> BenefitScope {
    BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new(format!("private-account-{index:02}")).expect("account"),
        None,
    )
}

fn lot(scope_index: usize, index: usize, expires_at_ms: i64) -> BenefitLotObservation {
    let identity = u64::try_from(scope_index)
        .expect("scope index")
        .checked_mul(100)
        .and_then(|value| value.checked_add(u64::try_from(index).expect("lot index")))
        .expect("lot identity");
    BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes(opaque(identity)),
        kind: BenefitKind::BankedRateLimitReset,
        quantity: u64::try_from(scope_index + 1).expect("quantity"),
        state: BenefitState::Available,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(OBSERVED_AT_MS - 1_000),
        expiry: BenefitExpiry::exact_utc(expires_at_ms).expect("expiry"),
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind: BenefitDetailKind::ProviderDetail,
        label_key: BenefitLabelKey::new("benefit.codex.banked_reset").expect("label"),
    })
    .expect("lot")
}

fn observation(
    scope_index: usize,
    lots: Vec<BenefitLotObservation>,
) -> BenefitInventoryObservation {
    BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope: scope(scope_index),
        observation_id: BenefitObservationId::from_bytes(opaque(
            u64::try_from(scope_index + 1).expect("observation identity"),
        )),
        observed_at_ms: OBSERVED_AT_MS + i64::try_from(scope_index).expect("observed offset"),
        fresh_until_ms: OBSERVED_AT_MS + 10_000,
        stale_after_ms: OBSERVED_AT_MS + 20_000,
        completeness: BenefitInventoryCompleteness::Complete,
        lots,
    })
    .expect("observation")
}

#[test]
fn overview_restores_every_scope_in_opaque_order_with_fefo_and_reminders() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-overview.sqlite3");
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        for index in [2_usize, 0, 1] {
            writer
                .apply_benefit_observation(&observation(
                    index,
                    vec![
                        lot(index, 2, OBSERVED_AT_MS + 20_000),
                        lot(index, 1, OBSERVED_AT_MS + 10_000),
                    ],
                ))
                .expect("benefit observation");
        }
    }

    let mut reader = UsageReadStore::open(&path).expect("reader");
    let capture = reader
        .capture_benefit_overview(
            BenefitOverviewQuery::new(Duration::from_secs(2)).expect("overview query"),
        )
        .expect("overview capture");
    assert_eq!(capture.benefit_revision().get(), 3);
    assert_eq!(capture.scopes().len(), 3);

    let mut expected = (0_usize..3)
        .map(|index| {
            (
                *benefit_scope_id(&scope(index)).as_bytes(),
                u64::try_from(index + 1).expect("quantity"),
            )
        })
        .collect::<Vec<_>>();
    expected.sort_by_key(|value| value.0);
    assert_eq!(
        capture
            .scopes()
            .iter()
            .map(|scope| scope.lots()[0].lot().quantity())
            .collect::<Vec<_>>(),
        expected.iter().map(|value| value.1).collect::<Vec<_>>()
    );
    for scope in capture.scopes() {
        assert_eq!(scope.lots().len(), 2);
        assert!(
            scope.lots()[0].lot().expiry().conservative_utc_ms()
                < scope.lots()[1].lot().expiry().conservative_utc_ms()
        );
        assert!(scope.reminder_profile().inherited());
        assert_eq!(
            scope
                .nearest_due()
                .expect("nearest reminder")
                .expiry_at_ms(),
            OBSERVED_AT_MS + 10_000
        );
    }
    let debug = format!("{capture:?}");
    for private in [
        "private-account-00",
        "private-account-01",
        "private-account-02",
    ] {
        assert!(!debug.contains(private), "overview Debug exposed {private}");
    }
}

#[test]
fn overview_accepts_its_exact_scope_and_total_lot_bounds() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-overview-bounds.sqlite3");
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        for scope_index in 0..MAX_BENEFIT_OVERVIEW_SCOPES {
            let lot_count = if scope_index < 4 { 64 } else { 0 };
            let lots = (0..lot_count)
                .map(|lot_index| {
                    lot(
                        scope_index,
                        lot_index,
                        OBSERVED_AT_MS + 100_000 + i64::try_from(lot_index).expect("expiry offset"),
                    )
                })
                .collect();
            writer
                .apply_benefit_observation(&observation(scope_index, lots))
                .expect("benefit observation");
        }
    }

    let mut reader = UsageReadStore::open(&path).expect("reader");
    let capture = reader
        .capture_benefit_overview(
            BenefitOverviewQuery::new(Duration::from_secs(2)).expect("overview query"),
        )
        .expect("bounded overview");
    assert_eq!(capture.scopes().len(), MAX_BENEFIT_OVERVIEW_SCOPES);
    assert_eq!(
        capture
            .scopes()
            .iter()
            .map(|scope| scope.lots().len())
            .sum::<usize>(),
        MAX_BENEFIT_OVERVIEW_LOTS
    );
}

#[test]
fn overview_rejects_scope_and_total_lot_lookahead_without_truncation() {
    let directory = TempDir::new().expect("temporary directory");
    let scope_path = directory
        .path()
        .join("benefit-overview-scope-overflow.sqlite3");
    {
        let mut writer = UsageStore::open(&scope_path).expect("writer");
        for scope_index in 0..=MAX_BENEFIT_OVERVIEW_SCOPES {
            writer
                .apply_benefit_observation(&observation(scope_index, Vec::new()))
                .expect("benefit observation");
        }
    }
    let scope_error = UsageReadStore::open(&scope_path)
        .expect("reader")
        .capture_benefit_overview(
            BenefitOverviewQuery::new(Duration::from_secs(2)).expect("overview query"),
        )
        .expect_err("33rd scope must fail closed");
    assert_eq!(scope_error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(
        scope_error.limit(),
        Some(MAX_BENEFIT_OVERVIEW_SCOPES as u64)
    );

    let lot_path = directory
        .path()
        .join("benefit-overview-lot-overflow.sqlite3");
    {
        let mut writer = UsageStore::open(&lot_path).expect("writer");
        for scope_index in 0..5 {
            let lot_count = if scope_index < 4 { 64 } else { 1 };
            let lots = (0..lot_count)
                .map(|lot_index| {
                    lot(
                        scope_index,
                        lot_index,
                        OBSERVED_AT_MS + 100_000 + i64::try_from(lot_index).expect("expiry offset"),
                    )
                })
                .collect();
            writer
                .apply_benefit_observation(&observation(scope_index, lots))
                .expect("benefit observation");
        }
    }
    let lot_error = UsageReadStore::open(&lot_path)
        .expect("reader")
        .capture_benefit_overview(
            BenefitOverviewQuery::new(Duration::from_secs(2)).expect("overview query"),
        )
        .expect_err("257th lot must fail closed");
    assert_eq!(lot_error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(lot_error.limit(), Some(MAX_BENEFIT_OVERVIEW_LOTS as u64));
}

#[test]
fn overview_rejects_corrupt_scope_identity_instead_of_exposing_or_skipping_it() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-overview-corrupt.sqlite3");
    UsageStore::open(&path)
        .expect("writer")
        .apply_benefit_observation(&observation(0, Vec::new()))
        .expect("benefit observation");
    Connection::open(&path)
        .expect("raw connection")
        .execute(
            "UPDATE benefit_scope SET account_id = 'different-private-account'",
            [],
        )
        .expect("corrupt scope identity");

    let error = UsageReadStore::open(&path)
        .expect("reader")
        .capture_benefit_overview(
            BenefitOverviewQuery::new(Duration::from_secs(2)).expect("overview query"),
        )
        .expect_err("corrupt scope must fail closed");
    assert_eq!(error.code(), StoreErrorCode::InvalidStoredValue);
    let debug = format!("{error:?}");
    assert!(!debug.contains("different-private-account"));
    assert!(!debug.contains(path.to_string_lossy().as_ref()));
}

#[test]
fn overview_scope_discovery_uses_primary_key_order_without_a_temp_sort() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-overview-plan.sqlite3");
    UsageStore::open(&path).expect("initialize archive");
    let connection = Connection::open(&path).expect("plan connection");
    let mut statement = connection
        .prepare(
            "EXPLAIN QUERY PLAN
             SELECT scope_id, provider_id, account_id, workspace_id, current_lot_count
             FROM benefit_scope
             ORDER BY scope_id
             LIMIT ?1",
        )
        .expect("overview plan");
    let details = statement
        .query_map(
            [i64::try_from(MAX_BENEFIT_OVERVIEW_SCOPES + 1).expect("limit")],
            |row| row.get::<_, String>(3),
        )
        .expect("plan rows")
        .collect::<Result<Vec<_>, _>>()
        .expect("plan details");
    assert!(
        details
            .iter()
            .any(|detail| detail.contains("benefit_scope")),
        "overview plan omitted benefit_scope: {details:?}"
    );
    assert!(
        !details.iter().any(|detail| detail.contains("TEMP B-TREE")),
        "overview discovery introduced a temp sort: {details:?}"
    );
}
