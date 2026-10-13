//! The native half of `import triplex`: thin PyO3 wrappers over the same
//! engine functions `apps/triplex-cli` calls, with no logic of their own.
//! A table arrives as a local path or as Arrow IPC stream bytes and leaves
//! as Arrow IPC stream bytes; a report leaves as JSON text. The Python side
//! (`python/triplex/__init__.py`) turns those into DataFrames and dicts.
//! No network calls.

use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use triplex_anonymizer::synthesize::bayesnet::{synthesize_bayes, BayesConfig, StructureKind};
use triplex_anonymizer::synthesize::{synthesize as synthesize_copula, SynthesizeConfig};
use triplex_core::Error;
use triplex_io::Sample;

pyo3::create_exception!(_native, TriplexError, pyo3::exceptions::PyException);

/// An engine error as `triplex.TriplexError`, its stable code on `.code`.
fn py_err(py: Python<'_>, e: Error) -> PyErr {
    let err = TriplexError::new_err(e.to_string());
    let _ = err.value(py).setattr("code", e.code());
    err
}

fn invalid(message: String) -> Error {
    Error::Validation(message)
}

fn json<T: serde::Serialize>(value: &T) -> Result<String, Error> {
    serde_json::to_string(value).map_err(|e| invalid(format!("could not serialize report: {e}")))
}

fn delimiter_byte(delimiter: Option<char>) -> Result<Option<u8>, Error> {
    match delimiter {
        Some(c) if c.is_ascii() => Ok(Some(c as u8)),
        Some(c) => Err(invalid(format!(
            "delimiter must be one ASCII character, got {c:?}"
        ))),
        None => Ok(None),
    }
}

/// Every row of the one table the caller passed: Arrow IPC bytes, or a
/// local `.csv`/`.parquet` path.
fn read_rows(path: Option<&Path>, ipc: Option<&[u8]>) -> Result<Sample, Error> {
    match (path, ipc) {
        (Some(path), None) => triplex_anonymizer::read_rows(path),
        (None, Some(bytes)) => triplex_io::read_arrow_ipc_full_bytes(bytes),
        _ => Err(invalid("pass exactly one of a path or a table".into())),
    }
}

fn infer(
    path: Option<&Path>,
    ipc: Option<&[u8]>,
    sample_rows: usize,
    delimiter: Option<char>,
) -> Result<triplex_schemacast::SchemaContract, Error> {
    match (path, delimiter_byte(delimiter)?) {
        (Some(path), Some(d)) => {
            triplex_schemacast::infer_schema_from_csv_with_delimiter(path, d, sample_rows)
        }
        (Some(path), None) if ipc.is_none() => {
            triplex_schemacast::infer_schema_from_path(path, sample_rows)
        }
        _ => Ok(triplex_schemacast::infer_schema(&read_rows(path, ipc)?)),
    }
}

/// The result table as Arrow IPC bytes, also written to `output` as CSV if given.
fn table_out<'py>(
    py: Python<'py>,
    sample: &Sample,
    output: Option<&Path>,
) -> Result<Bound<'py, PyBytes>, Error> {
    if let Some(output) = output {
        triplex_io::write_csv(output, &sample.headers, &sample.rows)?;
    }
    let ipc = triplex_schemacast::sample_to_arrow_ipc(sample)?;
    Ok(PyBytes::new(py, &ipc))
}

#[pyfunction]
#[pyo3(signature = (path, ipc, sample_rows, delimiter))]
fn infer_schema(
    py: Python<'_>,
    path: Option<PathBuf>,
    ipc: Option<&[u8]>,
    sample_rows: usize,
    delimiter: Option<char>,
) -> PyResult<String> {
    infer(path.as_deref(), ipc, sample_rows, delimiter)
        .and_then(|schema| json(&schema))
        .map_err(|e| py_err(py, e))
}

#[pyfunction]
#[pyo3(signature = (contract, path, ipc, sample_rows, delimiter))]
fn validate(
    py: Python<'_>,
    contract: PathBuf,
    path: Option<PathBuf>,
    ipc: Option<&[u8]>,
    sample_rows: usize,
    delimiter: Option<char>,
) -> PyResult<String> {
    triplex_schemacast::load_contract(&contract)
        .and_then(|declared| {
            let inferred = infer(path.as_deref(), ipc, sample_rows, delimiter)?;
            json(&triplex_schemacast::validate(&declared, &inferred))
        })
        .map_err(|e| py_err(py, e))
}

