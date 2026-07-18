use std::fs;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tempfile::TempDir;

use tokenmaster_platform::{
    ArchiveRecoveryScope, BackupDirectory, DURABLE_STAGE_ATTEMPTS, DurableFileTarget,
    ExclusiveFileLease, ValidatedLocalDirectory,
};
use tokenmaster_state::{
    BootstrapOutcome, PriorRunCondition, RecoveryCandidateIdentity, RecoveryJournalStore,
    RecoveryLaunchDecision, RunStateStore, SettingsStore, StateBootstrap,
};
use tokenmaster_store::{BackupControl, BackupStaging, UsageStore};

struct Fixture {
    _root: TempDir,
    reliable: ValidatedLocalDirectory,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary root");
        let reliable_path = root.path().join("reliable-state");
        fs::create_dir(&reliable_path).expect("reliable-state");
        let reliable =
            ValidatedLocalDirectory::new(&reliable_path).expect("validated reliable-state");
        Self {
            _root: root,
            reliable,
        }
    }

    fn store(&self) -> RunStateStore {
        RunStateStore::new(&self.reliable).expect("run-state store")
    }
}

fn candidate(seed: u8) -> RecoveryCandidateIdentity {
    RecoveryCandidateIdentity::from_persisted(13, 4096, [seed; 32]).expect("candidate identity")
}

#[test]
fn startup_publishes_unclean_and_clean_is_an_explicit_generation() {
    let fixture = Fixture::new();
    let store = fixture.store();
    assert_eq!(
        store.inspect().expect("initial inspection").condition(),
        PriorRunCondition::Missing
    );

    let mut session = store.begin().expect("first startup");
    assert_eq!(session.prior().condition(), PriorRunCondition::Missing);
    assert!(session.prior().requires_quick_check());
    assert_eq!(
        store.inspect().expect("current run").condition(),
        PriorRunCondition::Unclean
    );
    session.authorize_healthy_launch();
    session.mark_clean().expect("clean shutdown");

    let clean = store.inspect().expect("clean inspection");
    assert_eq!(clean.condition(), PriorRunCondition::Clean);
    assert!(!clean.requires_quick_check());
    let next = store.begin().expect("next startup");
    assert_eq!(next.prior().condition(), PriorRunCondition::Clean);
}

#[test]
fn missing_invalid_and_fallback_records_all_require_quick_check() {
    let fixture = Fixture::new();
    let store = fixture.store();
    assert!(store.inspect().expect("missing").requires_quick_check());

    fs::write(fixture.reliable.as_path().join("run-a.tms"), b"invalid")
        .expect("invalid first slot");
    assert_eq!(
        store.inspect().expect("invalid").condition(),
        PriorRunCondition::Invalid
    );
    assert!(store.inspect().expect("invalid").requires_quick_check());

    let mut session = store.begin().expect("explicit invalid-slot recovery");
    session.authorize_healthy_launch();
    session.mark_clean().expect("clean after recovered record");
    fs::write(
        fixture.reliable.as_path().join("run-a.tms"),
        b"invalid-again",
    )
    .expect("corrupt older peer");
    assert_eq!(
        store.inspect().expect("fallback").condition(),
        PriorRunCondition::Invalid
    );
    assert!(store.inspect().expect("fallback").requires_quick_check());
}

#[test]
fn stale_session_cannot_mark_a_newer_startup_clean() {
    let fixture = Fixture::new();
    let first_store = fixture.store();
    let mut first = first_store.begin().expect("first startup");
    first.authorize_healthy_launch();
    let second_store = fixture.store();
    let _second = second_store.begin().expect("second startup");

    assert!(first.mark_clean().is_err());
    assert_eq!(
        second_store
            .inspect()
            .expect("newer state retained")
            .condition(),
        PriorRunCondition::Unclean
    );
}

#[test]
fn run_session_owns_its_fixed_record_capability_through_joined_shutdown() {
    let fixture = Fixture::new();
    let mut session = {
        let store = fixture.store();
        store.begin().expect("startup")
    };

    session.authorize_healthy_launch();
    session.mark_clean().expect("clean after owner scope ended");
    assert_eq!(
        fixture
            .store()
            .inspect()
            .expect("clean inspection")
            .condition(),
        PriorRunCondition::Clean
    );
}

