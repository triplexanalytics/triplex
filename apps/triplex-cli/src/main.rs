//! Thin CLI surface. All logic lives in triplex-schemacast; this binary only
//! parses argv, calls the library, and prints/exits accordingly.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const SAMPLE_ROWS: usize = 1000;
const USAGE: &str = "usage:\n  triplex --version\n  triplex schema <path-to.csv> [--delimiter <char>]\n  triplex validate --schema <path-to.triplex.yaml> --input <path-to.csv> [--delimiter <char>]\n  triplex check-patterns --schema <path-to.triplex.yaml> --input <path>\n  triplex convert --input <path> --output <path> [--input-delimiter <char>] [--output-delimiter <char>]\n  triplex anonymize --recipe <path-to.triplex.yaml> --input <path> --output <path.csv> --mapping-out <path> [--mapping-passphrase-env <VAR>] [--exclude-rows <n,n,...>] [--profile-out <path.json>]\n  triplex reveal-mapping --mapping <path> --passphrase-env <VAR> --output <path.csv>\n  triplex driftguard (--baseline <path> | --baseline-profile <path.json>) (--candidate <path> | --candidate-profile <path.json>) --out-dir <dir> [--config <path.triplex.yaml>] [--max-null-percentage-delta <f>] [--max-mean-relative-delta <f>] [--max-type-consistency-delta <f>]\n  triplex modelcard --config <path.triplex.yaml> --input <path> --out-dir <dir>\n  triplex synthesize --input <path> --output <path.csv> --report-dir <dir> [--rows <n>] [--seed <n>] [--min-category-count <n>] [--method copula|bayes-net|marginals|sketch [--structure chow-liu|k-parent] [--parents <1-3>] [--bins <n>] [--classes <n>] [--min-class-size <n>]] [--sketch <path.triplex.yaml>] [--config <path.triplex.yaml> [--with-nonprivate-diagnostics]]\n\n--delimiter/--input-delimiter/--output-delimiter apply only to a .csv path (e.g. a semicolon-delimited European export) — use \"tab\" for a literal tab, or any single character otherwise.\n--exclude-rows takes comma-separated 0-based row numbers (e.g. \"1,4,7\") to drop from the output entirely before masking.\n--profile-out writes a DriftProfile JSON (this run's output schema plus its risk_assessment, if any) for a later `triplex driftguard --baseline-profile`/`--candidate-profile` to consume.\ntriplex driftguard writes audit.json, report.html, and reproduce.py into --out-dir, and exits 0 (no drift), 1 (statistical drift only), or 2 (a schema regression: a column removed or its type changed). A --*-profile input also diffs its risk_assessment against the other side's, when both have one. --config's `.triplex.yaml` may carry an optional `driftguard:` block (max_null_percentage_delta/max_mean_relative_delta/max_type_consistency_delta) as the default layer under any --max-* flag.\ntriplex modelcard reads the `modelcard:` block of --config (target_column, prediction_column, optional score_column, slice_by, protected_attribute, thresholds), evaluates every row of --input, writes model_card.json, report.html and reproduce.py into --out-dir, and exits 0 (pass), 1 (warning: a suppressed or undefined slice) or 2 (a fairness threshold breach). The card is a documentation aid, not a compliance claim.\ntriplex synthesize fits a Gaussian copula to --input (.csv or .parquet), or with --method bayes-net a Bayesian network (a Chow-Liu tree, or up to --parents parents per column) that also keeps categorical dependencies, or with --method marginals independent marginals (every column drawn on its own, a baseline that keeps no dependency between columns), and writes a synthetic CSV to --output, plus synthesis_report.json, report.html, reproduce.py and drift_check.json (a DriftGuard diff of the synthetic file against the original, on the synthesized columns) into --report-dir. Identifier-like columns are dropped, not synthesized. The output carries NO formal privacy guarantee; the report measures fidelity and leakage but certifies nothing. With --method sketch (Tier 3) the rows are grouped into --classes classes (default 4; a class under --min-class-size rows, default 20, is folded into the nearest one), each class is described per column by a range and a density or by a share per value, and rows are drawn class by class with every column independent inside a class. The sketch that was used is written to sketch.triplex.yaml in --report-dir; it is derived from the data, so handle it like the data. Edit that file and pass it back with --sketch to generate from the edited sketch.\ntriplex synthesize --config switches to differentially private mode: the `synthesize:` block of the config declares a required `epsilon` (no default) and each column's public bounds or category values; the fitted parameters are epsilon-DP for replacing one record with the row count public, using operating-system randomness for the noise (--seed only seeds sampling). With `method: bayesian_network` in the block (optional `structure: chow_liu|k_parent`, `max_parents`, `structure_share`) the DP model is a Bayesian network: the dependency structure is chosen by report-noisy-max and the conditional tables are noisy counts, both inside the same epsilon. With `method: independent_marginals` every column is a noisy histogram of its own and the whole epsilon goes to those histograms. It writes dp_report.json and report.html. The report holds no real-data figures unless --with-nonprivate-diagnostics is given, which adds them (and drift_check.json) under a warning that they are outside the guarantee.";

