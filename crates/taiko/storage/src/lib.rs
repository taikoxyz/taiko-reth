//! Collection of traits and trait implementations for taiko database operations.
pub mod l1_origin;
pub use l1_origin::*;

/// The trait for providing taiko database operations.
pub trait TaikoProvider: L1OriginReader + L1OriginWriter {}

impl<T> TaikoProvider for T where T: L1OriginReader + L1OriginWriter {}
