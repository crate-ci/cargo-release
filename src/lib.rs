//! # Semver Compatibility
//!
//! cargo-release's versioning tracks compatibility for the binaries, not the API.  We upload to
//! crates.io to distribute the binary.  If using this as a library, be sure to pin the version
//! with a `=` version requirement operator.

pub mod config;
pub mod error;
pub mod ops;
pub mod steps;
