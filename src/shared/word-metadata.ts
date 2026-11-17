/** A frozen, sense-deduplicated ranking sidecar for portable typing. */
export interface WordRanking {
  format: 'kmeow-word-ranking-v1';
  khmer: Array<{ word: string; level: 1 | 2 | 3 | 4 | 5 }>;
  english: Array<{ word: string; level: 1 | 2 | 3 | 4 | 5 }>;
}
/** Approved lexical metadata travels separately from compact typing bytes. */
export interface WordMetadataSource {
  url: string;
  sha256?: string;
  /** Hash of the separately retained comparison evidence, not the linked source. */
  snapshotSha256?: string;
  entryKey?: string;
  license?: string;
}
export interface WordTranslation {
  word: string;
  context: string | null;
  claimId: string;
}
export interface WordMetadata {
  id: string;
  word: string;
  context: string | null;
  partsOfSpeech: string[];
  commonnessLevel: 1 | 2 | 3 | 4 | 5 | null;
  meanings: string[];
  translations: WordTranslation[];
  sources: WordMetadataSource[];
}
export interface WordMetadataFile {
  /** v1 uses the historical three-level scale; v2 uses the five-level scale. */
  format: 'kmeow-word-metadata-v1' | 'kmeow-word-metadata-v2';
  language: 'khmer' | 'english';
  snapshotId: string;
  words: WordMetadata[];
  /** Approved translation claims, including endpoints without lexical approval. */
  translations?: Array<{ source: string; target: string; context: string | null; claimId: string }>;
}
export interface WordMetadataPage {
  words: WordMetadata[];
  nextAfter: string | null;
}
