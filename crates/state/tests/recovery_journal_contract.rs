use std::fs;

use tempfile::TempDir;
use tokenmaster_platform::{ArchiveRecoveryScope, ExclusiveFileLease, ValidatedLocalDirectory};
use tokenmaster_state::{
    PortableSettingsTarget, RecoveryArchiveFacts, RecoveryBackupIdentity,
    RecoveryCandidateIdentity, RecoveryJournal, RecoveryJournalLoad, RecoveryJournalStore,
    RecoveryPhase, RecoverySettingsMode, StateErrorCode,
};

struct Fixture {
    _root: TempDir,
    reliable: ValidatedLocalDirectory,
    journal: RecoveryJournalStore,
    operation: tokenmaster_platform::RecoveryOperation,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary root");
        let reliable_path = root.path().join("reliable-state");
        fs::create_dir(&reliable_path).expect("reliable-state");
        let data = ValidatedLocalDirectory::new(root.path()).expect("data root");
        let reliable = ValidatedLocalDirectory::new(&reliable_path).expect("reliable-state root");
        let scope = ArchiveRecoveryScope::new(&data, &reliable).expect("recovery scope");
        let lease = ExclusiveFileLease::for_archive(&root.path().join("tokenmaster.sqlite3"))
            .expect("lease")
            .try_acquire()
            .expect("guard");
        let operation = scope.reserve_operation(&lease).expect("operation");
        let journal = RecoveryJournalStore::new(&reliable).expect("journal store");
        Self {
            _root: root,
            reliable,
            journal,
            operation,
        }
    }

    fn prepared_at(&self, operation_generation: u64, settings: bool) -> RecoveryJournal {
        let backup =
            RecoveryBackupIdentity::from_persisted(2, 4096, [0x31; 32]).expect("backup identity");
        let candidate = RecoveryCandidateIdentity::from_persisted(12, 8192, [0x42; 32])
            .expect("candidate identity");
        let facts = RecoveryArchiveFacts::from_persisted(
            Some((16384, [0x53; 32])),
            Some((4096, [0x64; 32])),
            None,
        )
        .expect("archive facts");
        let target = settings.then(|| {
            PortableSettingsTarget::from_persisted(9, [0x75; 32]).expect("settings target")
        });
        RecoveryJournal::manual(
            operation_generation,
            self.operation.id(),
            backup,
            candidate,
            facts,
            target,
            1,
        )
        .expect("prepared journal")
    }

    fn prepared(&self, settings: bool) -> RecoveryJournal {
        self.prepared_at(1, settings)
    }
}

#[test]
fn prepared_is_durable_before_any_phase_can_advance() {
    let fixture = Fixture::new();
    let prepared = fixture.prepared(false);
    assert_eq!(prepared.phase(), RecoveryPhase::Prepared);
    assert_eq!(prepared.settings_mode(), RecoverySettingsMode::DataOnly);

    fixture.journal.begin(&prepared).expect("begin");
    let loaded = match fixture.journal.load().expect("load") {
        RecoveryJournalLoad::Pending(value) => value,
        other => panic!("expected pending journal, got {other:?}"),
    };
    assert_eq!(loaded, prepared);
    assert_eq!(format!("{loaded:?}"), "RecoveryJournal([redacted])");
}

#[test]
fn only_the_exact_six_state_sequence_is_accepted_and_same_step_is_idempotent() {
    let fixture = Fixture::new();
    let mut current = fixture.prepared(true);
    fixture.journal.begin(&current).expect("begin");
    assert_eq!(
        current.settings_mode(),
        RecoverySettingsMode::DataAndPortableSettings
    );

    let skip = fixture
        .journal
        .advance(&current, RecoveryPhase::MainReplaced)
        .expect_err("phase skip");
    assert_eq!(skip.code(), StateErrorCode::Integrity);

    for phase in [
        RecoveryPhase::SidecarsQuarantined,
        RecoveryPhase::MainReplaced,
        RecoveryPhase::ReopenedVerified,
        RecoveryPhase::SettingsPublished,
        RecoveryPhase::Complete,
    ] {
        current = fixture
            .journal
            .advance(&current, phase)
            .expect("exact advance");
        let same = fixture
            .journal
            .advance(&current, phase)
            .expect("same phase is idempotent");
        assert_eq!(same, current);
    }
}

