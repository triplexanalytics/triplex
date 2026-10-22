use serde::{Deserialize, Serialize};
use std::path::Path;
use triplex_core::{Error, Result};

/// The four Tier 1 mechanisms, plus `Drop`: not a masking strategy at all, but a
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
    /// Whether the method is keyed by a salt (`token_hmac`, `fpe`). Only these read `salt_env`;
    /// bucketing, partial masking and removal give the same output for anyone.
    pub fn is_keyed(self) -> bool {
        matches!(self, AnonymizeMethod::TokenHmac | AnonymizeMethod::Fpe)
    }

    /// The method's name as a recipe spells it, for messages about a recipe.
    pub fn as_str(self) -> &'static str {
        match self {
            AnonymizeMethod::TokenHmac => "token_hmac",
            AnonymizeMethod::Fpe => "fpe",
            AnonymizeMethod::Bucket => "bucket",
            AnonymizeMethod::PartialMask => "partial_mask",
            AnonymizeMethod::Drop => "drop",
        }
    }

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

/// Unknown keys are an error, not ignored: a misspelled `bucket_widht: 5` would otherwise
/// run with the default width and nobody would notice.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnonymizeColumn {
    pub name: String,
    pub method: AnonymizeMethod,
    /// Name of the environment variable holding the HMAC/FPE salt — never
    /// the salt value itself, so it never sits in a `.triplex.yaml` file, a
    /// shell history, or a process listing. Required for the keyed methods
    /// (`token_hmac`, `fpe`); the others don't key their transform, so a
    /// recipe may leave it out for them (it is then empty).
    #[serde(default)]
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

