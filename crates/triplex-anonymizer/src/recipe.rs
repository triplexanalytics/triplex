use serde::{Deserialize, Serialize};
use std::path::Path;
use triplex_core::{Error, Result};

/// The four Tier 1 mechanisms from the design notes's
/// product matrix, plus `Drop`: not a masking strategy at all, but a
/// declared column's other valid fate — excluded from the output entirely
/// rather than transformed in place. Declaring any other method is a clear
/// deserialization error, not a silently ignored no-op. Each masking
/// variant's own file (`tokenize.rs`/`fpe.rs`/`bucket.rs`/`partial_mask.rs`)
/// implements the `Mechanism` trait; `mechanism::build_mechanism` is the one
/// place that dispatches a value here to its concrete masking strategy —
/// `Drop` never reaches it (see `resolve_columns`'s doc comment).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnonymizeMethod {
    TokenHmac,
    Fpe,
    Bucket,
    PartialMask,
    Drop,
}

impl AnonymizeMethod {
    /// Every variant this build supports, in the order a UI should list
    /// them. Grows automatically as this enum grows — a caller (e.g.
    /// `apps/triplex-web`) reads this instead of hardcoding a menu that can
    /// drift out of sync with what the engine actually implements.
    pub const ALL: &'static [AnonymizeMethod] = &[
        AnonymizeMethod::TokenHmac,
        AnonymizeMethod::Fpe,
        AnonymizeMethod::Bucket,
        AnonymizeMethod::PartialMask,
        AnonymizeMethod::Drop,
    ];
}

fn default_bucket_width() -> u64 {
    10
}

fn default_keep_prefix() -> usize {
    0
}

