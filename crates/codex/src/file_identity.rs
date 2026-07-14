use std::path::{Component, Path};

use sha2::{Digest, Sha256};
use tokenmaster_domain::{UsageProfileId, UsageSessionId};

use crate::identity::push_hex;

const FILE_HINT_DOMAIN: &[u8] = b"tm-file-hint-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileIdentityError {
    InvalidRelativePath,
    InvalidGeneratedHint,
}

pub(crate) fn filename_session_hint(relative_path: &Path) -> Option<UsageSessionId> {
    relative_path
        .file_stem()
        .and_then(|value| value.to_str())
        .and_then(|stem| UsageSessionId::new(stem.to_owned()).ok())
}

pub(crate) fn hashed_session_hint(
    profile_id: &UsageProfileId,
    relative_path: &Path,
) -> Result<UsageSessionId, FileIdentityError> {
    let mut hasher = Sha256::new();
    hasher.update(FILE_HINT_DOMAIN);
    update_frame(&mut hasher, profile_id.as_str().as_bytes());

    let mut component_count = 0_usize;
    for component in relative_path.components() {
        let Component::Normal(value) = component else {
            return Err(FileIdentityError::InvalidRelativePath);
        };
        update_native_component(&mut hasher, value);
        component_count = component_count.saturating_add(1);
    }
    if component_count == 0 {
        return Err(FileIdentityError::InvalidRelativePath);
    }

    let digest = hasher.finalize();
    let mut generated = String::with_capacity(40);
    generated.push_str("session_");
    push_hex(&mut generated, &digest[..16]);
    UsageSessionId::new(generated).map_err(|_| FileIdentityError::InvalidGeneratedHint)
}

fn update_frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(windows)]
fn update_native_component(hasher: &mut Sha256, value: &std::ffi::OsStr) {
    use std::os::windows::ffi::OsStrExt;

    let units: Vec<u16> = value.encode_wide().collect();
    hasher.update((units.len().saturating_mul(2) as u64).to_le_bytes());
    for unit in units {
        hasher.update(unit.to_le_bytes());
    }
}

#[cfg(not(windows))]
fn update_native_component(hasher: &mut Sha256, value: &std::ffi::OsStr) {
    use std::os::unix::ffi::OsStrExt;

    update_frame(hasher, value.as_bytes());
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tokenmaster_domain::UsageProfileId;

    use super::{filename_session_hint, hashed_session_hint};

    fn profile() -> UsageProfileId {
        match UsageProfileId::new("profile_fixture") {
            Ok(value) => value,
            Err(error) => panic!("static profile fixture must be valid: {error}"),
        }
    }

    #[test]
    fn same_path_produces_the_same_fallback_hint() {
        let relative = Path::new("nested").join(format!("{}.jsonl", "界".repeat(171)));
        let first = hashed_session_hint(&profile(), &relative);
        let second = hashed_session_hint(&profile(), &relative);

        assert_eq!(first, second);
        assert!(first.is_ok_and(
            |value| value.as_str().starts_with("session_") && value.as_str().len() == 40
        ));
        assert!(filename_session_hint(&relative).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_path_uses_a_stable_fallback_hint() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let relative = Path::new("nested").join(OsString::from_vec(vec![
            0xff, b'.', b'j', b's', b'o', b'n', b'l',
        ]));
        let first = hashed_session_hint(&profile(), &relative);
        let second = hashed_session_hint(&profile(), &relative);

        assert_eq!(first, second);
        assert!(first.is_ok_and(|value| value.as_str().starts_with("session_")));
        assert!(filename_session_hint(&relative).is_none());
    }
}