/// Written with only the settings its method uses, so a recipe file never carries a salt
/// variable a bucketing column ignores or a bucket width a token column ignores.
impl Serialize for AnonymizeColumn {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = serializer.serialize_struct("AnonymizeColumn", 5)?;
        st.serialize_field("name", &self.name)?;
        st.serialize_field("method", &self.method)?;
        if self.method.is_keyed() {
            st.serialize_field("salt_env", &self.salt_env)?;
        }
        if self.method == AnonymizeMethod::Bucket {
            st.serialize_field("bucket_width", &self.bucket_width)?;
        }
        if self.method == AnonymizeMethod::PartialMask {
            st.serialize_field("keep_prefix", &self.keep_prefix)?;
            st.serialize_field("keep_suffix", &self.keep_suffix)?;
        }
        st.end()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AnonymizeRecipe {
    pub version: u32,
    pub columns: Vec<AnonymizeColumn>,
}

/// The recipe format version this build reads and writes.
pub const RECIPE_VERSION: u32 = 1;

/// Loads a declarative anonymization recipe from a local `.triplex.yaml` file.
pub fn load_recipe(path: &Path) -> Result<AnonymizeRecipe> {
    recipe_from_yaml(&std::fs::read_to_string(path)?)
}

/// Parses recipe YAML text and verifies it is well-formed — the one way every
/// surface (CLI, Python, browser) reads a recipe, so they all accept and reject
/// the same files. The version is checked before the columns are read, so a
/// recipe written for another format version is reported as that, not as
/// whatever column setting this build doesn't recognize. Keys at the top
/// level other than `version` and `columns` are ignored, so a recipe can share
/// a `.triplex.yaml` file with other sections.
pub fn recipe_from_yaml(text: &str) -> Result<AnonymizeRecipe> {
    let value: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(text).map_err(|e| Error::Yaml(e.to_string()))?;
    if !value.is_mapping() {
        return Err(Error::Validation(
            "not a recipe: expected version and columns at the top level".to_string(),
        ));
    }
    match value.get("version") {
        None => return Err(Error::Validation("recipe has no version".to_string())),
        Some(v) => match v.as_u64() {
            Some(v) if v == u64::from(RECIPE_VERSION) => {}
            Some(v) => return Err(unsupported_version(v)),
            None => {
                return Err(Error::Validation(
                    "recipe version must be a whole number".to_string(),
                ))
            }
        },
    }
    let recipe: AnonymizeRecipe = serde_yaml_ng::from_value(value.clone())
        .map_err(|e| column_error(&value).unwrap_or_else(|| Error::Yaml(e.to_string())))?;
    verify_recipe_well_formed(&recipe)?;
    Ok(recipe)
}

fn unsupported_version(version: u64) -> Error {
    Error::Validation(format!(
        "recipe version {version} is not supported: this build reads version {RECIPE_VERSION}"
    ))
}

/// Finds the first column that doesn't parse and reports it by name (or by
/// position when it has no name), since a parse error on the whole recipe can
/// only point at `columns[3]`. A `salt` key gets its own message: it most
/// likely holds the salt itself, which a recipe must never carry.
fn column_error(recipe: &serde_yaml_ng::Value) -> Option<Error> {
    let columns = recipe.get("columns")?.as_sequence()?;
    columns.iter().enumerate().find_map(|(i, column)| {
        let label = match column.get("name").and_then(|n| n.as_str()) {
            Some(name) => format!("column \"{name}\""),
            None => format!("column {}", i + 1),
        };
        if column.get("salt").is_some() {
            return Some(Error::Validation(format!(
                "{label} has a salt key: a recipe names the environment variable that holds \
                 the salt (salt_env), never the salt itself"
            )));
        }
        serde_yaml_ng::from_value::<AnonymizeColumn>(column.clone())
            .err()
            .map(|e| Error::Yaml(format!("{label}: {e}")))
    })
}

/// Verifies a recipe is internally well-formed — the version this build
/// reads, at least one column declared, none with an empty name, no column
/// declared twice, and a `salt_env` on every keyed column — independent of
/// checking it against any actual data.
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
    if recipe.version != RECIPE_VERSION {
        return Err(unsupported_version(u64::from(recipe.version)));
    }
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
        if column.method.is_keyed() && column.salt_env.is_empty() {
            return Err(Error::Validation(format!(
                "column \"{}\" uses {}, which needs salt_env: the environment variable holding \
                 its salt",
                column.name,
                column.method.as_str()
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
    fn a_salt_free_method_may_leave_out_salt_env() {
        let recipe: AnonymizeRecipe = serde_yaml_ng::from_str(
            "version: 1\ncolumns:\n- name: age\n  method: bucket\n  bucket_width: 10\n",
        )
        .unwrap();
        assert_eq!(recipe.columns[0].salt_env, "");
    }

    #[test]
    fn a_recipe_is_written_with_only_the_settings_its_method_uses() {
        let column = |name: &str, method| AnonymizeColumn {
            name: name.into(),
            method,
            salt_env: if method.is_keyed() {
                "TRIPLEX_SALT".into()
            } else {
                String::new()
            },
            bucket_width: default_bucket_width(),
            keep_prefix: default_keep_prefix(),
            keep_suffix: default_keep_suffix(),
        };
        let recipe = AnonymizeRecipe {
            version: 1,
            columns: vec![
                column("email", AnonymizeMethod::TokenHmac),
                column("age", AnonymizeMethod::Bucket),
                column("card", AnonymizeMethod::PartialMask),
                column("notes", AnonymizeMethod::Drop),
            ],
        };
        let yaml = serde_yaml_ng::to_string(&recipe).unwrap();
        assert_eq!(
            yaml,
            "version: 1\ncolumns:\n\
             - name: email\n  method: token_hmac\n  salt_env: TRIPLEX_SALT\n\
             - name: age\n  method: bucket\n  bucket_width: 10\n\
             - name: card\n  method: partial_mask\n  keep_prefix: 0\n  keep_suffix: 4\n\
             - name: notes\n  method: drop\n"
        );
        // And it reads back to the same recipe.
        let back: AnonymizeRecipe = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(serde_yaml_ng::to_string(&back).unwrap(), yaml);
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

    #[test]
    fn a_recipe_of_another_version_fails_verification() {
        let recipe = AnonymizeRecipe {
            version: 2,
            columns: vec![token_hmac_column("email", "HASH_SALT")],
        };
        let err = verify_recipe_well_formed(&recipe).unwrap_err();
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }

    #[test]
    fn a_keyed_column_without_salt_env_fails_verification() {
        let recipe = AnonymizeRecipe {
            version: 1,
            columns: vec![token_hmac_column("email", "")],
        };
        let err = verify_recipe_well_formed(&recipe).unwrap_err().to_string();
        assert!(
            err.contains("\"email\" uses token_hmac") && err.contains("salt_env"),
            "{err}"
        );
    }

    /// The message of a recipe that `recipe_from_yaml` rejects, with its error code.
    fn rejection(yaml: &str) -> (&'static str, String) {
        let err = recipe_from_yaml(yaml).unwrap_err();
        (err.code(), err.to_string())
    }

    #[test]
    fn loads_the_sample_recipe() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/sample.anonymize.yaml");
        let recipe = load_recipe(&path).unwrap();
        assert_eq!(recipe.columns.len(), 1);
        assert_eq!(recipe.columns[0].name, "name");
        assert_eq!(recipe.columns[0].salt_env, "TRIPLEX_DEMO_SALT");
    }

    #[test]
    fn a_recipe_read_from_yaml_is_written_back_unchanged() {
        let yaml = "version: 1\ncolumns:\n\
                    - name: email\n  method: token_hmac\n  salt_env: CUSTOMER_SALT\n\
                    - name: iban\n  method: fpe\n  salt_env: TRIPLEX_SALT\n\
                    - name: age\n  method: bucket\n  bucket_width: 5\n\
                    - name: card\n  method: partial_mask\n  keep_prefix: 2\n  keep_suffix: 3\n\
                    - name: notes\n  method: drop\n";
        let recipe = recipe_from_yaml(yaml).unwrap();
        assert_eq!(serde_yaml_ng::to_string(&recipe).unwrap(), yaml);
    }

    #[test]
    fn other_top_level_sections_are_left_alone() {
        let yaml = "version: 1\ncolumns:\n- name: age\n  method: bucket\ndriftguard:\n  \
                    max_null_percentage_delta: 0.5\n";
        assert_eq!(recipe_from_yaml(yaml).unwrap().columns.len(), 1);
    }

    #[test]
    fn text_that_is_not_yaml_is_a_parse_error() {
        let (code, _) = rejection("version: 1\ncolumns: [\n");
        assert_eq!(code, "YAML_PARSE_ERROR");
    }

    #[test]
    fn yaml_that_is_not_a_mapping_is_not_a_recipe() {
        let (code, message) = rejection("- name: email\n  method: token_hmac\n");
        assert_eq!(code, "VALIDATION_ERROR");
        assert!(message.contains("not a recipe"), "{message}");
    }

    #[test]
    fn an_unknown_method_names_the_column() {
        let (code, message) = rejection(
            "version: 1\ncolumns:\n- name: age\n  method: bucket\n\
             - name: email\n  method: mask_domain_preserve\n  salt_env: TRIPLEX_SALT\n",
        );
        assert_eq!(code, "YAML_PARSE_ERROR");
        assert!(
            message.contains("column \"email\"") && message.contains("mask_domain_preserve"),
            "{message}"
        );
    }

    #[test]
    fn an_unknown_column_setting_names_the_column_and_the_setting() {
        let (code, message) =
            rejection("version: 1\ncolumns:\n- name: age\n  method: bucket\n  bucket_widht: 5\n");
        assert_eq!(code, "YAML_PARSE_ERROR");
        assert!(
            message.contains("column \"age\"") && message.contains("bucket_widht"),
            "{message}"
        );
    }

    #[test]
    fn a_column_without_a_name_is_named_by_its_position() {
        let (_, message) =
            rejection("version: 1\ncolumns:\n- name: age\n  method: bucket\n- method: drop\n");
        assert!(message.contains("column 2"), "{message}");
    }

    #[test]
    fn a_salt_in_a_recipe_is_rejected_without_repeating_it() {
        let (code, message) = rejection(
            "version: 1\ncolumns:\n- name: email\n  method: token_hmac\n  \
             salt_env: TRIPLEX_SALT\n  salt: synthetic-test-salt\n",
        );
        assert_eq!(code, "VALIDATION_ERROR");
        assert!(
            message.contains("column \"email\"") && message.contains("salt_env"),
            "{message}"
        );
        assert!(!message.contains("synthetic-test-salt"), "{message}");
    }

    #[test]
    fn another_version_is_reported_before_its_columns_are_read() {
        let (code, message) =
            rejection("version: 2\ncolumns:\n- name: age\n  method: bucket\n  bins: 4\n");
        assert_eq!(code, "VALIDATION_ERROR");
        assert!(
            message.contains("version 2 is not supported") && message.contains("version 1"),
            "{message}"
        );
    }

    #[test]
    fn a_recipe_without_a_version_is_rejected() {
        let (code, message) = rejection("columns:\n- name: age\n  method: bucket\n");
        assert_eq!(code, "VALIDATION_ERROR");
        assert!(message.contains("no version"), "{message}");
    }

    #[test]
    fn a_version_that_is_not_a_number_is_rejected() {
        let (_, message) = rejection("version: one\ncolumns:\n- name: age\n  method: bucket\n");
        assert!(message.contains("whole number"), "{message}");
    }

    #[test]
    fn a_parsed_recipe_is_also_verified() {
        let (code, message) = rejection(
            "version: 1\ncolumns:\n- name: age\n  method: bucket\n- name: age\n  method: drop\n",
        );
        assert_eq!(code, "VALIDATION_ERROR");
        assert!(message.contains("more than once"), "{message}");
        let (_, message) = rejection("version: 1\ncolumns:\n- name: email\n  method: fpe\n");
        assert!(message.contains("needs salt_env"), "{message}");
    }
}
