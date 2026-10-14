"""The Triplex Analytics web UI as a Jupyter widget.

The engine is the same WebAssembly build the standalone browser app uses, and it runs in the
notebook's browser tab. This package only moves data between that tab and the kernel.
"""

import json
import os
import pathlib

import anywidget
import traitlets
from ipywidgets.widgets.trait_types import bytes_serialization

__all__ = ["TriplexWidget"]

_STATIC = pathlib.Path(__file__).parent / "static"

# The formats the browser engine reads. Parquet is not one of them (its codecs do not build for
# WebAssembly); a Parquet file goes in as a DataFrame instead.
_SUFFIXES = {".csv", ".tsv", ".jsonl"}


def _to_file(data):
    """Turns what the user passed into `(bytes, file name)`; the name's suffix tells the engine
    the format. DataFrames are recognized by method, so neither pandas nor Polars is imported."""
    if data is None:
        return None, "data.csv"
    if isinstance(data, (bytes, bytearray)):
        return bytes(data), "data.csv"
    if isinstance(data, (str, os.PathLike)):
        path = pathlib.Path(data)
        if path.suffix.lower() not in _SUFFIXES:
            raise ValueError(
                f"Cannot open {path.name}: the widget reads .csv, .tsv and .jsonl files. "
                "Load other formats (such as Parquet) into a DataFrame and pass that."
            )
        return path.read_bytes(), path.name
    if hasattr(data, "write_csv"):  # Polars
        return data.write_csv().encode(), "data.csv"
    if hasattr(data, "to_csv"):  # pandas
        return data.to_csv(index=False).encode(), "data.csv"
    raise TypeError(
        f"Cannot show a {type(data).__name__}: pass a pandas or Polars DataFrame, "
        "a path to a .csv, .tsv or .jsonl file, or CSV bytes."
    )


def _bytes_out():
    # bytes_serialization: the browser sends a buffer, which reaches the kernel as a memoryview;
    # a plain Bytes trait rejects that and the whole update is dropped.
    return traitlets.Bytes(None, allow_none=True).tag(
        sync=True, **bytes_serialization
    )


class TriplexWidget(anywidget.AnyWidget):
    """Shows `data` in the Triplex UI. Results appear on the instance as the user produces them.

    `data`: a pandas or Polars DataFrame, a path to a .csv/.tsv/.jsonl file, or CSV bytes.
    `height`: any CSS length for the widget's height.
    `expose_mapping`: let the mapping file of an anonymize run reach the kernel (see below).

    Read back (empty until the user has produced them in the UI):

    - `recipe_yaml`, `contract_yaml`: the `.triplex.yaml` texts the CLI also reads.
    - `masked_csv`, `anonymize_report`: output and report of the last anonymize run.
    - `synthetic_csv`, `synthesis_report`: output and report of the last synthesis run. Only a
      widget bundle built with Triplex Pro has the synthesis dialog that fills them.

    The mapping file reverses the pseudonyms, so it stays in the browser tab by default. It is
    sent to the kernel, as `mapping_csv`, only after an explicit opt-in: `expose_mapping=True`
    here, or the checkbox at the top of the widget (the two are the same setting). Opting out
    again clears `mapping_csv`. The salt typed into the UI is never sent.
    """

    _esm = _STATIC / "widget.js"
    _css = _STATIC / "style.css"

    data = traitlets.Bytes(None, allow_none=True).tag(sync=True)
    file_name = traitlets.Unicode("data.csv").tag(sync=True)
    height = traitlets.Unicode("640px").tag(sync=True)

    recipe_yaml = traitlets.Unicode("").tag(sync=True)
    contract_yaml = traitlets.Unicode("").tag(sync=True)
    masked_csv = _bytes_out()
    anonymize_report_json = traitlets.Unicode("").tag(sync=True)
    synthetic_csv = _bytes_out()
    synthesis_report_json = traitlets.Unicode("").tag(sync=True)

    expose_mapping = traitlets.Bool(False).tag(sync=True)
    mapping_csv = _bytes_out()

    @traitlets.validate("mapping_csv")
    def _mapping_needs_opt_in(self, proposal):
        # The page already holds the mapping back; this is the kernel's own check.
        return proposal["value"] if self.expose_mapping else None

    @traitlets.observe("expose_mapping")
    def _forget_mapping_on_opt_out(self, change):
        if not change["new"]:
            self.mapping_csv = None

    def __init__(self, data=None, *, height="640px", expose_mapping=False, **kwargs):
        if not (_STATIC / "widget.js").exists():
            raise RuntimeError(
                "triplex-jupyter was installed without its JavaScript bundle. "
                "From a source checkout, run 'make bundle' in packages/triplex-jupyter."
            )
        raw, file_name = _to_file(data)
        super().__init__(
            data=raw, file_name=file_name, height=height, expose_mapping=expose_mapping, **kwargs
        )

    @property
    def anonymize_report(self):
        """The last anonymize run's report as a dict, or None before the first run."""
        return json.loads(self.anonymize_report_json) if self.anonymize_report_json else None

    @property
    def synthesis_report(self):
        """The last synthesis run's report as a dict, or None before the first run."""
        return json.loads(self.synthesis_report_json) if self.synthesis_report_json else None
