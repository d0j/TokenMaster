use tempfile::TempDir;
use tokenmaster_domain::{
    BenefitConfidence, BenefitDetailKind, BenefitEvidenceSource, BenefitExpiry,
    BenefitInventoryCompleteness, BenefitInventoryObservation, BenefitInventoryObservationParts,
    BenefitKind, BenefitLabelKey, BenefitLotId, BenefitLotObservation, BenefitLotObservationParts,
    BenefitObservationId, BenefitScope, BenefitState, BenefitTarget, NotificationChannel,
    QuotaAccountId, ReminderLeadTime, ReminderProfile, ReminderProfileParts,
    ReminderProfileRevision, UsageProviderId,
};
use tokenmaster_query::{
    BenefitChangeKind, BenefitChangePageRequest, BenefitCurrentRequest, BenefitOverviewRequest,
    BenefitReminderCoverage, BenefitReminderProfileSource, BenefitWarningCode, PageSize,
    QueryClock, QueryError, QueryErrorCode, QueryFreshness, QueryQuality, QueryService,
    QueryTimeSample,
};
use tokenmaster_store::UsageStore;

const OBSERVED_AT_MS: i64 = 1_800_000_000_000;

#[derive(Clone, Copy)]
struct FixedClock(i64);

impl QueryClock for FixedClock {
    fn sample(&self) -> Result<QueryTimeSample, QueryError> {
        Ok(QueryTimeSample::new(self.0, 1))
    }
}

fn scope(account: &str) -> BenefitScope {
    BenefitScope::new(
        UsageProviderId::new("codex").expect("provider"),
        QuotaAccountId::new(account).expect("account"),
        None,
    )
}

fn lot(
    id: u8,
    kind: BenefitKind,
    quantity: u64,
    state: BenefitState,
    expiry: BenefitExpiry,
    detail_kind: BenefitDetailKind,
) -> BenefitLotObservation {
    BenefitLotObservation::new(BenefitLotObservationParts {
        lot_id: BenefitLotId::from_bytes([id; 32]),
        kind,
        quantity,
        state,
        target: BenefitTarget::Provider,
        granted_at_ms: Some(OBSERVED_AT_MS - 1_000),
        expiry,
        source: BenefitEvidenceSource::ProviderOfficial,
        confidence: BenefitConfidence::High,
        detail_kind,
        label_key: BenefitLabelKey::new("benefit.codex.inventory").expect("label"),
    })
    .expect("lot")
}

fn observation(
    scope: BenefitScope,
    id: u8,
    observed_at_ms: i64,
    completeness: BenefitInventoryCompleteness,
    lots: Vec<BenefitLotObservation>,
) -> BenefitInventoryObservation {
    BenefitInventoryObservation::new(BenefitInventoryObservationParts {
        scope,
        observation_id: BenefitObservationId::from_bytes([id; 32]),
        observed_at_ms,
        fresh_until_ms: observed_at_ms + 1_000,
        stale_after_ms: observed_at_ms + 2_000,
        completeness,
        lots,
    })
    .expect("observation")
}

