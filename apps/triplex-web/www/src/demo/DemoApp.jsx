import { useEffect, useState } from "react";
import App from "../App";
import { PRO_INFO_URL } from "../components/TierPicker.const";
import { DATASETS, datasetFromHash } from "./datasets";
import { siteLink } from "./links";

const SITE_URL = "https://triplexanalytics.io/";
const INSTALL_URL = "https://triplexanalytics.io/docs/";
const SOURCE_URL = "https://github.com/triplexanalytics/triplex";
// Below this width the app's table and side panel do not fit, so the demo does not start.
const WIDE_ENOUGH = "(min-width: 900px)";

function useWideEnough() {
  const [query] = useState(() =>
    typeof window.matchMedia === "function" ? window.matchMedia(WIDE_ENOUGH) : null,
  );
  const [wide, setWide] = useState(query ? query.matches : true);
  useEffect(() => {
    if (!query) return;
    const update = () => setWide(query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, [query]);
  return wide;
}

/** The dataset named in the address, kept in sync both ways so a link can open a given one.
 * @returns {[import("./datasets").DemoDataset, (id: string) => void]} */
function useDatasetFromAddress() {
  const [dataset, setDataset] = useState(() => datasetFromHash(window.location.hash));
  useEffect(() => {
    const update = () => setDataset(datasetFromHash(window.location.hash));
    window.addEventListener("hashchange", update);
    return () => window.removeEventListener("hashchange", update);
  }, []);
  const choose = (id) => {
    window.location.hash = `dataset=${id}`;
  };
  return [dataset, choose];
}

/** The hosted demo: the web UI on bundled synthetic datasets, with no way to load a file of
 * one's own. `extensions` are the build's generator extensions, passed on to the app; when there
 * are any, the page says they are Triplex Pro. */
export default function DemoApp({ extensions }) {
  const wide = useWideEnough();
  const [dataset, choose] = useDatasetFromAddress();
  const [file, setFile] = useState(null);
  const pro = extensions.length > 0;

  useEffect(() => {
    if (!wide) return;
    let current = true;
    dataset.load().then((text) => {
      if (current) setFile(new File([text], dataset.fileName, { type: "text/csv" }));
    });
    return () => {
      current = false;
    };
  }, [dataset, wide]);

  const picker = (
    <label className="demo-picker">
      Sample dataset
      <select value={dataset.id} onChange={(e) => choose(e.target.value)}>
        {DATASETS.map((d) => (
          <option key={d.id} value={d.id}>
            {d.label}
          </option>
        ))}
      </select>
    </label>
  );

  return (
    <div className="demo">
      <div className="demo-banner" role="note">
        <p>
          <strong>Demo on synthetic data.</strong> Everything runs in this browser tab, and the page
          is not allowed to send data anywhere. Your own files cannot be loaded here: to work with
          your own data, <a href={siteLink(INSTALL_URL, "install")}>install Triplex</a> and run it
          on your machine.
        </p>
        {pro && (
          <p>
            Synthesis and sketching (Tiers 2 and 3) are part of{" "}
            <a href={siteLink(PRO_INFO_URL, "pro")}>Triplex Pro</a>, a paid extension, shown here on
            the sample data.
          </p>
        )}
      </div>
      {wide ? (
        <div className="demo-app">
          <App initialFile={file} extensions={extensions} datasetPicker={picker} />
        </div>
      ) : (
        <div className="demo-small-screen">
          <p>The demo needs a larger screen. Open this page on a laptop or desktop.</p>
          <p>
            <a href={siteLink(SITE_URL, "small-screen")}>See what Triplex does</a>
          </p>
        </div>
      )}
      <footer className="demo-footer">
        The Triplex web UI is open source under the Apache License 2.0:{" "}
        <a href={SOURCE_URL}>source code</a>.
        {pro && " The Triplex Pro parts are proprietary and provided here for evaluation only."}
      </footer>
    </div>
  );
}
