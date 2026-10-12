"""Kernel-side contract of the widget. No browser: what the UI sends is simulated with
`set_state`, the call ipywidgets makes for an incoming message."""

import pathlib

import pytest

from triplex_jupyter import TriplexWidget, _to_file

FIXTURE = pathlib.Path(__file__).parents[3] / "tests" / "fixtures" / "sample.customers.csv"


class FakePandas:
    def to_csv(self, index=True):
        assert index is False, "the row index is not a column of the dataset"
        return "a,b\n1,2\n"


class FakePolars:
    def write_csv(self):
        return "a,b\n1,2\n"


def test_path_keeps_its_name_so_the_engine_knows_the_format():
    raw, name = _to_file(FIXTURE)
    assert name == "sample.customers.csv"
    assert raw == FIXTURE.read_bytes()


@pytest.mark.parametrize("frame", [FakePandas(), FakePolars()])
def test_dataframes_go_in_as_csv(frame):
    assert _to_file(frame) == (b"a,b\n1,2\n", "data.csv")


def test_parquet_path_is_refused_with_the_way_out():
    with pytest.raises(ValueError, match="DataFrame"):
        _to_file("table.parquet")


def test_unknown_object_is_refused():
    with pytest.raises(TypeError, match="pandas or Polars"):
        _to_file(42)


def test_widget_carries_the_dataset():
    w = TriplexWidget(FIXTURE, height="400px")
    assert (w.data, w.file_name, w.height) == (FIXTURE.read_bytes(), FIXTURE.name, "400px")


def test_results_from_the_browser_arrive_as_bytes_and_dicts():
    w = TriplexWidget()
    assert w.masked_csv is None and w.synthesis_report is None
    # Binary values reach the kernel as memoryviews, not bytes.
    w.set_state(
        {
            "synthetic_csv": memoryview(b"zip\n94110\n"),
            "synthesis_report_json": '{"method": "gaussian_copula"}',
        }
    )
    assert w.synthetic_csv == b"zip\n94110\n"
    assert w.synthesis_report == {"method": "gaussian_copula"}


MAPPING = b"token,original\n"


def test_mapping_is_dropped_without_opt_in_even_if_the_page_sends_it():
    w = TriplexWidget()
    w.set_state({"mapping_csv": memoryview(MAPPING)})
    assert w.mapping_csv is None


def test_mapping_arrives_after_opt_in_and_is_cleared_on_opt_out():
    w = TriplexWidget(expose_mapping=True)
    w.set_state({"mapping_csv": memoryview(MAPPING)})
    assert w.mapping_csv == MAPPING
    # The checkbox in the widget is the same setting, arriving as state from the page.
    w.set_state({"expose_mapping": False})
    assert w.mapping_csv is None


def test_ticking_the_option_in_the_widget_is_an_opt_in():
    w = TriplexWidget()
    w.set_state({"expose_mapping": True})
    w.set_state({"mapping_csv": memoryview(MAPPING)})
    assert w.mapping_csv == MAPPING


def test_salt_has_no_way_into_the_kernel():
    w = TriplexWidget()
    assert not [name for name in w.trait_names() if "salt" in name]
    w.set_state({"salt": "s3cret"})
    assert not hasattr(w, "salt")
