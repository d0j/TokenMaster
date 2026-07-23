use std::path::Path;

use tempfile::TempDir;
use tokenmaster_codex::{CodexRootInput, ConfiguredCodexRoot, build_discovery_request};
use tokenmaster_engine::{
    Adapter, AdapterSourceState, Clock, DiscoveredSource, MonotonicTime, OperationControl,
    PortError, RefreshAdmission, RefreshCoordinator, RefreshUrgency, ReplaySourceSink,
    ScopeIdentity, ScopeSink, SinkControl, SourceBatchReader,
};
use tokenmaster_provider::RepositoryActivityHint;
use tokenmaster_runtime::CodexAdapter;

struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(0)
    }
}

#[derive(Default)]
struct ScopeCollector {
    scopes: Vec<ScopeIdentity>,
}

impl ScopeSink for ScopeCollector {
    fn on_scope(&mut self, scope: ScopeIdentity) -> Result<SinkControl, PortError> {
        self.scopes.push(scope);
        Ok(SinkControl::Continue)
    }
}

struct HintCollector<'a> {
    control: &'a OperationControl<'a>,
    latest: Option<RepositoryActivityHint>,
}

impl ReplaySourceSink for HintCollector<'_> {
    fn on_source(
        &mut self,
        _source: DiscoveredSource,
        initial_state: AdapterSourceState,
        reader: &mut dyn SourceBatchReader,
    ) -> Result<SinkControl, PortError> {
        let mut checkpoint = reader.restore_checkpoint(initial_state.progress(), self.control)?;
        loop {
            let batch = reader.read_batch(&checkpoint, self.control)?;
            if let Some(hint) = reader.take_repository_activity_hint() {
                self.latest = Some(hint);
            }
            let state = batch.state();
            checkpoint = batch.next_checkpoint().clone();
            if state == tokenmaster_engine::BatchState::SnapshotEnd {
                break;
            }
        }
        Ok(SinkControl::Continue)
    }
}

fn adapter(root: &Path) -> CodexAdapter {
    let configured = [ConfiguredCodexRoot::new(root, None, true)];
    let request = build_discovery_request(CodexRootInput {
        user_profile: None,
        codex_home: None,
        configured: &configured,
    })
    .expect("discovery request");
    CodexAdapter::new(request).expect("Codex adapter")
}

#[test]
fn codex_adapter_transports_one_private_hint_beside_usage_batches() {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().join("codex-root");
    let repository = directory
        .path()
        .join("PRIVATE_RUNTIME_PARENT")
        .join("project-runtime");
    std::fs::create_dir(&root).expect("Codex root");
    std::fs::create_dir_all(&repository).expect("repository");
    let content = format!(
        "{}\n{}\n",
        serde_json::json!({
            "timestamp": "2026-07-10T08:00:00Z",
            "type": "session_meta",
            "payload": {
                "id": "session-runtime",
                "cwd": repository,
                "requested_model": "gpt-test"
            }
        }),
        serde_json::json!({
            "timestamp": "2026-07-10T08:01:00Z",
            "usage": {"total_tokens": 1}
        })
    );
    std::fs::write(root.join("session.jsonl"), content).expect("source");

    let mut coordinator = RefreshCoordinator::new();
    let RefreshAdmission::Started(permit) = coordinator
        .submit(
            RefreshUrgency::Recovery,
            None,
            MonotonicTime::from_millis(0),
        )
        .expect("refresh")
    else {
        panic!("refresh starts");
    };
    let control = OperationControl::new(&permit, &FixedClock);
    let mut adapter = adapter(&root);
    let mut scopes = ScopeCollector::default();
    adapter
        .visit_scopes(&control, &mut scopes)
        .expect("visit scopes");
    assert_eq!(scopes.scopes.len(), 1);
    let mut hints = HintCollector {
        control: &control,
        latest: None,
    };
    adapter
        .visit_replay_sources(&scopes.scopes[0], &control, &mut hints)
        .expect("visit sources");
    let hint = hints.latest.expect("repository hint");

    assert_eq!(hint.provider_id().as_str(), "codex");
    assert_eq!(hint.session_id().as_str(), "session-runtime");
    assert_eq!(
        hint.project().map(|value| value.as_str()),
        Some("project-runtime")
    );
    assert_eq!(hint.observed_at().unix_seconds(), 1_783_670_460);
    assert_eq!(
        hint.candidate().as_path(),
        repository.canonicalize().unwrap()
    );
    assert!(!format!("{hint:?}").contains("PRIVATE_RUNTIME_PARENT"));
}

#[test]
fn invalid_cwd_does_not_block_usage_replay_or_reuse_a_repository_hint() {
    let directory = TempDir::new().expect("temporary directory");
    let root = directory.path().join("codex-root");
    std::fs::create_dir(&root).expect("Codex root");
    let content = format!(
        "{}\n{}\n",
        serde_json::json!({
            "timestamp": "2026-07-10T08:00:00Z",
            "type": "session_meta",
            "payload": {
                "id": "session-runtime",
                "cwd": "relative/private/path",
                "requested_model": "gpt-test"
            }
        }),
        serde_json::json!({
            "timestamp": "2026-07-10T08:01:00Z",
            "usage": {"total_tokens": 1}
        })
    );
    std::fs::write(root.join("session.jsonl"), content).expect("source");

    let mut coordinator = RefreshCoordinator::new();
    let RefreshAdmission::Started(permit) = coordinator
        .submit(
            RefreshUrgency::Recovery,
            None,
            MonotonicTime::from_millis(0),
        )
        .expect("refresh")
    else {
        panic!("refresh starts");
    };
    let control = OperationControl::new(&permit, &FixedClock);
    let mut adapter = adapter(&root);
    let mut scopes = ScopeCollector::default();
    adapter
        .visit_scopes(&control, &mut scopes)
        .expect("visit scopes");
    let mut hints = HintCollector {
        control: &control,
        latest: None,
    };
    adapter
        .visit_replay_sources(&scopes.scopes[0], &control, &mut hints)
        .expect("invalid cwd is a non-blocking side-channel diagnostic");
    assert!(hints.latest.is_none());
}
