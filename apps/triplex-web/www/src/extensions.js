import { checkExtensions } from "./extensionApi";

// The generator extensions this build ships, in the order their dataset tabs appear. The open
// build ships none: Tiers 2 and 3 link to Triplex Pro, a separate build that replaces this module
// through a Vite alias on the specifier "./extensions", the same way the widget build swaps
// "./workers". Synthetic data itself stays available in the engine, the CLI and the Python
// package.
export const EXTENSIONS = checkExtensions([]);
