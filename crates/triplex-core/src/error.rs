use std::fmt;

/// Every variant carries a stable, self-explanatory `code()` — read directly
/// in logs, CLI output, and `audit.json`, not looked up in an external table.
/// Add a new variant + a matching `code()`/`Display` arm together; nothing
/// else needs to change for it to show up everywhere errors are reported.
#[derive(Debug)]
pub enum Error {
    /// A local file could not be opened/read at the OS level (permissions,
    /// does not exist, disk error, ...).
    Io(std::io::Error),
    /// The file was readable but is not valid CSV.
    Csv(String),
    /// The file was readable but is not valid `.triplex.yaml`.
    Yaml(String),
    /// The file was readable but is not valid Parquet.
    Parquet(String),
    /// The input path's extension isn't one SchemaCast knows how to ingest.
    UnsupportedExtension(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::Io(_) => "IO_ERROR",
            Error::Csv(_) => "CSV_PARSE_ERROR",
            Error::Yaml(_) => "YAML_PARSE_ERROR",
            Error::Parquet(_) => "PARQUET_READ_ERROR",
            Error::UnsupportedExtension(_) => "UNSUPPORTED_EXTENSION",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = self.code();
        match self {
            Error::Io(e) => write!(f, "[{code}] could not read local file: {e}"),
            Error::Csv(e) => write!(f, "[{code}] not a valid CSV file: {e}"),
            Error::Yaml(e) => write!(f, "[{code}] not a valid .triplex.yaml file: {e}"),
            Error::Parquet(e) => write!(f, "[{code}] not a valid Parquet file: {e}"),
            Error::UnsupportedExtension(msg) => write!(f, "[{code}] {msg}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_has_a_distinct_self_explanatory_code() {
        let errors = [
            Error::Io(std::io::Error::other("disk full")),
            Error::Csv("unexpected end of row".into()),
            Error::Yaml("invalid mapping".into()),
            Error::Parquet("bad magic bytes".into()),
            Error::UnsupportedExtension("unsupported file extension \"xlsx\"".into()),
        ];

        let codes: Vec<&str> = errors.iter().map(Error::code).collect();
        let unique: std::collections::HashSet<&&str> = codes.iter().collect();
        assert_eq!(unique.len(), codes.len(), "codes must not collide: {codes:?}");

        for e in &errors {
            let message = e.to_string();
            assert!(
                message.contains(e.code()),
                "message must surface its own code: {message}"
            );
        }
    }
}
