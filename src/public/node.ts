import { readFile } from "node:fs/promises";
import { initializeWasm } from "../bindings/runtime.js";
import { bindingError } from "../bindings/errors.js";
import { createTypingEngine, loadDictionary, validateOptions } from "./engine.js";
import type { EngineOptions, TypingEngine } from "./types.js";
export { EngineError } from "./engine.js";
export type * from "./types.js";

/** Loads the included local assets. No Studio, database or project cwd needed. */
export async function createEngine(options: EngineOptions = {}): Promise<TypingEngine> {
  validateOptions(options);
  try {
    const root = options.assets === undefined ? new URL("../../", import.meta.url) : new URL(options.assets, import.meta.url);
    const [dictionary] = await Promise.all([
      options.dictionary ?? Promise.all([readFile(new URL("compiled-data/khmer.kmd", root)), readFile(new URL("compiled-data/english.kmd", root))])
        .then(([khmer, english]) => ({ khmer, english })),
      initializeWasm(() => readFile(new URL("wasm/khmer_engine_bg.wasm", root))),
    ]);
    return createTypingEngine(loadDictionary(dictionary), options);
  } catch (error) { throw bindingError(error); }
}