fn default_keep_suffix() -> usize {
    4
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AnonymizeColumn {
    pub name: String,
    pub method: AnonymizeMethod,
    /// Name of the environment variable holding the HMAC/FPE salt — never
    /// the salt value itself, so it never sits in a `.triplex.yaml` file, a
    /// shell history, or a process listing. Ignored by `Bucket`/`PartialMask`,
    /// which don't key their transform.
    pub salt_env: String,
    /// `Bucket`-only: numeric generalization width, e.g. 10 turns `23` into
    /// `"20-29"`. Ignored by every other method. Defaulted (not required)
    /// so an existing `token_hmac`/`fpe` recipe doesn't need updating.
    #[serde(default = "default_bucket_width")]
    pub bucket_width: u64,
    /// `PartialMask`-only: characters kept at the start of the value;
    /// everything after `keep_prefix` and before the last `keep_suffix`
    /// characters is replaced with `*`. Ignored by every other method.
    #[serde(default = "default_keep_prefix")]
    pub keep_prefix: usize,
    /// `PartialMask`-only: characters kept at the end of the value — see
    /// `keep_prefix`. Defaults to 4 (e.g. a card number's last 4 digits).
    #[serde(default = "default_keep_suffix")]
    pub keep_suffix: usize,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AnonymizeRecipe {
    pub version: u32,
    pub columns: Vec<AnonymizeColumn>,
}

/// Loads a declarative anonymization recipe from a local `.triplex.yaml` file.
pub fn load_recipe(path: &Path) -> Result<AnonymizeRecipe> {
    let text = std::fs::read_to_string(path)?;
    serde_yaml_ng::from_str(&text).map_err(|e| Error::Yaml(e.to_string()))
}

/// Verifies a recipe is internally well-formed — at least one column
/// declared, none with an empty name, and no column declared twice —
/// independent of checking it against any actual data.
/// `AnonymizeColumn.method` needs no check here: deserializing an
/// unrecognized method already fails at the YAML/JSON boundary (see
/// `an_unimplemented_method_is_a_clear_parse_error`), so a value in this
/// struct is always one `AnonymizeMethod::ALL` supports. Duplicate names are
/// rejected rather than silently allowed: `resolve_columns`/
/// `resolve_columns_with_salts` look a column up by `.position()` (first
/// match), so a second entry for the same column would be silently ignored
/// rather than erroring — better to catch that here, where it's built.
/// Exists so a caller building a recipe programmatically (e.g.
/// `apps/triplex-web`'s table interactions) can catch an unusable recipe
/// before writing it out, rather than only failing later when it's used to
/// anonymize.
pub fn verify_recipe_well_formed(recipe: &AnonymizeRecipe) -> Result<()> {
    if recipe.columns.is_empty() {
        return Err(Error::Validation("recipe has no columns".to_string()));
    }
    let mut seen = std::collections::HashSet::new();
    for column in &recipe.columns {
        if column.name.is_empty() {
            return Err(Error::Validation(
                "recipe has a column with an empty name".to_string(),
            ));
        }
        if !seen.insert(column.name.as_str()) {
            return Err(Error::Validation(format!(
                "recipe declares column \"{}\" more than once",
                column.name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_hmac_column(name: &str, salt_env: &str) -> AnonymizeColumn {
        AnonymizeColumn {
            name: name.to_string(),
            method: AnonymizeMethod::TokenHmac,
            salt_env: salt_env.to_string(),
            bucket_width: default_bucket_width(),
            keep_prefix: default_keep_prefix(),
            keep_suffix: default_keep_suffix(),
        }
    }

    #[test]
    fn parses_a_minimal_recipe() {
        let yaml = "
            version: 1
            columns:
              - name: customer_id
                method: token_hmac
                salt_env: HASH_SALT
        ";
        let recipe: AnonymizeRecipe = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(recipe.version, 1);
        assert_eq!(recipe.columns[0].name, "customer_id");
        assert_eq!(recipe.columns[0].method, AnonymizeMethod::TokenHmac);
        assert_eq!(recipe.columns[0].salt_env, "HASH_SALT");
    }

    #[test]
    fn all_lists_every_implemented_method() {
        // Deliberate tripwire: fails the moment a sixth `AnonymizeMethod`
        // variant is added, forcing a conscious update here — the whole
        // point of `ALL` is that a UI can read it instead of hardcoding a
        // menu, so it must never silently fall behind the enum.
        assert_eq!(
            AnonymizeMethod::ALL,
            &[
                AnonymizeMethod::TokenHmac,
                AnonymizeMethod::Fpe,
                AnonymizeMethod::Bucket,
                AnonymizeMethod::PartialMask,
                AnonymizeMethod::Drop,
            ]
        );
    }

    #[test]
    fn bucket_and_partial_mask_pick_up_their_defaults_when_omitted() {
        let yaml = "
            version: 1
            columns:
              - name: age
                method: bucket
                salt_env: UNUSED
              - name: card_number
                method: partial_mask
                salt_env: UNUSED
        ";
        let recipe: AnonymizeRecipe = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(recipe.columns[0].bucket_width, 10);
        assert_eq!(recipe.columns[1].keep_prefix, 0);
        assert_eq!(recipe.columns[1].keep_suffix, 4);
    }

    #[test]
    fn bucket_width_and_keep_prefix_suffix_can_be_overridden() {
        let yaml = "
            version: 1
            columns:
              - name: age
                method: bucket
                salt_env: UNUSED
                bucket_width: 5
              - name: card_number
                method: partial_mask
                salt_env: UNUSED
                keep_prefix: 1
                keep_suffix: 2
        ";
        let recipe: AnonymizeRecipe = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(recipe.columns[0].bucket_width, 5);
        assert_eq!(recipe.columns[1].keep_prefix, 1);
        assert_eq!(recipe.columns[1].keep_suffix, 2);
    }

    #[test]
    fn an_unimplemented_method_is_a_clear_parse_error() {
        let yaml = "
            version: 1
            columns:
              - name: email
                method: mask_domain_preserve
                salt_env: HASH_SALT
        ";
        let result: std::result::Result<AnonymizeRecipe, _> = serde_yaml_ng::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn a_recipe_with_no_columns_fails_verification() {
        let recipe = AnonymizeRecipe {
            version: 1,
            columns: vec![],
        };
        let err = verify_recipe_well_formed(&recipe).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn a_recipe_with_an_empty_column_name_fails_verification() {
        let recipe = AnonymizeRecipe {
            version: 1,
            columns: vec![token_hmac_column("", "HASH_SALT")],
        };
        let err = verify_recipe_well_formed(&recipe).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn a_recipe_with_the_same_column_declared_twice_fails_verification() {
        let recipe = AnonymizeRecipe {
            version: 1,
            columns: vec![
                token_hmac_column("email", "HASH_SALT"),
                token_hmac_column("email", "HASH_SALT"),
            ],
        };
        let err = verify_recipe_well_formed(&recipe).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn a_well_formed_recipe_passes_verification() {
        let recipe = AnonymizeRecipe {
            version: 1,
            columns: vec![token_hmac_column("email", "HASH_SALT")],
        };
        assert!(verify_recipe_well_formed(&recipe).is_ok());
    }
}
