# triplex-python

Triplex Analytics in plain Python: infer and check schema contracts, anonymize a table, generate
synthetic data. No notebook, no browser, no server. The engine is the same native code the
`triplex` command line runs.

```sh
pip install triplexanalytics
```

The distribution is called `triplexanalytics`; the import is `import triplex`. Wheels cover Linux
(x86_64 and aarch64), macOS and Windows, for Python 3.9 and later.

To build from a source checkout instead (needs the Rust toolchain):

```sh
cd packages/triplex-python
pip install maturin
maturin develop --release
```

## Use

```python
import pandas as pd
import triplex

# 1. Check the raw export against the contract you keep in version control.
raw = pd.read_csv("customers.csv")
report = triplex.validate(raw, "customers.triplex.yaml")
if not report["valid"]:
    raise SystemExit(report["errors"])

# 2. Your own pandas work: clean, filter, derive, drop.
df = raw.dropna(subset=["email"])
df = df[df["plan"] != "free"]
df["age_band"] = (df["age"] // 10) * 10
df = df.drop(columns=["signup_date"])

# 3. See what the table holds now: types, null rates, likely identifiers.
schema = triplex.infer_schema(df)

# 4. Generate a synthetic copy for tests or a demo.
run = triplex.synthesize(df, rows=1000, seed=1)
synthetic = run.table.to_pandas()
run.report["leakage"]
```

| Function | What it does |
|---|---|
| `infer_schema(data)` | The inferred schema as a dict |
| `draft_contract(data, output=...)` | A starting contract: every column with its type, all required. Returns a dict; `output` also writes it as `.triplex.yaml` |
| `validate(data, contract)` | Checks `data` against a contract (a `.triplex.yaml` path or a dict); returns `{"valid": ..., "errors": [...]}` |
| `convert(input, output)` | Converts a file between `.csv`, `.tsv`, `.jsonl` and `.parquet`, by extension |
| `anonymize(data, recipe, mapping_out=...)` | Masks the columns a `.triplex.yaml` recipe declares |
| `synthesize(data, rows=..., seed=..., method=...)` | Generates a synthetic table; `method` is `"copula"` (default), `"bayes-net"` or `"marginals"` |

A contract is what you expect the data to be; an inferred schema is what one table holds right
now. `draft_contract` gives you a first contract to edit (add null-rate, range, mean or pattern
limits) and commit. Passing the dict from `infer_schema` to `validate` is an error.

`data` is a file path, a pandas or Polars DataFrame, a pyarrow Table, or anything else Arrow can
read. `anonymize` and `synthesize` read `.csv` and `.parquet` paths; `infer_schema` and
`validate` also read `.tsv` and `.jsonl`.

`anonymize` and `synthesize` return a result with two fields: `report` (a dict) and `table`.
Read the table with `table.to_pandas()`, `table.to_polars()` or `table.to_arrow()`, or pass
`output="result.csv"` to have it written as CSV as well.

An engine error is raised as `triplex.TriplexError`, with a stable `code` such as
`"COLUMN_NOT_FOUND"`.

pandas, Polars and pyarrow are optional: install the ones you use. pandas input and
`to_pandas()` need pyarrow; Polars works without it.

## Anonymizing

```python
import os
os.environ["CUSTOMER_SALT"] = "..."   # better: set it outside the script

run = triplex.anonymize(df, "recipe.triplex.yaml", mapping_out="mapping.csv")
masked = run.table.to_pandas()
```

- **The salt** is read from the environment variable each recipe column names (`salt_env`). It
  is never a function argument.
- **The mapping file** reverses the pseudonyms. It is written to the path you give in
  `mapping_out` and is not returned to Python. Pass `mapping_passphrase_env="SOME_VAR"` to have
  it encrypted with the passphrase in that variable.
- The result is pseudonymized, not anonymous, as long as the mapping file exists. The report says
  so in `report["warning"]`.

## Synthetic data

None of the three methods here carries a formal privacy guarantee. Every report includes a
`privacy_statement` and measured leakage figures; read them before sharing the output.
Identifier-like columns and dates are left out of the synthetic table and listed in
`report["columns_dropped"]`.

The differentially private methods and the sketch method of the command line are not available
from Python yet.

## Where your data goes

Nowhere. The engine runs inside your Python process and makes no network call. Files are read
from and written to the paths you pass.

## Limits

- Column types of a result table are inferred by the engine from the values (integer, float,
  boolean or text). They are not copied from the input, so a date column comes back as text.
- A table is copied when it is handed to the engine and again when it comes back. Large tables
  are untested.
- Tested on Linux with CPython 3.13, pandas, Polars and pyarrow.