#[test]
fn restored_candidate_is_limited_to_two_unclean_launches_and_clean_clears_it() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let restored = candidate(7);

    {
        let mut first = store.begin().expect("restore startup");
        assert_eq!(
            first
                .start_recovered_candidate(1, restored)
                .expect("first launch"),
            RecoveryLaunchDecision::Start { launch: 1 }
        );
    }

    {
        let mut second = store.begin().expect("second startup");
        assert_eq!(second.prior().recovery_launches(), Some(1));
        assert_eq!(
            second
                .continue_recovered_candidate()
                .expect("second launch"),
            RecoveryLaunchDecision::Start { launch: 2 }
        );
    }

    let mut third = store.begin().expect("third startup");
    assert_eq!(third.prior().recovery_launches(), Some(2));
    assert_eq!(
        third.continue_recovered_candidate().expect("cutoff"),
        RecoveryLaunchDecision::SafeMode { failed_launches: 2 }
    );
    assert!(third.mark_clean().is_err());
    assert_eq!(
        third
            .start_recovered_candidate(2, candidate(8))
            .expect("different verified recovery"),
        RecoveryLaunchDecision::Start { launch: 1 }
    );
    third.mark_clean().expect("joined clean close");
    assert_eq!(
        store
            .inspect()
            .expect("tracking cleared")
            .recovery_launches(),
        None
    );
    let mut accepted = store.begin().expect("accepted recovery startup");
    assert_eq!(
        accepted
            .start_recovered_candidate(2, candidate(8))
            .expect("accepted recovery generation"),
        RecoveryLaunchDecision::AlreadyAccepted {
            operation_generation: 2
        }
    );
    accepted.mark_clean().expect("accepted clean close");
}

#[test]
fn run_state_debug_and_records_do_not_expose_candidate_identity() {
    let fixture = Fixture::new();
    let store = fixture.store();
    let mut session = store.begin().expect("startup");
    session
        .start_recovered_candidate(1, candidate(9))
        .expect("tracked launch");

    assert_eq!(format!("{store:?}"), "RunStateStore([redacted])");
    assert!(!format!("{session:?}").contains(&"09".repeat(32)));
    let bytes = DurableFileTarget::exact_child(&fixture.reliable, "run-b.tms")
        .expect("run slot")
        .read_bounded(1024 * 1024)
        .expect("run bytes");
    assert!(bytes.is_some());
}

struct BootstrapFixture {
    _root: TempDir,
    data: ValidatedLocalDirectory,
    reliable: ValidatedLocalDirectory,
    scope: ArchiveRecoveryScope,
    backups: BackupDirectory,
    verification_staging: BackupStaging,
    settings: SettingsStore,
    journal: RecoveryJournalStore,
    run_state: RunStateStore,
    control: BackupControl,
}

impl BootstrapFixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("bootstrap root");
        let reliable_path = root.path().join("reliable-state");
        fs::create_dir(&reliable_path).expect("reliable-state");
        let data = ValidatedLocalDirectory::new(root.path()).expect("data root");
        let reliable = ValidatedLocalDirectory::new(&reliable_path).expect("reliable-state root");
        let scope = ArchiveRecoveryScope::new(&data, &reliable).expect("recovery scope");
        let backups = BackupDirectory::open_or_create(&reliable).expect("backup directory");
        let staging_root =
            ValidatedLocalDirectory::new(&reliable_path.join("staging")).expect("staging root");
        let verification_staging = BackupStaging::new(&staging_root).expect("verification staging");
        let settings = SettingsStore::new(&reliable).expect("settings store");
        let journal = RecoveryJournalStore::new(&reliable).expect("journal store");
        let run_state = RunStateStore::new(&reliable).expect("run-state store");
        let control = BackupControl::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(30))
            .expect("backup control");
        Self {
            _root: root,
            data,
            reliable,
            scope,
            backups,
            verification_staging,
            settings,
            journal,
            run_state,
            control,
        }
    }

    fn guard(&self) -> tokenmaster_platform::ExclusiveFileLeaseGuard {
        ExclusiveFileLease::for_archive(&self.data.as_path().join("tokenmaster.sqlite3"))
            .expect("archive lease")
            .try_acquire()
            .expect("held archive lease")
    }

    fn bootstrap(&self) -> StateBootstrap<'_> {
        StateBootstrap::new(
            &self.data,
            &self.scope,
            &self.verification_staging,
            &self.journal,
            &self.settings,
            &self.run_state,
            &self.backups,
        )
        .expect("bound bootstrap capabilities")
    }

    fn archive(&self) -> std::path::PathBuf {
        self.data.as_path().join("tokenmaster.sqlite3")
    }
}

#[test]
fn first_install_is_distinct_from_a_missing_damaged_archive() {
    let first = BootstrapFixture::new();
    let guard = first.guard();
    let prepared = first
        .bootstrap()
        .prepare(&guard, &first.control)
        .expect("first-install diagnosis");
    assert_eq!(prepared.report().outcome(), BootstrapOutcome::FirstInstall);
    assert!(!first.archive().exists());

    let damaged = BootstrapFixture::new();
    {
        let _prior = damaged.run_state.begin().expect("prior run marker");
    }
    let guard = damaged.guard();
    let prepared = damaged
        .bootstrap()
        .prepare(&guard, &damaged.control)
        .expect("damaged diagnosis");
    assert_eq!(
        prepared.report().outcome(),
        BootstrapOutcome::RecoveryRequired
    );
    assert!(!damaged.archive().exists());
}

