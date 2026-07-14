use chrono::{DateTime, Datelike, Timelike, Utc};
use sha2::{Digest, Sha256};
use tokenmaster_domain::{
    EventFingerprint, ModelKey, TokenCount, TokenUsage, UsageProfileId, UtcTimestamp,
};

pub(crate) fn canonical_fingerprint(
    timestamp: &UtcTimestamp,
    model: &ModelKey,
    profile_id: &UsageProfileId,
    usage: &TokenUsage,
) -> Option<EventFingerprint> {
    let value =
        DateTime::<Utc>::from_timestamp(timestamp.unix_seconds(), timestamp.subsec_nanos())?;
    if !(0..=9_999).contains(&value.year()) {
        return None;
    }

    let mut hasher = Sha256::new();
    update_fixed_decimal(&mut hasher, u64::try_from(value.year()).ok()?, 4);
    hasher.update(b"-");
    update_fixed_decimal(&mut hasher, u64::from(value.month()), 2);
    hasher.update(b"-");
    update_fixed_decimal(&mut hasher, u64::from(value.day()), 2);
    hasher.update(b"T");
    update_fixed_decimal(&mut hasher, u64::from(value.hour()), 2);
    hasher.update(b":");
    update_fixed_decimal(&mut hasher, u64::from(value.minute()), 2);
    hasher.update(b":");
    update_fixed_decimal(&mut hasher, u64::from(value.second()), 2);
    update_fraction(&mut hasher, value.nanosecond());
    hasher.update(b"Z|");
    hasher.update(model.as_str().as_bytes());
    hasher.update(b"|");
    hasher.update(profile_id.as_str().as_bytes());
    for count in [
        usage.input(),
        usage.cached(),
        usage.output(),
        usage.reasoning(),
        usage.total(),
    ] {
        hasher.update(b"|");
        update_token_count(&mut hasher, count);
    }
    Some(EventFingerprint::new(hasher.finalize().into()))
}

fn update_fraction(hasher: &mut Sha256, nanos: u32) {
    if nanos == 0 {
        return;
    }
    let mut digits = [b'0'; 9];
    let mut value = nanos;
    for index in (0..digits.len()).rev() {
        digits[index] = b'0' + u8::try_from(value % 10).unwrap_or(0);
        value /= 10;
    }
    let mut end = digits.len();
    while end > 0 && digits[end - 1] == b'0' {
        end -= 1;
    }
    hasher.update(b".");
    hasher.update(&digits[..end]);
}

fn update_fixed_decimal(hasher: &mut Sha256, value: u64, width: usize) {
    let mut digits = [b'0'; 20];
    let start = digits.len().saturating_sub(width);
    let mut remaining = value;
    for index in (start..digits.len()).rev() {
        digits[index] = b'0' + u8::try_from(remaining % 10).unwrap_or(0);
        remaining /= 10;
    }
    hasher.update(&digits[start..]);
}

fn update_token_count(hasher: &mut Sha256, count: TokenCount) {
    let TokenCount::Available(mut value) = count else {
        hasher.update(b"null");
        return;
    };
    let mut digits = [0_u8; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + u8::try_from(value % 10).unwrap_or(0);
        value /= 10;
        if value == 0 {
            break;
        }
    }
    hasher.update(&digits[start..]);
}
