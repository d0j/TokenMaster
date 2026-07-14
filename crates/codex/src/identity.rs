use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use tokenmaster_provider::{
    MAX_PATH_BYTES, ProfileId, ProviderError, ProviderId, SourceId, SourceKind,
};

pub fn profile_id_for_root(path: &Path) -> Result<ProfileId, ProviderError> {
    let normalized = normalize_absolute_path(path)?;
    let digest = Sha256::digest(comparison_key(&normalized));
    let mut id = String::with_capacity(24);
    id.push_str("profile_");
    push_hex(&mut id, &digest[..8]);
    ProfileId::new(id)
}

pub(crate) fn normalize_absolute_path(path: &Path) -> Result<PathBuf, ProviderError> {
    if !path.is_absolute() {
        return Err(ProviderError::invalid_path(MAX_PATH_BYTES));
    }

    let mut normalized = PathBuf::new();
    let mut normal_depth = 0_usize;
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if normal_depth == 0 {
                    return Err(ProviderError::invalid_path(MAX_PATH_BYTES));
                }
                normalized.pop();
                normal_depth -= 1;
            }
            Component::Normal(value) => {
                normalized.push(value);
                normal_depth += 1;
            }
        }
    }

    tokenmaster_provider::DiscoveryRoot::new(
        normalized.clone(),
        tokenmaster_provider::RootOrigin::Configured,
        None,
        true,
    )?;
    Ok(normalized)
}

pub(crate) fn comparison_key(path: &Path) -> Vec<u8> {
    if let Some(text) = path.to_str() {
        return text
            .bytes()
            .map(|byte| match byte {
                b'/' => b'\\',
                b'A'..=b'Z' => byte + (b'a' - b'A'),
                _ => byte,
            })
            .collect();
    }

    comparison_key_wide(path)
}

#[cfg(windows)]
fn comparison_key_wide(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    let mut key = vec![0xff, b'u', b'1', b'6'];
    for mut unit in path.as_os_str().encode_wide() {
        if unit == u16::from(b'/') {
            unit = u16::from(b'\\');
        } else if (u16::from(b'A')..=u16::from(b'Z')).contains(&unit) {
            unit += u16::from(b'a' - b'A');
        }
        key.extend_from_slice(&unit.to_le_bytes());
    }
    key
}

#[cfg(not(windows))]
fn comparison_key_wide(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    let mut key = vec![0xff, b'o', b's'];
    key.extend_from_slice(path.as_os_str().as_bytes());
    key
}

pub(crate) fn push_hex(output: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
}

pub(crate) fn source_id_for_root(
    provider_id: &ProviderId,
    profile_id: &ProfileId,
    kind: SourceKind,
    path: &Path,
) -> Result<SourceId, ProviderError> {
    let normalized = normalize_absolute_path(path)?;
    let kind_tag = match kind {
        SourceKind::Active => b"active".as_slice(),
        SourceKind::Direct => b"direct".as_slice(),
        SourceKind::Archived => b"archived".as_slice(),
    };
    let path_key = comparison_key(&normalized);
    let mut hasher = Sha256::new();
    for field in [
        provider_id.as_str().as_bytes(),
        profile_id.as_str().as_bytes(),
        kind_tag,
        path_key.as_slice(),
    ] {
        hasher.update((field.len() as u64).to_le_bytes());
        hasher.update(field);
    }
    let digest = hasher.finalize();
    let mut id = String::with_capacity(71);
    id.push_str("source_");
    push_hex(&mut id, &digest);
    SourceId::new(id)
}
