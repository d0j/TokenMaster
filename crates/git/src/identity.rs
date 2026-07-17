use std::fmt;

use sha2::{Digest, Sha256};
use tokenmaster_domain::{GitRepositoryId, ProjectAlias};

use crate::{GitCoreError, MAX_GIT_AUTHOR_BYTES, MAX_GIT_REF_NAME_BYTES, MAX_GIT_REFS};

const SHA1_HEX_BYTES: usize = 40;
const SHA256_HEX_BYTES: usize = 64;

macro_rules! opaque_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "([redacted])"))
            }
        }
    };
}

opaque_identity!(GitIdentitySalt);
opaque_identity!(GitAuthorFingerprint);
opaque_identity!(GitCommitFingerprint);
opaque_identity!(GitMailmapFingerprint);
opaque_identity!(GitProjectFingerprint);
opaque_identity!(GitRefFingerprint);

#[derive(Clone, Eq, PartialEq)]
pub struct GitRefHead {
    name: Box<[u8]>,
    object_id: Box<[u8]>,
}

impl GitRefHead {
    pub fn new(name: &[u8], object_id: &[u8]) -> Result<Self, GitCoreError> {
        validate_ref_name(name)?;
        validate_object_id(object_id)?;
        Ok(Self {
            name: name.into(),
            object_id: object_id.into(),
        })
    }
}

impl fmt::Debug for GitRefHead {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GitRefHead([redacted])")
    }
}

pub fn derive_repository_id(
    salt: &GitIdentitySalt,
    normalized_common_dir: &[u8],
) -> Result<GitRepositoryId, GitCoreError> {
    if normalized_common_dir.is_empty()
        || normalized_common_dir.len() > crate::MAX_GIT_PATH_BYTES
        || normalized_common_dir
            .iter()
            .any(|byte| *byte == 0 || byte.is_ascii_control())
    {
        return Err(GitCoreError::InvalidPath);
    }
    Ok(GitRepositoryId::from_bytes(framed_hash(
        b"tokenmaster.git.repository.v1",
        &[salt.as_bytes(), normalized_common_dir],
    )?))
}

pub fn derive_author_fingerprint(
    salt: &GitIdentitySalt,
    email: &[u8],
) -> Result<GitAuthorFingerprint, GitCoreError> {
    let email = normalize_email(email)?;
    Ok(GitAuthorFingerprint::from_bytes(framed_hash(
        b"tokenmaster.git.author.v1",
        &[salt.as_bytes(), &email],
    )?))
}

pub fn derive_commit_fingerprint(
    salt: &GitIdentitySalt,
    object_id: &[u8],
) -> Result<GitCommitFingerprint, GitCoreError> {
    validate_object_id(object_id)?;
    Ok(GitCommitFingerprint::from_bytes(framed_hash(
        b"tokenmaster.git.commit.v1",
        &[salt.as_bytes(), object_id],
    )?))
}

pub fn derive_mailmap_fingerprint(
    salt: &GitIdentitySalt,
    contents: Option<&[u8]>,
) -> Result<GitMailmapFingerprint, GitCoreError> {
    let presence = [u8::from(contents.is_some())];
    Ok(GitMailmapFingerprint::from_bytes(framed_hash(
        b"tokenmaster.git.mailmap.v1",
        &[salt.as_bytes(), &presence, contents.unwrap_or_default()],
    )?))
}

pub fn derive_project_fingerprint(
    salt: &GitIdentitySalt,
    project: &ProjectAlias,
) -> Result<GitProjectFingerprint, GitCoreError> {
    Ok(GitProjectFingerprint::from_bytes(framed_hash(
        b"tokenmaster.git.project.v1",
        &[salt.as_bytes(), project.as_str().as_bytes()],
    )?))
}

