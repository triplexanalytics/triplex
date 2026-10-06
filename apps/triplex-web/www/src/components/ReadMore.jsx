import { glossaryUrl } from "../state/glossary";

/** The "Read more…" line at the end of an info tooltip: opens the term's glossary page on the docs
 * site in a new tab. */
export default function ReadMore({ term }) {
  return (
    <a
      className="info-trigger-readmore"
      href={glossaryUrl(term)}
      target="_blank"
      rel="noopener noreferrer"
    >
      Read more…
    </a>
  );
}
