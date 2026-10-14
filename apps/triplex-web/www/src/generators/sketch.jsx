import { lazy } from "react";
import { EXTENSION_API_VERSION } from "../extensionApi";
import { useSketch } from "../hooks/useSketch";
import SketchDialog from "../components/SketchDialog";

/** Tier 3: propose a sketch of the table as classes, steer it, generate from it.
 * @type {import("../extensionApi").GeneratorExtension} */
export const sketchExtension = {
  apiVersion: EXTENSION_API_VERSION,
  id: "sketched",
  tier: 3,
  label: "Sketched",
  noun: "sketched",
  emptyHint: "Sketch data first (Tier 3 card)",
  useGenerator: (host) =>
    useSketch({
      fileBytes: host.fileBytes,
      extension: host.fileExtension,
      removedRowIndices: host.removedRowIndices,
      removedColumns: host.removedColumns,
    }),
  Dialog: ({ generator, host, onClose, onDone }) => (
    <SketchDialog
      sketching={generator}
      original={host.sample}
      excluded={host.removedRowIndices}
      fileName={host.fileName}
      onClose={onClose}
      onDone={onDone}
    />
  ),
  Report: lazy(() => import("../components/SynthesisReport")),
  history: {
    label: "Generate sketched data",
    detail: (generator) => `${generator.sketch?.classes.length ?? 0} classes`,
  },
};
