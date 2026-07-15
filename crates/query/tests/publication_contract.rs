use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use tokenmaster_query::{
    DatasetIdentity, PublicationGeneration, PublishOutcome, QueryEnvelope, QueryFreshness,
    QueryHeader, QueryHeaderParts, QueryQuality, QuerySnapshotSlot, SnapshotGeneration,
};

struct DropProbe(Arc<AtomicUsize>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

fn envelope<T>(generation: u64, payload: T) -> QueryEnvelope<T> {
    QueryEnvelope::new(
        QueryHeader::new(QueryHeaderParts {
            snapshot_generation: SnapshotGeneration::new(generation).expect("generation"),
            publication_generation: PublicationGeneration::new(1).expect("publication"),
            dataset_identity: DatasetIdentity::Empty,
            generated_at_ms: 1,
            data_through_ms: None,
            freshness: QueryFreshness::Unavailable,
            quality: QueryQuality::Authoritative,
            scopes: Vec::new(),
            warnings: Vec::new(),
        })
        .expect("header"),
        payload,
    )
}

#[test]
fn slot_rejects_older_and_coalesces_equal_without_replacement() {
    let mut slot = QuerySnapshotSlot::new();
    assert_eq!(
        slot.publish(envelope(5, "current")),
        PublishOutcome::Accepted
    );
    assert_eq!(
        slot.publish(envelope(4, "older")),
        PublishOutcome::RejectedOlder
    );
    assert_eq!(slot.current().expect("current").payload(), &"current");
    assert_eq!(
        slot.publish(envelope(5, "equal")),
        PublishOutcome::Coalesced
    );
    assert_eq!(slot.current().expect("current").payload(), &"current");
    assert_eq!(slot.publish(envelope(6, "newer")), PublishOutcome::Accepted);
    assert_eq!(slot.current().expect("current").payload(), &"newer");
    assert_eq!(slot.generation().expect("generation").get(), 6);
}

#[test]
fn ten_thousand_candidates_retain_only_one_payload() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut slot = QuerySnapshotSlot::new();
    for generation in 1..=10_000 {
        assert_eq!(
            slot.publish(envelope(generation, DropProbe(Arc::clone(&drops)))),
            PublishOutcome::Accepted
        );
    }
    assert_eq!(drops.load(Ordering::Relaxed), 9_999);
    assert_eq!(slot.generation().expect("generation").get(), 10_000);
    drop(slot);
    assert_eq!(drops.load(Ordering::Relaxed), 10_000);
}
