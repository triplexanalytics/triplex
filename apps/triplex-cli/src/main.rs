//! Thin CLI surface. All logic lives in triplex-schemacast; this binary only
//! parses argv and prints the resulting contract.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

const SAMPLE_ROWS: usize = 1000;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some("schema"), Some(path)) = (args.next().as_deref(), args.next()) else {
        eprintln!("usage: triplex schema <path-to.csv>");
        return ExitCode::FAILURE;
    };

    match triplex_schemacast::infer_schema_from_path(&PathBuf::from(path), SAMPLE_ROWS) {
        Ok(contract) => {
            println!("{}", serde_json::to_string_pretty(&contract).unwrap());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