/// Exit codes: 0 success, 1
/// non-blocking warning (`driftguard` statistical drift with no schema
/// regression), 2 critical gate failure (a contract breach, or a
/// `driftguard` schema regression), 127 runtime/config error (bad file, bad
/// usage — anything that isn't a data problem).
const EXIT_RUNTIME_CONFIG_ERROR: u8 = 127;
const EXIT_CONTRACT_BREACH: u8 = 2;
const EXIT_NON_BLOCKING_WARNING: u8 = 1;

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

/// Resolves an optional `--flag <f64>`-style threshold override as `None`
/// when the flag wasn't given -- for `driftguard`, where an unset CLI flag
/// should fall through to a `--config` value before this crate's own
/// built-in default (see `DriftThresholds::with_overrides`).
fn optional_f64_flag_no_default(
    args: &[String],
    flag: &str,
) -> std::result::Result<Option<f64>, ExitCode> {
    match flag_value(args, flag) {
        Some(value) => value.parse::<f64>().map(Some).map_err(|_| {
            eprintln!("error: {flag} expects a number, got {value:?}");
            ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
        }),
        None => Ok(None),
    }
}

/// Resolves one `driftguard` side (baseline or candidate) from exactly one
/// of a raw data file (`--baseline`/`--candidate`, re-inferred fresh) or an
/// already-minted `DriftProfile` JSON (`--baseline-profile`/
/// `--candidate-profile`, e.g. from `triplex anonymize --profile-out`) --
/// the latter also carries a `RiskAssessment` a bare data file can't
/// provide (see `triplex_driftguard::DriftProfile`'s doc comment for why).
fn load_driftguard_side(
    side: &str,
    file_path: Option<&str>,
    profile_path: Option<&str>,
    sample_rows: usize,
) -> std::result::Result<
    (
        triplex_schemacast::SchemaContract,
        Option<triplex_driftguard::RiskAssessment>,
    ),
    ExitCode,
> {
    match (file_path, profile_path) {
        (Some(_), Some(_)) => {
            eprintln!("error: --{side} and --{side}-profile are mutually exclusive");
            Err(ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR))
        }
        (Some(path), None) => {
            triplex_schemacast::infer_schema_from_path(&PathBuf::from(path), sample_rows)
                .map(|schema| (schema, None))
                .map_err(|e| {
                    eprintln!("error: {e}");
                    ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
                })
        }
        (None, Some(path)) => triplex_driftguard::load_profile(&PathBuf::from(path))
            .map(|profile| (profile.schema, profile.risk_assessment))
            .map_err(|e| {
                eprintln!("error: {e}");
                ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
            }),
        (None, None) => {
            eprintln!("error: need one of --{side} or --{side}-profile");
            Err(ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR))
        }
    }
}

