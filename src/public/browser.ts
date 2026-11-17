import { initializeWasm } from "../bindings/runtime.js";
import type { InitInput } from "../../wasm/khmer_engine.js";
import { bindingError } from "../bindings/errors.js";
import { createTypingEngine, loadDictionary, validateOptions } from "./engine.js";
import type { EngineOptions, TypingEngine } from "./types.js";
export { EngineError } from "./engine.js";
export type * from "./types.js";
export interface BrowserEngineOptions extends EngineOptions { wasm?: InitInput }

async function bytes(url: URL): Promise<Uint8Array> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Could not load ${url.pathname}: HTTP ${response.status}.`);
  return new Uint8Array(await response.arrayBuffer());
}

/** Fetch assets once at setup; queries stay synchronous and offline afterwards. */
export async function createEngine(options: BrowserEngineOptions = {}): Promise<TypingEngine> {
  validateOptions(options, true);
  try {
    const root = options.assets === undefined ? undefined : new URL(options.assets, import.meta.url);
    // Keep literal default URLs visible to asset-aware bundlers.
    const khmerURL = root ? new URL("compiled-data/khmer.kmd", root) : new URL("../../compiled-data/khmer.kmd", import.meta.url);
    const englishURL = root ? new URL("compiled-data/english.kmd", root) : new URL("../../compiled-data/english.kmd", import.meta.url);
    const wasmURL = root ? new URL("wasm/khmer_engine_bg.wasm", root) : new URL("../../wasm/khmer_engine_bg.wasm", import.meta.url);
    const [dictionary] = await Promise.all([
      options.dictionary ?? Promise.all([bytes(khmerURL), bytes(englishURL)])
        .then(([khmer, english]) => ({ khmer, english })),
      initializeWasm(() => options.wasm ?? wasmURL),
    ]);
    return createTypingEngine(loadDictionary(dictionary), options);
  } catch (error) { throw bindingError(error); }
}
