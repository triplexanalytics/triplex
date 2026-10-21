// What the hosted demo page may load and contact. Everything comes from the demo's own origin,
// and nothing may be sent anywhere else: the browser enforces this, so "nothing leaves the tab"
// does not rest on the app's code alone. The engine needs 'wasm-unsafe-eval' to compile its
// WebAssembly; style attributes need 'unsafe-inline' (React and the data grid set them).
// Applied twice: as a <meta> tag in the built page and as a response header (`_headers`, read
// by the static host), which can also forbid framing.

export const CONTENT_SECURITY_POLICY = [
  "default-src 'self'",
  "script-src 'self' 'wasm-unsafe-eval'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "font-src 'self' data:",
  "worker-src 'self'",
  "connect-src 'self'",
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
].join("; ");

/** The static host's header file. `noindex`: the site's own pages are what search engines get. */
export const HEADERS_FILE = `/*
  Content-Security-Policy: ${CONTENT_SECURITY_POLICY}; frame-ancestors 'none'
  Referrer-Policy: no-referrer
  X-Content-Type-Options: nosniff
  X-Robots-Tag: noindex
`;
