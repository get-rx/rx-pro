//! Semantic Versioning (SemVer) implementation
//!
//! A fast, correct implementation of the Semantic Versioning 2.0.0 specification.
//! Supports version parsing, comparison, bumping, and range satisfaction.

mod range;
mod version;

pub use range::{Comparator, Op, Range, VersionReq};
pub use version::{BuildMetadata, Prerelease, Version};
