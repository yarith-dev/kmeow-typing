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
  /** Lower ranks come first; defaults to 0. */
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

export interface Pronunciation { word: string; readAs: string; }

export interface TypingEngine {
  /** Preview up to 64 normal Sound map readings of a Khmer alternate spelling. */
  previewPronunciation(readAs: string): string[];
  /** Replace generated readings using Khmer respellings; [] restores defaults. Saved aliases remain. */
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
