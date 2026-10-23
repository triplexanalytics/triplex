// Stands in for the engine in tests that run the whole app: jsdom runs no wasm and no Web Workers.
// It reads a plain CSV (no quoted fields), profiles it the way the app reads a profile, and masks
// it with the recipe methods the tests use, producing the same values the engine does: a bucket is
// "1950-1959", a partial mask keeps its prefix and suffix and stars the rest, a dropped column is
// left out. Recipe "YAML" here is JSON text, as the fake wasm below writes it.

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function parseCsv(text) {
  const [head, ...lines] = text.trim().split(/\r?\n/);
  return { headers: head.split(","), rows: lines.map((line) => line.split(",")) };
}

function toCsv(headers, rows) {
  return [headers.join(","), ...rows.map((r) => r.join(","))].join("\n") + "\n";
}

/** The `{ sample, profile }` the profile worker resolves to, for CSV bytes. */
export function fakeProfile(bytes) {
  const { headers, rows } = parseCsv(decoder.decode(bytes));
  const sample = {
    headers,
    rowCount: rows.length,
    getCell: (row, col) => rows[row]?.[col] ?? "",
    getNumericColumn: (col) => Float64Array.from(rows, (r) => Number(r[col])),
  };
  const profile = {
    columns: headers.map((name, i) => {
      const values = rows.map((r) => r[i]);
      if (values.every((v) => /^-?\d+$/.test(v))) {
        return { name, inferred_type: "integer", null_percentage: 0, distribution: null };
      }
      if (values.every((v) => /^-?\d+\.\d+$/.test(v))) {
        return { name, inferred_type: "float", null_percentage: 0, distribution: null };
      }
      return {
        name,
        inferred_type: "string",
        null_percentage: 0,
        distribution: { kind: "categorical", cardinality: new Set(values).size, top_values: [] },
      };
    }),
  };
  return { sample, profile };
}

function maskValue(column, value, tokens) {
  switch (column.method) {
    case "bucket": {
      const width = column.bucket_width ?? 10;
      const low = Math.floor(Number(value) / width) * width;
      return `${low}-${low + width - 1}`;
    }
    case "partial_mask": {
      const prefix = column.keep_prefix ?? 0;
      const suffix = column.keep_suffix ?? 4;
      if (value.length <= prefix + suffix) return value;
      return (
        value.slice(0, prefix) +
        "*".repeat(value.length - prefix - suffix) +
        value.slice(value.length - suffix)
      );
    }
    default: {
      // A keyed pseudonym: one token per distinct value.
      if (!tokens.has(value)) tokens.set(value, `tok${tokens.size + 1}`);
      return tokens.get(value);
    }
  }
}

/** What the anonymize worker resolves to: the masked CSV as bytes and the run's report. */
export function fakeAnonymize(bytes, _extension, recipeYaml, _saltsJson, excludedRowIndices = []) {
  const { headers, rows } = parseCsv(decoder.decode(bytes));
  const recipe = JSON.parse(recipeYaml);
  const byName = new Map(recipe.columns.map((c) => [c.name, c]));
  const dropped = headers.filter((h) => byName.get(h)?.method === "drop");
  const kept = headers.map((h, i) => [h, i]).filter(([h]) => !dropped.includes(h));
  const tokens = new Map();
  const excluded = new Set(excludedRowIndices);
  const masked = rows
    .filter((_, row) => !excluded.has(row))
    .map((r) =>
      kept.map(([h, i]) => (byName.has(h) ? maskValue(byName.get(h), r[i], tokens) : r[i])),
    );
  return {
    masked_csv: encoder.encode(
      toCsv(
        kept.map(([h]) => h),
        masked,
      ),
    ),
    mapping_csv: encoder.encode("column,original,masked\n"),
    report_json: JSON.stringify({
      rows_processed: masked.length,
      columns_anonymized: recipe.columns.filter((c) => c.method !== "drop").map((c) => c.name),
      columns_dropped: dropped,
    }),
  };
}

/** The main-thread wasm exports the recipe flow uses. */
export const fakeWasm = {
  recipe_yaml_to_json: (text) => JSON.stringify(JSON.parse(text)),
  recipe_json_to_yaml: (json) => JSON.stringify(JSON.parse(json), null, 2),
  contract_json_to_yaml: (json) => json,
};

/** The recipe the municipal demo dataset is checked with. */
export const MUNICIPAL_RECIPE = {
  version: 1,
  columns: [
    { name: "client_id", method: "token_hmac", salt_env: "CLIENT_SALT" },
    { name: "name", method: "drop" },
    { name: "email", method: "drop" },
    { name: "start_date", method: "drop" },
    { name: "postcode", method: "partial_mask", keep_prefix: 2, keep_suffix: 0 },
    { name: "birth_year", method: "bucket", bucket_width: 10 },
  ],
};
