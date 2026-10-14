# triplex-jupyter

The Triplex Analytics web UI as a Jupyter widget: inspect a table, write its contract, anonymize
it, and read the results back in Python. The synthesis dialogs are part of Triplex Pro, a paid
extension; for synthetic data from Python, use `triplex.synthesize` in the `triplex` package.

Not published yet. Install from a source checkout (needs the Rust and Node toolchains that
`apps/triplex-web` uses, see its README):

```sh
cd packages/triplex-jupyter
make bundle
pip install -e .
```

## Use

```python
import pandas as pd
from triplex_jupyter import TriplexWidget

w = TriplexWidget(pd.read_csv("customers.csv"))
w
```

`TriplexWidget` takes a pandas or Polars DataFrame, a path to a `.csv`, `.tsv` or `.jsonl` file,
or CSV bytes. For Parquet, load the file into a DataFrame first.

Work in the UI, then read what you produced in a later cell:

| Attribute | What it holds |
|---|---|
| `w.contract_yaml`, `w.recipe_yaml` | The `.triplex.yaml` texts, the same ones the `triplex` CLI reads |
| `w.masked_csv`, `w.anonymize_report` | Output (CSV bytes) and report (dict) of the last anonymize run |
| `w.synthetic_csv`, `w.synthesis_report` | Output (CSV bytes) and report (dict) of the last synthesis run. Filled only by a widget bundle built with Triplex Pro |
| `w.mapping_csv` | The mapping file of the last anonymize run (CSV bytes). Empty unless you opted in, see below |

```python
import io
synthetic = pd.read_csv(io.BytesIO(w.synthetic_csv))
```

`examples/demo.ipynb` is a runnable walk-through on a synthetic sample file.

## Where your data goes

The engine runs in the browser tab that shows the notebook. The widget makes no network call of
its own and loads nothing from a CDN.

Data moves between two places only: the kernel and that browser tab. With a local Jupyter both are
on your machine. With a remote kernel (JupyterHub or a hosted notebook service) the table travels
over the connection your notebook already uses.

### The mapping file

An anonymize run also produces a mapping file, which reverses the pseudonyms. It stays in the
browser tab unless you opt in, in either of two ways (they are the same setting):

- in code: `TriplexWidget(df, expose_mapping=True)`, or `w.expose_mapping = True` later
- in the widget: tick "Send the mapping file to the notebook kernel" at the top

Then `w.mapping_csv` holds it after each anonymize run. Opting out again clears it from the
kernel. With a remote kernel, opting in sends the mapping file to that server.

The salt you type into the UI is never sent to the kernel.

## Tested with

JupyterLab 4.6 in Chromium, with a 25-row sample table. Other notebook front ends and large
tables are untested.
