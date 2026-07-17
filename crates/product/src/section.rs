use std::sync::Arc;

use tokenmaster_query::{QueryErrorCode, SnapshotGeneration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductSectionKind {
    Waiting,
    Ready,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductSectionFailure {
    code: QueryErrorCode,
}

impl ProductSectionFailure {
    #[must_use]
    pub const fn code(self) -> QueryErrorCode {
        self.code
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductSection<T> {
    attempt_generation: Option<SnapshotGeneration>,
    payload: Option<Arc<T>>,
    failure: Option<ProductSectionFailure>,
}

impl<T> ProductSection<T> {
    pub(crate) const fn waiting() -> Self {
        Self {
            attempt_generation: None,
            payload: None,
            failure: None,
        }
    }

    pub(crate) fn ready(generation: SnapshotGeneration, payload: T) -> Self {
        Self {
            attempt_generation: Some(generation),
            payload: Some(Arc::new(payload)),
            failure: None,
        }
    }

    pub(crate) const fn unavailable(generation: SnapshotGeneration, code: QueryErrorCode) -> Self {
        Self {
            attempt_generation: Some(generation),
            payload: None,
            failure: Some(ProductSectionFailure { code }),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ProductSectionKind {
        match (&self.payload, self.failure) {
            (Some(_), None) => ProductSectionKind::Ready,
            (None, Some(_)) => ProductSectionKind::Unavailable,
            (None, None) => ProductSectionKind::Waiting,
            (Some(_), Some(_)) => ProductSectionKind::Unavailable,
        }
    }

    #[must_use]
    pub const fn attempt_generation(&self) -> Option<SnapshotGeneration> {
        self.attempt_generation
    }

    #[must_use]
    pub const fn payload(&self) -> Option<&Arc<T>> {
        self.payload.as_ref()
    }

    #[must_use]
    pub const fn failure(&self) -> Option<ProductSectionFailure> {
        self.failure
    }
}
