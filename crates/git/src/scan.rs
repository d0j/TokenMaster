use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use tokenmaster_domain::GitRepositoryId;

use crate::{
    GitAuthorFingerprint, GitBackendError, GitBackendErrorCode, GitIdentitySalt, GitLogParseConfig,
    GitMailmapFingerprint, GitProcess, GitRefFingerprint, GitRefHead, GitRepositoryCandidate,
    GitScanSummary, GitStreamLimits, derive_author_fingerprint, derive_mailmap_fingerprint,
    derive_ref_fingerprint, derive_repository_id,
};

const REPOSITORY_INFO_STDOUT_BYTES: usize = 64 * 1024;
const AUTHOR_STDOUT_BYTES: usize = crate::MAX_GIT_AUTHOR_BYTES + 2;
const REFS_STDOUT_BYTES: usize = 4 * 1024 * 1024;

const REPOSITORY_INFO_ARGS: &[&str] = &[
    "rev-parse",
    "--path-format=absolute",
    "--git-common-dir",
    "--is-shallow-repository",
    "--show-object-format",
    "--show-toplevel",
];
const LOCAL_AUTHOR_ARGS: &[&str] = &["config", "--no-includes", "--local", "--get", "user.email"];
const GLOBAL_AUTHOR_ARGS: &[&str] = &["config", "--no-includes", "--global", "--get", "user.email"];
const REFS_ARGS: &[&str] = &[
    "for-each-ref",
    "--format=%(refname)%00%(objectname)",
    "refs/heads/",
];
const LOG_ARGS: &[&str] = &[
    "--no-pager",
    "--no-replace-objects",
    "-c",
    "core.pager=cat",
    "-c",
    "color.ui=false",
    "-c",
    "core.attributesFile=",
    "-c",
    "mailmap.file=",
    "-c",
    "mailmap.blob=",
    "-c",
    "log.showSignature=false",
    "log",
    "--branches",
    "--root",
    "--diff-merges=off",
    "--raw",
    "--numstat",
    "-z",
    "--no-color",
    "--no-ext-diff",
    "--no-textconv",
    "--use-mailmap",
    "--find-renames=50%",
    "--format=format:%x1e%H%x00%at%x00%ae%x00%aE%x00%P%x00",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitObjectFormat {
    Sha1,
    Sha256,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitAuthorSource {
    Repository,
    Global,
}

#[derive(Clone, Eq, PartialEq)]
pub struct GitRepositoryScan {
    repository_id: GitRepositoryId,
    ref_fingerprint: GitRefFingerprint,
    mailmap_fingerprint: GitMailmapFingerprint,
    author_fingerprint: GitAuthorFingerprint,
    author_source: GitAuthorSource,
    object_format: GitObjectFormat,
    shallow: bool,
    refs: Vec<GitRefHead>,
    summary: GitScanSummary,
}

impl GitRepositoryScan {
    #[must_use]
    pub const fn repository_id(&self) -> GitRepositoryId {
        self.repository_id
    }

    #[must_use]
    pub const fn ref_fingerprint(&self) -> GitRefFingerprint {
        self.ref_fingerprint
    }

    #[must_use]
    pub const fn mailmap_fingerprint(&self) -> GitMailmapFingerprint {
        self.mailmap_fingerprint
    }

    #[must_use]
    pub const fn author_fingerprint(&self) -> GitAuthorFingerprint {
        self.author_fingerprint
    }

    #[must_use]
    pub const fn author_source(&self) -> GitAuthorSource {
        self.author_source
    }

    #[must_use]
    pub const fn object_format(&self) -> GitObjectFormat {
        self.object_format
    }

    #[must_use]
    pub const fn is_shallow(&self) -> bool {
        self.shallow
    }

    #[must_use]
    pub fn ref_count(&self) -> usize {
        self.refs.len()
    }

    #[must_use]
    pub const fn summary(&self) -> &GitScanSummary {
        &self.summary
    }
}

impl fmt::Debug for GitRepositoryScan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitRepositoryScan")
            .field("repository_id", &self.repository_id)
            .field("ref_fingerprint", &self.ref_fingerprint)
            .field("mailmap_fingerprint", &self.mailmap_fingerprint)
            .field("author", &"[redacted]")
            .field("author_source", &self.author_source)
            .field("object_format", &self.object_format)
            .field("shallow", &self.shallow)
            .field("ref_count", &self.refs.len())
            .field("summary", &self.summary)
            .finish()
    }
}