#[test]
fn current_snapshot_is_fefo_owned_explicit_and_profile_aware_across_restart() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-query.sqlite3");
    let requested_scope = scope("private-account");
    let earliest = OBSERVED_AT_MS + 10_000;
    let later = OBSERVED_AT_MS + 20_000;
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        writer
            .apply_benefit_observation(&observation(
                requested_scope.clone(),
                1,
                OBSERVED_AT_MS,
                BenefitInventoryCompleteness::CompleteQuantityPartialDetails,
                vec![
                    lot(
                        3,
                        BenefitKind::UsageCredit,
                        3,
                        BenefitState::Available,
                        BenefitExpiry::unknown(),
                        BenefitDetailKind::ProviderAggregate,
                    ),
                    lot(
                        2,
                        BenefitKind::BankedRateLimitReset,
                        1,
                        BenefitState::Available,
                        BenefitExpiry::exact_utc(later).expect("later expiry"),
                        BenefitDetailKind::ProviderDetail,
                    ),
                    lot(
                        1,
                        BenefitKind::BankedRateLimitReset,
                        1,
                        BenefitState::Available,
                        BenefitExpiry::exact_utc(earliest).expect("earliest expiry"),
                        BenefitDetailKind::ProviderDetail,
                    ),
                ],
            ))
            .expect("benefit observation");
    }

    let snapshot = QueryService::open(&path, FixedClock(OBSERVED_AT_MS + 500))
        .expect("query service")
        .benefit_inventory(BenefitCurrentRequest::new(requested_scope.clone()))
        .expect("benefit snapshot");
    assert_eq!(snapshot.header().benefit_revision().get(), 1);
    assert_eq!(snapshot.header().freshness(), QueryFreshness::Fresh);
    assert_eq!(snapshot.header().quality(), QueryQuality::Partial);
    assert!(
        snapshot
            .header()
            .warnings()
            .contains(&BenefitWarningCode::QuantityPartialDetails)
    );
    assert!(
        snapshot
            .header()
            .warnings()
            .contains(&BenefitWarningCode::UnknownExpiry)
    );

    let inventory = snapshot.payload().inventory().expect("present inventory");
    assert_eq!(inventory.current_lots().len(), 3);
    assert_eq!(
        inventory
            .current_lots()
            .iter()
            .map(|lot| lot.opaque_id())
            .collect::<Vec<_>>(),
        vec![
            BenefitLotId::from_bytes([1; 32]),
            BenefitLotId::from_bytes([2; 32]),
            BenefitLotId::from_bytes([3; 32]),
        ]
    );
    assert_eq!(inventory.nearest_expiry_at_ms(), Some(earliest));
    assert_eq!(
        inventory.nearest_due_at_ms(),
        Some(earliest - 7 * 24 * 60 * 60 * 1_000)
    );
    assert_eq!(
        inventory.reminder_profile().source(),
        BenefitReminderProfileSource::Inherited
    );
    assert_eq!(
        inventory.reminder_profile().coverage(),
        BenefitReminderCoverage::InAppOnly
    );
    assert_eq!(
        inventory
            .reminder_profile()
            .lead_times()
            .iter()
            .map(|lead| lead.seconds())
            .collect::<Vec<_>>(),
        vec![604_800, 86_400, 43_200, 21_600, 3_600]
    );

    let reopened = QueryService::open(&path, FixedClock(OBSERVED_AT_MS + 1_500))
        .expect("reopened query service")
        .benefit_inventory(BenefitCurrentRequest::new(requested_scope.clone()))
        .expect("reopened snapshot");
    assert_eq!(reopened.header().freshness(), QueryFreshness::Aging);
    assert_eq!(reopened.payload(), snapshot.payload());

    let absent = QueryService::open(&path, FixedClock(OBSERVED_AT_MS + 500))
        .expect("absent query service")
        .benefit_inventory(BenefitCurrentRequest::new(scope("missing-account")))
        .expect("absent snapshot");
    assert!(absent.payload().inventory().is_none());
    assert_eq!(absent.header().freshness(), QueryFreshness::Unavailable);
    assert_eq!(absent.header().quality(), QueryQuality::Unknown);
    assert_eq!(
        absent.payload().reminder_profile().source(),
        BenefitReminderProfileSource::Inherited
    );
    assert_eq!(
        absent.payload().reminder_profile().coverage(),
        BenefitReminderCoverage::InAppOnly
    );
    assert!(
        absent
            .header()
            .warnings()
            .contains(&BenefitWarningCode::InventoryAbsent)
    );
    for private in ["private-account", "missing-account"] {
        assert!(
            !format!("{snapshot:?}{absent:?}").contains(private),
            "benefit Debug exposed {private}"
        );
    }
}

