use std::{num::NonZeroU64, sync::Arc};

use tokenmaster_query::QueryErrorCode;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProductAttemptGeneration(NonZeroU64);

impl ProductAttemptGeneration {
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

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
    attempt_generation: Option<ProductAttemptGeneration>,
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

    pub(crate) fn ready(generation: ProductAttemptGeneration, payload: T) -> Self {
        Self {
            attempt_generation: Some(generation),
            payload: Some(Arc::new(payload)),
            failure: None,
        }
    }

    pub(crate) const fn unavailable(
        generation: ProductAttemptGeneration,
        code: QueryErrorCode,
    ) -> Self {
        Self {
            attempt_generation: Some(generation),
            payload: None,
            failure: Some(ProductSectionFailure { code }),
        }
    }

    pub(crate) fn unavailable_retaining(
        generation: ProductAttemptGeneration,
        code: QueryErrorCode,
        current: &Self,
    ) -> Self {
        Self {
            attempt_generation: Some(generation),
            payload: current.payload.as_ref().map(Arc::clone),
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
    pub const fn attempt_generation(&self) -> Option<ProductAttemptGeneration> {
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

    #[must_use]
    pub const fn retains_payload(&self) -> bool {
        self.payload.is_some() && self.failure.is_some()
    }
}