#[test]
fn completed_operation_allows_one_new_redundant_prepared_generation() {
    let fixture = Fixture::new();
    let mut current = fixture.prepared(false);
    fixture.journal.begin(&current).expect("first begin");
    for phase in [
        RecoveryPhase::SidecarsQuarantined,
        RecoveryPhase::MainReplaced,
        RecoveryPhase::ReopenedVerified,
        RecoveryPhase::SettingsPublished,
        RecoveryPhase::Complete,
    ] {
        current = fixture
            .journal
            .advance(&current, phase)
            .expect("complete first operation");
    }

    let second = fixture.prepared_at(2, false);
    fixture.journal.begin(&second).expect("second begin");
    fs::write(
        fixture.reliable.as_path().join("recovery-a.tms"),
        b"corrupt-one-new-slot",
    )
    .expect("corrupt one second-operation slot");
    let fallback = match fixture.journal.load().expect("second fallback") {
        RecoveryJournalLoad::Pending(journal) => journal,
        other => panic!("expected second prepared fallback, got {other:?}"),
    };
    assert_eq!(fallback, second);
    assert_eq!(fallback.phase(), RecoveryPhase::Prepared);
}

#[test]
fn one_corrupt_slot_falls_back_but_two_invalid_slots_are_explicitly_unsafe() {
    let fixture = Fixture::new();
    let prepared = fixture.prepared(false);
    fixture.journal.begin(&prepared).expect("begin");
    fixture
        .journal
        .advance(&prepared, RecoveryPhase::SidecarsQuarantined)
        .expect("advance");

    fs::write(
        fixture.reliable.as_path().join("recovery-a.tms"),
        b"corrupt",
    )
    .expect("corrupt newest slot");
    let fallback = match fixture.journal.load().expect("fallback load") {
        RecoveryJournalLoad::Pending(value) => value,
        other => panic!("expected fallback journal, got {other:?}"),
    };
    assert_eq!(fallback.phase(), RecoveryPhase::Prepared);

    fs::write(
        fixture.reliable.as_path().join("recovery-b.tms"),
        b"also-corrupt",
    )
    .expect("corrupt peer slot");
    assert!(matches!(
        fixture.journal.load().expect("contained invalid slots"),
        RecoveryJournalLoad::Invalid
    ));
}

#[test]
fn missing_journal_is_distinct_from_invalid_artifacts() {
    let fixture = Fixture::new();
    assert!(matches!(
        fixture.journal.load().expect("empty journal"),
        RecoveryJournalLoad::Absent
    ));
    fs::write(
        fixture.reliable.as_path().join("recovery-a.tms"),
        b"unknown",
    )
    .expect("invalid artifact");
    assert!(matches!(
        fixture.journal.load().expect("invalid journal"),
        RecoveryJournalLoad::Invalid
    ));
}

#[test]
fn automatic_recovery_is_always_data_only_and_attempts_are_bounded() {
    let fixture = Fixture::new();
    let backup = RecoveryBackupIdentity::from_persisted(0, 4096, [1; 32]).expect("backup identity");
    let candidate =
        RecoveryCandidateIdentity::from_persisted(12, 4096, [2; 32]).expect("candidate identity");
    let facts =
        RecoveryArchiveFacts::from_persisted(Some((4096, [3; 32])), None, None).expect("facts");
    let journal =
        RecoveryJournal::automatic(1, fixture.operation.id(), backup, candidate, facts, 2)
            .expect("automatic journal");
    assert_eq!(
        journal.settings_mode(),
        RecoverySettingsMode::AutomaticDataOnly
    );
    assert!(journal.settings_target().is_none());

    assert_eq!(
        RecoveryJournal::automatic(1, fixture.operation.id(), backup, candidate, facts, 3,)
            .expect_err("third automatic attempt"),
        tokenmaster_state::StateError::from_code(StateErrorCode::CapacityExceeded)
    );
}