/// `triplex synthesize --config`: the differentially private path.
fn run_synthesize_dp(
    args: &[String],
    config_path: &std::path::Path,
    input: &std::path::Path,
    output: &std::path::Path,
    report_dir: &std::path::Path,
    rows: Option<usize>,
    seed: u64,
) -> ExitCode {
    let fail = |e: &dyn std::fmt::Display| {
        eprintln!("error: {e}");
        ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
    };
    let contract = match triplex_schemacast::load_contract(config_path) {
        Ok(c) => c,
        Err(e) => return fail(&e),
    };
    let Some(declared) = contract.synthesize.as_ref() else {
        return fail(&"the config has no `synthesize:` block");
    };
    let bayes = triplex_anonymizer::synthesize::bayesnet::dp::is_bayes(declared);
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return fail(&format!("could not create {parent:?}: {e}"));
        }
    }
    let with_diagnostics = args.iter().any(|a| a == "--with-nonprivate-diagnostics");
    let report = if bayes {
        let config = match triplex_anonymizer::synthesize::bayesnet::dp::config_from_declared(
            declared, rows, seed,
        ) {
            Ok(c) => c,
            Err(e) => return fail(&e),
        };
        match triplex_anonymizer::synthesize::bayesnet::dp::synthesize_bayes_dp_file(
            input,
            output,
            &config,
            with_diagnostics,
            &mut triplex_anonymizer::synthesize::dp::OsNoise,
        ) {
            Ok(r) => r,
            Err(e) => return fail(&e),
        }
    } else {
        let config =
            match triplex_anonymizer::synthesize::dp::config_from_declared(declared, rows, seed) {
                Ok(c) => c,
                Err(e) => return fail(&e),
            };
        match triplex_anonymizer::synthesize::dp::synthesize_dp_file(
            input,
            output,
            &config,
            with_diagnostics,
            &mut triplex_anonymizer::synthesize::dp::OsNoise,
        ) {
            Ok(r) => r,
            Err(e) => return fail(&e),
        }
    };
    if let Err(e) = std::fs::create_dir_all(report_dir) {
        return fail(&format!(
            "could not create --report-dir {report_dir:?}: {e}"
        ));
    }
    let report_json = serde_json::to_string_pretty(&report).unwrap();
    let mut files = vec![
        ("dp_report.json", report_json.clone()),
        (
            "report.html",
            triplex_anonymizer::synthesize::dp::render_html(&report),
        ),
    ];
    if with_diagnostics {
        // A DriftGuard diff reads the original data, so it is a non-private diagnostic too.
        let infer =
            |p: &std::path::Path| triplex_schemacast::infer_schema_from_path(p, SAMPLE_ROWS);
        if let (Ok(mut baseline), Ok(synthetic)) = (infer(input), infer(output)) {
            baseline
                .columns
                .retain(|c| report.columns.contains(&c.name));
            let drift = triplex_driftguard::compare_contracts(
                &baseline,
                &synthetic,
                &triplex_driftguard::DriftThresholds::default(),
            );
            files.push((
                "drift_check.json",
                serde_json::to_string_pretty(&drift).unwrap(),
            ));
        }
        eprintln!(
            "warning: {}",
            triplex_anonymizer::synthesize::dp::DIAGNOSTICS_WARNING
        );
    }
    for (name, contents) in files {
        if let Err(e) = std::fs::write(report_dir.join(name), contents) {
            return fail(&format!("could not write {name} into {report_dir:?}: {e}"));
        }
    }
    eprintln!(
        "note: epsilon-DP synthesis (epsilon {}, spent {:.6}); the guarantee covers the fitted parameters, with the row count, column list and bounds treated as public",
        report.privacy.epsilon_total, report.privacy.epsilon_spent
    );
    println!("{report_json}");
    ExitCode::SUCCESS
}

