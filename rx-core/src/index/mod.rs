//! PyPI index client and types

mod client;
mod types;

pub use client::PyPIClient;
pub use types::{FileDigests, FileInfo, PackageInfo, PackageMetadata};
