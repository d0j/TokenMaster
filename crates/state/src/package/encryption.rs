use core::fmt;
use core::iter;
use std::io::{self, Read, Write};

use age::secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

use super::PACKAGE_IO_BUFFER_BYTES;
use super::VerifiedBackupPackage;
use super::capability::{
    DurableCapabilityError, DurableFileReader, DurableReaderAdapter, DurableReaderFailure,
    DurableStagedFile, DurableWriterAdapter, TrackedDurableReaderAdapter, map_durable_error,
    resolve_codec_error,
};
use super::reader::read_authenticated_backup;
use crate::StateError;

pub const AGE_SCRYPT_LOG_N: u8 = 16;
pub const MIN_BACKUP_PASSPHRASE_SCALARS: usize = 12;
pub const MAX_BACKUP_PASSPHRASE_SCALARS: usize = 128;

/// Package-private proof that an age payload has passed outer authentication.
///
/// Only this module can construct the value. The inner generic stream therefore
/// never becomes crate-visible authority.
pub(super) struct AuthenticatedBackupPayload<'a>(&'a mut dyn Read);

impl<'a> AuthenticatedBackupPayload<'a> {
    fn new(source: &'a mut dyn Read) -> Self {
        Self(source)
    }
}

impl Read for AuthenticatedBackupPayload<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.read(buffer)
    }
}

/// Operation context for optional package encryption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackupEncryptionContext {
    /// Unattended recovery point, which must remain decryptable without a stored secret.
    AutomaticBackup,
    /// User-initiated portable export.
    ManualExport,
}

/// Owned passphrase that zeroizes its allocation on drop and never exposes its value.
pub struct BackupPassphrase {
    secret: SecretString,
}

impl BackupPassphrase {
    /// Validates a new passphrase and exact confirmation, clearing both inputs always.
    pub fn new(input: &mut String, confirmation: &mut String) -> Result<Self, StateError> {
        let input_secret = SecretString::from(core::mem::take(input));
        let confirmation_secret = SecretString::from(core::mem::take(confirmation));
        let input_value = input_secret.expose_secret();
        let confirmation_value = confirmation_secret.expose_secret();
        if !valid_scalar_count(input_value) || input_value != confirmation_value {
            return Err(StateError::invalid_input());
        }
        Ok(Self {
            secret: input_secret,
        })
    }

    /// Converts one existing passphrase input, clearing the caller-owned buffer always.
    pub fn existing(input: &mut String) -> Result<Self, StateError> {
        let secret = SecretString::from(core::mem::take(input));
        if !valid_scalar_count(secret.expose_secret()) {
            return Err(StateError::invalid_input());
        }
        Ok(Self { secret })
    }

    fn into_secret(self) -> SecretString {
        self.secret
    }
}

impl fmt::Debug for BackupPassphrase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BackupPassphrase([redacted])")
    }
}

fn valid_scalar_count(value: &str) -> bool {
    let count = value.chars().count();
    (MIN_BACKUP_PASSPHRASE_SCALARS..=MAX_BACKUP_PASSPHRASE_SCALARS).contains(&count)
}

/// Exact sealed-output identity returned by age protection.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ProtectedPackageReceipt {
    output_len: u64,
    output_sha256: [u8; 32],
}

impl ProtectedPackageReceipt {
    #[must_use]
    pub const fn output_len(self) -> u64 {
        self.output_len
    }

    #[must_use]
    pub const fn output_sha256(&self) -> &[u8; 32] {
        &self.output_sha256
    }
}

impl fmt::Debug for ProtectedPackageReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProtectedPackageReceipt([redacted])")
    }
}

/// Typed age v1 envelope for a complete manual TokenMaster backup package.
pub struct EncryptedBackupPackage;

impl EncryptedBackupPackage {
    /// Protects one complete manual package and seals the ciphertext stage.
    pub fn encrypt(
        context: BackupEncryptionContext,
        source: &mut DurableFileReader,
        verified: &VerifiedBackupPackage,
        passphrase: BackupPassphrase,
        destination: &mut DurableStagedFile,
    ) -> Result<ProtectedPackageReceipt, StateError> {
        let result = encrypt_package(context, source, verified, passphrase, destination);
        finish_or_discard(result, destination)
    }

    /// Authenticates one age package and seals only its verified inner database.
    pub fn decrypt(
        source: &mut DurableFileReader,
        passphrase: BackupPassphrase,
        database_destination: &mut DurableStagedFile,
    ) -> Result<VerifiedBackupPackage, StateError> {
        let result = decrypt_package(source, passphrase, database_destination);
        finish_or_discard(result, database_destination)
    }
}