impl GitProcess {
    pub fn scan(
        &self,
        candidate: &GitRepositoryCandidate,
        salt: GitIdentitySalt,
    ) -> Result<GitRepositoryScan, GitBackendError> {
        let deadline = self.operation_deadline()?;
        self.limited_to(deadline)?.version()?;
        let repository = self.limited_to(deadline)?.repository_info(candidate)?;
        let repository_id = derive_repository_id(&salt, &repository.normalized_common_dir)
            .map_err(|_| GitBackendError::new(GitBackendErrorCode::RepositoryPathRejected))?;
        let (author_fingerprint, author_source) =
            self.limited_to(deadline)?.author(candidate, &salt)?;
        let refs = self.limited_to(deadline)?.refs(candidate)?;
        let mailmap_fingerprint = fingerprint_mailmap(&repository.worktree_root, &salt)?;
        let ref_fingerprint =
            derive_ref_fingerprint(&salt, &refs).map_err(|error| match error {
                crate::GitCoreError::CapacityExceeded { limit } => {
                    GitBackendError::with_limit(GitBackendErrorCode::TooManyRefs, limit)
                }
                _ => GitBackendError::new(GitBackendErrorCode::ProtocolError),
            })?;
        let summary = if refs.is_empty() {
            crate::GitScanAccumulator::new()
                .finish()
                .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProtocolError))?
        } else {
            let config =
                GitLogParseConfig::new(salt, vec![author_fingerprint], GitStreamLimits::default())
                    .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProtocolError))?;
            self.limited_to(deadline)?
                .scan_log(LOG_ARGS, candidate.path(), config)?
        };
        let repository_after = self.limited_to(deadline)?.repository_info(candidate)?;
        let author_after = self
            .limited_to(deadline)?
            .author(candidate, &salt)
            .map_err(|error| {
                if error.code() == GitBackendErrorCode::AuthorIdentityMissing {
                    GitBackendError::new(GitBackendErrorCode::HistoryChangedDuringScan)
                } else {
                    error
                }
            })?;
        let refs_after = self.limited_to(deadline)?.refs(candidate)?;
        let mailmap_after = fingerprint_mailmap(&repository_after.worktree_root, &salt)?;
        self.limited_to(deadline)?;
        if repository_after != repository
            || author_after != (author_fingerprint, author_source)
            || refs_after != refs
            || mailmap_after != mailmap_fingerprint
        {
            return Err(GitBackendError::new(
                GitBackendErrorCode::HistoryChangedDuringScan,
            ));
        }
        Ok(GitRepositoryScan {
            repository_id,
            ref_fingerprint,
            mailmap_fingerprint,
            author_fingerprint,
            author_source,
            object_format: repository.object_format,
            shallow: repository.shallow,
            refs,
            summary,
        })
    }

    fn repository_info(
        &self,
        candidate: &GitRepositoryCandidate,
    ) -> Result<RepositoryInfo, GitBackendError> {
        let output = self.capture_status(
            REPOSITORY_INFO_ARGS,
            Some(candidate.path()),
            REPOSITORY_INFO_STDOUT_BYTES,
            crate::MAX_GIT_STDERR_BYTES,
        )?;
        if !output.status.success() {
            return Err(GitBackendError::new(
                GitBackendErrorCode::RepositoryNotFound,
            ));
        }
        let lines = split_lines(&output.stdout);
        if lines.len() != 4 {
            return Err(GitBackendError::new(GitBackendErrorCode::ProtocolError));
        }
        let common_dir = std::str::from_utf8(lines[0])
            .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProtocolError))?;
        let common_dir = crate::command::validate_private_directory(&PathBuf::from(common_dir))?;
        let normalized_common_dir = normalize_private_path(&common_dir)?;
        let shallow = match lines[1] {
            b"true" => true,
            b"false" => false,
            _ => return Err(GitBackendError::new(GitBackendErrorCode::ProtocolError)),
        };
        let object_format = match lines[2] {
            b"sha1" => GitObjectFormat::Sha1,
            b"sha256" => GitObjectFormat::Sha256,
            _ => {
                return Err(GitBackendError::new(
                    GitBackendErrorCode::UnsupportedObjectFormat,
                ));
            }
        };
        let worktree_root = std::str::from_utf8(lines[3])
            .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProtocolError))?;
        let worktree_root =
            crate::command::validate_private_directory(&PathBuf::from(worktree_root))?;
        Ok(RepositoryInfo {
            normalized_common_dir,
            worktree_root,
            shallow,
            object_format,
        })
    }

    fn author(
        &self,
        candidate: &GitRepositoryCandidate,
        salt: &GitIdentitySalt,
    ) -> Result<(GitAuthorFingerprint, GitAuthorSource), GitBackendError> {
        let local = self.capture_status(
            LOCAL_AUTHOR_ARGS,
            Some(candidate.path()),
            AUTHOR_STDOUT_BYTES,
            crate::MAX_GIT_STDERR_BYTES,
        )?;
        if local.status.success() {
            return parse_author(&local.stdout, salt)
                .map(|author| (author, GitAuthorSource::Repository));
        }
        if local.status.code() != Some(1) {
            return Err(GitBackendError::new(GitBackendErrorCode::ProcessFailed));
        }
        let global = self.capture_status(
            GLOBAL_AUTHOR_ARGS,
            None,
            AUTHOR_STDOUT_BYTES,
            crate::MAX_GIT_STDERR_BYTES,
        )?;
        if global.status.success() {
            return parse_author(&global.stdout, salt)
                .map(|author| (author, GitAuthorSource::Global));
        }
        if global.status.code() != Some(1) {
            return Err(GitBackendError::new(GitBackendErrorCode::ProcessFailed));
        }
        Err(GitBackendError::new(
            GitBackendErrorCode::AuthorIdentityMissing,
        ))
    }

    fn refs(&self, candidate: &GitRepositoryCandidate) -> Result<Vec<GitRefHead>, GitBackendError> {
        let output = self.capture_status(
            REFS_ARGS,
            Some(candidate.path()),
            REFS_STDOUT_BYTES,
            crate::MAX_GIT_STDERR_BYTES,
        )?;
        if !output.status.success() {
            return Err(GitBackendError::new(GitBackendErrorCode::ProcessFailed));
        }
        let mut refs = Vec::new();
        for line in output.stdout.split(|byte| *byte == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() {
                continue;
            }
            if refs.len() == crate::MAX_GIT_REFS {
                return Err(GitBackendError::with_limit(
                    GitBackendErrorCode::TooManyRefs,
                    crate::MAX_GIT_REFS,
                ));
            }
            let mut fields = line.split(|byte| *byte == 0);
            let name = fields
                .next()
                .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::ProtocolError))?;
            let object_id = fields
                .next()
                .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::ProtocolError))?;
            if fields.next().is_some() {
                return Err(GitBackendError::new(GitBackendErrorCode::ProtocolError));
            }
            refs.push(
                GitRefHead::new(name, object_id)
                    .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProtocolError))?,
            );
        }
        Ok(refs)
    }
}