#[test]
fn override_and_stale_partial_unknown_facts_are_not_coerced() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-query-profile.sqlite3");
    let requested_scope = scope("profile-account");
    let mut writer = UsageStore::open(&path).expect("writer");
    writer
        .apply_benefit_observation(&observation(
            requested_scope.clone(),
            1,
            OBSERVED_AT_MS,
            BenefitInventoryCompleteness::Partial,
            vec![lot(
                1,
                BenefitKind::Unknown,
                4,
                BenefitState::Ambiguous,
                BenefitExpiry::unknown(),
                BenefitDetailKind::ProviderAggregate,
            )],
        ))
        .expect("partial observation");
    let custom = ReminderProfile::new(ReminderProfileParts {
        revision: ReminderProfileRevision::new(2).expect("revision"),
        lead_times: vec![ReminderLeadTime::new(3 * 60 * 60).expect("three hours")],
        channels: vec![NotificationChannel::InApp, NotificationChannel::OsScheduled],
    })
    .expect("custom profile");
    writer
        .set_benefit_reminder_override(&requested_scope, Some(&custom))
        .expect("profile override");
    drop(writer);

    let snapshot = QueryService::open(&path, FixedClock(OBSERVED_AT_MS + 3_000))
        .expect("query service")
        .benefit_inventory(BenefitCurrentRequest::new(requested_scope))
        .expect("benefit snapshot");
    assert_eq!(snapshot.header().freshness(), QueryFreshness::Stale);
    assert_eq!(snapshot.header().quality(), QueryQuality::Partial);
    for warning in [
        BenefitWarningCode::PartialInventory,
        BenefitWarningCode::UnknownExpiry,
        BenefitWarningCode::OsScheduledUnavailable,
    ] {
        assert!(snapshot.header().warnings().contains(&warning));
    }
    let profile = snapshot
        .payload()
        .inventory()
        .expect("inventory")
        .reminder_profile();
    assert_eq!(profile.source(), BenefitReminderProfileSource::Override);
    assert_eq!(profile.coverage(), BenefitReminderCoverage::InAppOnly);
    assert_eq!(profile.lead_times().len(), 1);
}

#[test]
fn change_history_is_revision_and_scope_bound_without_consuming_failed_generations() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-query-history.sqlite3");
    let requested_scope = scope("history-account");
    let other_scope = scope("other-account");
    let initial = lot(
        1,
        BenefitKind::UsageCredit,
        1,
        BenefitState::Available,
        BenefitExpiry::exact_utc(OBSERVED_AT_MS + 10_000).expect("expiry"),
        BenefitDetailKind::ProviderDetail,
    );
    let changed = lot(
        1,
        BenefitKind::UsageCredit,
        2,
        BenefitState::Available,
        BenefitExpiry::exact_utc(OBSERVED_AT_MS + 10_000).expect("expiry"),
        BenefitDetailKind::ProviderDetail,
    );
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        writer
            .apply_benefit_observation(&observation(
                requested_scope.clone(),
                1,
                OBSERVED_AT_MS,
                BenefitInventoryCompleteness::Complete,
                vec![initial],
            ))
            .expect("initial");
        writer
            .apply_benefit_observation(&observation(
                requested_scope.clone(),
                2,
                OBSERVED_AT_MS + 1,
                BenefitInventoryCompleteness::Complete,
                vec![changed],
            ))
            .expect("changed");
    }

    let mut service =
        QueryService::open(&path, FixedClock(OBSERVED_AT_MS + 10)).expect("query service");
    let first = service
        .benefit_changes(
            BenefitChangePageRequest::first(
                requested_scope.clone(),
                PageSize::new(1).expect("page size"),
            )
            .expect("first request"),
        )
        .expect("first page");
    assert_eq!(first.header().snapshot_generation().get(), 1);
    assert_eq!(first.payload().changes().len(), 1);
    assert_eq!(
        first.payload().changes()[0].kind(),
        BenefitChangeKind::QuantityChanged
    );
    assert!(first.payload().changes()[0].before().is_some());
    assert!(first.payload().changes()[0].after().is_some());
    assert!(first.payload().has_more());
    let cursor = first.payload().next_cursor().cloned().expect("cursor");
    assert!(format!("{cursor:?}").contains("[redacted]"));

    let wrong_scope = BenefitChangePageRequest::continuation(
        other_scope,
        PageSize::new(1).expect("page size"),
        cursor.clone(),
    )
    .expect_err("scope-bound cursor");
    assert_eq!(wrong_scope.code(), QueryErrorCode::InvalidValue);

    {
        let mut writer = UsageStore::open(&path).expect("writer reopen");
        writer
            .apply_benefit_observation(&observation(
                requested_scope.clone(),
                3,
                OBSERVED_AT_MS + 2,
                BenefitInventoryCompleteness::Complete,
                Vec::new(),
            ))
            .expect("revision advance");
    }
    let stale = service
        .benefit_changes(
            BenefitChangePageRequest::continuation(
                requested_scope.clone(),
                PageSize::new(1).expect("page size"),
                cursor,
            )
            .expect("continuation request"),
        )
        .expect_err("stale revision");
    assert_eq!(stale.code(), QueryErrorCode::StaleSnapshot);

    let next = service
        .benefit_inventory(BenefitCurrentRequest::new(requested_scope))
        .expect("next successful snapshot");
    assert_eq!(next.header().snapshot_generation().get(), 2);
}

