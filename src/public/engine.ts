import { WasmEngine } from "../../wasm/khmer_engine.js";
import { bindingError, EngineBindingError } from "../bindings/errors.js";
import type { SuggestionFeatures, TypingContext } from "../shared/typing.js";
import type { EngineOptions, EngineSettings, PersonalWord, SuggestOptions, TypingEngine } from "./types.js";

export { EngineBindingError as EngineError } from "../bindings/errors.js";
export type * from "./types.js";

const defaults: EngineSettings = {
  language: "khmer", conversion: true, soundMap: true, completion: true, correction: false,
};
const settingKeys = Object.keys(defaults);
const optionKeys = [...settingKeys, "dictionary", "words", "assets", "wasm"];
const queryKeys = [...settingKeys, "previousWords", "before", "after"];

function object(value: unknown, keys: string[], name: string): asserts value is Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)
    || Object.keys(value).some(key => !keys.includes(key))) {
    throw new EngineBindingError(`Invalid ${name}. Expected only: ${keys.join(", ")}.`);
  }
}

export function validateOptions(options: EngineOptions, browser = false): void {
  object(options, browser ? optionKeys : optionKeys.filter(key => key !== "wasm"), "engine options");
  settings(defaults, options);
  if (options.dictionary === null) throw new EngineBindingError("Dictionary must not be null.");
}

function settings(current: EngineSettings, patch: Partial<EngineSettings>): EngineSettings {
  const next = { ...current };
  for (const key of settingKeys as Array<keyof EngineSettings>) {
    const value = patch[key];
    if (value === undefined) continue;
    if (key === "language") {
      if (value !== "khmer" && value !== "english") throw new EngineBindingError("Language must be khmer or english.");
      next.language = value;
    } else {
      if (typeof value !== "boolean") throw new EngineBindingError(`${key} must be true or false.`);
      next[key] = value;
    }
  }
  return next;
}

/** One mapping from the small public controls to the unchanged core controls. */
function features(value: EngineSettings): SuggestionFeatures {
  return { mode: value.language, romanized: value.conversion, soundMap: value.soundMap,
    khmerCompletion: value.completion, englishCompletion: value.completion, englishAutocorrection: value.correction };
}

function copyWords(words: PersonalWord[]): PersonalWord[] {
  return words.map(word => ({ ...word, spellings: [...word.spellings ?? []] }));
}

/** The caller initializes Wasm once; Rust validates/indexes the chosen data. */
export function loadDictionary(dictionary: NonNullable<EngineOptions["dictionary"]>): WasmEngine {
  try {
    if (dictionary instanceof Uint8Array) return WasmEngine.fromBinary(dictionary);
    if (dictionary && "khmer" in dictionary && "english" in dictionary) {
      if (!(dictionary.khmer instanceof Uint8Array) || !(dictionary.english instanceof Uint8Array)) {
        throw new EngineBindingError("Both dictionary files must be Uint8Array values.");
      }
      return WasmEngine.fromAppData(dictionary.khmer, dictionary.english);
    }
    return new WasmEngine(JSON.stringify(dictionary));
  } catch (error) { throw bindingError(error); }
}

