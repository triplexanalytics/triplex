//! Dispatch point tying a declared column's `AnonymizeMethod` to the
//! concrete masking strategy that runs against its values.
//!
//! This is a Strategy, not a Visitor: there's exactly one kind of element (a
//! `&str` cell value) and several interchangeable algorithms over it — the
//! textbook Strategy shape. A Visitor earns its keep with a closed set of
//! *different* element types and double dispatch, neither of which applies
//! here, so it would only add ceremony. `mask_rows` (see `lib.rs`) is
//! oblivious to which mechanism it's calling; adding a fifth one means
//! adding its variant in `recipe.rs`, its own file here, and one arm below —
//! nothing else in the crate changes.

use crate::recipe::{AnonymizeColumn, AnonymizeMethod};
use crate::{bucket, fpe, partial_mask, tokenize};

pub(crate) trait Mechanism {
    /// Transforms one cell's value into its masked form. Called once per
    /// distinct non-empty value in the column this mechanism was built for.
    fn mask(&self, value: &str) -> String;
}

/// Builds the masking strategy for one resolved column from its declared
/// method, salt, and method-specific parameters.
pub(crate) fn build_mechanism(column: &AnonymizeColumn, salt: &[u8]) -> Box<dyn Mechanism> {
    match column.method {
        AnonymizeMethod::TokenHmac => Box::new(tokenize::HmacTokenizer::new(salt)),
        AnonymizeMethod::Fpe => Box::new(fpe::Fpe::new(salt)),
        AnonymizeMethod::Bucket => Box::new(bucket::Bucketer::new(column.bucket_width)),
        AnonymizeMethod::PartialMask => Box::new(partial_mask::PartialMask::new(
            column.keep_prefix,
            column.keep_suffix,
        )),
    }
}
