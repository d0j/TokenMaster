use rusqlite::Connection;
use tempfile::TempDir;
use tokenmaster_store::{
    MAX_SCAN_SCOPES, ScanCounters, ScanId, ScanOutcome, ScanScope, ScanSetId, ScanSetManifest,
    SourceKey, SourceKind, SourceRegistration, SourceRegistrationParts, StoreErrorCode,
    StoredCheckpoint, StoredCheckpointParts, StoredVerification, UsageStore,
};

fn checkpoint(seed: u8) -> StoredCheckpoint {
    StoredCheckpoint::new(StoredCheckpointParts {
        parser_schema_version: 1,
        physical_identity: Some([seed; 32]),
        logical_identity: [seed.wrapping_add(1); 32],
        committed_offset: 0,
        scan_offset: 0,
        observed_file_length: 0,
        modified_time_ns: None,
        anchor_start: 0,
        anchor_len: 0,
        anchor_sha256: [seed.wrapping_add(2); 32],
        resume: Box::new([]),
        discarding_oversized_line: false,
        incomplete_tail: false,
        verification: StoredVerification::Incremental,
    })
    .expect("zero checkpoint")
}

fn register_source(
    store: &mut UsageStore,
    seed: u8,
    provider_id: &str,
    profile_id: &str,
) -> SourceKey {
    let checkpoint = checkpoint(seed);
    let source_key = SourceKey::from_bytes([seed; 32]);
    store
        .register_source(
            &SourceRegistration::new(SourceRegistrationParts {
                source_key,
                provider_id: provider_id.into(),
                profile_id: profile_id.into(),
                source_id: format!("source-{seed}").into_boxed_str(),
                source_kind: SourceKind::Active,
                logical_identity: *checkpoint.logical_identity(),
                physical_identity: checkpoint.physical_identity().copied(),
                initial_checkpoint: checkpoint,
            })
            .expect("source registration"),
        )
        .expect("register source");
    source_key
}

