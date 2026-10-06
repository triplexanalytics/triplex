// Where the "Read more" links in the info tooltips point: one page per term on the docs site
// (rendered from the design notes; the slugs are the `<!-- term: slug -->` markers
// there). The default is the public docs site; override it at build time with
// VITE_DOCS_BASE_URL (e.g. a self-hosted copy of the site). This is a plain link the user clicks:
// the app itself makes no request and sends nothing.
const DEFAULT_DOCS_BASE_URL = "https://triplexanalytics.com/docs/glossary/";

/** @param {string} slug */
export function glossaryUrl(slug) {
  const base = import.meta.env.VITE_DOCS_BASE_URL ?? DEFAULT_DOCS_BASE_URL;
  return `${base.replace(/\/?$/, "/")}${slug}.html`;
}

// The glossary term behind each anonymization method's info tooltip (ColumnMenu).
export const METHOD_GLOSSARY = {
  token_hmac: "hmac",
  fpe: "fpe",
  bucket: "bucketing",
  partial_mask: "partial-masking",
  drop: "drop",
};
