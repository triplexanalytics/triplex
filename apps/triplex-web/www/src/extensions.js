import { checkExtensions } from "./extensionApi";
import { synthesizeExtension } from "./generators/synthesize";
import { sketchExtension } from "./generators/sketch";

// The generator extensions this build ships, in the order their dataset tabs appear. A build
// with other extensions replaces this module through a Vite alias on the specifier
// "./extensions", the same way the widget build swaps "./workers".
export const EXTENSIONS = checkExtensions([synthesizeExtension, sketchExtension]);
