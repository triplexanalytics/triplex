// Pure logic behind the Tier 3 sketch view: the settings of a proposal and the
// steering edits that are plain changes of the sketch (move or narrow a range, drop a class,
// change a class size). Merging two classes is done by the engine (wasm `merge_sketch_classes`).
// Kept out of components so it is unit-tested (see sketchEdit.test.js). Edits never mutate.

export const DEFAULT_CLASSES = 4;
/** Prefilled value of the user's minimum class size. */
export const DEFAULT_MIN_CLASS_SIZE = 20;
export const DEFAULT_SKETCH_BINS = 16;
export const MAX_CLASSES = 12;

/** One color per class, by position; classes beyond the list reuse it. */
export const CLASS_COLORS = [
  "#6d3fc0",
  "#d97706",
  "#0e7c7b",
  "#c2410c",
  "#2563eb",
  "#be185d",
  "#4d7c0f",
  "#475467",
];
export const classColor = (index) => CLASS_COLORS[index % CLASS_COLORS.length];

const wholeNumber = (text, min, max) => {
  const n = Number(text);
  return String(text ?? "").trim() !== "" && Number.isInteger(n) && n >= min && n <= max;
};

/** @returns {{ field: string, message: string }[]} problems with the proposal settings. */
export function validateSketchSettings({ classes, minClassSize, bins }) {
  const errors = [];
  if (!wholeNumber(classes, 1, MAX_CLASSES)) {
    errors.push({
      field: "classes",
      message: `Classes must be a whole number from 1 to ${MAX_CLASSES}.`,
    });
  }
  if (!wholeNumber(minClassSize, 1, 4294967295)) {
    errors.push({
      field: "minClassSize",
      message: "The minimum class size must be a whole number of at least 1.",
    });
  }
  if (!wholeNumber(bins, 1, 256)) {
    errors.push({ field: "bins", message: "Use a whole number of bins, from 1 to 256." });
  }
  return errors;
}

const withClass = (sketch, classIndex, change) => ({
  ...sketch,
  classes: sketch.classes.map((c, i) => (i === classIndex ? change(c) : c)),
});

/** Moves one end (or both) of a class's range on a number axis. The range never inverts: an end
 * dragged past the other stops at it. Non-finite input leaves the sketch as it was.
 * @param {any} sketch
 * @param {number} classIndex
 * @param {number} columnIndex
 * @param {{ min?: number, max?: number }} range */
export function setAxisRange(sketch, classIndex, columnIndex, { min, max }) {
  return withClass(sketch, classIndex, (c) => ({
    ...c,
    axes: c.axes.map((axis, i) => {
      if (i !== columnIndex || axis.type !== "numeric") return axis;
      let lo = Number.isFinite(min) ? min : axis.min;
      let hi = Number.isFinite(max) ? max : axis.max;
      if (lo > hi) {
        if (Number.isFinite(min) && !Number.isFinite(max)) lo = hi;
        else hi = lo;
      }
      return { ...axis, min: lo, max: hi };
    }),
  }));
}

/** Sets how many rows a class stands for (a whole number of at least 0). */
export function setClassSize(sketch, classIndex, size) {
  const n = Number(size);
  if (!Number.isInteger(n) || n < 0) return sketch;
  return withClass(sketch, classIndex, (c) => ({ ...c, size: n }));
}

/** Removes a class. The last class cannot be removed: a sketch needs one. */
export function dropClass(sketch, classIndex) {
  if (sketch.classes.length <= 1) return sketch;
  return { ...sketch, classes: sketch.classes.filter((_, i) => i !== classIndex) };
}

const withAxis = (sketch, classIndex, columnIndex, change) =>
  withClass(sketch, classIndex, (c) => ({
    ...c,
    axes: c.axes.map((axis, i) => (i === columnIndex ? change(axis) : axis)),
  }));

/** Redraws one bin of a class's density on a number axis (slice T3.2). A weight below 0 or not a
 * number is ignored, and so is a change that would leave the density without any weight. */
export function setDensityBin(sketch, classIndex, columnIndex, bin, weight) {
  if (!Number.isFinite(weight) || weight < 0) return sketch;
  return withAxis(sketch, classIndex, columnIndex, (axis) => {
    if (axis.type !== "numeric" || bin < 0 || bin >= axis.density.length) return axis;
    const density = axis.density.map((w, i) => (i === bin ? weight : w));
    return density.some((w) => w > 0) ? { ...axis, density } : axis;
  });
}

/** Makes a class's density on a number axis flat (uniform over its range). */
export function flattenDensity(sketch, classIndex, columnIndex) {
  return withAxis(sketch, classIndex, columnIndex, (axis) =>
    axis.type === "numeric" ? { ...axis, density: axis.density.map(() => 1) } : axis,
  );
}

/** The columns a class can be split along, each with a default cut: a number column whose range
 * is not a single point (cut at the middle), a category column with at least two values present
 * (cut off the first present value). */
export function splittableColumns(sketch, classIndex) {
  const cls = sketch?.classes[classIndex];
  if (!cls) return [];
  return sketch.columns.flatMap((column, i) => {
    const axis = cls.axes[i];
    if (axis.null_share >= 1) return [];
    if (axis.type === "numeric") {
      if (!(axis.min < axis.max)) return [];
      return [{ index: i, name: column.name, type: "numeric", at: (axis.min + axis.max) / 2 }];
    }
    const present = axis.shares.map((share, k) => (share > 0 ? k : -1)).filter((k) => k >= 0);
    if (present.length < 2) return [];
    return [
      {
        index: i,
        name: column.name,
        type: "categorical",
        values: present.map((k) => ({ position: k, label: column.values[k] })),
      },
    ];
  });
}

/** Why a typed cut on a number axis cannot be used, or null. */
export function cutProblem(axis, at) {
  const value = Number(at);
  if (String(at ?? "").trim() === "" || !Number.isFinite(value)) return "Enter a number to cut at.";
  if (!(value > axis.min && value < axis.max)) {
    return `The cut must lie between ${axis.min} and ${axis.max}.`;
  }
  return null;
}

/** Why the sketch cannot be generated from, or null. The engine checks again and in more detail. */
export function sketchProblem(sketch) {
  if (!sketch || sketch.classes.length === 0) return "Propose a sketch first.";
  if (sketch.classes.every((c) => c.size === 0)) return "Every class has size 0.";
  return null;
}

/** Lowest and highest value any class reaches on a number axis, widened by `extra` values (the
 * data shown behind the classes). Returns [lo, hi] with lo < hi. */
export function axisExtent(sketch, columnIndex, extra = []) {
  let lo = Infinity;
  let hi = -Infinity;
  for (const c of sketch.classes) {
    const axis = c.axes[columnIndex];
    if (axis.type !== "numeric" || axis.null_share >= 1) continue;
    lo = Math.min(lo, axis.min);
    hi = Math.max(hi, axis.max);
  }
  for (const v of extra) {
    if (Number.isFinite(v)) {
      lo = Math.min(lo, v);
      hi = Math.max(hi, v);
    }
  }
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) return [0, 1];
  if (lo === hi) return [lo - 0.5, hi + 0.5];
  return [lo, hi];
}
