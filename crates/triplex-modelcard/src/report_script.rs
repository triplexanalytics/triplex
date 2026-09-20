//! `reproduce.py`: a standalone Polars script that re-derives every per-slice metric and every
//! fairness metric from the evaluation file, independently of this crate, and prints them as
//! JSON in the same shape as `model_card.json` (metrics only). Unlike DriftGuard's script, it
//! covers the whole card, so a reviewer can audit any number in it.

use triplex_schemacast::DeclaredModelCard;

use crate::DEFAULT_MIN_SLICE_SIZE;

fn py_str(s: &str) -> String {
    format!("{s:?}")
}

fn py_list(items: &[String]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|s| py_str(s))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub fn render_python_script(input_path: &str, config: &DeclaredModelCard) -> String {
    let mut slice_columns: Vec<String> = Vec::new();
    for name in config
        .slice_by
        .iter()
        .chain(config.protected_attribute.iter())
    {
        if !slice_columns.contains(name) {
            slice_columns.push(name.clone());
        }
    }
    let score = config
        .score_column
        .as_deref()
        .map_or("None".to_string(), py_str);
    let protected = config
        .protected_attribute
        .as_deref()
        .map_or("None".to_string(), py_str);
    format!(
        r#"#!/usr/bin/env python3
"""Independently re-derives Triplex ModelCard's metrics with Polars only and prints them as JSON.

Same definitions as the Rust engine: undefined ratios are null, ratios are min/max across groups
(1.0 when the maximum is 0), differences are max-min, equalized odds is the larger rate gap and
the smaller rate ratio. Slices under MIN_SLICE_SIZE are suppressed and left out of fairness.
"""

import json

import polars as pl

INPUT_PATH = {input_path}
TARGET = {target}
PREDICTION = {prediction}
SCORE = {score}
POSITIVE = {positive}.strip().lower()
SLICE_COLUMNS = {slice_columns}
PROTECTED = {protected}
MIN_SLICE_SIZE = {min_slice}


def read(path):
    if path.endswith(".parquet"):
        return pl.read_parquet(path)
    if path.endswith(".tsv"):
        return pl.read_csv(path, separator="\t")
    if path.endswith(".jsonl"):
        return pl.read_ndjson(path)
    return pl.read_csv(path, infer_schema_length=0)


def norm(column):
    return pl.col(column).cast(pl.Utf8).str.strip_chars().str.to_lowercase()


def div(a, b):
    return a / b if b else None


def auc(df):
    if SCORE is None:
        return None
    positives = int(df["y"].sum())
    negatives = df.height - positives
    if positives == 0 or negatives == 0:
        return None
    ranked = df.with_columns(pl.col("s").rank("average").alias("r"))
    rank_sum = ranked.filter(pl.col("y"))["r"].sum()
    return (rank_sum - positives * (positives + 1) / 2) / (positives * negatives)


def metrics(df):
    tp = int((df["y"] & df["p"]).sum())
    fp = int((~df["y"] & df["p"]).sum())
    tn = int((~df["y"] & ~df["p"]).sum())
    fn = int((df["y"] & ~df["p"]).sum())
    n = df.height
    return {{
        "selection_rate": (tp + fp) / n,
        "base_rate": (tp + fn) / n,
        "accuracy": (tp + tn) / n,
        "precision": div(tp, tp + fp),
        "recall": div(tp, tp + fn),
        "fpr": div(fp, fp + tn),
        "f1": div(2 * tp, 2 * tp + fp + fn),
        "roc_auc": auc(df),
    }}


def spread(values):
    return (min(values), max(values)) if len(values) >= 2 else None


def diff(values):
    s = spread(values)
    return None if s is None else s[1] - s[0]


def ratio(values):
    s = spread(values)
    return None if s is None else (1.0 if s[1] == 0 else s[0] / s[1])


def pick(a, b, fn):
    present = [x for x in (a, b) if x is not None]
    return fn(present) if present else None


def main():
    raw = read(INPUT_PATH)
    df = raw.with_columns(
        norm(TARGET).alias("_t"), norm(PREDICTION).alias("_p")
    ).filter(
        pl.col("_t").is_not_null() & (pl.col("_t") != "") & pl.col("_p").is_not_null() & (pl.col("_p") != "")
    )
    df = df.with_columns(
        (pl.col("_t") == POSITIVE).alias("y"), (pl.col("_p") == POSITIVE).alias("p")
    )
    if SCORE is not None:
        df = df.with_columns(pl.col(SCORE).cast(pl.Float64).alias("s"))
    out = {{"overall": metrics(df), "slices": {{}}}}
    kept = {{}}
    for column in SLICE_COLUMNS:
        groups = {{}}
        keyed = df.with_columns(
            pl.col(column).cast(pl.Utf8).fill_null("").alias("_g")
        ).with_columns(
            pl.when(pl.col("_g").str.strip_chars() == "").then(pl.lit("(null)")).otherwise(pl.col("_g")).alias("_g")
        )
        for (name,), part in keyed.group_by("_g", maintain_order=False):
            groups[name] = {{"n": part.height, "suppressed": True}} if part.height < MIN_SLICE_SIZE else {{"n": part.height, "suppressed": False, **metrics(part)}}
        out["slices"][column] = dict(sorted(groups.items()))
        if column == PROTECTED:
            kept = {{k: v for k, v in groups.items() if not v["suppressed"]}}
    if PROTECTED is not None:
        sel = [g["selection_rate"] for g in kept.values()]
        tpr = [g["recall"] for g in kept.values() if g["recall"] is not None]
        fpr = [g["fpr"] for g in kept.values() if g["fpr"] is not None]
        out["fairness"] = {{
            "demographic_parity_difference": diff(sel),
            "disparate_impact_ratio": ratio(sel),
            "tpr_difference": diff(tpr),
            "tpr_ratio": ratio(tpr),
            "fpr_difference": diff(fpr),
            "fpr_ratio": ratio(fpr),
            "equalized_odds_difference": pick(diff(tpr), diff(fpr), max),
            "equalized_odds_ratio": pick(ratio(tpr), ratio(fpr), min),
        }}
    print(json.dumps(out, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
"#,
        input_path = py_str(input_path),
        target = py_str(&config.target_column),
        prediction = py_str(&config.prediction_column),
        score = score,
        positive = py_str(config.positive_label.as_deref().unwrap_or("1")),
        slice_columns = py_list(&slice_columns),
        protected = protected,
        min_slice = config.min_slice_size.unwrap_or(DEFAULT_MIN_SLICE_SIZE),
    )
}