/// `--method bayes-net [--structure chow-tree|k-parent] [--parents n] [--bins n]` or
/// `--method marginals [--bins n]` (independent marginals) for the non-private
/// synthesizer; `None` keeps the Gaussian copula.
fn bayes_config_from_flags(
    args: &[String],
    base: &triplex_anonymizer::synthesize::SynthesizeConfig,
) -> std::result::Result<Option<triplex_anonymizer::synthesize::bayesnet::BayesConfig>, ExitCode> {
    use triplex_anonymizer::synthesize::bayesnet::{BayesConfig, StructureKind};
    let bad = |message: String| {
        eprintln!("error: {message}");
        ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
    };
    let method = flag_value(args, "--method");
    match method.as_deref() {
        None | Some("copula") => {
            for flag in ["--structure", "--parents", "--bins"] {
                if flag_value(args, flag).is_some() {
                    return Err(bad(format!("{flag} only applies to --method bayes-net")));
                }
            }
            return Ok(None);
        }
        Some("bayes-net") => {}
        Some("marginals") => {
            for flag in ["--structure", "--parents"] {
                if flag_value(args, flag).is_some() {
                    return Err(bad(format!("{flag} only applies to --method bayes-net")));
                }
            }
        }
        // The sketch method (Tier 3) is handled by `sketch_from_flags`; `--bins` is its too.
        Some("sketch") => {
            for flag in ["--structure", "--parents"] {
                if flag_value(args, flag).is_some() {
                    return Err(bad(format!("{flag} only applies to --method bayes-net")));
                }
            }
            return Ok(None);
        }
        Some(other) => {
            return Err(bad(format!(
                "--method expects copula, bayes-net, marginals or sketch, got {other:?}"
            )))
        }
    }
    let mut config = BayesConfig {
        base: base.clone(),
        ..BayesConfig::default()
    };
    config.structure = match flag_value(args, "--structure").as_deref() {
        None if method.as_deref() == Some("marginals") => StructureKind::Independent,
        None | Some("chow-liu") => StructureKind::ChowLiu,
        Some("k-parent") => StructureKind::KParent,
        Some(other) => {
            return Err(bad(format!(
                "--structure expects chow-liu or k-parent, got {other:?}"
            )))
        }
    };
    for (flag, target) in [
        ("--parents", &mut config.max_parents),
        ("--bins", &mut config.bins),
    ] {
        if let Some(v) = flag_value(args, flag) {
            *target = v
                .parse::<usize>()
                .map_err(|_| bad(format!("{flag} expects a whole number, got {v:?}")))?;
        }
    }
    Ok(Some(config))
}

