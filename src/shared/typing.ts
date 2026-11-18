/** Portable typing contracts. No Studio, storage or review dependencies. */
export interface ApprovedMapping {
  romanized: string;
  khmer: string;
  id: number;
  rank: number;
  status: "approved";
}
export interface ApprovedWord {
  id: number;
  khmer: string;
  rank: number;
  status: "approved";
}
export interface ApprovedStructure { wordId: number; componentIds: number[] }
export interface TypingContext { previousWords: string[]; englishWordContext?: { before: string; after: string } }
export type ApprovedDataset = (
  | { schemaVersion: 1; mappings: ApprovedMapping[]; words?: never }
  | { schemaVersion: 2; mappings: ApprovedMapping[]; words: ApprovedWord[] }
  | { schemaVersion: 3; mappings: ApprovedMapping[]; words: ApprovedWord[]; englishWords: ApprovedEnglishWord[] }) & { structures?: ApprovedStructure[]; /** Exact approved Khmer spellings using saved aliases only. Omitted means all defaults enabled. */ soundMapDisabledWords?: string[] };
export interface ApprovedEnglishWord {
  id: number;
  text: string;
  rank: number;
  status: "approved";
}
/** Exact candidates keep one mappingId. Assembled candidates use null and ordered mappingIds. */
export interface Candidate {
  mappingId: number | null;
  mappingIds?: number[];
  khmer: string;
  rank: number;
}
export interface ConversionResult {
  input: string;
  normalizedInput: string;
  candidates: Candidate[];
}
/** completion names an approved input, derived joined form or finite English contraction; the typed prefix is not a new mapping. */
export type Suggestion = (Candidate & { tapOnly?: boolean; text?: never; completion?: string; /** Preferred generated spelling for display; exact matches stay exact. */ preferredInput?: string; /** Corrected existing key; requires manual selection. */ correction?: string }) | {
  /** Zero for a runtime English contraction rule with no lexical approval identity. */
  mappingId: number;
  mappingIds?: never;
  khmer?: never;
  text: string;
  rank: number;
  completion: string;
  /** Boundary correction from the screened vocabulary or finite contraction rules. */
  correction?: string;
  /** Runtime commonness policy: visible, but requires explicit selection. */
  tapOnly?: boolean;
  /** Requires an explicit tap; never commit this replacement on Space/Return. */
  manualOnly?: boolean;
  /** Whole-word alternative, accepted according to Auto/Manual mode. */
  wordAlternative?: boolean;
};
export interface SuggestionResult extends Omit<ConversionResult, "candidates"> {
  candidates: Suggestion[];
  /** Complete-word matches on context-aware queries, separate from longer completions. */
  exactCandidates?: Candidate[];
  /** Context-aware offline touch priors, 0–1000 per ASCII letter; not word ranks. */
  nextLetterWeights?: Record<string, number>;
  kind?: "khmer-completion" | "english-completion" | "next-word";
  disabledFeature?: "romanized" | "khmer-completion" | "english-completion";
  ranking?: RankingEnvelope;
}
/** Optional model-independent transport. Never changes ordinary suggestion arrays. */
export interface RankingRequest { maxCandidates?: number; englishPreviousWords?: string[] }
export interface RankingCandidate { id: number; baselineIndex?: number; suggestion: Suggestion }
export interface RankingGroup { id: number; reorderable: boolean; displaySlots: number; candidates: RankingCandidate[] }
export interface RankingEnvelope {
  format: 'kmeow-ranking-envelope-v1'; language: 'english' | 'khmer'; contextWords: string[];
  maxCandidates: number; displayLimit: number; truncated: boolean; fallbackReason?: string;
  baselineAutomaticCandidate?: Suggestion; baselineExactCandidate?: Candidate; groups: RankingGroup[];
}
/** Completion features default on; English autocorrection is opt-in. Preferences never change the dataset. */
export interface SuggestionFeatures {
  mode?: "khmer" | "english";
  romanized?: boolean;
  /** Match pinned sound chunks against approved Khmer words; defaults on. */
  soundMap?: boolean;
  khmerCompletion?: boolean;
  englishCompletion?: boolean;
  /** Conservative boundary correction from the screened English vocabulary; defaults off. */
  englishAutocorrection?: boolean;
}
export type { WordMetadata, WordMetadataFile, WordMetadataPage, WordMetadataSource, WordTranslation, WordRanking } from './word-metadata.js';
export interface AppDataFiles { khmer: Uint8Array; english: Uint8Array }
export interface AppDataProvenance {
  compactManifest: EngineManifest;
  metadataManifest: { snapshotId: string; compactArtifactHash: string; [key: string]: unknown };
  notices: { file: string; text: string }[];
  [key: string]: unknown;
}
export interface AppDataInfo {
  format: 'kmeow-app-data-v1'; pairId: string; datasetHash: string;
  compactArtifactHash: string; snapshotId: string; provenance: AppDataProvenance;
}
export interface ConversionEngine {
  compileAppData(files: { khmer: import('./word-metadata.js').WordMetadataFile | string; english: import('./word-metadata.js').WordMetadataFile | string }, provenance: AppDataProvenance): AppDataFiles;
  appDataInfo(): AppDataInfo | null;
  loadWordMetadata(files: { khmer: import('./word-metadata.js').WordMetadataFile; english: import('./word-metadata.js').WordMetadataFile }): void;
  loadWordRanking(ranking: import('./word-metadata.js').WordRanking): void;
  lookupWordMetadata(language: 'khmer' | 'english', word: string): import('./word-metadata.js').WordMetadata[];
  lookupWordTranslations(language: 'khmer' | 'english', word: string): import('./word-metadata.js').WordTranslation[];
  browseWordMetadata(language: 'khmer' | 'english', query: string, after?: string, limit?: number): import('./word-metadata.js').WordMetadataPage;
  browseWords(query: string, after?: string, limit?: number): TypingDictionaryPage;
  convert(input: string): ConversionResult;
  suggest(input: string, features?: SuggestionFeatures, context?: TypingContext, ranking?: RankingRequest): SuggestionResult;
  rankingVocabulary(language: 'english' | 'khmer'): string[];
  exportBinary(): Uint8Array;
  stats(): EngineStats;
  dispose(): void;
}
export interface TypingDictionaryWord { khmer: string; romanizations: string[]; soundMapInputs?: string[]; soundMapInputsTruncated?: boolean; metadata?: import('./word-metadata.js').WordMetadata[] }
export interface TypingDictionaryPage { words: TypingDictionaryWord[]; nextAfter: string | null }
/** Mapping counts exclude derived spacing-only lookup forms. */
export interface EngineStats { mappings: number; words: number; englishWords: number; encodedBytes: number }
export interface EngineManifest extends EngineStats {
  /** Absent in legacy manifests, whose compact bytes use the default ordering. */
  wordRanking?: import('./word-metadata.js').WordRanking;
  rankingVersion?: string;
  format: 'kmeow-compact-v1' | 'kmeow-compact-v2' | 'kmeow-compact-v3' | 'kmeow-compact-v4';
  datasetHash: string;
  artifactHash: string;
  engineVersion: string;
}
