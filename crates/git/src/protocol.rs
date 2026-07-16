use std::fmt;

use tokenmaster_domain::{GitLineMetrics, GitOutputDay};

use crate::{
    GitAuthorFingerprint, GitCommitAccumulator, GitCommitSink, GitCoreError, GitIdentitySalt,
    GitPathStat, MAX_GIT_AUTHOR_BYTES, MAX_GIT_PATH_BYTES, MAX_GIT_PATHS_PER_COMMIT,
    derive_author_fingerprint, derive_commit_fingerprint, identity::hash_path,
    identity::validate_object_id,
};

const RECORD_SEPARATOR: u8 = 0x1e;
const MAX_HEADER_BYTES: usize = 256;
const MAX_AUTHORS: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GitStreamLimits {
    header_bytes: usize,
    author_bytes: usize,
    path_bytes: usize,
    paths_per_commit: usize,
}

impl GitStreamLimits {
    pub fn new(
        header_bytes: usize,
        author_bytes: usize,
        path_bytes: usize,
        paths_per_commit: usize,
    ) -> Result<Self, GitCoreError> {
        if header_bytes == 0
            || header_bytes > MAX_HEADER_BYTES
            || author_bytes == 0
            || author_bytes > MAX_GIT_AUTHOR_BYTES
            || path_bytes == 0
            || path_bytes > MAX_GIT_PATH_BYTES
            || paths_per_commit == 0
            || paths_per_commit > MAX_GIT_PATHS_PER_COMMIT
        {
            return Err(GitCoreError::InvalidLimit);
        }
        Ok(Self {
            header_bytes,
            author_bytes,
            path_bytes,
            paths_per_commit,
        })
    }
}

