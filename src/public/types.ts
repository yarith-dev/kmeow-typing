import type { AppDataFiles, ApprovedDataset, ConversionResult, SuggestionResult } from "../shared/typing.js";

export type { Candidate, ConversionResult, Suggestion, SuggestionResult } from "../shared/typing.js";

export interface EngineSettings {
  language: "khmer" | "english";
  conversion: boolean;
  soundMap: boolean;
  /** Direct Khmer/English word completion; Latin suggestions follow conversion. */
  completion: boolean;
  correction: boolean;
}

/** Personal additions only; never modifies the supplied dictionary or Studio. */
export interface PersonalWord {
  word: string;
  /** Defaults to Khmer. English words do not accept Romanized spellings. */
  language?: "khmer" | "english";
  spellings?: string[];
  /** Integer from -1,000,000 to 1,000,000. Lower ranks come first; defaults to 0. */
  rank?: number;
}

export interface EngineOptions extends Partial<EngineSettings> {
  /** Omit to use the included dictionary pair. */
  dictionary?: AppDataFiles | ApprovedDataset | Uint8Array;
  words?: PersonalWord[];
  /** Package root containing wasm/ and compiled-data/. Normally automatic. */
  assets?: string | URL;
}

export interface SuggestOptions extends Partial<EngineSettings> {
  /** Committed Khmer words. For English context, use before/after. */
  previousWords?: string[];
  /** Transient English text before/after the active word. Never retained. */
  before?: string;
  after?: string;
}

export interface WordComponent {
  text: string;
  /** Unicode character offsets, not JavaScript UTF-16 offsets. */
  start: number;
  end: number;
  /** Absent for unresolved text. */
  wordId?: number;
}
export interface ComponentAnalysis {
  word: string;
  source: "saved" | "automatic";
  /** Detection describes dictionary matches, not linguistic approval. */
  status: "complete" | "partial" | "ambiguous" | "unknown";
  parts: WordComponent[];
  /** At most two competing analyses; no automatic choice for ambiguous words. */
  alternatives: WordComponent[][];
  moreAlternatives: boolean;
}

export interface Pronunciation { word: string; readAs: string | string[]; }
export interface WordInputExclusion { word: string; inputs: string[]; }
export interface WordShorthand { word: string; inputs: string[]; }
export interface SoundMapInputPage {
  inputs: string[];
  nextAfter: string | null;
}

export interface TypingEngine {
  /** Read a bundled word's generated spellings, 64 per page. Includes excluded/disabled keys, omits saved aliases and shorthands. */
  soundMapInputs(word: string, after?: string): SoundMapInputPage;
  /** Replace exact Romanized exclusions for bundled and personal words. Survives personal changes; [] clears it. */
  setInputExclusions(words: WordInputExclusion[]): void;
  /** Whole-word suggestion-only inputs. [] restores defaults; an empty word inputs list disables its shorthand. */
  setShorthands(words: WordShorthand[]): void;
  wordShorthands(word: string): string[];
  /** Combine the previews of one or more Khmer spellings, omitting duplicate readings. */
  previewPronunciation(readAs: string | readonly string[]): string[];
  /** Edit a word or built-in part using one or more Khmer respellings; [] restores defaults. Saved aliases remain. */
  setPronunciations(rules: Pronunciation[]): void;
  /** Saved whole-input readings and composition, unchanged by suggestion settings. */
  convert(input: string): ConversionResult;
  /** Live typing with the same candidate order and acceptance flags as the core. */
  suggest(input: string, options?: SuggestOptions): SuggestionResult;
  /** Cached Khmer analysis: coverage, then fewer matched parts. Saved definitions win; typing is unchanged. */
  analyzeComponents(word: string): ComponentAnalysis;
  readonly settings: Readonly<EngineSettings>;
  configure(settings: Partial<EngineSettings>): void;
  /** Adds words/spellings atomically. Repeated words merge their spellings. */
  addWords(words: PersonalWord[]): void;
  /** Defensive JSON-safe copy for caller-managed persistence. */
  getWords(): PersonalWord[];
  clearWords(): void;
  dispose(): void;
}
