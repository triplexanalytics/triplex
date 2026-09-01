use serde::Deserialize;
use std::path::Path;
use triplex_core::{Error, Result};

/// Only `token_hmac` is implemented in this slice. Declaring any other
/// method (e.g. the generalization/bucketing methods in the wider spec) is a
/// clear deserialization error, not a silently ignored no-op.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnonymizeMethod {
    TokenHmac,
}

#[derive(Debug, Deserialize)]
pub struct AnonymizeColumn {
    pub name: String,
    pub method: AnonymizeMethod,
    /// Name of the environment variable holding the HMAC salt — never the
    /// salt value itself, so it never sits in a `.triplex.yaml` file, a
    /// shell history, or a process listing.
    pub salt_env: String,
}

#[derive(Debug, Deserialize)]
pub struct AnonymizeRecipe {
    pub version: u32,
    pub columns: Vec<AnonymizeColumn>,
}

/// Loads a declarative anonymization recipe from a local `.triplex.yaml` file.
pub fn load_recipe(path: &Path) -> Result<AnonymizeRecipe> {
    let text = std::fs::read_to_string(path)?;
    serde_yaml_ng::from_str(&text).map_err(|e| Error::Yaml(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
