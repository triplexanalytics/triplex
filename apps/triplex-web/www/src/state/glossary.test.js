/* global process */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { glossaryUrl, METHOD_GLOSSARY } from "./glossary";

// vitest runs from apps/triplex-web/www; paths are relative to it.
const read = (rel) => readFileSync(resolve(process.cwd(), "src/state", rel), "utf8");

// Slugs published on the docs site: `<!-- term: slug -->` markers outside the internal
// "Process terms" section of the canonical glossary.
function publishedSlugs() {
  const glossary = readFileSync(
    resolve(process.cwd(), "../../../the design notes"),
    "utf8",
  );
  const slugs = new Set();
  let publish = true;
  for (const line of glossary.split("\n")) {
    const heading = line.match(/^##\s+(.*)$/);
    if (heading) publish = !/^Process terms/i.test(heading[1]);
    const marker = line.match(/^<!--\s*term:\s*([a-z0-9-]+)\s*-->$/);
    if (marker && publish) slugs.add(marker[1]);
  }
  return slugs;
}

describe("glossary links", () => {
  it("every term the app links to has a published glossary entry", () => {
    const linked = new Set(Object.values(METHOD_GLOSSARY));
    for (const file of ["riskExplanations.jsx", "../components/ColumnMenu.jsx"]) {
      for (const m of read(file).matchAll(/<ReadMore term="([a-z0-9-]+)"/g)) linked.add(m[1]);
    }
    expect(linked.size).toBeGreaterThan(8);
    const published = publishedSlugs();
    for (const slug of linked) expect(published, `missing glossary entry: ${slug}`).toContain(slug);
  });

  it("no info tooltip is left with a dead, non-link 'Read more'", () => {
    for (const file of ["riskExplanations.jsx", "../components/ColumnMenu.jsx"]) {
      expect(read(file)).not.toContain('<span className="info-trigger-readmore">');
    }
  });

  it("builds a page URL under the docs base", () => {
    expect(glossaryUrl("k-anonymity")).toBe(
      "https://triplexanalytics.com/docs/glossary/k-anonymity.html",
    );
  });
});
