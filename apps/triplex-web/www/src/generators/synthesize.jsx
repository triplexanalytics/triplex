import { lazy } from "react";
import { EXTENSION_API_VERSION } from "../extensionApi";
import { useSynthesize } from "../hooks/useSynthesize";
import SynthesizeDialog from "../components/SynthesizeDialog";

/** Tier 2: synthetic data from a fitted model of the table, with or without differential
 * privacy. @type {import("../extensionApi").GeneratorExtension} */
export const synthesizeExtension = {
  apiVersion: EXTENSION_API_VERSION,
  id: "synthetic",
  tier: 2,
  label: "Synthetic",
  noun: "synthetic",
  emptyHint: "Generate synthetic data first (Tier 2 card)",
  useGenerator: (host) =>
    useSynthesize({
      fileBytes: host.fileBytes,
      extension: host.fileExtension,
      headers: host.sample?.headers,
      totalRows: host.sample?.rowCount ?? 0,
      removedRowIndices: host.removedRowIndices,
      removedColumns: host.removedColumns,
    }),
  Dialog: ({ generator, host, onClose, onDone }) => (
    <SynthesizeDialog
      synthesis={generator}
      fileName={host.fileName}
      onClose={onClose}
      onDone={onDone}
    />
  ),
  // Lazy like the analysis panel it appears in: nothing needs it before a run has finished.
  Report: lazy(() => import("../components/SynthesisReport")),
  history: {
    label: "Generate synthetic data",
    detail: (generator) => (generator.mode === "dp" ? "DP" : "standard"),
  },
  hostState: (result) => ({
    synthetic_csv: result?.csv ?? null,
    synthesis_report_json: result?.reportJson ?? "",
  }),
};
