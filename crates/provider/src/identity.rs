use crate::{ProviderError, ProviderErrorCode};

pub const MAX_PROVIDER_ID_BYTES: usize = 64;
pub const MAX_PUBLIC_ID_BYTES: usize = 128;

fn validate_id(value: &str, max_bytes: usize) -> Result<(), ProviderError> {
    if value.is_empty()
        || value.len() > max_bytes
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ProviderError::with_limit(
            ProviderErrorCode::InvalidId,
            max_bytes,
        ));
    }
    Ok(())
}

macro_rules! public_id {
    ($name:ident, $limit:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ProviderError> {
                let value = value.into();
                validate_id(&value, $limit)?;
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

public_id!(ProviderId, MAX_PROVIDER_ID_BYTES);
public_id!(ProfileId, MAX_PUBLIC_ID_BYTES);
public_id!(SourceId, MAX_PUBLIC_ID_BYTES);
