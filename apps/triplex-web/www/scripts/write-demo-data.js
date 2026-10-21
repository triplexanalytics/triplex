// Writes the demo's sample datasets into tests/fixtures/demo/. Run after changing
// src/demo/generate.js (`npm run demo-data`); a test fails while the files and the generator
// disagree.
import { mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { municipalCsv, statisticalCsv } from "../src/demo/generate.js";

const dir = fileURLToPath(new URL("../../../../tests/fixtures/demo/", import.meta.url));
mkdirSync(dir, { recursive: true });
writeFileSync(`${dir}municipal.csv`, municipalCsv());
writeFileSync(`${dir}statistical.csv`, statisticalCsv());
console.log(`Wrote municipal.csv and statistical.csv to ${dir}`);