#[pyfunction]
#[pyo3(signature = (input, output, input_delimiter, output_delimiter))]
fn convert(
    py: Python<'_>,
    input: PathBuf,
    output: PathBuf,
    input_delimiter: Option<char>,
    output_delimiter: Option<char>,
) -> PyResult<String> {
    delimiter_byte(input_delimiter)
        .and_then(|i| Ok((i, delimiter_byte(output_delimiter)?)))
        .and_then(|(i, o)| triplex_schemacast::convert_file_with_delimiters(&input, &output, i, o))
        .and_then(|report| json(&report))
        .map_err(|e| py_err(py, e))
}

/// Salts come from the environment variables the recipe names and the
/// mapping goes to `mapping_out` on disk, exactly as in `triplex anonymize`:
/// neither crosses into Python.
#[pyfunction]
#[pyo3(signature = (recipe, mapping_out, path, ipc, output, mapping_passphrase_env, exclude_rows))]
#[allow(clippy::too_many_arguments)]
fn anonymize<'py>(
    py: Python<'py>,
    recipe: PathBuf,
    mapping_out: PathBuf,
    path: Option<PathBuf>,
    ipc: Option<&[u8]>,
    output: Option<PathBuf>,
    mapping_passphrase_env: Option<String>,
    exclude_rows: HashSet<usize>,
) -> PyResult<(Bound<'py, PyBytes>, String)> {
    let run = || {
        let passphrase = mapping_passphrase_env
            .map(|var| std::env::var(&var).map_err(|_| Error::MissingEnvVar(var)))
            .transpose()?;
        let recipe = triplex_anonymizer::load_recipe(&recipe)?;
        let (masked, report) = triplex_anonymizer::anonymize_sample(
            read_rows(path.as_deref(), ipc)?,
            &recipe,
            &mapping_out,
            passphrase.as_deref(),
            &exclude_rows,
        )?;
        Ok((table_out(py, &masked, output.as_deref())?, json(&report)?))
    };
    run().map_err(|e| py_err(py, e))
}

/// The three synthesizers that carry no formal privacy guarantee. The
/// differentially private ones and the sketch need a contract or a sketch
/// file and are not exposed yet.
#[pyfunction]
#[pyo3(signature = (path, ipc, output, rows, seed, min_category_count, method))]
#[allow(clippy::too_many_arguments)]
fn synthesize<'py>(
    py: Python<'py>,
    path: Option<PathBuf>,
    ipc: Option<&[u8]>,
    output: Option<PathBuf>,
    rows: Option<usize>,
    seed: u64,
    min_category_count: usize,
    method: &str,
) -> PyResult<(Bound<'py, PyBytes>, String)> {
    let run = || {
        let base = SynthesizeConfig {
            rows,
            seed,
            min_category_count,
            ..SynthesizeConfig::default()
        };
        let bayes = |structure| BayesConfig {
            base: base.clone(),
            structure,
            ..BayesConfig::default()
        };
        let sample = read_rows(path.as_deref(), ipc)?;
        let result = match method {
            "copula" => synthesize_copula(&sample, &base),
            "bayes-net" => synthesize_bayes(&sample, &bayes(StructureKind::ChowLiu)),
            "marginals" => synthesize_bayes(&sample, &bayes(StructureKind::Independent)),
            other => Err(invalid(format!(
                "method expects copula, bayes-net or marginals, got {other:?}"
            ))),
        }?;
        Ok((
            table_out(py, &result.sample, output.as_deref())?,
            json(&result.report)?,
        ))
    };
    run().map_err(|e| py_err(py, e))
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("TriplexError", m.py().get_type::<TriplexError>())?;
    m.add_function(wrap_pyfunction!(infer_schema, m)?)?;
    m.add_function(wrap_pyfunction!(validate, m)?)?;
    m.add_function(wrap_pyfunction!(convert, m)?)?;
    m.add_function(wrap_pyfunction!(anonymize, m)?)?;
    m.add_function(wrap_pyfunction!(synthesize, m)?)?;
    Ok(())
}
