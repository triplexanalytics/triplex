"""The Python surface against the real engine, on the repository's synthetic fixtures."""

import pathlib

import pandas as pd
import polars as pl
import pyarrow as pa
import pytest

import triplex

FIXTURES = pathlib.Path(__file__).parents[3] / "tests" / "fixtures"
SAMPLE = FIXTURES / "sample.csv"
CUSTOMERS = FIXTURES / "sample.customers.csv"
SALT_ENV = "TRIPLEX_DEMO_SALT"  # the variable tests/fixtures/sample.anonymize.yaml names


def columns(schema):
    return [(c["name"], c["inferred_type"]) for c in schema["columns"]]


@pytest.mark.parametrize(
    "load", [pd.read_csv, pl.read_csv, lambda path: pa.Table.from_pandas(pd.read_csv(path))]
)
def test_a_table_in_memory_gets_the_schema_its_file_gets(load):
    assert columns(triplex.infer_schema(load(SAMPLE))) == columns(triplex.infer_schema(SAMPLE))


def test_the_pandas_row_index_is_not_a_column():
    frame = pd.read_csv(SAMPLE).set_index(pd.Index(["a", "b", "c", "d"]))
    assert [name for name, _ in columns(triplex.infer_schema(frame))] == list(frame.columns)


def test_a_delimiter_is_one_ascii_character():
    semicolons = triplex.infer_schema(FIXTURES / "sample.semicolon.csv", delimiter=";")
    assert columns(semicolons) == columns(triplex.infer_schema(SAMPLE))
    with pytest.raises(triplex.TriplexError):
        triplex.infer_schema(SAMPLE, delimiter="é")


def test_validate_reports_a_missing_column():
    contract = FIXTURES / "sample.contract.yaml"
    frame = pd.read_csv(SAMPLE)
    assert triplex.validate(SAMPLE, contract)["valid"]
    assert triplex.validate(frame, contract)["valid"]
    report = triplex.validate(frame.drop(columns=["price"]), contract)
    assert not report["valid"] and report["errors"]


def test_a_draft_contract_holds_every_column_and_validates_its_table(tmp_path):
    contract = triplex.draft_contract(SAMPLE, output=tmp_path / "sample.triplex.yaml")
    inferred = triplex.infer_schema(SAMPLE)
    assert [(c["name"], c["type"]) for c in contract["columns"]] == columns(inferred)
    assert all(c["required"] for c in contract["columns"])
    assert triplex.validate(SAMPLE, contract)["valid"]
    assert triplex.validate(SAMPLE, tmp_path / "sample.triplex.yaml")["valid"]


def test_a_contract_dict_catches_what_a_transformation_changed():
    frame = pd.read_csv(SAMPLE)
    contract = triplex.draft_contract(frame)
    assert triplex.validate(frame, contract)["valid"]
    report = triplex.validate(frame.drop(columns=["price"]), contract)
    assert not report["valid"] and report["errors"]


def test_an_inferred_schema_is_not_taken_for_a_contract():
    with pytest.raises(TypeError, match="draft_contract"):
        triplex.validate(SAMPLE, triplex.infer_schema(SAMPLE))


def test_a_malformed_contract_dict_is_an_engine_error():
    with pytest.raises(triplex.TriplexError):
        triplex.validate(SAMPLE, {"version": 1, "columns": [{"name": "id"}]})


def test_convert_writes_the_other_format(tmp_path):
    report = triplex.convert(SAMPLE, tmp_path / "sample.jsonl")
    assert report["rows_converted"] == 4
    assert len((tmp_path / "sample.jsonl").read_text().splitlines()) == 4


def test_anonymize_masks_the_declared_column_only(tmp_path, monkeypatch):
    monkeypatch.setenv(SALT_ENV, "synthetic-test-salt")
    frame = pd.read_csv(SAMPLE)
    recipe = FIXTURES / "sample.anonymize.yaml"

    result = triplex.anonymize(
        frame, recipe, mapping_out=tmp_path / "mapping.csv", output=tmp_path / "masked.csv"
    )
    masked = result.table.to_pandas()

    assert not set(masked["name"]) & set(frame["name"])
    for untouched in ["id", "price", "in_stock"]:
        pd.testing.assert_series_equal(masked[untouched], frame[untouched])
    assert result.report["columns_anonymized"] == ["name"]
    assert "pseudonymized" in result.report["warning"]
    # The mapping is on disk, where the caller said, and holds the originals.
    assert set(pd.read_csv(tmp_path / "mapping.csv")["original_value"]) == set(frame["name"])
    assert list(pd.read_csv(tmp_path / "masked.csv")["name"]) == list(masked["name"])

    # Same salt, same tokens, whichever way the table came in or goes out.
    from_path = triplex.anonymize(SAMPLE, recipe, mapping_out=tmp_path / "mapping2.csv")
    assert from_path.table.to_polars()["name"].to_list() == list(masked["name"])
    assert pa.table(result.table).column("name").to_pylist() == list(masked["name"])


def test_exclude_rows_and_an_encrypted_mapping(tmp_path, monkeypatch):
    monkeypatch.setenv(SALT_ENV, "synthetic-test-salt")
    monkeypatch.setenv("TRIPLEX_TEST_PASSPHRASE", "synthetic-passphrase")
    result = triplex.anonymize(
        pl.read_csv(SAMPLE),
        FIXTURES / "sample.anonymize.yaml",
        mapping_out=tmp_path / "mapping.json",
        mapping_passphrase_env="TRIPLEX_TEST_PASSPHRASE",
        exclude_rows=[0],
    )
    assert result.report["rows_excluded"] == 1
    assert result.table.to_polars()["id"].to_list() == [2, 3, 4]
    assert "widget" not in (tmp_path / "mapping.json").read_text()


def test_an_engine_error_carries_its_code(tmp_path, monkeypatch):
    monkeypatch.delenv(SALT_ENV, raising=False)
    with pytest.raises(triplex.TriplexError) as error:
        triplex.anonymize(
            SAMPLE, FIXTURES / "sample.anonymize.yaml", mapping_out=tmp_path / "m.csv"
        )
    assert error.value.code == "MISSING_ENV_VAR"
    assert not (tmp_path / "m.csv").exists()


@pytest.mark.parametrize("method", ["copula", "bayes-net", "marginals"])
def test_synthesize_is_deterministic_given_a_seed(method, tmp_path):
    frame = pl.read_csv(CUSTOMERS)
    first = triplex.synthesize(frame, rows=10, seed=7, method=method, output=tmp_path / "s.csv")
    again = triplex.synthesize(CUSTOMERS, rows=10, seed=7, method=method)
    other = triplex.synthesize(frame, rows=10, seed=8, method=method)

    assert first.report["rows_out"] == 10 == first.table.to_polars().height
    assert first.table.to_polars().equals(again.table.to_polars())
    assert not first.table.to_polars().equals(other.table.to_polars())
    assert first.report["privacy_statement"]
    assert first.table.to_polars().columns == first.report["columns_synthesized"]
    assert pl.read_csv(tmp_path / "s.csv").height == 10


def test_an_unknown_method_is_a_clear_error():
    with pytest.raises(triplex.TriplexError) as error:
        triplex.synthesize(CUSTOMERS, method="gan")
    assert error.value.code == "VALIDATION_ERROR"
