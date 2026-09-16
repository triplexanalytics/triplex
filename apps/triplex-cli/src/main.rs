//! Thin CLI surface. All logic lives in triplex-schemacast; this binary only
//! parses argv, calls the library, and prints/exits accordingly.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const SAMPLE_ROWS: usize = 1000;
const USAGE: &str = "usage:\n  triplex schema <path-to.csv> [--delimiter <char>]\n  triplex validate --schema <path-to.triplex.yaml> --input <path-to.csv> [--delimiter <char>]\n  triplex check-patterns --schema <path-to.triplex.yaml> --input <path>\n  triplex convert --input <path> --output <path> [--input-delimiter <char>] [--output-delimiter <char>]\n  triplex anonymize --recipe <path-to.triplex.yaml> --input <path> --output <path.csv> --mapping-out <path> [--mapping-passphrase-env <VAR>] [--exclude-rows <n,n,...>]\n  triplex reveal-mapping --mapping <path> --passphrase-env <VAR> --output <path.csv>\n\n--delimiter/--input-delimiter/--output-delimiter apply only to a .csv path (e.g. a semicolon-delimited European export) — use \"tab\" for a literal tab, or any single character otherwise.\n--exclude-rows takes comma-separated 0-based row numbers (e.g. \"1,4,7\") to drop from the output entirely before masking.";

/// Exit codes per the design notes: 0 success, 1
/// non-blocking warning (no coercion modes exist yet, so unused today), 2
/// critical gate failure (a contract breach), 127 runtime/config error (bad
/// file, bad usage — anything that isn't a data problem).
const EXIT_RUNTIME_CONFIG_ERROR: u8 = 127;
const EXIT_CONTRACT_BREACH: u8 = 2;

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn usage_error() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
}

/// Reads a secret (salt, passphrase) from an env var named by the user —
/// the secret itself is never a CLI argument, so it never lands in shell
/// history or a process listing.
fn read_env_var(var: &str) -> std::result::Result<String, ExitCode> {
    env::var(var).map_err(|_| {
        eprintln!("error: environment variable \"{var}\" is not set");
        ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
    })
}

/// Parses a `--delimiter`-style flag value into a single byte. `"tab"` is a
/// named alias since a literal tab character is awkward to type on a
/// command line; anything else is taken as its first byte (covers the
/// common cases: `;`, `|`, `:`, ...).
fn parse_delimiter(flag: &str, value: &str) -> std::result::Result<u8, ExitCode> {
    if value.eq_ignore_ascii_case("tab") {
        return Ok(b'\t');
    }
    value.bytes().next().ok_or_else(|| {
        eprintln!("error: {flag} needs a character (or \"tab\"), got an empty value");
        ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
    })
}

/// Resolves an optional `--delimiter`-style flag, or `Ok(None)` if the flag
/// wasn't given at all.
fn optional_delimiter(args: &[String], flag: &str) -> std::result::Result<Option<u8>, ExitCode> {
    match flag_value(args, flag) {
        Some(value) => parse_delimiter(flag, &value).map(Some),
        None => Ok(None),
    }
}