fn encrypt_package(
    context: BackupEncryptionContext,
    source: &mut DurableFileReader,
    verified: &VerifiedBackupPackage,
    passphrase: BackupPassphrase,
    destination: &mut DurableStagedFile,
) -> Result<ProtectedPackageReceipt, StateError> {
    if context != BackupEncryptionContext::ManualExport {
        return Err(StateError::invalid_input());
    }
    let expected = verified.receipt();
    if source.len() != expected.package_len() {
        return Err(StateError::integrity());
    }

    let mut recipient = age::scrypt::Recipient::new(passphrase.into_secret());
    recipient.set_work_factor(AGE_SCRYPT_LOG_N);
    let encryptor = age::Encryptor::with_recipients(iter::once(&recipient as &dyn age::Recipient))
        .map_err(|_| StateError::internal_invariant())?;

    let mut source_adapter = DurableReaderAdapter::new(source);
    let mut output = DigestingWriter::new(destination);
    let mut source_hasher = Sha256::new();
    let mut source_len = 0_u64;
    let operation = (|| {
        let mut encrypted = encryptor
            .wrap_output(&mut output)
            .map_err(|_| StateError::unavailable())?;
        let mut buffer = [0_u8; PACKAGE_IO_BUFFER_BYTES];
        loop {
            let count = source_adapter
                .read(&mut buffer)
                .map_err(|_| StateError::integrity())?;
            if count == 0 {
                break;
            }
            source_hasher.update(&buffer[..count]);
            source_len = source_len
                .checked_add(u64::try_from(count).map_err(|_| StateError::integrity())?)
                .ok_or_else(StateError::capacity_exceeded)?;
            encrypted
                .write_all(&buffer[..count])
                .map_err(|_| StateError::unavailable())?;
        }
        encrypted.finish().map_err(|_| StateError::unavailable())?;
        let source_sha256: [u8; 32] = source_hasher.finalize().into();
        if source_len != expected.package_len() || &source_sha256 != expected.file_sha256() {
            return Err(StateError::integrity());
        }
        Ok(())
    })();
    let operation = resolve_codec_error(
        operation,
        &[source_adapter.failure(), output.adapter_failure()],
    );
    operation?;
    let receipt = finish_output(output)?;
    destination
        .seal(receipt.output_len, receipt.output_sha256)
        .map_err(map_durable_error)?;
    Ok(receipt)
}

fn decrypt_package(
    source: &mut DurableFileReader,
    passphrase: BackupPassphrase,
    database_destination: &mut DurableStagedFile,
) -> Result<VerifiedBackupPackage, StateError> {
    let source_failure = DurableReaderFailure::new();
    let source_adapter = TrackedDurableReaderAdapter::new(source, &source_failure);
    let decryptor = age::Decryptor::new(source_adapter)
        .map_err(map_decrypt_error)
        .and_then(|decryptor| {
            if decryptor.is_scrypt() {
                Ok(decryptor)
            } else {
                Err(StateError::integrity())
            }
        });
    let decryptor = resolve_codec_error(decryptor, &[source_failure.get()])?;

    let mut identity = age::scrypt::Identity::new(passphrase.into_secret());
    identity.set_max_work_factor(AGE_SCRYPT_LOG_N);
    let plaintext = decryptor
        .decrypt(iter::once(&identity as &dyn age::Identity))
        .map_err(map_decrypt_error);
    let mut plaintext = resolve_codec_error(plaintext, &[source_failure.get()])?;

    let authenticated_payload = AuthenticatedBackupPayload::new(&mut plaintext);
    let result = read_authenticated_backup(authenticated_payload, database_destination);
    drop(plaintext);
    resolve_codec_error(result, &[source_failure.get()])
}

fn map_decrypt_error(error: age::DecryptError) -> StateError {
    if matches!(error, age::DecryptError::ExcessiveWork { .. }) {
        StateError::capacity_exceeded()
    } else {
        StateError::integrity()
    }
}

fn finish_or_discard<T>(
    result: Result<T, StateError>,
    destination: &mut DurableStagedFile,
) -> Result<T, StateError> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => match destination.discard() {
            Ok(()) => Err(error),
            Err(_) => Err(StateError::recovery_required()),
        },
    }
}

fn finish_output(output: DigestingWriter<'_>) -> Result<ProtectedPackageReceipt, StateError> {
    let (output_len, output_sha256, adapter_failure) = output.finish();
    if let Some(error) = adapter_failure {
        return Err(map_durable_error(error));
    }
    Ok(ProtectedPackageReceipt {
        output_len,
        output_sha256,
    })
}

struct DigestingWriter<'a> {
    inner: DurableWriterAdapter<'a>,
    hasher: Sha256,
    written: u64,
}

impl<'a> DigestingWriter<'a> {
    fn new(destination: &'a mut DurableStagedFile) -> Self {
        Self {
            inner: DurableWriterAdapter::new(destination),
            hasher: Sha256::new(),
            written: 0,
        }
    }

    const fn adapter_failure(&self) -> Option<DurableCapabilityError> {
        self.inner.failure()
    }

    fn finish(self) -> (u64, [u8; 32], Option<DurableCapabilityError>) {
        (
            self.written,
            self.hasher.finalize().into(),
            self.inner.failure(),
        )
    }
}

impl Write for DigestingWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.inner.write(bytes)?;
        self.hasher.update(&bytes[..count]);
        self.written = self
            .written
            .checked_add(u64::try_from(count).map_err(|_| io::ErrorKind::InvalidData)?)
            .ok_or(io::ErrorKind::OutOfMemory)?;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
