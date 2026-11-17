//! Optional approved lexical metadata and commonness ranking. No I/O.
use crate::{invalid, Engine, EngineError};
use crate::profanity::{contains, ProfanityLanguage};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WordSource {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WordTranslation {
    pub word: String,
    pub context: Option<String>,
    pub claim_id: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WordMetadata {
    pub id: String,
    pub word: String,
    pub context: Option<String>,
    pub parts_of_speech: Vec<String>,
    pub commonness_level: Option<u8>,
    /// English sense summaries already recorded by the lexical review.
    pub meanings: Vec<String>,
    pub translations: Vec<WordTranslation>,
    pub sources: Vec<WordSource>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MetadataFile {
    format: String,
    language: String,
    snapshot_id: String,
    words: Vec<WordMetadata>,
    #[serde(default)]
    translations: Vec<TranslationClaim>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TranslationClaim { source: String, target: String, context: Option<String>, claim_id: String }
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WordMetadataPage {
    pub words: Vec<WordMetadata>,
    pub next_after: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request { language: String, query: String, after: Option<String>, limit: usize }
#[derive(Default)]
pub(crate) struct Lexicon {
    khmer: BTreeMap<String, WordMetadata>,
    english: BTreeMap<String, WordMetadata>,
    khmer_translations: BTreeMap<String, Vec<WordTranslation>>,
    english_translations: BTreeMap<String, Vec<WordTranslation>>,
    pub(crate) loaded: bool,
}
/// One level per output: typing cannot choose a meaning context, so use the
/// most common reviewed sense. Missing classifications sort after level five.
#[derive(Clone, Default)]
pub(crate) struct Commonness {
    khmer: BTreeMap<String, u8>,
    english: BTreeMap<String, u8>,
}
impl Commonness {
    fn from_ranking(json: &str) -> Result<Self, EngineError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct LevelWord { word: String, level: u8 }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Ranking { format: String, khmer: Vec<LevelWord>, english: Vec<LevelWord> }
        if json.len() > 32 * 1024 * 1024 { return Err(invalid("Word ranking is too large.")); }
        let ranking: Ranking = serde_json::from_str(json).map_err(|e| invalid(&e.to_string()))?;
        if ranking.format != "kmeow-word-ranking-v1" { return Err(invalid("Invalid word ranking format.")); }
        let mut result = Self::default();
        for (language, words) in [("khmer", ranking.khmer), ("english", ranking.english)] {
            if words.len() > 200_000 { return Err(invalid("Too many word ranking entries.")); }
            let mut seen = BTreeSet::new();
            for word in words {
                if !text(&word.word, 240) || !(1..=5).contains(&word.level) || !seen.insert(word.word.clone()) {
                    return Err(invalid("Invalid or duplicate word ranking entry."));
                }
                result.insert(language, &word.word, Some(word.level));
            }
        }
        Ok(result)
    }
    fn insert(&mut self, language: &str, word: &str, level: Option<u8>) {
        let Some(level) = level else { return; };
        let (entries, key) = if language == "english" {
            (&mut self.english, word.to_ascii_lowercase())
        } else { (&mut self.khmer, word.to_owned()) };
        entries.entry(key).and_modify(|old| *old = (*old).min(level)).or_insert(level);
    }
    pub(crate) fn level(&self, language: &str, word: &str) -> u8 {
        if language == "english" { self.english.get(&word.to_ascii_lowercase()) }
        else { self.khmer.get(word) }.copied().unwrap_or(6)
    }
    pub(crate) fn has_levels(&self, language: &str) -> bool {
        if language == "english" { !self.english.is_empty() } else { !self.khmer.is_empty() }
    }
    fn from_lexicon(lexicon: &Lexicon) -> Self {
        let mut result = Self::default();
        for (language, entries) in [("khmer", &lexicon.khmer), ("english", &lexicon.english)] {
            for word in entries.values() { result.insert(language, &word.word, word.commonness_level); }
        }
        result
    }
    /// Read only ranking fields at app startup; full meanings and sources stay lazy.
    pub(crate) fn from_pair(khmer_json: &str, english_json: &str) -> Result<Self, EngineError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct LevelWord { word: String, commonness_level: Option<u8> }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct LevelFile { format: String, language: String, snapshot_id: String, words: Vec<LevelWord> }
        let mut result = Self::default();
        let mut snapshot = None;
        for (language, json) in [("khmer", khmer_json), ("english", english_json)] {
            if json.len() > 32 * 1024 * 1024 { return Err(invalid("Word metadata is too large.")); }
            let file: LevelFile = serde_json::from_str(json).map_err(|e| invalid(&e.to_string()))?;
            let legacy = file.format == "kmeow-word-metadata-v1";
            if (!legacy && file.format != "kmeow-word-metadata-v2") || file.language != language
                || !hash(&file.snapshot_id) || file.words.len() > 200_000 {
                return Err(invalid("Invalid word metadata header."));
            }
            if snapshot.as_ref().is_some_and(|id| id != &file.snapshot_id) {
                return Err(invalid("Word metadata snapshots do not match."));
            }
            snapshot = Some(file.snapshot_id);
            for word in file.words {
                if !text(&word.word, 240) || word.commonness_level.is_some_and(|v| !(1..=if legacy { 3 } else { 5 }).contains(&v)) {
                    return Err(invalid("Invalid word commonness entry."));
                }
                result.insert(language, &word.word, word.commonness_level.map(|v| if legacy { v + 2 } else { v }));
            }
        }
        Ok(result)
    }
}
fn text(value: &str, max: usize) -> bool {
    !value.is_empty() && value.chars().count() <= max && !value.chars().any(char::is_control)
}
// Sense contexts are free-form Studio text, not dictionary keys. Keep their
// exact Unicode identity, including line breaks and tabs; NUL stays invalid.
fn context_text(value: &str) -> bool {
    !value.is_empty() && value.chars().count() <= 4000 && !value.contains('\0')
}
fn key(word: &WordMetadata) -> String { format!("{}\t{}", word.word, word.id) }
fn hash(value: &str) -> bool { value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) }
impl Lexicon {
    fn parse(json: &str, language: &str) -> Result<(String, BTreeMap<String, WordMetadata>, BTreeMap<String, Vec<WordTranslation>>), EngineError> {
        if json.len() > 32 * 1024 * 1024 { return Err(invalid("Word metadata is too large.")); }
        let file: MetadataFile = serde_json::from_str(json).map_err(|e| invalid(&e.to_string()))?;
        if !matches!(file.format.as_str(), "kmeow-word-metadata-v1" | "kmeow-word-metadata-v2") || file.language != language
            || file.snapshot_id.len() != 64 || !file.snapshot_id.bytes().all(|b| b.is_ascii_hexdigit())
            || file.words.len() > 200_000 {
            return Err(invalid("Invalid word metadata header."));
        }
        let mut result = BTreeMap::new();
        let mut ids = BTreeSet::new();
        let mut senses = BTreeSet::new();
        let legacy_scale = file.format == "kmeow-word-metadata-v1";
        for mut word in file.words {
            if !text(&word.id, 128) || !text(&word.word, 240)
                || word.context.as_ref().is_some_and(|v| !context_text(v))
                || word.commonness_level.is_some_and(|v| !(1..=if legacy_scale { 3 } else { 5 }).contains(&v))
                || !ids.insert(word.id.clone()) || !senses.insert((word.word.clone(), word.context.clone()))
                || word.parts_of_speech.len() > 16 || word.parts_of_speech.iter().any(|p|
                    !matches!(p.as_str(), "noun"|"verb"|"adj"|"adv"|"prep"|"pron"|"conj"|"det"|"intj"|"num"|"particle"|"classifier"|"aux"|"article"))
                || word.meanings.len() > 32 || word.meanings.iter().any(|v| !text(v, 4000))
                || word.translations.len() > 256 || word.translations.iter().any(|t|
                    !text(&t.word, 240) || !text(&t.claim_id, 128) || t.context.as_ref().is_some_and(|v| !context_text(v)))
                || word.sources.len() > 256 || word.sources.iter().any(|s|
                    !text(&s.url, 4096) || !(s.url.starts_with("https://") || s.url.starts_with("http://"))
                    || s.sha256.as_ref().is_some_and(|v| !hash(v))
                    || s.snapshot_sha256.as_ref().is_some_and(|v| !hash(v))
                    || s.entry_key.as_ref().is_some_and(|v| !text(v, 500))
                    || s.license.as_ref().is_some_and(|v| !text(v, 500))) {
                return Err(invalid("Invalid or duplicate word metadata entry."));
            }
            // Historical metadata retains its classifications on a current app.
            // Keep the same reviewed meaning on the five-level ranking scale.
            if legacy_scale { word.commonness_level = word.commonness_level.map(|v| v + 2); }
            result.insert(key(&word), word);
        }
        let mut translations: BTreeMap<String, Vec<WordTranslation>> = BTreeMap::new();
        let mut claim_ids = BTreeSet::new();
        if file.translations.len()>500_000 { return Err(invalid("Too many translation claims.")); }
        for claim in file.translations {
            if !text(&claim.source,240) || !text(&claim.target,240) || !text(&claim.claim_id,128)
                || claim.context.as_ref().is_some_and(|v| !context_text(v)) || !claim_ids.insert(claim.claim_id.clone()) {
                return Err(invalid("Invalid or duplicate translation claim."));
            }
            translations.entry(claim.source).or_default().push(WordTranslation {
                word:claim.target, context:claim.context, claim_id:claim.claim_id,
            });
        }
        let claims: BTreeMap<_, _> = translations.iter().flat_map(|(source, values)| values.iter()
            .map(move |claim| (claim.claim_id.as_str(), (source.as_str(), claim)))).collect();
        for word in result.values() {
            let mut attached = BTreeSet::new();
            for translation in &word.translations {
                if !attached.insert(translation.claim_id.as_str())
                    || word.context.as_ref().is_some_and(|context| translation.context.as_ref() != Some(context))
                    || !claims.get(translation.claim_id.as_str()).is_some_and(|(source, claim)|
                        *source == word.word && *claim == translation) {
                    return Err(invalid("Attached translation does not match its catalog claim."));
                }
            }
        }
        Ok((file.snapshot_id, result, translations))
    }
    pub(crate) fn entries(&self, language: &str) -> Result<&BTreeMap<String, WordMetadata>, EngineError> {
        match language { "khmer" => Ok(&self.khmer), "english" => Ok(&self.english), _ => Err(invalid("Expected Khmer or English language.")) }
    }
    pub(crate) fn lookup(&self, language: &str, word: &str) -> Result<Vec<WordMetadata>, EngineError> {
        Ok(self.entries(language)?.range(format!("{word}\t")..)
            .take_while(|(_,v)| v.word==word).map(|(_,v)| v)
            .filter(|v| !contains(&v.word, ProfanityLanguage::Auto)).map(clean).collect())
    }
}
fn clean(word: &WordMetadata) -> WordMetadata {
    let mut word = word.clone();
    word.translations.retain(|t| !contains(&t.word, ProfanityLanguage::Auto));
    word
}
impl Engine {
    /// Validate the portable ranking sidecar atomically, without loading lexical details.
    pub fn load_word_ranking_json(&mut self, json: &str) -> Result<(), EngineError> {
        self.set_commonness(Commonness::from_ranking(json)?);
        Ok(())
    }
    /// Both files are validated before changing state. Compact bytes are untouched.
    pub fn load_word_metadata(&mut self, khmer_json: &str, english_json: &str) -> Result<(), EngineError> {
        let lexicon = Lexicon::from_pair(khmer_json, english_json)?;
        self.set_commonness(Commonness::from_lexicon(&lexicon));
        self.lexicon = lexicon;
        if let Some(app) = &mut self.app_data { app.metadata = None; }
        Ok(())
    }
    pub fn lookup_word_metadata_json(&self, language: &str, word: &str) -> Result<String, EngineError> {
        if !text(word, 240) { return Err(invalid("Invalid metadata word.")); }
        serde_json::to_string(&self.word_lexicon()?.lookup(language, word)?).map_err(|e| invalid(&e.to_string()))
    }
    pub fn lookup_word_translations_json(&self, language: &str, word: &str) -> Result<String, EngineError> {
        if !text(word,240) { return Err(invalid("Invalid translation word.")); }
        let lexicon = self.word_lexicon()?;
        let catalog = match language { "khmer"=>&lexicon.khmer_translations, "english"=>&lexicon.english_translations,
            _=>return Err(invalid("Expected Khmer or English language.")) };
        let result: Vec<_> = catalog.get(word).into_iter().flatten()
            .filter(|t| !contains(word,ProfanityLanguage::Auto) && !contains(&t.word,ProfanityLanguage::Auto)).collect();
        serde_json::to_string(&result).map_err(|e| invalid(&e.to_string()))
    }
    pub fn browse_word_metadata_json(&self, json: &str) -> Result<String, EngineError> {
        let request: Request = serde_json::from_str(json).map_err(|e| invalid(&e.to_string()))?;
        if request.query.chars().count()>512 || request.query.chars().any(char::is_control)
            || !(1..=100).contains(&request.limit) || request.after.as_ref().is_some_and(|a| a.chars().count()>500) {
            return Err(invalid("Invalid word metadata query."));
        }
        let query = request.query.trim().to_lowercase();
        // Search the same saved aliases and Sound map lattices as the word
        // browser, then page lexical senses using their original cursor keys.
        let mut romanized_words = BTreeSet::new();
        if request.language == "khmer" && query.bytes().any(|b| b.is_ascii_alphabetic()) {
            let typing_query = crate::normalize(&query)?;
            let joined_query = typing_query.replace(' ', "");
            let mut cursor = self.dictionary.romanized.cursor(0);
            while let Some(row) = cursor.next()? {
                if row.source.is_none() && !self.blocked_mapping(&row)
                    && (row.key.contains(&typing_query) || row.key.replace(' ', "").contains(&joined_query)) {
                    romanized_words.insert(self.dictionary.pool.get(row.target).to_owned());
                }
            }
            let sounds = self.sound_index.get_or_init(|| crate::sound_map::Index::new(&self.dictionary));
            romanized_words.extend(sounds.browse_matches(&typing_query).into_iter().map(str::to_owned));
        }
        let mut selected: Vec<_> = self.word_lexicon()?.entries(&request.language)?.iter()
            .filter(|(key, word)| request.after.as_ref().is_none_or(|a| *key>a)
                && (word.word.to_lowercase().contains(&query) || romanized_words.contains(&word.word))
                && !contains(&word.word, ProfanityLanguage::Auto))
            .take(request.limit+1).collect();
        let more = selected.len()>request.limit;
        if more { selected.pop(); }
        let next_after = if more { selected.last().map(|(key,_)| (*key).clone()) } else { None };
        let words = selected.into_iter().map(|(_, word)| clean(word)).collect();
        serde_json::to_string(&WordMetadataPage { words, next_after }).map_err(|e| invalid(&e.to_string()))
    }
}

impl Lexicon {
    pub(crate) fn from_pair(khmer_json: &str, english_json: &str) -> Result<Self, EngineError> {
        let (khmer_id, khmer, khmer_translations) = Lexicon::parse(khmer_json, "khmer")?;
        let (english_id, english, english_translations) = Lexicon::parse(english_json, "english")?;
        if khmer_id != english_id { return Err(invalid("Word metadata snapshots do not match.")); }
        // The two directional catalogs must describe the same exact claims.
        let km_claims: BTreeSet<_> = khmer_translations.iter().flat_map(|(source, values)| values.iter()
            .map(move |t| (t.claim_id.clone(), t.word.clone(), source.clone(), t.context.clone()))).collect();
        let en_claims: BTreeSet<_> = english_translations.iter().flat_map(|(source, values)| values.iter()
            .map(move |t| (t.claim_id.clone(), source.clone(), t.word.clone(), t.context.clone()))).collect();
        if km_claims!=en_claims { return Err(invalid("Translation catalogs do not match.")); }
        Ok(Lexicon { khmer, english, khmer_translations, english_translations, loaded: true })
    }
}