/// Parses `--exclude-rows`'s comma-separated 0-based row-index list (e.g.
/// `"1,4,7"`) into the `HashSet<usize>` `anonymize_file` expects — an empty
/// set (no rows excluded) if the flag wasn't given at all, so this is
/// backward compatible with every existing `triplex anonymize` invocation.
fn parse_excluded_rows(
    args: &[String],
) -> std::result::Result<std::collections::HashSet<usize>, ExitCode> {
    let Some(value) = flag_value(args, "--exclude-rows") else {
        return Ok(std::collections::HashSet::new());
    };
    value
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<usize>().map_err(|_| {
                eprintln!("error: --exclude-rows expects comma-separated row numbers, got {s:?}");
                ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
            })
        })
        .collect()
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("schema") => {
            let Some(path) = args.get(1) else {
                return usage_error();
            };
            let delimiter = match optional_delimiter(&args, "--delimiter") {
                Ok(d) => d,
                Err(code) => return code,
            };
            let result = match delimiter {
                Some(d) => triplex_schemacast::infer_schema_from_csv_with_delimiter(
                    &PathBuf::from(path),
                    d,
                    SAMPLE_ROWS,
                ),
                None => {
                    triplex_schemacast::infer_schema_from_path(&PathBuf::from(path), SAMPLE_ROWS)
                }
            };
            match result {
                Ok(contract) => {
                    println!("{}", serde_json::to_string_pretty(&contract).unwrap());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
                }
            }
        }
        Some("validate") => {
            let (Some(schema), Some(input)) =
                (flag_value(&args, "--schema"), flag_value(&args, "--input"))
            else {
                return usage_error();
            };
            let delimiter = match optional_delimiter(&args, "--delimiter") {
                Ok(d) => d,
                Err(code) => return code,
            };
            let result = match delimiter {
                Some(d) => triplex_schemacast::validate_csv_with_delimiter(
                    &PathBuf::from(schema),
                    &PathBuf::from(input),
                    d,
                    SAMPLE_ROWS,
                ),
                None => triplex_schemacast::validate_path(
                    &PathBuf::from(schema),
                    &PathBuf::from(input),
                    SAMPLE_ROWS,
                ),
            };
            match result {
                Ok(report) => {
                    println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    if report.valid {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(EXIT_CONTRACT_BREACH)
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
                }
            }
        }
        Some("check-patterns") => {
            let (Some(schema), Some(input)) =
                (flag_value(&args, "--schema"), flag_value(&args, "--input"))
            else {
                return usage_error();
            };
            match triplex_schemacast::check_patterns_path(
                &PathBuf::from(schema),
                &PathBuf::from(input),
            ) {
                Ok(report) => {
                    println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    if report.valid {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(EXIT_CONTRACT_BREACH)
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
                }
            }
        }
        Some("convert") => {
            let (Some(input), Some(output)) =
                (flag_value(&args, "--input"), flag_value(&args, "--output"))
            else {
                return usage_error();
            };
            let input_delimiter = match optional_delimiter(&args, "--input-delimiter") {
                Ok(d) => d,
                Err(code) => return code,
            };
            let output_delimiter = match optional_delimiter(&args, "--output-delimiter") {
                Ok(d) => d,
                Err(code) => return code,
            };
            match triplex_schemacast::convert_file_with_delimiters(
                &PathBuf::from(input),
                &PathBuf::from(output),
                input_delimiter,
                output_delimiter,
            ) {
                Ok(report) => {
                    println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
                }
            }
        }
        Some("anonymize") => {
            let (Some(recipe), Some(input), Some(output), Some(mapping_out)) = (
                flag_value(&args, "--recipe"),
                flag_value(&args, "--input"),
                flag_value(&args, "--output"),
                flag_value(&args, "--mapping-out"),
            ) else {
                return usage_error();
            };

            let passphrase = match flag_value(&args, "--mapping-passphrase-env") {
                Some(var) => match read_env_var(&var) {
                    Ok(value) => Some(value),
                    Err(code) => return code,
                },
                None => None,
            };

            let excluded_rows = match parse_excluded_rows(&args) {
                Ok(set) => set,
                Err(code) => return code,
            };

            let recipe = match triplex_anonymizer::load_recipe(&PathBuf::from(recipe)) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                }
            };

            match triplex_anonymizer::anonymize_file(
                &PathBuf::from(input),
                &recipe,
                &PathBuf::from(output),
                &PathBuf::from(mapping_out),
                passphrase.as_deref(),
                &excluded_rows,
            ) {
                Ok(report) => {
                    eprintln!("warning: {}", report.warning);
                    println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
                }
            }
        }
        Some("reveal-mapping") => {
            let (Some(mapping), Some(passphrase_env), Some(output)) = (
                flag_value(&args, "--mapping"),
                flag_value(&args, "--passphrase-env"),
                flag_value(&args, "--output"),
            ) else {
                return usage_error();
            };

            let passphrase = match read_env_var(&passphrase_env) {
                Ok(value) => value,
                Err(code) => return code,
            };

            match triplex_anonymizer::reveal_mapping(
                &PathBuf::from(mapping),
                &passphrase,
                &PathBuf::from(&output),
            ) {
                Ok(()) => {
                    eprintln!(
                        "warning: {output} now contains real, reversible personal data — \
                         handle it with the same care as raw customer data."
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
                }
            }
        }
        _ => usage_error(),
    }
}