fn source_presence(path: &std::path::Path, source_key: SourceKey) -> (Option<i64>, i64) {
    Connection::open(path)
        .expect("inspect source presence")
        .query_row(
            "SELECT last_seen_scan_id, missing FROM usage_source WHERE file_key = ?1",
            [source_key.as_bytes().as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("source presence")
}

#[test]
fn scan_scope_and_manifest_are_bounded_provider_qualified_values() {
    let codex = ScanScope::new("codex", "default").expect("codex scope");
    let hermes = ScanScope::new("hermes", "default").expect("same profile, other provider");
    assert_eq!(codex.provider_id(), "codex");
    assert_eq!(codex.profile_id(), "default");
    assert_ne!(codex, hermes);

    for (provider, profile) in [
        ("", "default"),
        ("codex", ""),
        ("codex/unsafe", "default"),
        ("codex", "private path"),
    ] {
        assert_eq!(
            ScanScope::new(provider, profile)
                .expect_err("invalid scope")
                .code(),
            StoreErrorCode::InvalidValue
        );
    }
    assert_eq!(
        ScanScope::new("p".repeat(65), "default")
            .expect_err("provider too long")
            .code(),
        StoreErrorCode::InvalidValue
    );
    assert_eq!(
        ScanScope::new("codex", "p".repeat(129))
            .expect_err("profile too long")
            .code(),
        StoreErrorCode::InvalidValue
    );

    let manifest = ScanSetManifest::new(vec![hermes.clone(), codex.clone()].into_boxed_slice())
        .expect("sorted manifest");
    assert_eq!(manifest.scope_count(), 2);
    assert_eq!(manifest.scopes(), &[codex.clone(), hermes.clone()]);
    assert_eq!(
        ScanSetManifest::new(Box::new([]))
            .expect_err("empty manifest")
            .code(),
        StoreErrorCode::InvalidValue
    );
    assert_eq!(
        ScanSetManifest::new(vec![codex.clone(), codex].into_boxed_slice())
            .expect_err("duplicate scope")
            .code(),
        StoreErrorCode::InvalidValue
    );

    let oversized = (0..=MAX_SCAN_SCOPES)
        .map(|index| ScanScope::new("codex", format!("profile-{index}")))
        .collect::<Result<Vec<_>, _>>()
        .expect("valid individual scopes");
    let error = ScanSetManifest::new(oversized.into_boxed_slice()).expect_err("oversized manifest");
    assert_eq!(error.code(), StoreErrorCode::CapacityExceeded);
    assert_eq!(error.limit(), Some(MAX_SCAN_SCOPES as u64));

    let debug = format!("{manifest:?}");
    assert!(debug.contains("scope_count"));
    assert!(!debug.contains("default"));
}

#[test]
fn scan_ids_outcomes_and_counters_are_typed_and_checked() {
    assert_eq!(ScanSetId::new(7).expect("scan set ID").get(), 7);
    assert_eq!(ScanId::new(11).expect("scan ID").get(), 11);
    assert_eq!(
        ScanSetId::new(i64::MAX as u64 + 1)
            .expect_err("scan set ID overflow")
            .code(),
        StoreErrorCode::InvalidValue
    );
    assert_eq!(
        ScanId::new(i64::MAX as u64 + 1)
            .expect_err("scan ID overflow")
            .code(),
        StoreErrorCode::InvalidValue
    );

    for outcome in [
        ScanOutcome::Complete,
        ScanOutcome::Partial,
        ScanOutcome::Cancelled,
        ScanOutcome::Failed,
        ScanOutcome::TimedOut,
    ] {
        assert_ne!(format!("{outcome:?}"), "Running");
    }

    let counters = ScanCounters::new(3, 4, 5, 6).expect("bounded counters");
    assert_eq!(counters.files_read(), 3);
    assert_eq!(counters.bytes_read(), 4);
    assert_eq!(counters.events_observed(), 5);
    assert_eq!(counters.diagnostics(), 6);
    assert_eq!(
        ScanCounters::default(),
        ScanCounters::new(0, 0, 0, 0).unwrap()
    );
    assert_eq!(
        ScanCounters::new(0, i64::MAX as u64 + 1, 0, 0)
            .expect_err("counter overflow")
            .code(),
        StoreErrorCode::InvalidValue
    );
}

#[test]
fn scan_set_lifecycle_is_provider_scoped_and_complete_only() {
    let mut store = UsageStore::in_memory().expect("usage store");
    let codex_seen = register_source(&mut store, 1, "codex", "default");
    let _codex_unseen = register_source(&mut store, 2, "codex", "default");
    let hermes_seen = register_source(&mut store, 3, "hermes", "default");
    let manifest = ScanSetManifest::new(
        vec![
            ScanScope::new("hermes", "default").unwrap(),
            ScanScope::new("codex", "default").unwrap(),
        ]
        .into_boxed_slice(),
    )
    .expect("scan manifest");

    let running = store
        .begin_scan_set(&manifest, 1_000)
        .expect("begin scan set");
    assert_eq!(running.expected_scope_count(), 2);
    assert_eq!(running.outcome(), None);
    assert_eq!(
        store
            .begin_scan_set(&manifest, 1_001)
            .expect_err("second running set")
            .code(),
        StoreErrorCode::ScanInProgress
    );

    let scans = store
        .scan_page(running.id(), None, usize::MAX)
        .expect("scan page");
    assert_eq!(scans.len(), 2);
    assert_eq!(scans[0].scope().provider_id(), "codex");
    assert_eq!(scans[1].scope().provider_id(), "hermes");
    assert_eq!(scans[0].outcome(), None);
    let codex_scan = scans[0].id();
    let hermes_scan = scans[1].id();

    store
        .observe_scan_source(codex_scan, codex_seen)
        .expect("observe codex source");
    store
        .observe_scan_source(codex_scan, codex_seen)
        .expect("duplicate observation is idempotent");
    assert_eq!(
        store
            .observe_scan_source(codex_scan, hermes_seen)
            .expect_err("foreign scope")
            .code(),
        StoreErrorCode::InvalidValue
    );
    store
        .observe_scan_source(hermes_scan, hermes_seen)
        .expect("observe hermes source");

    let codex_done = store
        .finish_scan(
            codex_scan,
            ScanOutcome::Complete,
            1_100,
            ScanCounters::new(2, 30, 4, 0).unwrap(),
        )
        .expect("finish complete scope");
    assert_eq!(codex_done.sources_seen(), 1);
    assert_eq!(codex_done.outcome(), Some(ScanOutcome::Complete));
    assert_eq!(
        store
            .observe_scan_source(codex_scan, codex_seen)
            .expect_err("closed scan")
            .code(),
        StoreErrorCode::StaleScan
    );

    let hermes_done = store
        .finish_scan(
            hermes_scan,
            ScanOutcome::Partial,
            1_120,
            ScanCounters::new(1, 10, 1, 1).unwrap(),
        )
        .expect("finish partial scope");
    assert_eq!(hermes_done.sources_seen(), 1);
    let set_done = store
        .finish_scan_set(running.id(), 1_130)
        .expect("finish scan set");
    assert_eq!(set_done.outcome(), Some(ScanOutcome::Partial));
    assert_eq!(
        store
            .finish_scan_set(running.id(), 1_140)
            .expect_err("set closes once")
            .code(),
        StoreErrorCode::StaleScan
    );
}

#[test]
fn incomplete_scan_set_cannot_close_and_complete_scan_restores_missing_source() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory.path().join("scan-private.sqlite3");
    let mut store = UsageStore::open(&path).expect("usage store");
    let source = register_source(&mut store, 7, "codex", "default");
    let manifest =
        ScanSetManifest::new(vec![ScanScope::new("codex", "default").unwrap()].into_boxed_slice())
            .unwrap();

    let first = store.begin_scan_set(&manifest, 2_000).unwrap();
    let first_scan = store.scan_page(first.id(), None, 1).unwrap()[0].id();
    store
        .finish_scan(
            first_scan,
            ScanOutcome::Complete,
            2_010,
            ScanCounters::default(),
        )
        .unwrap();
    assert_eq!(
        store.finish_scan_set(first.id(), 2_020).unwrap().outcome(),
        Some(ScanOutcome::Complete)
    );
    drop(store);
    assert_eq!(source_presence(&path, source), (None, 1));

    let mut store = UsageStore::open(&path).expect("reopen missing source");
    let second = store.begin_scan_set(&manifest, 3_000).unwrap();
    let second_scan = store.scan_page(second.id(), None, 1).unwrap()[0].id();
    assert_eq!(
        store
            .finish_scan_set(second.id(), 3_001)
            .expect_err("running child blocks set close")
            .code(),
        StoreErrorCode::PendingScan
    );
    store.observe_scan_source(second_scan, source).unwrap();
    store
        .finish_scan(
            second_scan,
            ScanOutcome::Partial,
            3_010,
            ScanCounters::default(),
        )
        .unwrap();
    assert_eq!(
        store.finish_scan_set(second.id(), 3_020).unwrap().outcome(),
        Some(ScanOutcome::Partial)
    );
    drop(store);
    assert_eq!(
        source_presence(&path, source),
        (Some(second_scan.get() as i64), 1),
        "partial scan records seen evidence but cannot restore missing state"
    );

    let mut store = UsageStore::open(&path).expect("reopen partial source");
    let third = store.begin_scan_set(&manifest, 4_000).unwrap();
    let third_scan = store.scan_page(third.id(), None, 1).unwrap()[0].id();
    store.observe_scan_source(third_scan, source).unwrap();
    store
        .finish_scan(
            third_scan,
            ScanOutcome::Complete,
            4_010,
            ScanCounters::default(),
        )
        .unwrap();
    assert_eq!(
        store.finish_scan_set(third.id(), 4_020).unwrap().outcome(),
        Some(ScanOutcome::Complete)
    );
    drop(store);
    assert_eq!(
        source_presence(&path, source),
        (Some(third_scan.get() as i64), 0)
    );
}

#[test]
fn source_registered_after_complete_scan_starts_missing_until_observed() {
    let directory = TempDir::new().expect("temporary directory");
    let path = directory
        .path()
        .join("post-scan-registration-private.sqlite3");
    let manifest =
        ScanSetManifest::new(vec![ScanScope::new("codex", "default").unwrap()].into_boxed_slice())
            .unwrap();
    let mut store = UsageStore::open(&path).expect("usage store");
    let scan_set = store.begin_scan_set(&manifest, 5_000).unwrap();
    let scan = store.scan_page(scan_set.id(), None, 1).unwrap()[0].id();
    store
        .finish_scan(scan, ScanOutcome::Complete, 5_010, ScanCounters::default())
        .unwrap();
    store.finish_scan_set(scan_set.id(), 5_020).unwrap();

    let late_source = register_source(&mut store, 9, "codex", "default");
    drop(store);
    assert_eq!(
        source_presence(&path, late_source),
        (None, 1),
        "a registration cannot invent presence after complete scan authority"
    );
}
