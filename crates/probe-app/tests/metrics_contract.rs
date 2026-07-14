use tokenmaster_m0::metrics::ProcessSample;

#[test]
fn current_process_sample_has_live_bounded_counters() {
    let sample = ProcessSample::capture().expect("process sample");
    assert!(sample.private_bytes > 0);
    assert!(sample.working_set_bytes > 0);
    assert!(sample.handle_count > 0);
    assert!(sample.thread_count > 0);
    assert!(sample.monotonic_ns > 0);
}