#[test]
fn overview_is_multi_scope_identity_free_kind_preserving_and_reminder_aware() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("benefit-query-overview.sqlite3");
    let first_scope = scope("overview-private-first");
    let second_scope = scope("overview-private-second");
    let earliest = OBSERVED_AT_MS + 10_000;
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        writer
            .apply_benefit_observation(&observation(
                first_scope.clone(),
                10,
                OBSERVED_AT_MS,
                BenefitInventoryCompleteness::Complete,
                vec![
                    lot(
                        1,
                        BenefitKind::BankedRateLimitReset,
                        2,
                        BenefitState::Available,
                        BenefitExpiry::exact_utc(earliest).expect("earliest expiry"),
                        BenefitDetailKind::ProviderDetail,
                    ),
                    lot(
                        2,
                        BenefitKind::UsageCredit,
                        4,
                        BenefitState::Available,
                        BenefitExpiry::unknown(),
                        BenefitDetailKind::ProviderAggregate,
                    ),
                ],
            ))
            .expect("first observation");
        writer
            .apply_benefit_observation(&observation(
                second_scope.clone(),
                11,
                OBSERVED_AT_MS + 1,
                BenefitInventoryCompleteness::Partial,
                vec![
                    lot(
                        3,
                        BenefitKind::BankedRateLimitReset,
                        7,
                        BenefitState::Expired,
                        BenefitExpiry::exact_utc(OBSERVED_AT_MS + 20_000)
                            .expect("expired lot expiry"),
                        BenefitDetailKind::ProviderDetail,
                    ),
                    lot(
                        4,
                        BenefitKind::TemporaryUsage,
                        3,
                        BenefitState::ActivationPending,
                        BenefitExpiry::unknown(),
                        BenefitDetailKind::ProviderDetail,
                    ),
                    lot(
                        5,
                        BenefitKind::Unknown,
                        5,
                        BenefitState::Ambiguous,
                        BenefitExpiry::unknown(),
                        BenefitDetailKind::ProviderAggregate,
                    ),
                ],
            ))
            .expect("second observation");
    }

    let overview = QueryService::open(&path, FixedClock(OBSERVED_AT_MS + 500))
        .expect("query service")
        .benefit_overview(BenefitOverviewRequest::new())
        .expect("benefit overview");
    assert_eq!(overview.header().schema_version(), 1);
    assert_eq!(overview.header().benefit_revision().get(), 2);
    assert_eq!(overview.header().freshness(), QueryFreshness::Fresh);
    assert_eq!(overview.header().quality(), QueryQuality::Partial);
    assert!(
        overview
            .header()
            .warnings()
            .contains(&BenefitWarningCode::PartialInventory)
    );
    assert_eq!(overview.payload().scopes().len(), 2);

    let lots = overview
        .payload()
        .scopes()
        .iter()
        .flat_map(|scope| scope.current_lots().iter())
        .collect::<Vec<_>>();
    assert_eq!(
        lots.iter()
            .filter(|lot| {
                lot.kind() == BenefitKind::BankedRateLimitReset
                    && lot.state() == BenefitState::Available
            })
            .map(|lot| lot.quantity())
            .sum::<u64>(),
        2
    );
    assert_eq!(
        lots.iter()
            .find(|lot| lot.kind() == BenefitKind::UsageCredit)
            .expect("usage credit")
            .quantity(),
        4
    );
    assert!(lots.iter().any(|lot| {
        lot.kind() == BenefitKind::TemporaryUsage && lot.state() == BenefitState::ActivationPending
    }));
    assert!(lots.iter().any(|lot| {
        lot.kind() == BenefitKind::Unknown && lot.state() == BenefitState::Ambiguous
    }));
    let reset_scope = overview
        .payload()
        .scopes()
        .iter()
        .find(|scope| scope.nearest_expiry_at_ms() == Some(earliest))
        .expect("reset scope");
    assert_eq!(
        reset_scope.reminder_profile().coverage(),
        BenefitReminderCoverage::InAppOnly
    );
    assert_eq!(
        reset_scope.nearest_due_at_ms(),
        Some(earliest - 7 * 24 * 60 * 60 * 1_000)
    );
    assert!(reset_scope.freshness() == QueryFreshness::Fresh);

    let debug = format!("{overview:?}");
    for private in ["overview-private-first", "overview-private-second"] {
        assert!(!debug.contains(private), "overview Debug exposed {private}");
    }

    let exact = QueryService::open(&path, FixedClock(OBSERVED_AT_MS + 500))
        .expect("exact query service")
        .benefit_inventory(BenefitCurrentRequest::new(first_scope))
        .expect("exact benefit snapshot");
    assert!(format!("{:?}", exact.header().filter()).contains("[redacted]"));
}