pub fn derive_ref_fingerprint(
    salt: &GitIdentitySalt,
    refs: &[GitRefHead],
) -> Result<GitRefFingerprint, GitCoreError> {
    if refs.len() > MAX_GIT_REFS {
        return Err(GitCoreError::CapacityExceeded {
            limit: MAX_GIT_REFS,
        });
    }
    let mut ordered = refs.iter().collect::<Vec<_>>();
    ordered.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    if ordered.windows(2).any(|pair| pair[0].name == pair[1].name) {
        return Err(GitCoreError::DuplicateValue);
    }

    let mut hasher = Sha256::new();
    update_frame(&mut hasher, b"tokenmaster.git.refs.v1")?;
    update_frame(&mut hasher, salt.as_bytes())?;
    update_frame(
        &mut hasher,
        &u64::try_from(ordered.len())
            .map_err(|_| GitCoreError::Overflow)?
            .to_be_bytes(),
    )?;
    for reference in ordered {
        update_frame(&mut hasher, &reference.name)?;
        update_frame(&mut hasher, &reference.object_id)?;
    }
    Ok(GitRefFingerprint::from_bytes(hasher.finalize().into()))
}

pub(crate) fn hash_path(path: &[u8]) -> Result<[u8; 32], GitCoreError> {
    framed_hash(b"tokenmaster.git.path-match.v1", &[path])
}

pub(crate) fn validate_object_id(value: &[u8]) -> Result<(), GitCoreError> {
    if !matches!(value.len(), SHA1_HEX_BYTES | SHA256_HEX_BYTES)
        || !value.iter().all(u8::is_ascii_hexdigit)
    {
        return Err(GitCoreError::InvalidObjectId);
    }
    Ok(())
}

fn validate_ref_name(value: &[u8]) -> Result<(), GitCoreError> {
    if value.len() <= b"refs/heads/".len()
        || value.len() > MAX_GIT_REF_NAME_BYTES
        || !value.starts_with(b"refs/heads/")
        || value.iter().any(|byte| {
            *byte == 0
                || byte.is_ascii_control()
                || matches!(
                    *byte,
                    b' ' | b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\'
                )
        })
        || value.ends_with(b"/")
        || value.windows(2).any(|pair| pair == b"..")
        || value.windows(2).any(|pair| pair == b"//")
    {
        return Err(GitCoreError::InvalidRef);
    }
    Ok(())
}

fn normalize_email(value: &[u8]) -> Result<Vec<u8>, GitCoreError> {
    let value = trim_ascii(value);
    if value.is_empty() || value.len() > MAX_GIT_AUTHOR_BYTES {
        return Err(GitCoreError::InvalidAuthor);
    }
    let text = std::str::from_utf8(value).map_err(|_| GitCoreError::InvalidAuthor)?;
    if text
        .chars()
        .any(|character| character.is_control() || character.is_whitespace())
        || value.iter().any(|byte| matches!(*byte, b'<' | b'>'))
    {
        return Err(GitCoreError::InvalidAuthor);
    }
    let Some(separator) = value.iter().position(|byte| *byte == b'@') else {
        return Err(GitCoreError::InvalidAuthor);
    };
    if separator == 0 || separator + 1 == value.len() || value[separator + 1..].contains(&b'@') {
        return Err(GitCoreError::InvalidAuthor);
    }
    let mut normalized = String::with_capacity(value.len());
    for character in text.chars().flat_map(char::to_lowercase) {
        normalized.push(character);
        if normalized.len() > MAX_GIT_AUTHOR_BYTES {
            return Err(GitCoreError::CapacityExceeded {
                limit: MAX_GIT_AUTHOR_BYTES,
            });
        }
    }
    Ok(normalized.into_bytes())
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn framed_hash(domain: &[u8], fields: &[&[u8]]) -> Result<[u8; 32], GitCoreError> {
    let mut hasher = Sha256::new();
    update_frame(&mut hasher, domain)?;
    for field in fields {
        update_frame(&mut hasher, field)?;
    }
    Ok(hasher.finalize().into())
}

fn update_frame(hasher: &mut Sha256, value: &[u8]) -> Result<(), GitCoreError> {
    let length = u64::try_from(value.len()).map_err(|_| GitCoreError::Overflow)?;
    hasher.update(length.to_be_bytes());
    hasher.update(value);
    Ok(())
}
