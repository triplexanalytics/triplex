import { useEffect, useState } from "react";
import { initWasm } from "../wasm";

/** Loads the wasm module once and lists the anonymization methods it
 * currently implements — split out of App.jsx so wasm lifecycle isn't
 * tangled with file/anonymize state. */
export function useWasm() {
  const [wasm, setWasm] = useState(null);
  const [methods, setMethods] = useState([]);

  useEffect(() => {
    initWasm().then((mod) => {
      setWasm(mod);
      setMethods(JSON.parse(mod.list_anonymize_methods()));
    });
  }, []);

  return { wasm, methods };
}
