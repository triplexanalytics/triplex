import { useMemo } from "react";
import { useColumnRisk } from "./useColumnRisk";
import { useDiversityMatrix } from "./useDiversityMatrix";
import { useRowFilter } from "./useRowFilter";
import { useColumnSubsetDistributions } from "./useColumnSubsetDistributions";
import { computeAssociation } from "../state/association";

/** The read-only analysis every dataset view needs (per-column risk, the diversity matrices, the
 * row filter and its per-column subsets, the column association matrix). App.jsx runs it once for
 * the original file and once for the synthetic table, so the panel, the table and the Compare
 * view can show either one with the same components. All hooks tolerate a null `sample`, so the
 * synthetic instance is idle until a run exists. */
export function useDatasetAnalysis(wasm, sample, profile, removedRowIndices) {
  const columnRisk = useColumnRisk(sample, profile, removedRowIndices);
  const diversityMatrix = useDiversityMatrix(sample, profile, removedRowIndices);
  const rowFilter = useRowFilter(wasm, sample, profile);
  const columnSubsets = useColumnSubsetDistributions(sample, profile, rowFilter.filteredRowIndices);
  const association = useMemo(
    () => (sample && profile ? computeAssociation(sample, profile, removedRowIndices) : null),
    [sample, profile, removedRowIndices],
  );
  return { columnRisk, diversityMatrix, rowFilter, columnSubsets, association };
}
