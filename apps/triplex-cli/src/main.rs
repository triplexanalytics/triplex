//! Thin CLI surface. All logic lives in triplex-schemacast; this binary only
//! parses argv, calls the library, and prints/exits accordingly.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const SAMPLE_ROWS: usize = 1000;
const USAGE: &str = "usage:\n  triplex schema <path-to.csv>\n  triplex validate --schema <path-to.triplex.yaml> --input <path-to.csv>\n  triplex anonymize --recipe <path-to.triplex.yaml> --input <path> --output <path.csv> --mapping-out <path> [--mapping-passphrase-env <VAR>]\n  triplex reveal-mapping --mapping <path> --passphrase-env <VAR> --output <path.csv>";

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

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("schema") => {
            let Some(path) = args.get(1) else {
                return usage_error();
            };
            match triplex_schemacast::infer_schema_from_path(&PathBuf::from(path), SAMPLE_ROWS) {
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
            match triplex_schemacast::validate_path(
                &PathBuf::from(schema),
                &PathBuf::from(input),
                SAMPLE_ROWS,
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