export function createTypingEngine(raw: WasmEngine, options: EngineOptions): TypingEngine {
  let disposed = false;
  let personal: WasmEngine | undefined;
  let words: PersonalWord[] = [];
  let configured: EngineSettings;
  function check() { if (disposed) throw new EngineBindingError("Engine has been disposed."); }
  function addWords(entries: PersonalWord[]) {
    check();
    if (!Array.isArray(entries)) throw new EngineBindingError("Personal words must be an array.");
    if (entries.length === 0) return;
    const next = new Map(words.map(word => [JSON.stringify([word.language, word.word]), word]));
    for (const entry of entries) {
      object(entry, ["word", "language", "spellings", "rank"], "personal word");
      if (typeof entry.word !== "string") throw new EngineBindingError("A personal word needs word text.");
      if (entry.spellings !== undefined && (!Array.isArray(entry.spellings) || entry.spellings.some(spelling => typeof spelling !== "string"))) {
        throw new EngineBindingError("Personal spellings must be an array of text values.");
      }
      if (entry.language !== undefined && entry.language !== "khmer" && entry.language !== "english") {
        throw new EngineBindingError("Personal word language must be khmer or english.");
      }
      if (entry.rank !== undefined && (!Number.isSafeInteger(entry.rank) || entry.rank < -1_000_000 || entry.rank > 1_000_000)) {
        throw new EngineBindingError("Personal word rank must be an integer from -1000000 to 1000000.");
      }
      const language = entry.language ?? "khmer";
      const key = JSON.stringify([language, entry.word]);
      const previous = next.get(key);
      next.set(key, { ...entry, language, rank: entry.rank ?? previous?.rank ?? 0,
        spellings: [...new Set([...(previous?.spellings ?? []), ...(entry.spellings ?? [])])] });
    }
    // Validate/index the whole replacement first. Failures leave the old view intact.
    const saved = [...next.values()];
    let replacement: WasmEngine;
    try { replacement = WasmEngine.fromPersonalWords(JSON.stringify(saved)); }
    catch (error) { throw bindingError(error); }
    raw.clearComponentExtensionCache();
    personal?.free();
    personal = replacement;
    words = saved;
  }
  try {
    configured = settings(defaults, options);
    if (options.words !== undefined) addWords(options.words);
  } catch (error) { personal?.free(); raw.free(); throw bindingError(error); }

  return {
    soundMapInputs(word, after = "") {
      check();
      if (typeof word !== "string" || typeof after !== "string") {
        throw new EngineBindingError("Sound map word and cursor must be text.");
      }
      try { return JSON.parse(raw.soundMapInputs(word, after)); }
      catch (error) { throw bindingError(error); }
    },
    previewPronunciation(readAs) {
      check();
      const spellings = typeof readAs === "string" ? [readAs] : readAs;
      if (!Array.isArray(spellings) || spellings.length === 0 || spellings.length > 1000
        || !spellings.every(spelling => typeof spelling === "string")) {
        throw new EngineBindingError("Enter one or more Khmer pronunciation spellings.");
      }
      try {
        const inputs: string[] = spellings.flatMap(spelling => JSON.parse(WasmEngine.pronunciationInputs(spelling)) as string[]);
        return [...new Set(inputs)];
      }
      catch (error) { throw bindingError(error); }
    },
    wordShorthands(word) {
      check();
      if (typeof word !== "string") throw new EngineBindingError("Word must be text.");
      try { return JSON.parse(raw.wordShorthands(word)); }
      catch (error) { throw bindingError(error); }
    },
    setShorthands(words) {
      check();
      try {
        const json=JSON.stringify(words);
        if(personal) raw.setShorthandsWithExtension(personal,json);
        else raw.setShorthands(json);
      } catch(error) { throw bindingError(error); }
    },
    setInputExclusions(words) {
      check();
      try {
        const json = JSON.stringify(words);
        if (personal) raw.setInputExclusionsWithExtension(personal, json);
        else raw.setInputExclusions(json);
      }
      catch (error) { throw bindingError(error); }
    },
    setPronunciations(rules) {
      check();
      try { raw.setPronunciations(JSON.stringify(rules)); }
      catch (error) { throw bindingError(error); }
    },
    analyzeComponents(word) {
      check();
      if (typeof word !== "string") throw new EngineBindingError("Word must be a string.");
      try { return JSON.parse(personal ? raw.analyzeComponentsWithExtension(personal,word) : raw.analyzeComponents(word)); }
      catch (error) { throw bindingError(error); }
    },
    convert(input) {
      check();
      if (typeof input !== "string") throw new EngineBindingError("Input must be a string.");
      try { return JSON.parse(personal ? raw.convertWithExtension(personal, input) : raw.convert(input)); }
      catch (error) { throw bindingError(error); }
    },
    suggest(input, options: SuggestOptions = {}) {
      check();
      if (typeof input !== "string") throw new EngineBindingError("Input must be a string.");
      object(options, queryKeys, "suggestion options");
      const selected = settings(configured, options);
      const hasContext = options.previousWords !== undefined || options.before !== undefined || options.after !== undefined;
      if ((options.before !== undefined && typeof options.before !== "string")
        || (options.after !== undefined && typeof options.after !== "string")
        || (options.previousWords !== undefined && (!Array.isArray(options.previousWords)
          || options.previousWords.some(word => typeof word !== "string")))) {
        throw new EngineBindingError("Context needs previousWords text and/or before/after text.");
      }
      const context: TypingContext = { previousWords: options.previousWords ?? [] };
      if (options.before !== undefined || options.after !== undefined) {
        context.englishWordContext = { before: options.before ?? "", after: options.after ?? "" };
      }
      try {
        if (personal) return JSON.parse(raw.suggestWithExtension(personal, input, JSON.stringify(features(selected)), JSON.stringify(context)));
        if (hasContext) return JSON.parse(raw.suggestWithContext(input, JSON.stringify(features(selected)), JSON.stringify(context)));
        return JSON.parse(raw.suggestWithFeatures(input, JSON.stringify(features(selected))));
      } catch (error) { throw bindingError(error); }
    },
    get settings() { check(); return Object.freeze({ ...configured }); },
    configure(patch) { check(); object(patch, settingKeys, "settings"); configured = settings(configured, patch); },
    addWords,
    getWords() { check(); return copyWords(words); },
    clearWords() { check(); raw.clearComponentExtensionCache(); personal?.free(); personal = undefined; words = []; },
    dispose() { if (!disposed) { personal?.free(); raw.free(); disposed = true; } },
  };
}