/// The sketch to generate from (Tier 3), or `None` for the other methods:
/// `--sketch <path.triplex.yaml>` reads one, `--method sketch [--classes n] [--min-class-size n]
/// [--bins n]` proposes one from the input.
fn sketch_from_flags(
    args: &[String],
    input: &std::path::Path,
    base: &triplex_anonymizer::synthesize::SynthesizeConfig,
) -> std::result::Result<Option<triplex_anonymizer::synthesize::sketch::Sketch>, ExitCode> {
    use triplex_anonymizer::synthesize::sketch::{propose_file, sketch_from_yaml, SketchConfig};
    let bad = |message: String| {
        eprintln!("error: {message}");
        ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
    };
    let method = flag_value(args, "--method");
    let proposing = method.as_deref() == Some("sketch");
    if let Some(path) = flag_value(args, "--sketch") {
        if method.is_some() && !proposing {
            return Err(bad("--sketch only goes with --method sketch".into()));
        }
        for flag in ["--classes", "--min-class-size", "--bins"] {
            if flag_value(args, flag).is_some() {
                return Err(bad(format!(
                    "{flag} shapes a proposed sketch; it does not apply to a sketch read with --sketch"
                )));
            }
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| bad(format!("could not read --sketch {path:?}: {e}")))?;
        return sketch_from_yaml(&text)
            .map(Some)
            .map_err(|e| bad(e.to_string()));
    }
    if !proposing {
        for flag in ["--classes", "--min-class-size"] {
            if flag_value(args, flag).is_some() {
                return Err(bad(format!("{flag} only applies to --method sketch")));
            }
        }
        return Ok(None);
    }
    let mut config = SketchConfig {
        base: base.clone(),
        ..SketchConfig::default()
    };
    for (flag, target) in [
        ("--classes", &mut config.classes),
        ("--min-class-size", &mut config.min_class_size),
        ("--bins", &mut config.bins),
    ] {
        if let Some(v) = flag_value(args, flag) {
            *target = v
                .parse::<usize>()
                .map_err(|_| bad(format!("{flag} expects a whole number, got {v:?}")))?;
        }
    }
    propose_file(input, &config)
        .map(Some)
        .map_err(|e| bad(e.to_string()))
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            println!("triplex {}", env!("TRIPLEX_VERSION"));
            ExitCode::SUCCESS
        }
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

            let profile_out = flag_value(&args, "--profile-out").map(PathBuf::from);

            match triplex_anonymizer::anonymize_file(
                &PathBuf::from(input),
                &recipe,
                &PathBuf::from(output),
                &PathBuf::from(mapping_out),
                passphrase.as_deref(),
                &excluded_rows,
                profile_out.as_deref(),
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
        Some("synthesize") => {
            let (Some(input), Some(output), Some(report_dir)) = (
                flag_value(&args, "--input"),
                flag_value(&args, "--output"),
                flag_value(&args, "--report-dir"),
            ) else {
                return usage_error();
            };
            let parse_u64 = |flag: &str| -> std::result::Result<Option<u64>, ExitCode> {
                match flag_value(&args, flag) {
                    Some(v) => v.parse::<u64>().map(Some).map_err(|_| {
                        eprintln!("error: {flag} expects a whole number, got {v:?}");
                        ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR)
                    }),
                    None => Ok(None),
                }
            };
            let mut config = triplex_anonymizer::synthesize::SynthesizeConfig::default();
            match parse_u64("--rows") {
                Ok(v) => config.rows = v.map(|v| v as usize),
                Err(code) => return code,
            }
            match parse_u64("--seed") {
                Ok(v) => config.seed = v.unwrap_or(config.seed),
                Err(code) => return code,
            }
            match parse_u64("--min-category-count") {
                Ok(v) => {
                    config.min_category_count = v.map_or(config.min_category_count, |v| v as usize)
                }
                Err(code) => return code,
            }
            let (input, output, report_dir) = (
                PathBuf::from(input),
                PathBuf::from(output),
                PathBuf::from(report_dir),
            );
            let bayes_config = match bayes_config_from_flags(&args, &config) {
                Ok(c) => c,
                Err(code) => return code,
            };
            if let Some(config_path) = flag_value(&args, "--config") {
                return run_synthesize_dp(
                    &args,
                    &PathBuf::from(config_path),
                    &input,
                    &output,
                    &report_dir,
                    config.rows,
                    config.seed,
                );
            }
            if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("error: could not create {parent:?}: {e}");
                    return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                }
            }
            let sketch = match sketch_from_flags(&args, &input, &config) {
                Ok(s) => s,
                Err(code) => return code,
            };
            let result = match (&sketch, &bayes_config) {
                (Some(sketch), _) => {
                    triplex_anonymizer::synthesize::sketch::synthesize_sketch_file(
                        &input, &output, sketch, &config,
                    )
                }
                (None, Some(bayes)) => {
                    triplex_anonymizer::synthesize::bayesnet::synthesize_bayes_file(
                        &input, &output, bayes,
                    )
                }
                (None, None) => {
                    triplex_anonymizer::synthesize::synthesize_file(&input, &output, &config)
                }
            };
            let report = match result {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                }
            };
            // DriftGuard diff of the synthetic file against the original, restricted to the
            // columns that were synthesized (dropped identifiers would read as removed columns).
            let infer =
                |path: &PathBuf| triplex_schemacast::infer_schema_from_path(path, SAMPLE_ROWS);
            let (mut baseline, synthetic) = match (infer(&input), infer(&output)) {
                (Ok(b), Ok(s)) => (b, s),
                (Err(e), _) | (_, Err(e)) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                }
            };
            baseline
                .columns
                .retain(|c| report.columns_synthesized.contains(&c.name));
            let drift = triplex_driftguard::compare_contracts(
                &baseline,
                &synthetic,
                &triplex_driftguard::DriftThresholds::default(),
            );
            if let Err(e) = std::fs::create_dir_all(&report_dir) {
                eprintln!("error: could not create --report-dir {report_dir:?}: {e}");
                return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
            }
            let report_json = serde_json::to_string_pretty(&report).unwrap();
            let mut files = vec![
                ("synthesis_report.json", report_json.clone()),
                (
                    "report.html",
                    triplex_anonymizer::synthesize::render_html(&report),
                ),
                (
                    "drift_check.json",
                    serde_json::to_string_pretty(&drift).unwrap(),
                ),
            ];
            // The sketch that was used, so the run can be repeated or the sketch edited.
            if let Some(sketch) = &sketch {
                match triplex_anonymizer::synthesize::sketch::sketch_to_yaml(sketch) {
                    Ok(yaml) => files.push(("sketch.triplex.yaml", yaml)),
                    Err(e) => {
                        eprintln!("error: {e}");
                        return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                    }
                }
            }
            // reproduce.py re-derives the evaluation from the two files, whichever model wrote
            // the synthetic one.
            files.push((
                "reproduce.py",
                triplex_anonymizer::synthesize::render_python_script(
                    &input.to_string_lossy(),
                    &output.to_string_lossy(),
                    config.min_category_count,
                ),
            ));
            for (name, contents) in files {
                if let Err(e) = std::fs::write(report_dir.join(name), contents) {
                    eprintln!("error: could not write {name} into {report_dir:?}: {e}");
                    return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                }
            }
            eprintln!("note: {}", report.privacy_statement);
            eprintln!(
                "drift check (synthetic vs original, synthesized columns): {} finding(s), {}",
                drift.findings.len(),
                if drift.has_regression {
                    "regression"
                } else {
                    "no regression"
                }
            );
            println!("{report_json}");
            ExitCode::SUCCESS
        }
        Some("modelcard") => {
            let (Some(config_path), Some(input), Some(out_dir)) = (
                flag_value(&args, "--config"),
                flag_value(&args, "--input"),
                flag_value(&args, "--out-dir"),
            ) else {
                return usage_error();
            };
            let contract = match triplex_schemacast::load_contract(&PathBuf::from(&config_path)) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                }
            };
            let config = match triplex_modelcard::config_from_contract(&contract) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                }
            };
            let report = match triplex_modelcard::evaluate_path(&PathBuf::from(&input), config) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                }
            };
            let out_dir = PathBuf::from(&out_dir);
            if let Err(e) = std::fs::create_dir_all(&out_dir) {
                eprintln!("error: could not create --out-dir {out_dir:?}: {e}");
                return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
            }
            let card_json = serde_json::to_string_pretty(&report).unwrap();
            for (name, contents) in [
                ("model_card.json", card_json.clone()),
                ("report.html", triplex_modelcard::render_html(&report)),
                (
                    "reproduce.py",
                    triplex_modelcard::render_python_script(&input, config),
                ),
            ] {
                if let Err(e) = std::fs::write(out_dir.join(name), contents) {
                    eprintln!("error: could not write {name} into {out_dir:?}: {e}");
                    return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                }
            }
            println!("{card_json}");
            ExitCode::from(report.status.exit_code())
        }
        Some("driftguard") => {
            let Some(out_dir) = flag_value(&args, "--out-dir") else {
                return usage_error();
            };

            // Config-file thresholds (a `.triplex.yaml` contract's optional
            // `driftguard:` block) are the middle layer between this crate's
            // built-in defaults and an explicit `--max-*` flag, which always
            // wins when given.
            let declared_thresholds = match flag_value(&args, "--config") {
                Some(path) => match triplex_schemacast::load_contract(&PathBuf::from(path)) {
                    Ok(contract) => contract.driftguard,
                    Err(e) => {
                        eprintln!("error: {e}");
                        return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                    }
                },
                None => None,
            };
            let config_value =
                |field: fn(&triplex_schemacast::DeclaredDriftGuardThresholds) -> Option<f64>| {
                    declared_thresholds.as_ref().and_then(field)
                };

            let max_null_percentage_delta =
                match optional_f64_flag_no_default(&args, "--max-null-percentage-delta") {
                    Ok(v) => v.or_else(|| config_value(|d| d.max_null_percentage_delta)),
                    Err(code) => return code,
                };
            let max_mean_relative_delta =
                match optional_f64_flag_no_default(&args, "--max-mean-relative-delta") {
                    Ok(v) => v.or_else(|| config_value(|d| d.max_mean_relative_delta)),
                    Err(code) => return code,
                };
            let max_type_consistency_delta =
                match optional_f64_flag_no_default(&args, "--max-type-consistency-delta") {
                    Ok(v) => v.or_else(|| config_value(|d| d.max_type_consistency_delta)),
                    Err(code) => return code,
                };
            let thresholds = triplex_driftguard::DriftThresholds::with_overrides(
                max_null_percentage_delta,
                max_mean_relative_delta,
                max_type_consistency_delta,
            );

            let baseline_label = flag_value(&args, "--baseline")
                .or_else(|| flag_value(&args, "--baseline-profile"))
                .unwrap_or_default();
            let candidate_label = flag_value(&args, "--candidate")
                .or_else(|| flag_value(&args, "--candidate-profile"))
                .unwrap_or_default();

            let (baseline_schema, baseline_risk) = match load_driftguard_side(
                "baseline",
                flag_value(&args, "--baseline").as_deref(),
                flag_value(&args, "--baseline-profile").as_deref(),
                SAMPLE_ROWS,
            ) {
                Ok(v) => v,
                Err(code) => return code,
            };
            let (candidate_schema, candidate_risk) = match load_driftguard_side(
                "candidate",
                flag_value(&args, "--candidate").as_deref(),
                flag_value(&args, "--candidate-profile").as_deref(),
                SAMPLE_ROWS,
            ) {
                Ok(v) => v,
                Err(code) => return code,
            };

            let report = triplex_driftguard::compare_with_risk(
                &baseline_schema,
                baseline_risk.as_ref(),
                &candidate_schema,
                candidate_risk.as_ref(),
                &thresholds,
            );

            let out_dir = PathBuf::from(&out_dir);
            if let Err(e) = std::fs::create_dir_all(&out_dir) {
                eprintln!("error: could not create --out-dir {out_dir:?}: {e}");
                return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
            }

            let audit_json = serde_json::to_string_pretty(&report).unwrap();
            let html = triplex_driftguard::render_html(&report, &baseline_label, &candidate_label);
            // `reproduce.py` re-derives stats from the raw files themselves
            // (see its module doc) -- meaningless when a side came from a
            // `--*-profile` JSON instead of a data file, since there is no
            // raw file for Polars to read.
            let used_a_profile = flag_value(&args, "--baseline-profile").is_some()
                || flag_value(&args, "--candidate-profile").is_some();
            let script = if used_a_profile {
                "# reproduce.py is not generated for a --baseline-profile/--candidate-profile \
                 run: there is no raw data file left to independently re-derive stats from. \
                 See audit.json/report.html for the full finding set.\n"
                    .to_string()
            } else {
                triplex_driftguard::render_python_script(
                    &baseline_label,
                    &candidate_label,
                    &thresholds,
                )
            };

            for (name, contents) in [
                ("audit.json", audit_json.as_str()),
                ("report.html", html.as_str()),
                ("reproduce.py", script.as_str()),
            ] {
                if let Err(e) = std::fs::write(out_dir.join(name), contents) {
                    eprintln!("error: could not write {name} into {out_dir:?}: {e}");
                    return ExitCode::from(EXIT_RUNTIME_CONFIG_ERROR);
                }
            }

            println!("{audit_json}");
            if report.has_regression {
                ExitCode::from(EXIT_CONTRACT_BREACH)
            } else if report.has_drift {
                ExitCode::from(EXIT_NON_BLOCKING_WARNING)
            } else {
                ExitCode::SUCCESS
            }
        }
        _ => usage_error(),
    }
}
