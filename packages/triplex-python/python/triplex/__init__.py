"""Triplex Analytics in plain Python: schema contracts, anonymization and synthetic data.

The engine is the same native code the `triplex` command line runs. Everything happens in this
process, on this machine; nothing is sent anywhere.

A table goes in as a file path, a pandas or Polars DataFrame, or anything else Arrow can read.
A result table comes back as a `Table`, which converts to whichever of those you use.
"""

import io
import json
import os
from dataclasses import dataclass

from . import _native
from ._native import TriplexError

__all__ = [
    "Result",
    "Table",
    "TriplexError",
    "anonymize",
    "convert",
    "infer_schema",
    "synthesize",
    "validate",
]

# Rows read from a file to infer its schema, as in the command line. A table passed in memory
# is always read in full.
SAMPLE_ROWS = 1000


class Table:
    """A result table, held as Arrow IPC stream bytes until you ask for it in a library's form."""

    def __init__(self, ipc):
        self.ipc = ipc

    def to_arrow(self):
        import pyarrow as pa

        return pa.ipc.open_stream(self.ipc).read_all()

    def to_pandas(self):
        return self.to_arrow().to_pandas()

    def to_polars(self):
        import polars as pl

        return pl.read_ipc_stream(io.BytesIO(self.ipc))

    def __arrow_c_stream__(self, requested_schema=None):
        return self.to_arrow().__arrow_c_stream__(requested_schema)


@dataclass(frozen=True)
class Result:
    """What an anonymize or synthesize run produced: the table and the run's report."""

    table: Table
    report: dict


def _input(data):
    """`(path, ipc)` for the engine: a path stays a path, a table becomes Arrow IPC bytes."""
    if isinstance(data, (str, os.PathLike)):
        return os.fspath(data), None
    if isinstance(data, Table):
        return None, data.ipc
    if hasattr(data, "write_ipc_stream"):  # Polars, which needs no pyarrow for this
        buffer = io.BytesIO()
        data.write_ipc_stream(buffer)
        return None, buffer.getvalue()

    import pyarrow as pa

    if type(data).__module__.partition(".")[0] == "pandas":
        # The row index is not a column of the dataset.
        table = pa.Table.from_pandas(data, preserve_index=False)
    else:
        table = pa.table(data)
    sink = pa.BufferOutputStream()
    with pa.ipc.new_stream(sink, table.schema) as writer:
        writer.write_table(table)
    return None, sink.getvalue().to_pybytes()


def infer_schema(data, *, sample_rows=SAMPLE_ROWS, delimiter=None):
    """Infer the schema of `data` and return it as a dict (what `triplex schema` prints).

    `delimiter` is the field separator of a `.csv` path that does not use a comma.
    """
    path, ipc = _input(data)
    return json.loads(_native.infer_schema(path, ipc, sample_rows, delimiter))


def validate(data, contract, *, sample_rows=SAMPLE_ROWS, delimiter=None):
    """Check `data` against the `.triplex.yaml` contract at `contract`.

    Returns the validation report: `{"valid": bool, "errors": [...]}`.
    """
    path, ipc = _input(data)
    return json.loads(_native.validate(contract, path, ipc, sample_rows, delimiter))


def convert(input, output, *, input_delimiter=None, output_delimiter=None):
    """Convert the file at `input` to the format of `output`'s extension. Returns the report."""
    return json.loads(_native.convert(input, output, input_delimiter, output_delimiter))


def anonymize(
    data, recipe, *, mapping_out, output=None, mapping_passphrase_env=None, exclude_rows=()
):
    """Mask the columns the `.triplex.yaml` recipe at `recipe` declares.

    Each salt is read from the environment variable its recipe column names. The mapping file,
    which can reverse the result, is written to `mapping_out` and is not returned; name an
    environment variable in `mapping_passphrase_env` to have it encrypted. `output` also writes
    the masked table as CSV. `exclude_rows` are 0-based rows to leave out.

    The result is pseudonymized, not anonymous: see `report["warning"]`.
    """
    path, ipc = _input(data)
    table, report = _native.anonymize(
        recipe, mapping_out, path, ipc, output, mapping_passphrase_env, set(exclude_rows)
    )
    return Result(Table(table), json.loads(report))


def synthesize(data, *, rows=None, seed=42, min_category_count=5, method="copula", output=None):
    """Generate a synthetic table shaped like `data`.

    `method` is `"copula"`, `"bayes-net"` or `"marginals"`. None of the three carries a formal
    privacy guarantee: read `report["privacy_statement"]` and the leakage figures in the report.
    `rows` defaults to as many as the input has. `output` also writes the table as CSV.
    """
    path, ipc = _input(data)
    table, report = _native.synthesize(path, ipc, output, rows, seed, min_category_count, method)
    return Result(Table(table), json.loads(report))
