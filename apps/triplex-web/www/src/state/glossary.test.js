/* global process */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { glossaryUrl, METHOD_GLOSSARY } from "./glossary";

// vitest runs from apps/triplex-web/www; paths are relative to it.
const read = (rel) => readFileSync(resolve(process.cwd(), "src/state", rel), "utf8");

const LINK_FILES = ["riskExplanations.jsx", "../components/ColumnMenu.jsx"];

// Glossary pages the app may link to. Add a slug here only once its page is live on the docs
// site, so a new or misspelled term fails this test instead of shipping a dead link.
const PUBLISHED = new Set([
  "bayesian-network",
  "beta-likeness",
  "bucketing",
  "chow-liu-tree",
  "combined-k-anonymity",
  "delta-disclosure",
  "distinct-values",
  "drop",
  "ebm",
  "fpe",
  "hmac",
  "independent-marginals",
  "k-anonymity",
  "k-parent-network",
  "l-diversity",
  "partial-masking",
  "suda",
  "t-closeness",
]);

describe("glossary links", () => {
  it("every term the app links to has a published glossary entry", () => {
    const linked = new Set(Object.values(METHOD_GLOSSARY));
    for (const file of LINK_FILES) {
      for (const m of read(file).matchAll(/<ReadMore term="([a-z0-9-]+)"/g)) linked.add(m[1]);
    }
    expect(linked.size).toBeGreaterThan(8);
    for (const slug of linked) expect(PUBLISHED, `missing glossary entry: ${slug}`).toContain(slug);
  });

  it("no info tooltip is left with a dead, non-link 'Read more'", () => {
    for (const file of LINK_FILES) {
      expect(read(file)).not.toContain('<span className="info-trigger-readmore">');
    }
  });

  it("builds a page URL under the docs base", () => {
    expect(glossaryUrl("k-anonymity")).toBe(
      "https://triplexanalytics.io/docs/glossary/k-anonymity.html",
    );
  });
});
