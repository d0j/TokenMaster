#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum DomainError {
    #[error("{field} must contain between 1 and {max_chars} characters")]
    InvalidText {
        field: &'static str,
        max_chars: usize,
    },
    #[error("used ratio must be finite and within 0.0..=1.0")]
    InvalidUsedRatio,
    #[error("reset time must be positive")]
    InvalidResetTime,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct QuotaTarget {
    provider: String,
    id: String,
    label: String,
    used_ratio: f64,
    resets_at_ms: i64,
}

impl QuotaTarget {
    pub fn new(
        provider: impl Into<String>,
        id: impl Into<String>,
        label: impl Into<String>,
        used_ratio: f64,
        resets_at_ms: i64,
    ) -> Result<Self, DomainError> {
        let provider = validated_text(provider.into(), "provider", 64)?;
        let id = validated_text(id.into(), "id", 64)?;
        let label = validated_text(label.into(), "label", 128)?;
        if !used_ratio.is_finite() || !(0.0..=1.0).contains(&used_ratio) {
            return Err(DomainError::InvalidUsedRatio);
        }
        if resets_at_ms <= 0 {
            return Err(DomainError::InvalidResetTime);
        }

        Ok(Self {
            provider,
            id,
            label,
            used_ratio,
            resets_at_ms,
        })
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn used_ratio(&self) -> f64 {
        self.used_ratio
    }

    pub fn resets_at_ms(&self) -> i64 {
        self.resets_at_ms
    }
}

fn validated_text(
    value: String,
    field: &'static str,
    max_chars: usize,
) -> Result<String, DomainError> {
    let value = value.trim();
    let length = value.chars().count();
    if length == 0 || length > max_chars {
        return Err(DomainError::InvalidText { field, max_chars });
    }
    Ok(value.to_owned())
}
