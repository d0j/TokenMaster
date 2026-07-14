use tokenmaster_store::{EXPECTED_SQLITE_VERSION, ProbeStore};

#[test]
fn bundled_sqlite_matches_reviewed_version() {
    let store = ProbeStore::in_memory().expect("store");
    assert_eq!(
        store.sqlite_version().expect("version"),
        EXPECTED_SQLITE_VERSION
    );
    assert_eq!(EXPECTED_SQLITE_VERSION, "3.53.2");
}

#[test]
fn keyset_pages_are_newest_first_and_capped() {
    let mut store = ProbeStore::in_memory().expect("store");
    store.seed_sessions(1_001).expect("seed");

    let first = store.page_before(None, usize::MAX).expect("page");
    assert_eq!(first.len(), 256);
    assert_eq!(first[0].id, 1_001);

    let second = store
        .page_before(first.last().map(|row| row.id), 20)
        .expect("page");
    assert_eq!(second.len(), 20);
    assert!(second[0].id < first[255].id);
}

#[test]
fn file_store_reopens_without_recreating_data() {
    let directory = tempfile::tempdir().expect("temp directory");
    let path = directory.path().join("probe.sqlite3");
    {
        let mut store = ProbeStore::open(&path).expect("first open");
        store.seed_sessions(3).expect("seed");
    }

    let reopened = ProbeStore::open(&path).expect("second open");
    assert_eq!(reopened.session_count().expect("count"), 3);
}

#[test]
fn seed_count_is_fail_closed_above_one_million() {
    let mut store = ProbeStore::in_memory().expect("store");
    assert!(store.seed_sessions(1_000_001).is_err());
    assert_eq!(store.session_count().expect("count"), 0);
}

#[test]
#[ignore = "M0 scale gate; run explicitly"]
fn one_million_rows_remain_page_bounded() {
    let mut store = ProbeStore::in_memory().expect("store");
    store.seed_sessions(1_000_000).expect("seed");
    assert_eq!(store.session_count().expect("count"), 1_000_000);
    assert_eq!(store.page_before(None, 10_000).expect("page").len(), 256);
}
