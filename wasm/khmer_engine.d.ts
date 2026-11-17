/* tslint:disable */
/* eslint-disable */
export function checkProfanity(text: string, language: string): string;
export class WasmEngine {
  free(): void;
  [Symbol.dispose](): void;
  static fromBinary(bytes: Uint8Array): WasmEngine;
  browseWords(request: string): string;
  appDataInfo(): string;
  exportBinary(): Uint8Array;
  static fromAppData(khmer: Uint8Array, english: Uint8Array): WasmEngine;
  compileAppData(khmer: string, english: string, provenance: string, language: string): Uint8Array;
  loadWordRanking(json: string): void;
  analyzeComponents(word: string): string;
  loadWordMetadata(khmer: string, english: string): void;
  rankingVocabulary(language: string): string;
  setPronunciations(json: string): void;
  static fromPersonalWords(json: string): WasmEngine;
  browseWordMetadata(request: string): string;
  lookupWordMetadata(language: string, word: string): string;
  static pronunciationInputs(text: string): string;
  suggestWithContext(input: string, features_json: string, context_json: string): string;
  suggestWithRanking(input: string, features_json: string, context_json: string, ranking_json: string): string;
  wordTypingSettings(language: string, word: string): string;
  suggestWithFeatures(input: string, features_json: string): string;
  withTypingOverrides(json: string): WasmEngine;
  convertWithExtension(extension: WasmEngine, input: string): string;
  suggestWithExtension(extension: WasmEngine, input: string, features_json: string, context_json: string): string;
  lookupWordTranslations(language: string, word: string): string;
  clearComponentExtensionCache(): void;
  analyzeComponentsWithExtension(extension: WasmEngine, word: string): string;
  constructor(dataset_json: string);
  stats(): string;
  convert(input: string): string;
  suggest(input: string): string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_wasmengine_free: (a: number, b: number) => void;
  readonly checkProfanity: (a: number, b: number, c: number, d: number) => [number, number, number, number];
  readonly wasmengine_analyzeComponents: (a: number, b: number, c: number) => [number, number, number, number];
  readonly wasmengine_analyzeComponentsWithExtension: (a: number, b: number, c: number, d: number) => [number, number, number, number];
  readonly wasmengine_appDataInfo: (a: number) => [number, number];
  readonly wasmengine_browseWordMetadata: (a: number, b: number, c: number) => [number, number, number, number];
  readonly wasmengine_browseWords: (a: number, b: number, c: number) => [number, number, number, number];
  readonly wasmengine_clearComponentExtensionCache: (a: number) => void;
  readonly wasmengine_compileAppData: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number, number];
  readonly wasmengine_convert: (a: number, b: number, c: number) => [number, number, number, number];
  readonly wasmengine_convertWithExtension: (a: number, b: number, c: number, d: number) => [number, number, number, number];
  readonly wasmengine_exportBinary: (a: number) => [number, number];
  readonly wasmengine_fromAppData: (a: number, b: number, c: number, d: number) => [number, number, number];
  readonly wasmengine_fromBinary: (a: number, b: number) => [number, number, number];
  readonly wasmengine_fromPersonalWords: (a: number, b: number) => [number, number, number];
  readonly wasmengine_loadWordMetadata: (a: number, b: number, c: number, d: number, e: number) => [number, number];
  readonly wasmengine_loadWordRanking: (a: number, b: number, c: number) => [number, number];
  readonly wasmengine_lookupWordMetadata: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
  readonly wasmengine_lookupWordTranslations: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
  readonly wasmengine_new: (a: number, b: number) => [number, number, number];
  readonly wasmengine_pronunciationInputs: (a: number, b: number) => [number, number, number, number];
  readonly wasmengine_rankingVocabulary: (a: number, b: number, c: number) => [number, number, number, number];
  readonly wasmengine_setPronunciations: (a: number, b: number, c: number) => [number, number];
  readonly wasmengine_stats: (a: number) => [number, number];
  readonly wasmengine_suggest: (a: number, b: number, c: number) => [number, number, number, number];
  readonly wasmengine_suggestWithContext: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number, number];
  readonly wasmengine_suggestWithExtension: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
  readonly wasmengine_suggestWithFeatures: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
  readonly wasmengine_suggestWithRanking: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number, number];
  readonly wasmengine_withTypingOverrides: (a: number, b: number, c: number) => [number, number, number];
  readonly wasmengine_wordTypingSettings: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
  readonly __wbindgen_export_0: WebAssembly.Table;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;

export const wasmBuildFingerprint: string;
