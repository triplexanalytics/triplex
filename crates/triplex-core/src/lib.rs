//! Shared error type and result alias used across Triplex crates.

mod error;
mod numeric;

pub use error::{Error, Result};
pub use numeric::normalize_numeric_string;
