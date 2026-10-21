import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { DATASETS, datasetFromHash } from "./datasets";
import { municipalCsv, statisticalCsv } from "./generate";

// Tests run from www/ (npm test); under jsdom, import.meta.url is not a file URL.
const fixture = (name) => readFileSync(resolve("../../../tests/fixtures/demo", name), "utf8");

const parse = (text) => {
  const [header, ...lines] = text.trimEnd().split("\n");
  const names = header.split(",");
  return lines.map((line) => Object.fromEntries(line.split(",").map((v, i) => [names[i], v])));
};

const groupSizes = (rows, key) => {
  const sizes = new Map();
  for (const row of rows) sizes.set(key(row), (sizes.get(key(row)) ?? 0) + 1);
  return sizes;
};
const uniqueCount = (rows, key) => {
  const sizes = groupSizes(rows, key);
  return rows.filter((row) => sizes.get(key(row)) === 1).length;
};

describe("demo datasets", () => {
  it("are the generator's output, byte for byte", () => {
    expect(fixture("municipal.csv")).toBe(municipalCsv());
    expect(fixture("statistical.csv")).toBe(statisticalCsv());
  });

  it("load through the demo's dataset list", async () => {
    for (const dataset of DATASETS) {
      expect(await dataset.load()).toBe(fixture(dataset.fileName));
    }
  });

  it("municipal: postcode, birth year and sex single people out only together", () => {
    const rows = parse(fixture("municipal.csv"));
    const quasi = ["postcode", "birth_year", "sex"];
    for (const column of quasi) {
      const smallest = Math.min(...groupSizes(rows, (r) => r[column]).values());
      expect(smallest, column).toBeGreaterThan(1);
    }
    const together = uniqueCount(rows, (r) => quasi.map((c) => r[c]).join("|"));
    expect(together / rows.length).toBeGreaterThan(0.5);
    // Generalising the birth year to its decade, as the masking step can, removes most of it.
    const byDecade = uniqueCount(rows, (r) =>
      [r.postcode, Math.floor(Number(r.birth_year) / 10), r.sex].join("|"),
    );
    expect(byDecade).toBeLessThan(together / 5);
  });

  it("statistical: income rises with education, and rare combinations exist", () => {
    const rows = parse(fixture("statistical.csv"));
    const order = ["primary", "secondary", "vocational", "bachelor", "master", "doctorate"];
    const means = order.map((level) => {
      const incomes = rows.filter((r) => r.education === level).map((r) => +r.annual_income);
      return incomes.reduce((a, b) => a + b, 0) / incomes.length;
    });
    for (let i = 1; i < means.length; i++) expect(means[i]).toBeGreaterThan(means[i - 1]);
    const rare = uniqueCount(rows, (r) => [r.province, r.education].join("|"));
    expect(rare).toBeGreaterThan(0);
  });
});

describe("datasetFromHash", () => {
  it("picks the dataset the address names, or the first", () => {
    expect(datasetFromHash("#dataset=statistical").id).toBe("statistical");
    expect(datasetFromHash("dataset=municipal").id).toBe("municipal");
    expect(datasetFromHash("#dataset=unknown").id).toBe(DATASETS[0].id);
    expect(datasetFromHash("").id).toBe(DATASETS[0].id);
  });
});
