/** A link back to the site, tagged so the site's own visitor count can tell it came from the
 * demo and from which link. The demo itself counts nothing. */
export function siteLink(url, content) {
  const link = new URL(url);
  link.searchParams.set("utm_source", "demo");
  link.searchParams.set("utm_medium", "app");
  link.searchParams.set("utm_content", content);
  return link.toString();
}