impl Default for GitStreamLimits {
    fn default() -> Self {
        Self {
            header_bytes: MAX_HEADER_BYTES,
            author_bytes: MAX_GIT_AUTHOR_BYTES,
            path_bytes: MAX_GIT_PATH_BYTES,
            paths_per_commit: MAX_GIT_PATHS_PER_COMMIT,
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct GitLogParseConfig {
    salt: GitIdentitySalt,
    authors: Vec<GitAuthorFingerprint>,
    limits: GitStreamLimits,
}

impl GitLogParseConfig {
    pub fn new(
        salt: GitIdentitySalt,
        mut authors: Vec<GitAuthorFingerprint>,
        limits: GitStreamLimits,
    ) -> Result<Self, GitCoreError> {
        if authors.is_empty() || authors.len() > MAX_AUTHORS {
            return Err(GitCoreError::CapacityExceeded { limit: MAX_AUTHORS });
        }
        authors.sort_unstable();
        if authors.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(GitCoreError::DuplicateValue);
        }
        Ok(Self {
            salt,
            authors,
            limits,
        })
    }
}

impl fmt::Debug for GitLogParseConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitLogParseConfig")
            .field("salt", &"[redacted]")
            .field("authors", &self.authors.len())
            .field("limits", &self.limits)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParseState {
    ExpectRecordSeparator,
    HeaderObjectId,
    HeaderTimestamp,
    HeaderRawAuthor,
    HeaderCanonicalAuthor,
    HeaderParents,
    BodyStart,
    RawHeader,
    RawSourcePath,
    RawDestinationPath,
    NumAdded,
    NumRemoved,
    NumPathStart,
    NumPath,
    NumRenameSource,
    NumRenameDestination,
    Failed,
    Finished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RawKind {
    TextOrBinary,
    Submodule,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawEntry {
    destination_hash: [u8; 32],
    kind: RawKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingRaw {
    kind: RawKind,
    renamed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NumValue {
    Text(u64),
    Binary,
}

struct CurrentCommit {
    accumulator: Option<GitCommitAccumulator>,
    raw_entries: Vec<RawEntry>,
    numstat_index: usize,
    body_terminated: bool,
}

struct PendingHeader {
    fingerprint: Option<crate::GitCommitFingerprint>,
    day_index: Option<i32>,
    selected_author: bool,
}

impl PendingHeader {
    const fn new() -> Self {
        Self {
            fingerprint: None,
            day_index: None,
            selected_author: false,
        }
    }
}

pub struct GitLogStreamParser {
    config: GitLogParseConfig,
    state: ParseState,
    token: Vec<u8>,
    pending_header: PendingHeader,
    pending_raw: Option<PendingRaw>,
    pending_added: Option<NumValue>,
    pending_removed: Option<NumValue>,
    current: Option<CurrentCommit>,
    processed_commits: usize,
    emitted_commits: u64,
}

impl GitLogStreamParser {
    #[must_use]
    pub fn new(config: GitLogParseConfig) -> Self {
        Self {
            token: Vec::with_capacity(config.limits.header_bytes),
            config,
            state: ParseState::ExpectRecordSeparator,
            pending_header: PendingHeader::new(),
            pending_raw: None,
            pending_added: None,
            pending_removed: None,
            current: None,
            processed_commits: 0,
            emitted_commits: 0,
        }
    }

    pub fn push(
        &mut self,
        bytes: &[u8],
        sink: &mut impl GitCommitSink,
    ) -> Result<(), GitCoreError> {
        if matches!(self.state, ParseState::Failed | ParseState::Finished) {
            return Err(GitCoreError::InvalidProtocol);
        }
        for byte in bytes {
            if let Err(error) = self.push_byte(*byte, sink) {
                self.fail();
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn finish(&mut self, sink: &mut impl GitCommitSink) -> Result<(), GitCoreError> {
        if self.state == ParseState::Finished {
            return Err(GitCoreError::InvalidProtocol);
        }
        if self.state == ParseState::Failed {
            return Err(GitCoreError::InvalidProtocol);
        }
        let result = match self.state {
            ParseState::ExpectRecordSeparator if self.current.is_none() => Ok(()),
            ParseState::BodyStart if self.token.is_empty() => self.finish_current(sink),
            _ => Err(GitCoreError::IncompleteProtocol),
        };
        if result.is_ok() {
            self.state = ParseState::Finished;
        } else {
            self.fail();
        }
        result
    }

    fn push_byte(&mut self, byte: u8, sink: &mut impl GitCommitSink) -> Result<(), GitCoreError> {
        match self.state {
            ParseState::ExpectRecordSeparator => {
                if byte != RECORD_SEPARATOR {
                    return Err(GitCoreError::InvalidProtocol);
                }
                self.start_header();
            }
            ParseState::HeaderObjectId => {
                if byte == 0 {
                    validate_object_id(&self.token)?;
                    self.pending_header.fingerprint =
                        Some(derive_commit_fingerprint(&self.config.salt, &self.token)?);
                    self.token.clear();
                    self.state = ParseState::HeaderTimestamp;
                } else {
                    self.push_token(byte, self.config.limits.header_bytes)?;
                }
            }
            ParseState::HeaderTimestamp => {
                if byte == 0 {
                    let timestamp = parse_i64(&self.token)?;
                    let day = timestamp.div_euclid(86_400);
                    let day_index =
                        i32::try_from(day).map_err(|_| GitCoreError::InvalidTimestamp)?;
                    GitOutputDay::new(day_index, 0, 0, GitLineMetrics::new(0, 0))
                        .map_err(|_| GitCoreError::InvalidTimestamp)?;
                    self.pending_header.day_index = Some(day_index);
                    self.token.clear();
                    self.state = ParseState::HeaderRawAuthor;
                } else {
                    self.push_token(byte, self.config.limits.header_bytes)?;
                }
            }
            ParseState::HeaderRawAuthor => {
                if byte == 0 {
                    let fingerprint = derive_author_fingerprint(&self.config.salt, &self.token)?;
                    self.pending_header.selected_author =
                        self.config.authors.binary_search(&fingerprint).is_ok();
                    self.token.clear();
                    self.state = ParseState::HeaderCanonicalAuthor;
                } else {
                    self.push_token(byte, self.config.limits.author_bytes)?;
                }
            }
            ParseState::HeaderCanonicalAuthor => {
                if byte == 0 {
                    let fingerprint = derive_author_fingerprint(&self.config.salt, &self.token)?;
                    self.pending_header.selected_author |=
                        self.config.authors.binary_search(&fingerprint).is_ok();
                    self.token.clear();
                    self.state = ParseState::HeaderParents;
                } else {
                    self.push_token(byte, self.config.limits.author_bytes)?;
                }
            }
            ParseState::HeaderParents => {
                if byte == 0 {
                    let parent_count = parse_parent_count(&self.token)?;
                    let fingerprint = self
                        .pending_header
                        .fingerprint
                        .ok_or(GitCoreError::IncoherentState)?;
                    let day_index = self
                        .pending_header
                        .day_index
                        .ok_or(GitCoreError::IncoherentState)?;
                    let accumulator = self
                        .pending_header
                        .selected_author
                        .then(|| GitCommitAccumulator::new(fingerprint, day_index, parent_count))
                        .transpose()?;
                    self.current = Some(CurrentCommit {
                        accumulator,
                        raw_entries: Vec::with_capacity(
                            self.config.limits.paths_per_commit.min(256),
                        ),
                        numstat_index: 0,
                        body_terminated: false,
                    });
                    self.token.clear();
                    self.state = ParseState::BodyStart;
                } else {
                    self.push_token(byte, self.config.limits.header_bytes)?;
                }
            }
            ParseState::BodyStart => match byte {
                RECORD_SEPARATOR => {
                    self.finish_current(sink)?;
                    self.start_header();
                }
                0 => {
                    let current = self.current.as_mut().ok_or(GitCoreError::InvalidProtocol)?;
                    if current.body_terminated {
                        return Err(GitCoreError::InvalidProtocol);
                    }
                    if current.numstat_index != current.raw_entries.len() {
                        return Err(GitCoreError::ProtocolMismatch);
                    }
                    current.body_terminated = true;
                }
                b'\n' | b'\r' => {
                    if self
                        .current
                        .as_ref()
                        .is_some_and(|current| current.body_terminated)
                    {
                        return Err(GitCoreError::InvalidProtocol);
                    }
                }
                b':' => {
                    self.require_open_body()?;
                    self.token.push(byte);
                    self.state = ParseState::RawHeader;
                }
                b'-' | b'0'..=b'9' => {
                    self.require_open_body()?;
                    self.token.push(byte);
                    self.state = ParseState::NumAdded;
                }
                _ => return Err(GitCoreError::InvalidProtocol),
            },
            ParseState::RawHeader => {
                if byte == 0 {
                    self.pending_raw = Some(parse_raw_header(&self.token)?);
                    self.token.clear();
                    self.state = ParseState::RawSourcePath;
                } else {
                    self.push_token(byte, self.config.limits.header_bytes)?;
                }
            }
            ParseState::RawSourcePath => {
                if byte == 0 {
                    let pending = self.pending_raw.ok_or(GitCoreError::IncoherentState)?;
                    if pending.renamed {
                        crate::classify_destination_path(&self.token)?;
                        self.token.clear();
                        self.state = ParseState::RawDestinationPath;
                    } else {
                        let token = std::mem::take(&mut self.token);
                        let result = self.finish_raw_path(pending.kind, &token);
                        self.token = token;
                        self.token.clear();
                        result?;
                        self.pending_raw = None;
                        self.state = ParseState::BodyStart;
                    }
                } else {
                    self.push_token(byte, self.config.limits.path_bytes)?;
                }
            }
            ParseState::RawDestinationPath => {
                if byte == 0 {
                    let pending = self.pending_raw.ok_or(GitCoreError::IncoherentState)?;
                    let token = std::mem::take(&mut self.token);
                    let result = self.finish_raw_path(pending.kind, &token);
                    self.token = token;
                    self.token.clear();
                    result?;
                    self.pending_raw = None;
                    self.state = ParseState::BodyStart;
                } else {
                    self.push_token(byte, self.config.limits.path_bytes)?;
                }
            }
            ParseState::NumAdded => {
                if byte == b'\t' {
                    self.pending_added = Some(parse_num_value(&self.token)?);
                    self.token.clear();
                    self.state = ParseState::NumRemoved;
                } else {
                    self.push_token(byte, self.config.limits.header_bytes)?;
                }
            }
            ParseState::NumRemoved => {
                if byte == b'\t' {
                    self.pending_removed = Some(parse_num_value(&self.token)?);
                    self.token.clear();
                    self.state = ParseState::NumPathStart;
                } else {
                    self.push_token(byte, self.config.limits.header_bytes)?;
                }
            }
            ParseState::NumPathStart => {
                if byte == 0 {
                    self.state = ParseState::NumRenameSource;
                } else {
                    self.push_token(byte, self.config.limits.path_bytes)?;
                    self.state = ParseState::NumPath;
                }
            }
            ParseState::NumPath => {
                if byte == 0 {
                    let token = std::mem::take(&mut self.token);
                    let result = self.finish_numstat_path(&token);
                    self.token = token;
                    self.token.clear();
                    result?;
                    self.state = ParseState::BodyStart;
                } else {
                    self.push_token(byte, self.config.limits.path_bytes)?;
                }
            }
            ParseState::NumRenameSource => {
                if byte == 0 {
                    crate::classify_destination_path(&self.token)?;
                    self.token.clear();
                    self.state = ParseState::NumRenameDestination;
                } else {
                    self.push_token(byte, self.config.limits.path_bytes)?;
                }
            }
            ParseState::NumRenameDestination => {
                if byte == 0 {
                    let token = std::mem::take(&mut self.token);
                    let result = self.finish_numstat_path(&token);
                    self.token = token;
                    self.token.clear();
                    result?;
                    self.state = ParseState::BodyStart;
                } else {
                    self.push_token(byte, self.config.limits.path_bytes)?;
                }
            }
            ParseState::Failed | ParseState::Finished => {
                return Err(GitCoreError::InvalidProtocol);
            }
        }
        Ok(())
    }

    fn start_header(&mut self) {
        self.pending_header = PendingHeader::new();
        self.token.clear();
        self.state = ParseState::HeaderObjectId;
    }

    fn push_token(&mut self, byte: u8, limit: usize) -> Result<(), GitCoreError> {
        if self.token.len() == limit {
            return Err(GitCoreError::CapacityExceeded { limit });
        }
        self.token.push(byte);
        Ok(())
    }

    fn require_open_body(&self) -> Result<(), GitCoreError> {
        match self.current.as_ref() {
            Some(current) if !current.body_terminated => Ok(()),
            _ => Err(GitCoreError::InvalidProtocol),
        }
    }

    fn finish_raw_path(&mut self, kind: RawKind, path: &[u8]) -> Result<(), GitCoreError> {
        let current = self.current.as_mut().ok_or(GitCoreError::InvalidProtocol)?;
        if current.raw_entries.len() == self.config.limits.paths_per_commit {
            return Err(GitCoreError::CapacityExceeded {
                limit: self.config.limits.paths_per_commit,
            });
        }
        crate::classify_destination_path(path)?;
        current.raw_entries.push(RawEntry {
            destination_hash: hash_path(path)?,
            kind,
        });
        Ok(())
    }

    fn finish_numstat_path(&mut self, path: &[u8]) -> Result<(), GitCoreError> {
        let current = self.current.as_mut().ok_or(GitCoreError::InvalidProtocol)?;
        let raw = current
            .raw_entries
            .get(current.numstat_index)
            .ok_or(GitCoreError::ProtocolMismatch)?;
        if raw.destination_hash != hash_path(path)? {
            return Err(GitCoreError::ProtocolMismatch);
        }
        let added = self
            .pending_added
            .take()
            .ok_or(GitCoreError::IncoherentState)?;
        let removed = self
            .pending_removed
            .take()
            .ok_or(GitCoreError::IncoherentState)?;
        let stat = match (raw.kind, added, removed) {
            (RawKind::Submodule, _, _) => GitPathStat::submodule(path)?,
            (RawKind::TextOrBinary, NumValue::Binary, NumValue::Binary) => {
                GitPathStat::binary(path)?
            }
            (RawKind::TextOrBinary, NumValue::Text(added), NumValue::Text(removed)) => {
                GitPathStat::text(path, added, removed)?
            }
            _ => return Err(GitCoreError::InvalidProtocol),
        };
        if let Some(accumulator) = current.accumulator.as_mut() {
            accumulator.record(stat)?;
        }
        current.numstat_index = current
            .numstat_index
            .checked_add(1)
            .ok_or(GitCoreError::Overflow)?;
        Ok(())
    }

    fn finish_current(&mut self, sink: &mut impl GitCommitSink) -> Result<(), GitCoreError> {
        let Some(current) = self.current.take() else {
            return Err(GitCoreError::InvalidProtocol);
        };
        if current.numstat_index != current.raw_entries.len() {
            return Err(GitCoreError::ProtocolMismatch);
        }
        if self.processed_commits == crate::MAX_GIT_SCANNED_COMMITS {
            return Err(GitCoreError::CapacityExceeded {
                limit: crate::MAX_GIT_SCANNED_COMMITS,
            });
        }
        self.processed_commits = self
            .processed_commits
            .checked_add(1)
            .ok_or(GitCoreError::Overflow)?;
        if let Some(accumulator) = current.accumulator {
            sink.push_commit(accumulator.finish()?)?;
            self.emitted_commits = self
                .emitted_commits
                .checked_add(1)
                .ok_or(GitCoreError::Overflow)?;
        }
        Ok(())
    }

    fn fail(&mut self) {
        self.token.clear();
        self.pending_raw = None;
        self.pending_added = None;
        self.pending_removed = None;
        self.current = None;
        self.state = ParseState::Failed;
    }
}

impl fmt::Debug for GitLogStreamParser {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GitLogStreamParser")
            .field("state", &self.state)
            .field("token_bytes", &self.token.len())
            .field("has_current", &self.current.is_some())
            .field("processed_commits", &self.processed_commits)
            .field("emitted_commits", &self.emitted_commits)
            .finish()
    }
}

fn parse_i64(value: &[u8]) -> Result<i64, GitCoreError> {
    let value = std::str::from_utf8(value).map_err(|_| GitCoreError::InvalidTimestamp)?;
    value
        .parse::<i64>()
        .map_err(|_| GitCoreError::InvalidTimestamp)
}

fn parse_parent_count(value: &[u8]) -> Result<u16, GitCoreError> {
    if value.is_empty() {
        return Ok(0);
    }
    let mut count = 0_u16;
    for parent in value.split(|byte| *byte == b' ') {
        validate_object_id(parent)?;
        count = count.checked_add(1).ok_or(GitCoreError::Overflow)?;
        if count > 64 {
            return Err(GitCoreError::CapacityExceeded { limit: 64 });
        }
    }
    Ok(count)
}

fn parse_raw_header(value: &[u8]) -> Result<PendingRaw, GitCoreError> {
    if value.first() != Some(&b':') {
        return Err(GitCoreError::InvalidProtocol);
    }
    let fields = value[1..]
        .split(|byte| byte.is_ascii_whitespace())
        .filter(|field| !field.is_empty())
        .collect::<Vec<_>>();
    if fields.len() != 5
        || !valid_mode(fields[0])
        || !valid_mode(fields[1])
        || fields[2].is_empty()
        || fields[3].is_empty()
        || fields[2].len() > 64
        || fields[3].len() > 64
        || !fields[2].iter().all(u8::is_ascii_hexdigit)
        || !fields[3].iter().all(u8::is_ascii_hexdigit)
    {
        return Err(GitCoreError::InvalidProtocol);
    }
    let status = fields[4];
    let renamed = status
        .first()
        .is_some_and(|kind| matches!(*kind, b'R' | b'C'));
    if !valid_status(status) {
        return Err(GitCoreError::InvalidProtocol);
    }
    Ok(PendingRaw {
        kind: if fields[0] == b"160000" || fields[1] == b"160000" {
            RawKind::Submodule
        } else {
            RawKind::TextOrBinary
        },
        renamed,
    })
}

fn valid_mode(value: &[u8]) -> bool {
    value.len() == 6 && value.iter().all(|byte| matches!(*byte, b'0'..=b'7'))
}

fn valid_status(value: &[u8]) -> bool {
    match value {
        [b'A' | b'D' | b'M' | b'T' | b'U' | b'X' | b'B'] => true,
        [b'R' | b'C', score @ ..] => {
            !score.is_empty() && score.len() <= 3 && score.iter().all(u8::is_ascii_digit)
        }
        _ => false,
    }
}

fn parse_num_value(value: &[u8]) -> Result<NumValue, GitCoreError> {
    if value == b"-" {
        return Ok(NumValue::Binary);
    }
    let value = std::str::from_utf8(value).map_err(|_| GitCoreError::InvalidProtocol)?;
    value
        .parse::<u64>()
        .map(NumValue::Text)
        .map_err(|_| GitCoreError::InvalidProtocol)
}
