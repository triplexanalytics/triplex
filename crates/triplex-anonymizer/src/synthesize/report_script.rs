//! `reproduce.py`: re-derives the fidelity metrics and the exact-copy count from the real and the
//! synthetic file with Polars, independently of this crate. It checks the evaluation, not the
//! generation (the generator's PRNG is this crate's own), and it does not recompute the
//! nearest-record distances.

fn py(s: &str) -> String {
    format!("{s:?}")
}

pub fn render_python_script(
    real_path: &str,
    synthetic_path: &str,
    min_category_count: usize,
) -> String {
    format!(
        r#"#!/usr/bin/env python3
"""Independently re-derives Triplex synthesis fidelity metrics with Polars and prints them as JSON.

Covers per-column KS / total variation / null rate, numeric correlation error and exact copies. It
does not regenerate the data and does not recompute nearest-record distances.
"""

import json
from collections import Counter

import polars as pl

REAL_PATH = {real}
SYNTHETIC_PATH = {synthetic}
MIN_CATEGORY_COUNT = {min_count}


def read(path):
    if path.endswith(".parquet"):
        return pl.read_parquet(path).cast(pl.Utf8)
    return pl.read_csv(path, infer_schema_length=0)


def cells(df, column):
    return [None if v is None or str(v).strip() == "" else str(v).strip() for v in df[column].to_list()]


def as_number(v):
    return None if v is None else float(v)


def ks(a, b):
    a, b = sorted(a), sorted(b)
    if not a or not b:
        return None
    i = j = 0
    worst = 0.0
    while i < len(a) and j < len(b):
        x = min(a[i], b[j])
        while i < len(a) and a[i] <= x:
            i += 1
        while j < len(b) and b[j] <= x:
            j += 1
        worst = max(worst, abs(i / len(a) - j / len(b)))
    return worst


def distribution(values):
    counts = Counter(v for v in values if v is not None)
    total = sum(counts.values())
    return {{k: c / total for k, c in counts.items()}}


def tvd(a, b):
    da, db = distribution(a), distribution(b)
    if not da or not db:
        return None
    return sum(abs(da.get(k, 0) - db.get(k, 0)) for k in set(da) | set(db)) / 2


def pearson(a, b):
    pairs = [(x, y) for x, y in zip(a, b) if x is not None and y is not None]
    if len(pairs) < 3:
        return None
    n = len(pairs)
    ma = sum(p[0] for p in pairs) / n
    mb = sum(p[1] for p in pairs) / n
    sab = sum((x - ma) * (y - mb) for x, y in pairs)
    saa = sum((x - ma) ** 2 for x, _ in pairs)
    sbb = sum((y - mb) ** 2 for _, y in pairs)
    if saa <= 1e-12 or sbb <= 1e-12:
        return None
    return sab / (saa ** 0.5 * sbb ** 0.5)


def is_numeric(values):
    present = [v for v in values if v is not None]
    if not present:
        return False
    try:
        [float(v) for v in present]
    except ValueError:
        return False
    return True


def main():
    real, synthetic = read(REAL_PATH), read(SYNTHETIC_PATH)
    columns = [c for c in synthetic.columns if c in real.columns]
    real_cells = {{c: cells(real, c) for c in columns}}
    synth_cells = {{c: cells(synthetic, c) for c in columns}}
    numeric = {{c: is_numeric(real_cells[c]) for c in columns}}
    out = {{"columns": {{}}}}
    real_numbers, synth_numbers = {{}}, {{}}
    for c in columns:
        r, s = real_cells[c], synth_cells[c]
        entry = {{"null_rate_real": sum(v is None for v in r) / len(r), "null_rate_synthetic": sum(v is None for v in s) / len(s)}}
        if numeric[c]:
            rn, sn = [as_number(v) for v in r], [as_number(v) for v in s]
            real_numbers[c], synth_numbers[c] = rn, sn
            entry["ks_statistic"] = ks([v for v in rn if v is not None], [v for v in sn if v is not None])
        else:
            counts = Counter(v for v in r if v is not None)
            merged = [None if v is None else ("(other)" if counts[v] < MIN_CATEGORY_COUNT else v) for v in r]
            entry["total_variation"] = tvd(merged, s)
        out["columns"][c] = entry
    numeric_columns = [c for c in columns if numeric[c]]
    errors = []
    for i, a in enumerate(numeric_columns):
        for b in numeric_columns[:i]:
            pr, ps = pearson(real_numbers[a], real_numbers[b]), pearson(synth_numbers[a], synth_numbers[b])
            if pr is not None and ps is not None:
                errors.append(abs(pr - ps))
    out["correlation"] = {{
        "pairs_compared": len(errors),
        "max_abs_error": max(errors) if errors else None,
        "mean_abs_error": sum(errors) / len(errors) if errors else None,
    }}
    real_rows = set(
        tuple(as_number(real_cells[c][i]) if numeric[c] else real_cells[c][i] for c in columns)
        for i in range(len(real_cells[columns[0]]))
    )
    copies = sum(
        tuple(as_number(synth_cells[c][i]) if numeric[c] else synth_cells[c][i] for c in columns) in real_rows
        for i in range(len(synth_cells[columns[0]]))
    )
    out["exact_copy_rows"] = copies
    print(json.dumps(out, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
"#,
        real = py(real_path),
        synthetic = py(synthetic_path),
        min_count = min_category_count,
    )
}
