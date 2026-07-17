use tokenmaster_state::{StateError, StateErrorCode, StateLimits};

#[test]
fn error_codes_are_fixed_serializable_and_path_private() {
    let cases = [
        (StateErrorCode::InvalidInput, "invalid_input"),
        (StateErrorCode::UnsupportedVersion, "unsupported_version"),
        (StateErrorCode::CapacityExceeded, "capacity_exceeded"),
        (StateErrorCode::Integrity, "integrity"),
        (StateErrorCode::Unavailable, "unavailable"),
        (StateErrorCode::Busy, "busy"),
        (StateErrorCode::DiskCapacity, "disk_capacity"),
        (StateErrorCode::RecoveryRequired, "recovery_required"),
        (StateErrorCode::InternalInvariant, "internal_invariant"),
    ];

    for (code, stable) in cases {
        assert_eq!(code.as_str(), stable);
        assert_eq!(
            serde_json::to_string(&code).unwrap(),
            format!("\"{stable}\"")
        );

        let error = StateError::from_code(code);
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), format!("state error: {stable}"));

        let debug = format!("{error:?}");
        assert!(!debug.contains('/'));
        assert!(!debug.contains('\\'));
        assert!(!debug.contains("PRIVATE_STATE_PATH"));
    }
}

#[test]
fn byte_and_item_bounds_accept_limits_and_reject_excess_or_overflow() {
    let limits = StateLimits::new(1_024, 8);

    assert_eq!(limits.max_bytes(), 1_024);
    assert_eq!(limits.max_items(), 8);
    assert_eq!(limits.checked_bytes(1_000, 24), Ok(1_024));
    assert_eq!(limits.checked_items(5, 3), Ok(8));

    for result in [
        limits.checked_bytes(1_024, 1).map(|_| ()),
        limits.checked_bytes(u64::MAX, 1).map(|_| ()),
        limits.checked_items(8, 1).map(|_| ()),
        limits.checked_items(usize::MAX, 1).map(|_| ()),
    ] {
        assert_eq!(
            result.expect_err("excess or overflow must fail").code(),
            StateErrorCode::CapacityExceeded
        );
    }
}