#[derive(Eq, PartialEq)]
struct RepositoryInfo {
    normalized_common_dir: Vec<u8>,
    worktree_root: PathBuf,
    shallow: bool,
    object_format: GitObjectFormat,
}

fn fingerprint_mailmap(
    worktree_root: &Path,
    salt: &GitIdentitySalt,
) -> Result<GitMailmapFingerprint, GitBackendError> {
    let path = worktree_root.join(".mailmap");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return derive_mailmap_fingerprint(salt, None)
                .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProtocolError));
        }
        Err(_) => {
            return Err(GitBackendError::new(
                GitBackendErrorCode::RepositoryPathRejected,
            ));
        }
    };
    if !metadata.is_file() || is_reparse_point(&metadata) {
        return Err(GitBackendError::new(
            GitBackendErrorCode::RepositoryPathRejected,
        ));
    }
    let length = usize::try_from(metadata.len()).map_err(|_| {
        GitBackendError::with_limit(
            GitBackendErrorCode::CapacityExceeded,
            crate::MAX_GIT_MAILMAP_BYTES,
        )
    })?;
    if length > crate::MAX_GIT_MAILMAP_BYTES {
        return Err(GitBackendError::with_limit(
            GitBackendErrorCode::CapacityExceeded,
            crate::MAX_GIT_MAILMAP_BYTES,
        ));
    }
    let mut file = open_without_following_reparse(&path)?;
    let mut contents = Vec::with_capacity(length);
    let capacity_error = || {
        GitBackendError::with_limit(
            GitBackendErrorCode::CapacityExceeded,
            crate::MAX_GIT_MAILMAP_BYTES,
        )
    };
    let read_limit = u64::try_from(crate::MAX_GIT_MAILMAP_BYTES)
        .map_err(|_| capacity_error())?
        .checked_add(1)
        .ok_or_else(capacity_error)?;
    (&mut file)
        .take(read_limit)
        .read_to_end(&mut contents)
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::RepositoryPathRejected))?;
    if contents.len() > crate::MAX_GIT_MAILMAP_BYTES {
        return Err(GitBackendError::with_limit(
            GitBackendErrorCode::CapacityExceeded,
            crate::MAX_GIT_MAILMAP_BYTES,
        ));
    }
    derive_mailmap_fingerprint(salt, Some(&contents))
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::ProtocolError))
}

#[cfg(windows)]
fn open_without_following_reparse(path: &Path) -> Result<fs::File, GitBackendError> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::RepositoryPathRejected))
}

#[cfg(not(windows))]
fn open_without_following_reparse(path: &Path) -> Result<fs::File, GitBackendError> {
    fs::File::open(path)
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::RepositoryPathRejected))
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn parse_author(
    output: &[u8],
    salt: &GitIdentitySalt,
) -> Result<GitAuthorFingerprint, GitBackendError> {
    let lines = split_lines(output);
    if lines.len() != 1 {
        return Err(GitBackendError::new(
            GitBackendErrorCode::AuthorIdentityMissing,
        ));
    }
    derive_author_fingerprint(salt, lines[0])
        .map_err(|_| GitBackendError::new(GitBackendErrorCode::AuthorIdentityMissing))
}

fn split_lines(output: &[u8]) -> Vec<&[u8]> {
    output
        .split(|byte| *byte == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
        .filter(|line| !line.is_empty())
        .collect()
}

fn normalize_private_path(path: &std::path::Path) -> Result<Vec<u8>, GitBackendError> {
    let value = path
        .to_str()
        .ok_or_else(|| GitBackendError::new(GitBackendErrorCode::RepositoryPathRejected))?;
    Ok(value
        .bytes()
        .map(|byte| {
            if byte == b'\\' {
                b'/'
            } else if cfg!(windows) {
                byte.to_ascii_lowercase()
            } else {
                byte
            }
        })
        .collect())
}