#[test]
fn bootstrap_rejects_cross_root_capability_composition_before_run_state_mutation() {
    let left = BootstrapFixture::new();
    let right = BootstrapFixture::new();

    assert!(
        StateBootstrap::new(
            &left.data,
            &left.scope,
            &right.verification_staging,
            &right.journal,
            &right.settings,
            &right.run_state,
            &right.backups,
        )
        .is_err()
    );
    assert_eq!(
        left.run_state
            .inspect()
            .expect("left run state")
            .condition(),
        PriorRunCondition::Missing
    );
    assert_eq!(
        right
            .run_state
            .inspect()
            .expect("right run state")
            .condition(),
        PriorRunCondition::Missing
    );

    assert!(
        StateBootstrap::new(
            &left.data,
            &left.scope,
            &left.verification_staging,
            &left.journal,
            &left.settings,
            &left.run_state,
            &right.backups,
        )
        .is_err()
    );
}

#[test]
fn clean_and_unclean_current_startups_select_normal_and_quick_validation() {
    let clean = BootstrapFixture::new();
    drop(UsageStore::open(clean.archive()).expect("current archive"));
    let mut prior = clean.run_state.begin().expect("prior startup");
    prior.authorize_healthy_launch();
    prior.mark_clean().expect("prior clean shutdown");
    let guard = clean.guard();
    let mut prepared = clean
        .bootstrap()
        .prepare(&guard, &clean.control)
        .expect("clean bootstrap");
    assert_eq!(prepared.report().outcome(), BootstrapOutcome::Healthy);
    assert!(!prepared.report().quick_check_performed());
    prepared
        .session_mut()
        .mark_clean()
        .expect("joined clean close");

    let unclean = BootstrapFixture::new();
    drop(UsageStore::open(unclean.archive()).expect("current archive"));
    {
        let _prior = unclean.run_state.begin().expect("unclean prior startup");
    }
    let guard = unclean.guard();
    let prepared = unclean
        .bootstrap()
        .prepare(&guard, &unclean.control)
        .expect("unclean bootstrap");
    assert_eq!(prepared.report().outcome(), BootstrapOutcome::Healthy);
    assert!(prepared.report().quick_check_performed());
}

#[test]
fn corrupt_archive_is_never_opened_or_rewritten_as_healthy() {
    let corrupt = BootstrapFixture::new();
    fs::write(corrupt.archive(), b"corrupt-active").expect("corrupt archive");
    let before = fs::read(corrupt.archive()).expect("corrupt bytes");
    let guard = corrupt.guard();
    let prepared = corrupt
        .bootstrap()
        .prepare(&guard, &corrupt.control)
        .expect("corruption diagnosis");
    assert_eq!(
        prepared.report().outcome(),
        BootstrapOutcome::RecoveryRequired
    );
    assert!(!prepared.report().quick_check_performed());
    assert_eq!(fs::read(corrupt.archive()).expect("preserved"), before);
}

#[test]
fn failed_unclean_publication_stops_before_archive_validation() {
    let fixture = BootstrapFixture::new();
    fs::write(fixture.archive(), b"unchanged-corrupt-active").expect("active bytes");
    for attempt in 0..DURABLE_STAGE_ATTEMPTS {
        fs::write(
            fixture
                .reliable
                .as_path()
                .join(format!(".run-a.tms.tokenmaster-stage-{attempt:02}")),
            b"occupied",
        )
        .expect("occupied run stage");
    }
    let guard = fixture.guard();

    assert!(
        fixture
            .bootstrap()
            .prepare(&guard, &fixture.control)
            .is_err()
    );
    assert_eq!(
        fs::read(fixture.archive()).expect("unchanged archive"),
        b"unchanged-corrupt-active"
    );
}

#[test]
fn unknown_staging_evidence_is_preserved_and_enters_safe_mode_after_exact_cleanup() {
    let fixture = BootstrapFixture::new();
    let unknown = fixture
        .reliable
        .as_path()
        .join("staging")
        .join("unknown.bin");
    fs::write(&unknown, b"unknown-evidence").expect("unknown staging evidence");
    let guard = fixture.guard();

    let prepared = fixture
        .bootstrap()
        .prepare(&guard, &fixture.control)
        .expect("safe-mode diagnosis");

    assert_eq!(prepared.report().outcome(), BootstrapOutcome::SafeMode);
    assert_eq!(
        fs::read(unknown).expect("preserved evidence"),
        b"unknown-evidence"
    );
    assert!(!fixture.archive().exists());
}