#[test]
fn overview_capacity_failure_does_not_consume_a_snapshot_generation() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("benefit-query-overview-capacity.sqlite3");
    {
        let mut writer = UsageStore::open(&path).expect("writer");
        for index in 0_u8..33 {
            writer
                .apply_benefit_observation(&observation(
                    scope(&format!("capacity-private-{index:02}")),
                    index.saturating_add(100),
                    OBSERVED_AT_MS + i64::from(index),
                    BenefitInventoryCompleteness::Complete,
                    Vec::new(),
                ))
                .expect("benefit observation");
        }
    }

    let mut service =
        QueryService::open(&path, FixedClock(OBSERVED_AT_MS + 500)).expect("query service");
    let error = service
        .benefit_overview(BenefitOverviewRequest::new())
        .expect_err("33rd scope must fail closed");
    assert_eq!(error.code(), QueryErrorCode::CapacityExceeded);
    let next = service
        .benefit_inventory(BenefitCurrentRequest::new(scope("missing-after-capacity")))
        .expect("next successful snapshot");
    assert_eq!(next.header().snapshot_generation().get(), 1);
    let debug = format!("{error:?}");
    assert!(!debug.contains("capacity-private"));
    assert!(!debug.contains(path.to_string_lossy().as_ref()));
}

#[test]
fn empty_overview_is_explicitly_unavailable_instead_of_a_zero_inventory() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("benefit-query-overview-empty.sqlite3");
    UsageStore::open(&path).expect("initialize archive");

    let overview = QueryService::open(&path, FixedClock(OBSERVED_AT_MS))
        .expect("query service")
        .benefit_overview(BenefitOverviewRequest::new())
        .expect("empty overview");
    assert!(overview.payload().scopes().is_empty());
    assert_eq!(overview.header().snapshot_generation().get(), 1);
    assert_eq!(overview.header().benefit_revision().get(), 0);
    assert_eq!(overview.header().data_through_ms(), None);
    assert_eq!(overview.header().freshness(), QueryFreshness::Unavailable);
    assert_eq!(overview.header().quality(), QueryQuality::Unknown);
    assert_eq!(
        overview.header().warnings().as_ref(),
        &[BenefitWarningCode::InventoryAbsent]
    );
}
