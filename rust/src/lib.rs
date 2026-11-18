//! Offline lookup, sentence composition and live completions over approved mappings.
//! No storage, I/O, platform, model or server dependencies.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
mod compact;
mod sound_map;
mod browse;
pub use browse::{DictionaryPage, DictionaryWord, SoundMapInputPage};
mod app_data;
pub use app_data::MAX_APP_DATA_BYTES;
mod word_metadata;
pub use word_metadata::{WordMetadata, WordMetadataPage};
mod context;
mod components;
pub use components::{ComponentAnalysis, ComponentSource, ComponentStatus, WordComponent};
mod recovery;
mod english_correction;
mod english_surface_forms;
mod english_word_choices;
mod shared_words;
mod next_letters;
mod personal;
mod pronunciations;
mod input_exclusions;
mod shorthands;
pub use shorthands::WordShorthand;
pub use input_exclusions::WordInputExclusion;
pub use pronunciations::{Pronunciation, ReadAs};
mod ranking;
pub use ranking::{RankingRequest, RankingEnvelope, RankingGroup, RankingCandidate, RankingSuggestionResult};
pub use personal::{PersonalWord, TypingOverride};
pub mod profanity;
use profanity::{contains as contains_profanity, ProfanityLanguage};

pub const MIN_RANK: i32 = -1_000_000;
pub const MAX_RANK: i32 = 1_000_000;
fn valid_rank(rank: i32) -> bool { (MIN_RANK..=MAX_RANK).contains(&rank) }

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_COMPOSED_CANDIDATES: usize = 20;
const MAX_COMPOSED_SEGMENTS: usize = 32;
const MAX_COMPLETIONS: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ApprovedStatus {
    Approved,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedMapping {
    pub id: u64,
    pub romanized: String,
    pub khmer: String,
    pub rank: i32,
    pub status: ApprovedStatus,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovedDataset {
    pub schema_version: u32,
    #[serde(deserialize_with = "deserialize_object_rows")]
    pub mappings: Vec<ApprovedMapping>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_object_rows")]
    pub words: Option<Vec<ApprovedWord>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_object_rows")]
    pub english_words: Option<Vec<ApprovedEnglishWord>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_optional_object_rows")]
    pub structures: Option<Vec<ApprovedStructure>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_map_disabled_words: Option<Vec<String>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovedStructure { pub word_id: u64, pub component_ids: Vec<u64> }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypingContext {
    #[serde(default)]
    pub previous_words: Vec<String>,
    /// Verified transient English text around the active word; never stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub english_word_context: Option<EnglishWordContext>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnglishWordContext {
    pub before: String,
    pub after: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContextualSuggestionResult {
    #[serde(flatten)]
    pub result: SuggestionResult,
    pub exact_candidates: Vec<Candidate>,
    /// Heuristic next-letter mass in thousandths; never a change to word ranking.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub next_letter_weights: BTreeMap<String, u16>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedEnglishWord {
    pub id: u64,
    pub text: String,
    pub rank: i32,
    pub status: ApprovedStatus,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedWord {
    pub id: u64,
    pub khmer: String,
    pub rank: i32,
    pub status: ApprovedStatus,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub mapping_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mapping_ids: Vec<u64>,
    pub khmer: String,
    pub rank: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConversionResult {
    pub input: String,
    pub normalized_input: String,
    pub candidates: Vec<Candidate>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    /// Preferred generated spelling for display; does not change exactness or acceptance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_input: Option<String>,
    /// Reviewed Uncommon/Academic outputs remain selectable, never automatic.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tap_only: bool,
    #[serde(flatten)]
    pub candidate: SuggestionCandidate,
    /// An approved input, derived joined lookup or finite English contraction;
    /// never an approval of the prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion: Option<String>,
    /// One-edit recovery or finite English contraction formatting. Khmer is
    /// manual-only; English boundary correction is opt-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correction: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum SuggestionCandidate {
    Khmer(Candidate),
    English(EnglishCandidate),
}
impl SuggestionCandidate {
    fn rank(&self) -> i32 {
        match self {
            Self::Khmer(c) => c.rank,
            Self::English(c) => c.rank,
        }
    }
    pub fn mapping_id(&self) -> Option<u64> {
        match self {
            Self::Khmer(c) => c.mapping_id,
            Self::English(c) => Some(c.mapping_id),
        }
    }
}
fn suggestion_output(suggestion: &Suggestion) -> &str {
    match &suggestion.candidate {
        SuggestionCandidate::Khmer(candidate) => &candidate.khmer,
        SuggestionCandidate::English(candidate) => &candidate.text,
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnglishCandidate {
    /// Zero for a runtime contraction rule without a lexical approval identity.
    pub mapping_id: u64,
    pub text: String,
    pub rank: i32,
    /// An explicit replacement choice which must never be accepted on a boundary.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub manual_only: bool,
    /// Whole-word choice; follows the keyboard's ordinary Auto/Manual mode.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub word_alternative: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionResult {
    pub input: String,
    pub normalized_input: String,
    pub candidates: Vec<Suggestion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<SuggestionKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_feature: Option<SuggestionFeature>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct SuggestionFeatures {
    pub mode: TypingMode,
    pub romanized: bool,
    pub sound_map: bool,
    pub khmer_completion: bool,
    pub english_completion: bool,
    pub english_autocorrection: bool,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TypingMode {
    Khmer,
    English,
}
impl Default for SuggestionFeatures {
    fn default() -> Self {
        Self {
            mode: TypingMode::Khmer,
            romanized: true,
            sound_map: true,
            khmer_completion: true,
            english_completion: true,
            english_autocorrection: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SuggestionFeature {
    #[serde(rename = "romanized")]
    Romanized,
    #[serde(rename = "khmer-completion")]
    KhmerCompletion,
    #[serde(rename = "english-completion")]
    EnglishCompletion,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SuggestionKind {
    #[serde(rename = "khmer-completion")]
    KhmerCompletion,
    #[serde(rename = "english-completion")]
    EnglishCompletion,
    #[serde(rename = "next-word")]
    NextWord,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineError(pub String);
impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for EngineError {}
fn invalid(message: &str) -> EngineError {
    EngineError(message.to_owned())
}
fn normalize(input: &str) -> Result<String, EngineError> {
    if input.chars().count() > 512 {
        return Err(invalid("Input exceeds 512 characters."));
    }
    Ok(input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase())
}

// Serde's derived struct parser also accepts positional sequences. JSON entry
// points require objects, while the original map stream must retain duplicate
// fields for the typed parser to reject them.
struct JsonObject<T>(T);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for JsonObject<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Visitor<T> {
            type Value = JsonObject<T>;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(serde::de::value::MapAccessDeserializer::new(map)).map(JsonObject)
            }
        }
        deserializer.deserialize_map(Visitor(std::marker::PhantomData))
    }
}
fn deserialize_object_rows<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where D: serde::Deserializer<'de>, T: Deserialize<'de> {
    Vec::<JsonObject<T>>::deserialize(deserializer).map(|rows| rows.into_iter().map(|row| row.0).collect())
}
fn deserialize_optional_object_rows<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where D: serde::Deserializer<'de>, T: Deserialize<'de> {
    Option::<Vec<JsonObject<T>>>::deserialize(deserializer)
        .map(|rows| rows.map(|rows| rows.into_iter().map(|row| row.0).collect()))
}

fn parse_request_object<T: serde::de::DeserializeOwned>(json: &str, label: &str) -> Result<T, EngineError> {
    serde_json::from_str::<JsonObject<T>>(json).map(|object| object.0)
        .map_err(|e| invalid(&format!("Invalid {label}: {e}")))
}

#[cfg(test)]
mod request_tests {
    use super::*;

    #[test]
    fn suggestion_features_and_context_require_objects_on_every_request_path() {
        for json in ["[]", "[\"khmer\",true,true,true,true]", "null", "true", "0", "\"object\""] {
            assert!(parse_request_object::<SuggestionFeatures>(json, "suggestion features").is_err(), "{json}");
            assert!(parse_request_object::<TypingContext>(json, "typing context").is_err(), "{json}");
        }
        assert!(parse_request_object::<TypingContext>("[[]]", "typing context").is_err());
        assert!(parse_request_object::<SuggestionFeatures>(r#"{"extra":true}"#, "suggestion features").is_err());
        assert!(parse_request_object::<TypingContext>(r#"{"previousWords":{}}"#, "typing context").is_err());
        assert!(parse_request_object::<SuggestionFeatures>("{}", "suggestion features").unwrap().sound_map);
        assert!(parse_request_object::<TypingContext>("{}", "typing context").unwrap().previous_words.is_empty());
        let context: TypingContext = parse_request_object(r#"{"previousWords":["កា"]}"#, "typing context").unwrap();
        assert_eq!(context.previous_words, ["កា"]);
    }

    #[test]
    fn object_parsing_retains_duplicate_field_rejection() {
        for json in [r#"{"mode":"khmer","mode":"english"}"#,
            r#"{"romanized":null,"romanized":true}"#,
            r#"{"soundMap":true,"soundMap":true}"#] {
            assert!(parse_request_object::<SuggestionFeatures>(json, "suggestion features").is_err(), "{json}");
        }
        for json in [r#"{"previousWords":["invalid"],"previousWords":[]}"#,
            r#"{"previousWords":[],"previousWords":[]}"#] {
            assert!(parse_request_object::<TypingContext>(json, "typing context").is_err(), "{json}");
        }
    }

    #[test]
    fn approved_json_requires_objects_at_the_root_and_every_row_boundary() {
        use serde_json::json;
        let valid = json!({"schemaVersion":3,
            "mappings":[{"id":1,"romanized":"a","khmer":"ក","rank":0,"status":"approved"}],
            "words":[{"id":1,"khmer":"ក","rank":0,"status":"approved"},
                {"id":2,"khmer":"ខ","rank":0,"status":"approved"},
                {"id":3,"khmer":"កខ","rank":0,"status":"approved"}],
            "englishWords":[{"id":1,"text":"alpha","rank":0,"status":"approved"}],
            "structures":[{"wordId":3,"componentIds":[1,2]}]});
        let engine = Engine::from_json(&valid.to_string()).unwrap();
        assert_eq!(engine.convert("a").unwrap().candidates[0].khmer, "ក");
        assert_eq!(Engine::from_binary(engine.to_binary()).unwrap().convert("a").unwrap(), engine.convert("a").unwrap());
        assert!(Engine::from_json("{\"schemaVersion\":1,\"mappings\":[]}").is_ok());
        assert!(Engine::from_json("{\"schemaVersion\":2,\"mappings\":[],\"words\":[],\"structures\":null}").is_ok());
        assert!(Engine::from_json("[1,[],null,null,null,null]").is_err());
        assert!(Engine::from_json("{\"schemaVersion\":1,\"mappings\":[[1,\"a\",\"ក\",0,\"approved\"]]}").is_err());
        for (field, row) in [("mappings", json!([1,"a","ក",0,"approved"])),
            ("words", json!([1,"ក",0,"approved"])),
            ("englishWords", json!([1,"alpha",0,"approved"])),
            ("structures", json!([3,[1,2]]))] {
            let mut malformed = valid.clone();
            malformed[field][0] = row;
            assert!(Engine::from_json(&malformed.to_string()).is_err(), "{field}");
        }
        for json in [r#"{"schemaVersion":1,"schemaVersion":1,"mappings":[]}"#,
            r#"{"schemaVersion":1,"mappings":[{"id":1,"romanized":"a","khmer":"ក","rank":0,"rank":0,"status":"approved"}]}"#] {
            assert!(Engine::from_json(json).is_err(), "{json}");
        }
    }
}

fn valid_romanized(input: &str) -> bool {
    if input.is_empty() || input.len() > 120 {
        return false;
    }
    let mut previous_letter = false;
    for ch in input.chars() {
        if ch.is_ascii_lowercase() {
            previous_letter = true;
        } else if matches!(ch, ' ' | '\'' | '-') && previous_letter {
            previous_letter = false;
        } else {
            return false;
        }
    }
    previous_letter
}
fn valid_khmer(input: &str) -> bool {
    !input.is_empty()
        && input.chars().count() <= 240
        && input.trim() == input
        && input
            .chars()
            .all(|ch| ('\u{1780}'..='\u{17ff}').contains(&ch) || ch == ' ')
        && input
            .chars()
            .any(|ch| ('\u{1780}'..='\u{17b3}').contains(&ch))
}
fn valid_english(input: &str) -> bool {
    !input.is_empty()
        && input.len() <= 120
        && valid_romanized(&input.to_ascii_lowercase())
        && !input.contains(' ')
}

pub struct Engine {
    dictionary: compact::Dictionary,
    max_key_bytes: usize,
    lexicon: word_metadata::Lexicon,
    commonness: word_metadata::Commonness,
    shared_words: Option<std::sync::Arc<shared_words::SharedWords>>,
    personal_source: Option<std::sync::Arc<shared_words::PersonalSource>>,
    rule_context: context::RuleIndex,
    app_data: Option<app_data::Loaded>,
    sound_index: std::sync::OnceLock<sound_map::Index>,
    component_index: std::sync::OnceLock<components::Index>,
    component_definitions: std::sync::Arc<components::Definitions>,
}
impl Engine {
    /// Ready-to-use native engine with the unchanged distributed dictionaries.
    /// Opt-in so existing mobile/native builds never embed a second copy.
    #[cfg(feature = "bundled-dictionaries")]
    pub fn bundled() -> Result<Self, EngineError> {
        Self::from_app_data(include_bytes!("../../compiled-data/khmer.kmd"),
            include_bytes!("../../compiled-data/english.kmd"))
    }

    /// Validate and index a caller-supplied approved snapshot; no I/O is performed.
    /// Sound map (including approved parent repetition) is prepared once here,
    /// before the first typing query. Compact loaders perform the same setup.
    pub fn new(dataset: ApprovedDataset) -> Result<Self, EngineError> {
        if !matches!(
            (
                dataset.schema_version,
                dataset.words.is_some(),
                dataset.english_words.is_some()
            ),
            (1, false, false) | (2, true, false) | (3, true, true)
        ) {
            return Err(invalid(
                "Expected dataset v1, v2 with words, or v3 with words and englishWords.",
            ));
        }
        let mut english_words: BTreeMap<String, Vec<EnglishCandidate>> = BTreeMap::new();
        let mut english_ids = BTreeSet::new();
        for word in dataset.english_words.unwrap_or_default() {
            if word.id == 0
                || word.id > MAX_SAFE_INTEGER
                || !valid_rank(word.rank)
                || !valid_english(&word.text)
                || !english_ids.insert(word.id)
            {
                return Err(invalid(
                    "Invalid or duplicate approved English word fields.",
                ));
            }
            let candidate = EnglishCandidate {
                mapping_id: word.id,
                text: word.text.clone(),
                rank: word.rank,
                manual_only: false,
                word_alternative: false,
            };
            let entries = english_words
                .entry(word.text.to_ascii_lowercase())
                .or_default();
            if let Some(existing) = entries.iter_mut().find(|entry| entry.text == word.text) {
                if (candidate.rank, candidate.mapping_id) < (existing.rank, existing.mapping_id) {
                    *existing = candidate;
                }
            } else {
                entries.push(candidate);
            }
        }
        for entries in english_words.values_mut() {
            entries.sort_by(|a, b| {
                (&a.text, a.rank, a.mapping_id).cmp(&(&b.text, b.rank, b.mapping_id))
            });
        }
        let mut words: BTreeMap<String, Candidate> = BTreeMap::new();
        // Keep every raw word identity before deduplicating Khmer completion
        // outputs. Matching both identity and target proves the reusable boundary;
        // a mapping to the same text under some other ID is not sufficient.
        let mut word_ids = BTreeMap::new();
        for word in dataset.words.unwrap_or_default() {
            if word.id == 0
                || word.id > MAX_SAFE_INTEGER
                || !valid_rank(word.rank)
                || !valid_khmer(&word.khmer)
                || word_ids.insert(word.id, word.khmer.clone()).is_some()
            {
                return Err(invalid("Invalid or duplicate approved word fields."));
            }
            let candidate = Candidate {
                mapping_id: Some(word.id),
                mapping_ids: Vec::new(),
                khmer: word.khmer.clone(),
                rank: word.rank,
            };
            words
                .entry(word.khmer)
                .and_modify(|current| {
                    if (candidate.rank, candidate.mapping_id) < (current.rank, current.mapping_id) {
                        *current = candidate.clone();
                    }
                })
                .or_insert(candidate);
        }
        let mut disabled = BTreeSet::new();
        for word in dataset.sound_map_disabled_words.unwrap_or_default() {
            if !words.contains_key(&word) || !disabled.insert(word) {
                return Err(invalid("Sound map policy requires distinct approved Khmer words."));
            }
        }
        let mut ids = BTreeSet::new();
        let mut pairs = BTreeSet::new();
        let mut index: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
        let mut joined_candidates = Vec::new();
        for row in dataset.mappings {
            if row.id == 0
                || row.id > MAX_SAFE_INTEGER
                || !valid_rank(row.rank)
                || !valid_romanized(&row.romanized)
                || !valid_khmer(&row.khmer)
            {
                return Err(invalid("Invalid approved mapping fields."));
            }
            if !ids.insert(row.id) || !pairs.insert((row.romanized.clone(), row.khmer.clone())) {
                return Err(invalid("Duplicate mapping ID or Romanized/Khmer pair."));
            }
            let candidate = Candidate {
                mapping_id: Some(row.id),
                mapping_ids: Vec::new(),
                khmer: row.khmer,
                rank: row.rank,
            };
            if row.romanized.contains(' ') && word_ids.get(&row.id) == Some(&candidate.khmer) {
                joined_candidates.push((row.romanized.replace(' ', ""), candidate.clone()));
            }
            index.entry(row.romanized).or_default().push(candidate);
        }
        // An explicitly saved joined spelling retains its own identity. If
        // several spaced aliases derive the same output, use the best ranked
        // source deterministically, without dropping other approved outputs.
        joined_candidates.sort_by_key(|(_, row)| (row.rank, row.mapping_id));
        let mut joined = BTreeSet::new();
        for (key, candidate) in joined_candidates {
            let entries = index.entry(key.clone()).or_default();
            if !entries.iter().any(|row| row.khmer == candidate.khmer) {
                joined.insert((key, candidate.mapping_id.unwrap()));
                entries.push(candidate);
            }
        }
        for values in index.values_mut() {
            values.sort_by_key(|row| (row.rank, row.mapping_id));
        }
        let mut structures = BTreeSet::new();
        for structure in dataset.structures.unwrap_or_default() {
            if !(2..=64).contains(&structure.component_ids.len()) { return Err(invalid("Connected words require 2–64 components.")); }
            let parent = word_ids.get(&structure.word_id).ok_or_else(|| invalid("Missing connected word identity."))?;
            let children = structure.component_ids.iter().map(|id| word_ids.get(id).cloned().ok_or_else(|| invalid("Missing component word identity."))).collect::<Result<Vec<_>, _>>()?;
            if children.concat() != *parent || children.iter().any(|text| text.contains(' ')) {
                return Err(invalid("Components must join exactly to the connected word."));
            }
            structures.insert((parent.clone(), children));
        }
        let dictionary = compact::compile(dataset.schema_version, index, words, english_words, joined, structures, disabled)?;
        let sound_index = std::sync::OnceLock::from(sound_map::Index::new(&dictionary));
        let rule_context = sound_map::starting_rule_context(&dictionary);
        Ok(Self {
            max_key_bytes: dictionary.max_key_bytes,
            dictionary,
            lexicon: Default::default(),
            commonness: Default::default(),
            shared_words: None,
            personal_source: None,
            rule_context,
            app_data: None,
            sound_index,
            component_index: Default::default(),
            component_definitions: Default::default(),
        })
    }
    pub fn from_binary(bytes: Vec<u8>) -> Result<Self, EngineError> {
        let dictionary = compact::Dictionary::from_bytes(bytes)?;
        let sound_index = std::sync::OnceLock::from(sound_map::Index::new(&dictionary));
        let rule_context = sound_map::starting_rule_context(&dictionary);
        Ok(Self {
            max_key_bytes: dictionary.max_key_bytes,
            dictionary,
            lexicon: Default::default(),
            commonness: Default::default(),
            shared_words: None,
            personal_source: None,
            rule_context,
            app_data: None,
            sound_index,
            component_index: Default::default(),
            component_definitions: Default::default(),
        })
    }
    pub fn to_binary(&self) -> Vec<u8> {
        self.dictionary.data.as_ref().clone()
    }
    pub fn compiled_bytes(&self) -> &[u8] {
        &self.dictionary.data
    }
    /// Build a pre-sorted, distinct English vocabulary without materializing JSON
    /// or a BTreeMap. Intended for release/build tools; no I/O occurs in the core.
    pub fn compile_sorted_english(
        words: impl IntoIterator<Item = ApprovedEnglishWord>,
    ) -> Result<Self, EngineError> {
        let dictionary = compact::compile_sorted_english(words)?;
        Ok(Self {
            max_key_bytes: 0,
            dictionary,
            lexicon: Default::default(),
            commonness: Default::default(),
            shared_words: None,
            personal_source: None,
            rule_context: Default::default(),
            app_data: None,
            sound_index: Default::default(),
            component_index: Default::default(),
            component_definitions: Default::default(),
        })
    }
    pub fn stats_json(&self) -> String {
        serde_json::json!({"mappings": self.dictionary.mapping_count, "words": self.dictionary.khmer.len, "englishWords": self.dictionary.english.len, "encodedBytes": self.dictionary.data.len()}).to_string()
    }
    fn set_commonness(&mut self, commonness: word_metadata::Commonness) {
        if let Some(index) = self.sound_index.get_mut() { index.set_commonness(&commonness); }
        self.dictionary.set_completion_commonness(&commonness);
        self.commonness = commonness;
    }
    fn suggestion_level(&self, suggestion: &Suggestion) -> u8 {
        match &suggestion.candidate {
            SuggestionCandidate::Khmer(c) => self.commonness.level("khmer", &c.khmer),
            SuggestionCandidate::English(c) => self.commonness.level("english", &c.text),
        }
    }
    fn mark_tap_only(&self, suggestions: &mut [Suggestion], extension: Option<&Engine>, _input: &str) {
        for suggestion in suggestions {
            let (language, bundled) = match &suggestion.candidate {
                SuggestionCandidate::Khmer(word) => ("khmer", self.dictionary.khmer.contains(&word.khmer)),
                SuggestionCandidate::English(word) => ("english", self.dictionary.english.contains(&word.text.to_ascii_lowercase())),
            };
            // A personal duplicate cannot weaken the bundled word's gate.
            // Unrated/legacy words keep their existing behavior.
            let source = if bundled { self } else { extension.unwrap_or(self) };
            suggestion.tap_only |= matches!(source.commonness.level(language, suggestion_output(suggestion)), 4 | 5);

        }
    }
    fn blocked_mapping(&self, row: &compact::Row<'_>) -> bool {
        if contains_profanity(row.key, ProfanityLanguage::Auto)
            || contains_profanity(self.dictionary.pool.get(row.target), ProfanityLanguage::Auto)
        {
            return true;
        }
        // Joined lookups inherit their canonical alias's policy as well as its
        // identity. Removing a space must not bypass a blocked English token.
        row.source.is_some_and(|source| {
            let mut cursor = self.dictionary.romanized.cursor(source);
            let original = cursor.next().expect("validated compact dictionary").unwrap();
            contains_profanity(original.key, ProfanityLanguage::Auto)
        })
    }
    fn exact_candidates(&self, input: &str, limit: usize) -> Vec<Candidate> {
        let index = &self.dictionary.romanized;
        let mut cursor = index.cursor(index.bound(input, false));
        let mut candidates = Vec::new();
        while let Some(row) = cursor.next().expect("validated compact dictionary") {
            if row.key != input || candidates.len() >= limit {
                break;
            }
            let target = self.dictionary.pool.get(row.target);
            if self.blocked_mapping(&row) {
                continue;
            }
            candidates.push(Candidate {
                mapping_id: Some(row.id),
                mapping_ids: Vec::new(),
                khmer: target.to_owned(),
                rank: row.rank,
            });
        }
        if index.has_local_ranks() { candidates.sort_by_key(|candidate| (candidate.rank, candidate.mapping_id)); }
        candidates
    }
    /// This policy is applied to the complete query only. The raw exact
    /// lookup is also used by composition and must keep its original order.
    fn saved_match_priorities(&self, input: &str) -> BTreeMap<u64, i64> {
        let index = &self.dictionary.romanized;
        let mut cursor = index.cursor(index.bound(input, false));
        let mut priorities = BTreeMap::new();
        while let Some(row) = cursor.next().expect("validated compact dictionary") {
            if row.key != input { break; }
            // Joined spellings inherit an independently approved source key;
            // they do not acquire canonical saved-input match policies.
            if row.source.is_some() || self.blocked_mapping(&row) { continue; }
            if let Some(priority) = sound_map::saved_input_match_priority(input, self.dictionary.pool.get(row.target)) {
                priorities.insert(row.id, priority);
            }
        }
        priorities
    }
    pub fn from_json(json: &str) -> Result<Self, EngineError> {
        let data: ApprovedDataset = parse_request_object(json, "approved dataset JSON")?;
        Self::new(data)
    }
    pub fn convert(&self, input: &str) -> Result<ConversionResult, EngineError> {
        if let Some(result) = self.shared_query(None, |extension, layer| self.convert_with_extension_base(extension, input, layer)) { return result; }
        self.convert_base(input)
    }
    fn convert_base(&self, input: &str) -> Result<ConversionResult, EngineError> {
        let normalized_input = normalize(input)?;
        if contains_profanity(&normalized_input, ProfanityLanguage::Auto) {
            return Ok(ConversionResult { input: input.to_owned(), normalized_input, candidates: Vec::new() });
        }
        let mut candidates = self.exact_candidates(&normalized_input, usize::MAX);
        if candidates.is_empty() {
            candidates = self.compose(&normalized_input);
        } else {
            let priorities = self.saved_match_priorities(&normalized_input);
            if !priorities.is_empty() || self.commonness.has_levels("khmer") {
                candidates.sort_by_key(|candidate| (std::cmp::Reverse(
                    candidate.mapping_id.and_then(|id| priorities.get(&id).copied()).unwrap_or(0)),
                    self.commonness.level("khmer", &candidate.khmer)));
            }
        }
        candidates.retain(|candidate| !self.dictionary.excluded_khmer.contains(&candidate.khmer)
            && !self.input_excluded(&normalized_input, &candidate.khmer, None));
        Ok(ConversionResult {
            input: input.to_owned(),
            normalized_input,
            candidates,
        })
    }
    pub fn convert_json(&self, input: &str) -> Result<String, EngineError> {
        serde_json::to_string(&self.convert(input)?).map_err(|e| invalid(&e.to_string()))
    }

    /// Live typing: retain complete conversion results, then fill remaining
    /// slots with approved whole-input completions. Seeking the ordered index avoids a
    /// dictionary scan on every keystroke. Prefix candidates are ranked before
    /// truncation, while complete conversion results retain their priority.
    pub fn suggest(&self, input: &str) -> Result<SuggestionResult, EngineError> {
        self.suggest_with_features(input, SuggestionFeatures::default())
    }
    /// Features are caller preferences, independent of data and review decisions.
    pub fn suggest_with_features(
        &self,
        input: &str,
        features: SuggestionFeatures,
    ) -> Result<SuggestionResult, EngineError> {
        if let Some(result) = self.shared_query(None, |extension, layer|
            self.suggest_with_extension_base(extension, input, features, &TypingContext::default(), layer)) {
            return result.map(|result| result.result);
        }
        let mut result = self.suggest_with_features_and_policy(input, features, None, None)?;
        self.mark_tap_only(&mut result.candidates, None, &result.normalized_input);
        Ok(result)
    }
    fn suggest_with_features_and_policy(&self, input: &str, features: SuggestionFeatures,
        base_policy: Option<&compact::Dictionary>, context: Option<&context::Scores>) -> Result<SuggestionResult, EngineError> {
        let normalized_input = normalize(input)?;
        let prefix = &normalized_input;
        // Check completed candidates, not an unfinished word: ក្ត must still
        // be able to complete to the neutral ក្តៅ. Raw input stays untouched.
        if features.mode == TypingMode::English {
            // Complete one active word only. No translation, next-word prediction,
            // or case changes to the approved spelling are inferred.
            let mut candidates = if !features.english_completion
                || prefix.is_empty()
                || input.chars().last().is_some_and(char::is_whitespace)
                || !prefix
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || matches!(ch, '\'' | '-'))
            {
                Vec::new()
            } else {
                self.english_completions(prefix)
            };
            if features.english_autocorrection {
                if let Some(corrections) = self.english_correction(input, prefix, None) {
                    candidates.splice(0..0, corrections);
                    candidates.truncate(MAX_COMPLETIONS);
                }
            }
            if features.english_completion {
                self.preserve_english_surface(&mut candidates, input, prefix);
                self.english_word_choices(&mut candidates, input, prefix, None, None);
            }
            Self::guard_english_contractions(&mut candidates, input, prefix, None);
            candidates.truncate(MAX_COMPLETIONS);
            return Ok(SuggestionResult {
                input: input.to_owned(),
                normalized_input,
                candidates,
                kind: Some(SuggestionKind::EnglishCompletion),
                disabled_feature: (!features.english_completion)
                    .then_some(SuggestionFeature::EnglishCompletion),
            });
        }
        if !prefix.is_empty()
            && prefix
                .chars()
                .all(|ch| ('\u{1780}'..='\u{17ff}').contains(&ch))
        {
            // A separator finishes the active word; this is not next-word prediction.
            let candidates = if !features.khmer_completion
                || input.chars().last().is_some_and(char::is_whitespace)
            {
                Vec::new()
            } else {
                self.ranked_completions(&self.dictionary.khmer, prefix, context)
            };
            return Ok(SuggestionResult {
                input: input.to_owned(),
                normalized_input,
                candidates,
                kind: Some(SuggestionKind::KhmerCompletion),
                disabled_feature: (!features.khmer_completion)
                    .then_some(SuggestionFeature::KhmerCompletion),
            });
        }
        if !features.romanized {
            return Ok(SuggestionResult {
                input: input.to_owned(),
                normalized_input,
                candidates: Vec::new(),
                kind: None,
                disabled_feature: Some(SuggestionFeature::Romanized),
            });
        }
        let result = self.convert(input)?;
        let saved_priorities = self.saved_match_priorities(&result.normalized_input);
        // Re-score the independent saved and generated paths together from
        // their original tie order, rather than a conversion-only ordering.
        let mut saved = if saved_priorities.is_empty() { result.candidates }
            else { self.exact_candidates(&result.normalized_input, usize::MAX) };
        saved.retain(|word| !self.input_excluded(prefix, &word.khmer, base_policy));
        let mut candidates: Vec<_> = saved
            .into_iter()
            .map(|candidate| Suggestion { tap_only: false, preferred_input: None,
                candidate: SuggestionCandidate::Khmer(candidate),
                completion: None,
                correction: None,
            })
            .collect();
        let sounds = if features.sound_map && !contains_profanity(&result.normalized_input, ProfanityLanguage::Auto) {
            self.sound_index.get_or_init(|| sound_map::Index::new(&self.dictionary))
                .suggest_with_context_policy(&result.normalized_input, !input.chars().last().is_some_and(char::is_whitespace), base_policy, context)
        } else { Vec::new() };
        let saved_outputs: BTreeSet<_> = candidates.iter().map(|hit| suggestion_output(hit).to_owned()).collect();
        for sound in sounds.iter().map(|hit| &hit.suggestion).filter(|s| s.completion.is_none()) {
            if !candidates.iter().any(|c| suggestion_output(c) == suggestion_output(sound)) {
                candidates.push(sound.clone());
            }
        }
        let local_ranks = candidates.iter().any(|hit| self.candidate_has_local_rank(&hit.candidate));
        if !saved_priorities.is_empty() || self.commonness.has_levels("khmer") || local_ranks {
            let sound_priorities: BTreeMap<_, _> = sounds.iter().filter(|hit| hit.suggestion.completion.is_none())
                .map(|hit| (suggestion_output(&hit.suggestion), hit.priority)).collect();
            candidates.sort_by_key(|suggestion| (std::cmp::Reverse(match &suggestion.candidate {
                SuggestionCandidate::Khmer(candidate) => candidate.mapping_id
                    .and_then(|id| saved_priorities.get(&id).copied())
                    .map(|priority| sound_priorities.get(candidate.khmer.as_str()).map_or(priority, |sound| priority.max(*sound)))
                    .unwrap_or_else(|| if saved_outputs.contains(&candidate.khmer) { 0 }
                        else { sound_priorities.get(candidate.khmer.as_str()).copied().unwrap_or(0) }),
                _ => 0,
            }), self.suggestion_level(suggestion), !saved_outputs.contains(suggestion_output(suggestion)),
                if local_ranks { suggestion.candidate.rank() } else { 0 }));
        }
        let mut completions = Vec::new();
        if candidates.len() < MAX_COMPLETIONS && !result.normalized_input.is_empty() {
            // A trailing space starts another word; "ou " must not complete "oun".
            let mut prefix = result.normalized_input.clone();
            if input.chars().last().is_some_and(char::is_whitespace) {
                prefix.push(' ');
            }
            for completion in self.ranked_completions(&self.dictionary.romanized, &prefix, context) {
                if candidates.iter().any(|existing| suggestion_output(existing) == suggestion_output(&completion)) {
                    continue;
                }
                completions.push((completion, 0));
            }
        }
        for sound in sounds.into_iter().filter(|hit| hit.suggestion.completion.is_some()) {
            if candidates.iter().any(|c| suggestion_output(c) == suggestion_output(&sound.suggestion)) { continue; }
            if let Some((_, priority)) = completions.iter_mut().find(|(hit, _)| suggestion_output(hit) == suggestion_output(&sound.suggestion)) {
                *priority = (*priority).max(sound.priority);
            } else {
                completions.push((sound.suggestion, sound.priority));
            }
        }
        let local_completion_ranks = completions.iter().any(|(hit, _)| self.candidate_has_local_rank(&hit.candidate));
        if context.is_some() || self.commonness.has_levels("khmer") || local_completion_ranks {
            completions.sort_by_key(|(hit, priority)| (std::cmp::Reverse(*priority),
                std::cmp::Reverse(context::score(context, suggestion_output(hit))), self.suggestion_level(hit),
                hit.candidate.rank(), hit.completion.clone(), hit.candidate.mapping_id(),
                suggestion_output(hit).to_owned()));
        }
        candidates.extend(completions.into_iter().take(MAX_COMPLETIONS.saturating_sub(candidates.len())).map(|(hit, _)| hit));
        let mut result = SuggestionResult {
            input: result.input,
            normalized_input: result.normalized_input,
            candidates,
            kind: None,
            disabled_feature: None,
        };
        if features.khmer_completion {
            self.add_recoveries(&mut result, features.sound_map, base_policy, context);
        }
        self.filter_excluded_inputs(&mut result.candidates, &result.normalized_input, base_policy);
        Ok(result)
    }
    /// Context ranks existing exact and prefix matches before completion limits.
    /// Empty input offers reviewed lexical continuations. Neither those nor typo
    /// recoveries enter exact_candidates or modify saved ranks/approvals.
    pub fn suggest_with_context(&self, input: &str, features: SuggestionFeatures, context: &TypingContext) -> Result<ContextualSuggestionResult, EngineError> {
        self.suggest_with_context_using(input,features,context,None)
    }
    fn suggest_with_context_using(&self, input: &str, features: SuggestionFeatures, context: &TypingContext,
        scores: Option<&context::Scores>) -> Result<ContextualSuggestionResult, EngineError> {
        if let Some(result) = self.shared_query(None, |extension, layer| self.suggest_with_extension_using(extension, input, features, context, layer, scores)) { return result; }
        self.suggest_with_context_base(input, features, context, scores)
    }
    fn suggest_with_context_base(&self, input: &str, features: SuggestionFeatures, context: &TypingContext,
        scores: Option<&context::Scores>) -> Result<ContextualSuggestionResult, EngineError> {
        let mut result = self.suggest_with_context_and_policy(input, features, context, None, scores)?;
        self.mark_tap_only(&mut result.result.candidates, None, &result.result.normalized_input);
        Ok(result)
    }
    fn suggest_with_context_and_policy(&self, input: &str, features: SuggestionFeatures,
        context: &TypingContext, base_policy: Option<&compact::Dictionary>, supplied_scores: Option<&context::Scores>) -> Result<ContextualSuggestionResult, EngineError> {
        if context.english_word_context.as_ref().is_some_and(|value|
            value.before.chars().count() > 512 || value.after.chars().count() > 128) {
            return Err(invalid("English word context exceeds its bounded window."));
        }
        if context.previous_words.len() > context::MAX_WORDS || context.previous_words.iter().any(|word| !valid_khmer(word) || word.contains(' ')) {
            return Err(invalid("Context requires at most 63 complete Khmer words."));
        }
        let computed = if supplied_scores.is_none() && features.mode == TypingMode::Khmer {
            self.component_context_scores(&context.previous_words,None)
        } else { Default::default() };
        let scores = supplied_scores.unwrap_or(&computed);
        let scores = (!scores.is_empty()).then_some(scores);
        if input.is_empty() && features.mode == TypingMode::Khmer && features.khmer_completion {
            return Ok(ContextualSuggestionResult {
                result: SuggestionResult {
                    input: input.into(), normalized_input: String::new(),
                    candidates: self.next_words(scores), kind: Some(SuggestionKind::NextWord), disabled_feature: None,
                }, exact_candidates: Vec::new(), next_letter_weights: BTreeMap::new(),
            });
        }
        let mut result = self.suggest_with_features_and_policy(input, features, base_policy, scores)?;
        let mut exact_candidates = Vec::new();
        if result.kind == Some(SuggestionKind::KhmerCompletion) {
            exact_candidates = result.candidates.iter().filter_map(|suggestion| match &suggestion.candidate {
                SuggestionCandidate::Khmer(candidate) if candidate.khmer == result.normalized_input => Some(candidate.clone()),
                _ => None,
            }).collect();
        }
        if features.mode == TypingMode::Khmer && features.romanized && result.kind.is_none() && !contains_profanity(&result.normalized_input, ProfanityLanguage::Auto) {
            exact_candidates = self.exact_candidates(&result.normalized_input, usize::MAX);
            // A saved exact/joined mapping is its own neutral match, even when
            // a weaker Sound map path produces the same output.
            let saved_outputs: BTreeSet<_> = exact_candidates.iter().map(|c| c.khmer.clone()).collect();
            let saved_priorities = self.saved_match_priorities(&result.normalized_input);
            if features.sound_map {
                for suggestion in &result.candidates {
                    if suggestion.completion.is_none() {
                        if let SuggestionCandidate::Khmer(candidate) = &suggestion.candidate {
                            if candidate.mapping_id.is_some() && !exact_candidates.iter().any(|c| c.khmer == candidate.khmer) {
                                exact_candidates.push(candidate.clone());
                            }
                        }
                    }
                }
            }
            let local_ranks = exact_candidates.iter().any(|word|
                self.candidate_has_local_rank(&SuggestionCandidate::Khmer(word.clone())));
            if scores.is_some() || !saved_priorities.is_empty() || self.commonness.has_levels("khmer") || local_ranks {
                let priorities = if features.sound_map {
                    self.sound_index.get().map(|sounds| sounds.exact_priorities(&result.normalized_input, !saved_priorities.is_empty(), base_policy)).unwrap_or_default()
                } else { BTreeMap::new() };
                exact_candidates.sort_by_key(|candidate| (
                    std::cmp::Reverse(candidate.mapping_id.and_then(|id| saved_priorities.get(&id).copied())
                        .map(|priority| priorities.get(&candidate.khmer).map_or(priority, |sound| priority.max(*sound)))
                        .unwrap_or_else(|| if saved_outputs.contains(&candidate.khmer) { 0 } else { priorities.get(&candidate.khmer).copied().unwrap_or(0) })),
                    std::cmp::Reverse(context::score(scores, &candidate.khmer)),
                    // Prefer common words across both match sources; saved
                    // spellings break equal-commonness ties.
                    self.commonness.level("khmer", &candidate.khmer),
                    !saved_outputs.contains(&candidate.khmer),
                    if local_ranks { candidate.rank } else { 0 },
                ));
            }
            if !exact_candidates.is_empty() {
                let ids: BTreeSet<_> = exact_candidates.iter().map(|c| c.mapping_id).collect();
                let preferred: BTreeMap<_,_> = result.candidates.iter().filter_map(|hit|
                    hit.preferred_input.as_ref().map(|input|(suggestion_output(hit).to_owned(),input.clone()))).collect();
                let mut candidates = exact_candidates.iter().cloned().map(|candidate| Suggestion { tap_only: false,
                    preferred_input: (!saved_outputs.contains(&candidate.khmer)).then(||preferred.get(&candidate.khmer).cloned()).flatten(),
                    candidate: SuggestionCandidate::Khmer(candidate), completion: None, correction: None }).collect::<Vec<_>>();
                candidates.extend(result.candidates.into_iter().filter(|candidate| candidate.completion.is_some() || !ids.contains(&candidate.candidate.mapping_id())));
                result.candidates = candidates;
            }
        }
        self.filter_excluded_inputs(&mut result.candidates, &result.normalized_input, base_policy);
        exact_candidates.retain(|word| !self.input_excluded(&result.normalized_input, &word.khmer, base_policy));
        self.place_recoveries(&mut result.candidates, None, features.mode, &result.normalized_input);
        if features.mode == TypingMode::English {
            Self::prioritize_english_corrections(&mut result.candidates, &result.normalized_input);
        }
        if features.mode == TypingMode::English && features.english_completion {
            self.english_word_choices(&mut result.candidates, input, &result.normalized_input,
                context.english_word_context.as_ref(), None);
        }
        if features.mode == TypingMode::English {
            Self::guard_english_contractions(&mut result.candidates, input, &result.normalized_input,
                context.english_word_context.as_ref());
            result.candidates.truncate(MAX_COMPLETIONS);
        }
        let next_letter_weights = self.next_letter_weights(input, features, None);
        Ok(ContextualSuggestionResult { result, exact_candidates, next_letter_weights })
    }
    pub fn suggest_with_context_json(&self, input: &str, features: SuggestionFeatures, context: &TypingContext) -> Result<String, EngineError> {
        serde_json::to_string(&self.suggest_with_context(input, features, context)?).map_err(|e| invalid(&e.to_string()))
    }
    /// Query a separately user-saved dictionary without modifying the base.
    /// Exact choices and complete conversions precede prefixes; duplicates keep base
    /// identities, and personal completions precede other base completions.
    pub fn suggest_with_extension(&self, extension: &Engine, input: &str, features: SuggestionFeatures,
        context: &TypingContext) -> Result<ContextualSuggestionResult, EngineError> {
        self.shared_query(Some(extension), |extension, layer| self.suggest_with_extension_base(extension, input, features, context, layer)).expect("extension supplied")
    }
    fn suggest_with_extension_base(&self, extension: &Engine, input: &str, features: SuggestionFeatures,
        context: &TypingContext, layer: bool) -> Result<ContextualSuggestionResult, EngineError> {
        self.suggest_with_extension_using(extension,input,features,context,layer,None)
    }
    fn suggest_with_extension_using(&self, extension: &Engine, input: &str, features: SuggestionFeatures,
        context: &TypingContext, layer: bool, supplied_scores: Option<&context::Scores>) -> Result<ContextualSuggestionResult, EngineError> {
        // English correction must evaluate the union, including alternatives
        // that were ambiguous within either source and would otherwise vanish.
        let mut dictionary_features = features;
        if features.mode == TypingMode::English { dictionary_features.english_autocorrection = false; }
        // Compute one cached union view for both sources. A personal component
        // may resolve or make a formerly clear base split ambiguous.
        let computed = if supplied_scores.is_none() && features.mode == TypingMode::Khmer {
            self.component_context_scores(&context.previous_words,Some(extension))
        } else { Default::default() };
        let scores = supplied_scores.unwrap_or(&computed);
        let mut base = if layer { self.suggest_with_context_using(input, dictionary_features, context, Some(scores))? }
            else { self.suggest_with_context_base(input, dictionary_features, context, Some(scores))? };
        let mut extra = extension.suggest_with_context_and_policy(input, dictionary_features, context, Some(&self.dictionary), Some(scores))?;
        self.filter_excluded_inputs(&mut base.result.candidates, &base.result.normalized_input, Some(&extension.dictionary));
        self.filter_excluded_inputs(&mut extra.result.candidates, &base.result.normalized_input, Some(&extension.dictionary));
        base.exact_candidates.retain(|word| !self.input_excluded(&base.result.normalized_input, &word.khmer, Some(&extension.dictionary)));
        extra.exact_candidates.retain(|word| !self.input_excluded(&base.result.normalized_input, &word.khmer, Some(&extension.dictionary)));
        let mut preferred: BTreeMap<_,_> = extra.result.candidates.iter().chain(&base.result.candidates).filter_map(|hit|
            hit.preferred_input.as_ref().map(|input|(suggestion_output(hit).to_owned(),input.clone()))).collect();
        if !preferred.is_empty() {
            for hit in base.result.candidates.iter().chain(&extra.result.candidates) {
                if hit.preferred_input.is_none() && hit.completion.is_none() && hit.correction.is_none() {
                    preferred.remove(suggestion_output(hit));
                }
            }
            // An exact spelling saved in either dictionary is authoritative,
            // even when a generated alternate count yields the same output.
            for saved in self.exact_candidates(&base.result.normalized_input,usize::MAX).into_iter()
                .chain(extension.exact_candidates(&base.result.normalized_input,usize::MAX)) {
                preferred.remove(&saved.khmer);
            }
        }
        let competing_exact = !base.exact_candidates.is_empty() && !extra.exact_candidates.is_empty();
        let mut exact_outputs: BTreeSet<_> = base.exact_candidates.iter().map(|c| c.khmer.clone()).collect();
        for candidate in extra.exact_candidates {
            if exact_outputs.insert(candidate.khmer.clone()) { base.exact_candidates.push(candidate); }
        }
        self.rank_local_exact(&mut base.exact_candidates, extension, &base.result.normalized_input, features, scores);
        // A saved/strict match in the personal layer also precedes a newly
        // tolerated base match, even when neither dictionary has local ranks.
        base.exact_candidates.sort_by_key(|candidate|preferred.contains_key(&candidate.khmer));
        if competing_exact && features.mode == TypingMode::Khmer && features.sound_map {
            self.rank_added_exact(&mut base.exact_candidates, extension, &base.result.normalized_input);
        }
        let mut candidates = base.exact_candidates.iter().cloned().map(|candidate| Suggestion { tap_only: false,
            preferred_input: preferred.get(&candidate.khmer).cloned(),
            candidate: SuggestionCandidate::Khmer(candidate), completion: None, correction: None,
        }).collect::<Vec<_>>();
        let mut outputs: BTreeSet<_> = candidates.iter().map(|s| suggestion_output(s).to_owned()).collect();
        // Complete compositions are not approved exact_candidates, but must
        // precede prefixes and survive the limit. English complete words carry
        // a completion key, so recognize them by input instead. Base identities
        // win duplicates without promoting compositions to exact matches.
        let skip_runtime_english_exact = features.mode == TypingMode::English &&
            (self.dictionary.english.contains(&base.result.normalized_input)
                || extension.dictionary.english.contains(&base.result.normalized_input)
                || self.excluded_english_surface(&base.result.normalized_input, Some(extension)));
        for candidate in base.result.candidates.iter().chain(&extra.result.candidates) {
            // A base/extension preservation identity must never shadow a real
            // exact spelling saved in the other dictionary, or reintroduce
            // a selected surface explicitly withheld in either view.
            if skip_runtime_english_exact && Self::runtime_english_exact(candidate, &base.result.normalized_input) { continue; }
            if (candidate.completion.is_none() || matches!(&candidate.candidate, SuggestionCandidate::English(word)
                if word.text.eq_ignore_ascii_case(&base.result.normalized_input)))
                && outputs.insert(suggestion_output(candidate).to_owned()) {
                candidates.push(candidate.clone());
            }
        }
        let complete_count = candidates.len();
        // Prefer ordinary matches over recovered copies of the same output.
        // Stable partition retains personal-first order within each class.
        let mut remaining = extra.result.candidates;
        remaining.extend(base.result.candidates);
        remaining.sort_by_key(|candidate| (candidate.correction.is_some(),
            features.mode==TypingMode::Khmer && sound_map::omitted_apostrophe_completion(candidate,&base.result.normalized_input)));
        for candidate in remaining {
            if skip_runtime_english_exact && Self::runtime_english_exact(&candidate, &base.result.normalized_input) { continue; }
            if outputs.insert(suggestion_output(&candidate).to_owned()) { candidates.push(candidate); }
        }
        // A complete input in either dictionary makes recovery unnecessary.
        // Do not let a base typo suggestion outrank a valid personal spelling.
        if complete_count > 0 {
            candidates.retain(|candidate| candidate.correction.is_none());
        }
        if features.mode == TypingMode::English && features.english_autocorrection {
            if let Some(corrections) = self.english_correction(input, &base.result.normalized_input, Some(extension)) {
                candidates.splice(0..0, corrections);
            }
        }
        base.result.candidates = candidates;
        self.rank_local_merged(&mut base.result.candidates, extension, &base.result.normalized_input, features, scores);
        if features.mode==TypingMode::Khmer {
            // Local ranks/context preserve their order inside each class;
            // new apostrophe completions cannot overtake ordinary prefixes.
            base.result.candidates.sort_by_key(|hit| (hit.completion.is_some(),hit.correction.is_some(),
                sound_map::omitted_apostrophe_completion(hit,&base.result.normalized_input)));
        }
        // Preserve each language's correction policy before the display cap.
        // Complete alternatives still survive the normal cap.
        self.place_recoveries(&mut base.result.candidates, Some(extension), features.mode, &base.result.normalized_input);
        if features.mode == TypingMode::English {
            Self::prioritize_english_corrections(&mut base.result.candidates, &base.result.normalized_input);
        }
        if features.mode == TypingMode::English && features.english_completion {
            self.english_word_choices(&mut base.result.candidates, input, &base.result.normalized_input,
                context.english_word_context.as_ref(), Some(extension));
        }
        if features.mode == TypingMode::English {
            Self::guard_english_contractions(&mut base.result.candidates, input, &base.result.normalized_input,
                context.english_word_context.as_ref());
        }
        base.result.candidates.truncate(MAX_COMPLETIONS.max(complete_count));
        Self::align_exact_suggestions(&mut base.result.candidates, &base.exact_candidates);
        base.next_letter_weights = self.next_letter_weights(input, features, Some(extension));
        self.mark_tap_only(&mut base.result.candidates, Some(extension), &base.result.normalized_input);
        Ok(base)
    }
    pub fn suggest_with_extension_json(&self, extension: &Engine, input: &str, features: SuggestionFeatures,
        context: &TypingContext) -> Result<String, EngineError> {
        serde_json::to_string(&self.suggest_with_extension(extension, input, features, context)?)
            .map_err(|e| invalid(&e.to_string()))
    }
    pub fn suggest_json(&self, input: &str) -> Result<String, EngineError> {
        serde_json::to_string(&self.suggest(input)?).map_err(|e| invalid(&e.to_string()))
    }
    pub fn suggest_with_features_json(
        &self,
        input: &str,
        features: SuggestionFeatures,
    ) -> Result<String, EngineError> {
        serde_json::to_string(&self.suggest_with_features(input, features)?)
            .map_err(|e| invalid(&e.to_string()))
    }

    fn ranked_completions(&self, index: &compact::Index, prefix: &str,
        context: Option<&context::Scores>) -> Vec<Suggestion> {
        self.ranked_completions_with_limit(index, prefix, context, MAX_COMPLETIONS)
    }
    fn ranked_completions_with_limit(&self, index: &compact::Index, prefix: &str,
        context: Option<&context::Scores>, limit: usize) -> Vec<Suggestion> {
        let romanized = index.kind == compact::Kind::Romanized;
        debug_assert!(romanized || index.kind == compact::Kind::Khmer);
        let (start, end) = index.prefix(prefix);
        let exact_end = index.bound(prefix, true);
        // Exact word, context, commonness, saved rank, then lexical key/ID ties.
        // Keep only the best twenty distinct outputs, even for a broad prefix.
        let mut best: BinaryHeap<(bool, std::cmp::Reverse<context::Score>, u8, i32, String, u64, String)> =
            BinaryHeap::with_capacity(limit);
        let mut kept_outputs = BTreeSet::new();
        let mut cursor = index.cursor(start);
        while cursor.next < end {
            if context.is_none() && !index.has_local_ranks() && cursor.next % compact::BLOCK == 0
                && cursor.next + compact::BLOCK <= end
                && cursor.next >= exact_end
                && best.len() == limit
            {
                let worst = best.peek().unwrap();
                // Equal ranks cannot win either: the scan has already passed
                // every kept lexical key (and same-key IDs are ordered).
                // Never prune a block containing the exact word.
                if worst.0 && index.completion_block_min(cursor.next / compact::BLOCK,
                    self.commonness.has_levels("khmer")).is_some_and(|(level, rank, _)| (level, rank) >= (worst.2, worst.3)) {
                    cursor = index.cursor(cursor.next + compact::BLOCK);
                    continue;
                }
            }
            let row = cursor.next().expect("validated compact dictionary").unwrap();
            let blocked = if romanized {
                self.blocked_mapping(&row)
            } else {
                contains_profanity(row.key, ProfanityLanguage::Khmer)
            };
            if blocked {
                continue;
            }
            let target = if romanized { self.dictionary.pool.get(row.target) } else { row.key };
            if romanized && self.input_excluded(prefix.trim_end(), target, None) { continue; }
            let ranked = (row.key != prefix, std::cmp::Reverse(context::score(context, target)),
                self.commonness.level("khmer", target), row.rank, row.key, row.id);
            if kept_outputs.contains(target) {
                let existing = best.iter().find(|hit| hit.6 == target).unwrap();
                if ranked >= (existing.0, existing.1, existing.2, existing.3, existing.4.as_str(), existing.5) {
                    continue;
                }
                // Different saved spellings can share an output. Keep its
                // best match before limiting, including a later better alias.
                best.retain(|hit| hit.6 != target);
            }
            if best.len() < limit
                || best.peek().is_some_and(|last| ranked < (last.0, last.1, last.2, last.3, last.4.as_str(), last.5))
            {
                if best.len() == limit {
                    kept_outputs.remove(&best.pop().unwrap().6);
                }
                kept_outputs.insert(target.to_owned());
                best.push((ranked.0, ranked.1, ranked.2, row.rank, row.key.to_owned(), row.id, target.to_owned()));
            }
        }
        best.into_sorted_vec().into_iter().map(|(_, _, _, rank, completion, id, khmer)| Suggestion { tap_only: false, preferred_input: None,
            candidate: SuggestionCandidate::Khmer(Candidate {
                mapping_id: Some(id), mapping_ids: Vec::new(), khmer, rank,
            }),
            completion: Some(completion),
            correction: None,
        }).collect()
    }

    fn english_completions(&self, prefix: &str) -> Vec<Suggestion> {
        self.english_completions_with_limit(prefix, MAX_COMPLETIONS)
    }
    fn english_completions_with_limit(&self, prefix: &str, limit: usize) -> Vec<Suggestion> {
        let index = &self.dictionary.english;
        let (start, end) = index.prefix(prefix);
        let exact_end = index.bound(prefix, true);
        let mut best: BinaryHeap<(bool, u8, i32, usize, String, String, u64)> =
            BinaryHeap::with_capacity(limit);
        let mut cursor = index.cursor(start);
        while cursor.next < end {
            // A verified block summary can discard a block only if even its best
            // possible rank/length loses. Exact words always retain priority.
            if !index.has_local_ranks() && cursor.next % compact::BLOCK == 0
                && cursor.next + compact::BLOCK <= end
                && cursor.next >= exact_end
                && best.len() == limit
            {
                let worst = best.peek().unwrap();
                if !worst.0 || index.completion_block_min(cursor.next / compact::BLOCK,
                    self.commonness.has_levels("english")).is_some_and(|bound| bound > (worst.1, worst.2, worst.3)) {
                    cursor = index.cursor(cursor.next + compact::BLOCK);
                    continue;
                }
            }
            let row = cursor
                .next()
                .expect("validated compact dictionary")
                .unwrap();
            let text = compact::english_text(&row);
            if contains_profanity(&text, ProfanityLanguage::English) { continue; }
            let ranked = (
                row.key != prefix,
                self.commonness.level("english", &text),
                row.rank,
                text.len(),
                row.key,
                text.as_ref(),
                row.id,
            );
            let keep = best.len() < limit
                || best.peek().is_some_and(|last| {
                    ranked
                        < (
                            last.0,
                            last.1,
                            last.2,
                            last.3,
                            last.4.as_str(),
                            last.5.as_str(),
                            last.6,
                        )
                });
            if keep {
                let owned = (
                    ranked.0,
                    ranked.1,
                    ranked.2,
                    ranked.3,
                    ranked.4.to_owned(),
                    ranked.5.to_owned(),
                    ranked.6,
                );
                if best.len() < limit {
                    best.push(owned);
                } else {
                    *best.peek_mut().unwrap() = owned;
                }
            }
        }
        best.into_sorted_vec()
            .into_iter()
            .map(|(_, _, rank, _, _, text, mapping_id)| Suggestion { tap_only: false, preferred_input: None,
                candidate: SuggestionCandidate::English(EnglishCandidate {
                    mapping_id,
                    text: text.clone(),
                    rank,
                    manual_only: false,
                word_alternative: false,
                }),
                completion: Some(text),
                correction: None,
            })
            .collect()
    }

    fn compose(&self, input: &str) -> Vec<Candidate> {
        if input.is_empty() || self.dictionary.romanized.len == 0 {
            return Vec::new();
        }
        let words: Vec<&str> = input.split(' ').collect();
        // Retain twenty paths per profanity boundary state. Global suffix
        // truncation is unsafe: a later prefix can block all twenty retained
        // outputs while a discarded suffix would form a clean complete path.
        // The policy bounds these states; complete outputs still cap at twenty.
        type Paths = BTreeMap<profanity::CompositionBoundary, Vec<Candidate>>;
        let mut suffixes = vec![Paths::new(); words.len() + 1];
        suffixes[words.len()].entry(Default::default()).or_default().push(Candidate {
            mapping_id: None,
            mapping_ids: Vec::new(),
            khmer: String::new(),
            rank: 0,
        });
        for start in (0..words.len()).rev() {
            let mut paths = Paths::new();
            let mut key = String::new();
            for end in start..words.len() {
                if end > start {
                    key.push(' ');
                }
                key.push_str(words[end]);
                if key.len() > self.max_key_bytes {
                    break;
                }
                // Stream all component alternatives. Their eligibility after
                // joining is unknown until filtering the assembled output.
                let mut cursor = self.dictionary.romanized.cursor(self.dictionary.romanized.bound(&key, false));
                while let Some(first) = cursor.next().expect("validated compact dictionary") {
                    if first.key != key { break; }
                    if self.blocked_mapping(&first) { continue; }
                    for rest in suffixes[end + 1].values().flatten() {
                        if rest.mapping_ids.len() >= MAX_COMPOSED_SEGMENTS {
                            continue;
                        }
                        let mut mapping_ids = Vec::with_capacity(rest.mapping_ids.len() + 1);
                        mapping_ids.push(first.id);
                        mapping_ids.extend_from_slice(&rest.mapping_ids);
                        let path = Candidate {
                            mapping_id: None,
                            mapping_ids,
                            khmer: format!("{}{}", self.dictionary.pool.get(first.target), rest.khmer),
                            rank: first.rank + rest.rank,
                        };
                        // Safe segments can form a blocked term when joined.
                        if contains_profanity(&path.khmer, ProfanityLanguage::Auto) { continue; }
                        if start == 0 && self.dictionary.excluded_khmer.contains(&path.khmer) { continue; }
                        let boundary = if start == 0 { Default::default() }
                            else { profanity::composition_boundary(&path.khmer) };
                        keep_path(paths.entry(boundary).or_default(), path);
                    }
                }
            }
            suffixes[start] = paths;
        }
        suffixes[0].remove(&Default::default()).unwrap_or_default()
    }
}

fn compare_paths(a: &Candidate, b: &Candidate) -> std::cmp::Ordering {
    a.mapping_ids
        .len()
        .cmp(&b.mapping_ids.len())
        .then_with(|| a.rank.cmp(&b.rank))
        .then_with(|| a.mapping_ids.cmp(&b.mapping_ids))
}

fn keep_path(paths: &mut Vec<Candidate>, path: Candidate) {
    if let Some(index) = paths
        .iter()
        .position(|existing| existing.khmer == path.khmer)
    {
        if !compare_paths(&path, &paths[index]).is_lt() {
            return;
        }
        paths.remove(index);
    }
    let position = paths
        .binary_search_by(|existing| compare_paths(existing, &path))
        .unwrap_or_else(|position| position);
    if position < MAX_COMPOSED_CANDIDATES {
        paths.insert(position, path);
        paths.truncate(MAX_COMPOSED_CANDIDATES);
    }
}

#[cfg(test)]
mod joined_tests {
    use super::*;

    fn dataset(rows: Vec<(u64, String, &str, i32)>) -> ApprovedDataset {
        ApprovedDataset {
            schema_version: 2,
            words: Some(
                rows.iter()
                    .map(|(id, _, khmer, rank)| ApprovedWord {
                        id: *id,
                        khmer: (*khmer).into(),
                        rank: *rank,
                        status: ApprovedStatus::Approved,
                    })
                    .collect(),
            ),
            mappings: rows
                .into_iter()
                .map(|(id, romanized, khmer, rank)| ApprovedMapping {
                    id,
                    romanized,
                    khmer: khmer.into(),
                    rank,
                    status: ApprovedStatus::Approved,
                })
                .collect(),
            english_words: None,
            structures: None, sound_map_disabled_words: None,
        }
    }

    #[test]
    fn joined_collisions_keep_ranked_sources_and_distinct_outputs_deterministically() {
        let mut source = dataset(vec![
            (10, "abc d".into(), "ក", 5),
            (20, "ab cd".into(), "ក", 1),
            (30, "abc d".into(), "ខ", 3),
            (40, "ab c d".into(), "ក", 1),
        ]);
        let engine = Engine::new(source.clone()).unwrap();
        let candidates = engine.convert("abcd").unwrap().candidates;
        assert_eq!(
            candidates
                .iter()
                .map(|row| row.mapping_id)
                .collect::<Vec<_>>(),
            [Some(20), Some(30)]
        );
        assert_eq!(
            candidates.iter().map(|row| row.rank).collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&engine.stats_json()).unwrap()["mappings"],
            4
        );
        source.mappings.reverse();
        source.words.as_mut().unwrap().reverse();
        assert_eq!(
            Engine::new(source.clone()).unwrap().to_binary(),
            engine.to_binary()
        );
        source.mappings.push(ApprovedMapping {
            id: 50,
            romanized: "abcd".into(),
            khmer: "ក".into(),
            rank: 100,
            status: ApprovedStatus::Approved,
        });
        // Explicit aliases retain their own identity even when a derived source
        // has a better rank; other approved outputs remain independently eligible.
        let explicit = Engine::new(source)
            .unwrap()
            .convert("abcd")
            .unwrap()
            .candidates;
        assert_eq!(
            explicit
                .iter()
                .map(|row| row.mapping_id)
                .collect::<Vec<_>>(),
            [Some(30), Some(50)]
        );
    }

    #[test]
    fn joined_prefix_duplicates_do_not_consume_the_completion_limit() {
        let outputs: Vec<_> = (0..26).map(|i| char::from_u32(0x1780 + i).unwrap().to_string()).collect();
        let source = dataset(
            (b'a'..=b'z')
                .enumerate()
                .map(|(index, letter)| {
                    (
                        index as u64 + 1,
                        format!("s{} a", char::from(letter)),
                        outputs[index].as_str(),
                        0,
                    )
                })
                .collect(),
        );
        let engine = Engine::new(source).unwrap();
        let loaded = Engine::from_binary(engine.to_binary()).unwrap();
        let suggestions = engine.suggest("s").unwrap();
        assert_eq!(suggestions.candidates.len(), MAX_COMPLETIONS);
        assert_eq!(
            suggestions
                .candidates
                .iter()
                .map(|row| row.candidate.mapping_id())
                .collect::<Vec<_>>(),
            (1..=MAX_COMPLETIONS as u64).map(Some).collect::<Vec<_>>()
        );
        assert_eq!(suggestions, loaded.suggest("s").unwrap());
    }

    #[test]
    fn khmer_prefix_ranks_the_full_range_but_keeps_the_exact_word_first() {
        let mut source = ApprovedDataset {
            schema_version: 2,
            mappings: Vec::new(),
            english_words: None,
            structures: None, sound_map_disabled_words: None,
            words: Some((1..=48).map(|id| ApprovedWord {
                id,
                khmer: "ក".repeat(id as usize + 1),
                rank: if id == 48 { 0 } else { 100 + id as i32 },
                status: ApprovedStatus::Approved,
            }).chain(std::iter::once(ApprovedWord {
                id: 100,
                khmer: "ក".into(),
                rank: 1_000_000,
                status: ApprovedStatus::Approved,
            })).collect()),
        };
        for expected in [
            [vec![100, 48], (1..=18).collect()].concat(),
            [vec![100], (1..=19).collect()].concat(),
        ] {
            let engine = Engine::new(source.clone()).unwrap();
            let result = engine.suggest("ក").unwrap();
            assert_eq!(result.candidates.iter().map(|row| row.candidate.mapping_id().unwrap()).collect::<Vec<_>>(), expected);
            assert_eq!(result, Engine::from_binary(engine.to_binary()).unwrap().suggest("ក").unwrap());
            assert!(engine.suggest("ក ").unwrap().candidates.is_empty());
            // A changed rank must move a candidate even beyond the old first-20 cutoff.
            source.words.as_mut().unwrap().iter_mut().find(|word| word.id == 48).unwrap().rank = 500;
        }
    }

    #[test]
    fn romanized_prefix_ranks_before_limiting_and_deduplicates_joined_aliases() {
        let outputs: Vec<_> = (0..26).map(|i| char::from_u32(0x1780 + i).unwrap().to_string()).collect();
        let mut source = dataset((b'a'..=b'z').enumerate().map(|(index, letter)| (
            index as u64 + 1, format!("s{} a", char::from(letter)), outputs[index].as_str(),
            if letter == b'z' { 0 } else { 10 },
        )).collect());
        let engine = Engine::new(source.clone()).unwrap();
        let suggestions = engine.suggest("s").unwrap();
        assert_eq!(suggestions.candidates.iter().map(|row| row.candidate.mapping_id().unwrap()).collect::<Vec<_>>(),
            [vec![26], (1..=19).collect()].concat());
        // Equal ranks retain lexical key order and each canonical alias appears once.
        assert_eq!(suggestions.candidates[0].completion.as_deref(), Some("sz a"));
        assert_eq!(suggestions, Engine::from_binary(engine.to_binary()).unwrap().suggest("s").unwrap());
        source.mappings.reverse();
        source.words.as_mut().unwrap().reverse();
        assert_eq!(suggestions, Engine::new(source.clone()).unwrap().suggest("s").unwrap());
        source.mappings.iter_mut().find(|row| row.id == 26).unwrap().rank = 20;
        let changed = Engine::new(source).unwrap().suggest("s").unwrap();
        assert_eq!(changed.candidates.iter().map(|row| row.candidate.mapping_id().unwrap()).collect::<Vec<_>>(),
            (1..=20).collect::<Vec<_>>());
    }

    #[test]
    fn ranked_prefixes_do_not_displace_complete_conversion_or_bypass_policy() {
        let engine = Engine::new(dataset(vec![
            (1, "sa".into(), "ក", 100),
            (2, "sa".into(), "ខ", 50),
            (3, "sa a".into(), "គ", 0),
            (4, "sb".into(), "ក្ត", 0),
        ])).unwrap();
        assert_eq!(engine.suggest("sa").unwrap().candidates.iter().map(|row| row.candidate.mapping_id()).collect::<Vec<_>>(),
            vec![Some(2), Some(1), Some(3)]);
        assert_eq!(engine.convert("sa").unwrap().candidates.iter().map(|row| row.mapping_id).collect::<Vec<_>>(),
            vec![Some(2), Some(1)]);
        assert!(engine.suggest("s").unwrap().candidates.iter().all(|row| row.candidate.mapping_id() != Some(4)));
        assert_eq!(engine.suggest("sa ").unwrap().candidates.iter().map(|row| row.candidate.mapping_id()).collect::<Vec<_>>(),
            vec![Some(2), Some(1), Some(3)]);
    }
}

// Boundary-only adapter; the same Engine implementation is used on both targets.
#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::{Engine, SuggestionFeatures, TypingContext, parse_request_object};
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen(js_name = checkProfanity)]
    pub fn check_profanity(text: &str, language: &str) -> Result<String, JsValue> {
        super::profanity::check_profanity_json(text, language)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
    #[wasm_bindgen]
    pub struct WasmEngine {
        engine: Engine,
    }
    #[wasm_bindgen]
    impl WasmEngine {
        #[wasm_bindgen(constructor)]
        pub fn new(dataset_json: &str) -> Result<WasmEngine, JsValue> {
            Engine::from_json(dataset_json)
                .map(|engine| Self { engine })
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = fromPersonalWords)]
        pub fn from_personal_words(json: &str) -> Result<WasmEngine, JsValue> {
            Engine::from_personal_words_json(json).map(|engine| Self { engine })
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = pronunciationInputs)]
        pub fn pronunciation_inputs(text: &str) -> Result<String, JsValue> {
            Engine::pronunciation_inputs(text).map(|value| serde_json::to_string(&value).unwrap())
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = wordShorthands)]
        pub fn word_shorthands(&self, word: &str) -> String { serde_json::to_string(&self.engine.word_shorthands(word)).unwrap() }
        #[wasm_bindgen(js_name = setShorthands)]
        pub fn set_shorthands(&mut self, json: &str) -> Result<(), JsValue> {
            self.engine.set_shorthands_json(json).map_err(|e|JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = setShorthandsWithExtension)]
        pub fn set_shorthands_with_extension(&mut self, extension: &WasmEngine, json: &str) -> Result<(), JsValue> {
            self.engine.set_shorthands_with_extension_json(&extension.engine,json).map_err(|e|JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = setInputExclusions)]
        pub fn set_input_exclusions(&mut self, json: &str) -> Result<(), JsValue> {
            self.engine.set_input_exclusions_json(json).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = setInputExclusionsWithExtension)]
        pub fn set_input_exclusions_with_extension(&mut self, extension: &WasmEngine, json: &str) -> Result<(), JsValue> {
            self.engine.set_input_exclusions_with_extension_json(&extension.engine, json)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = setPronunciations)]
        pub fn set_pronunciations(&mut self, json: &str) -> Result<(), JsValue> {
            let rules=serde_json::from_str(json).map_err(|e| JsValue::from_str(&format!("Invalid pronunciations: {e}")))?;
            self.engine.set_pronunciations(rules).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = fromBinary)]
        pub fn from_binary(bytes: Vec<u8>) -> Result<WasmEngine, JsValue> {
            Engine::from_binary(bytes)
                .map(|engine| Self { engine })
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = withTypingOverrides)]
        pub fn with_typing_overrides(&self, json: &str) -> Result<WasmEngine, JsValue> {
            self.engine.with_typing_overrides_json(json).map(|engine| Self { engine })
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = wordTypingSettings)]
        pub fn word_typing_settings(&self, language: &str, word: &str) -> Result<String, JsValue> {
            self.engine.word_typing_settings_json(language, word).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = fromAppData)]
        pub fn from_app_data(khmer: Vec<u8>, english: Vec<u8>) -> Result<WasmEngine, JsValue> {
            Engine::from_app_data(&khmer, &english).map(|engine| Self { engine })
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = compileAppData)]
        pub fn compile_app_data(&self, khmer: &str, english: &str, provenance: &str, language: &str) -> Result<Vec<u8>, JsValue> {
            let (khmer, english) = self.engine.compile_app_data(khmer, english, provenance)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            match language { "khmer" => Ok(khmer), "english" => Ok(english), _ => Err(JsValue::from_str("Invalid app data language.")) }
        }
        #[wasm_bindgen(js_name = appDataInfo)]
        pub fn app_data_info(&self) -> String { self.engine.app_data_info_json() }
        #[wasm_bindgen(js_name = exportBinary)]
        pub fn export_binary(&self) -> Vec<u8> {
            self.engine.to_binary()
        }
        pub fn stats(&self) -> String {
            self.engine.stats_json()
        }
        #[wasm_bindgen(js_name = browseWords)]
        pub fn browse_words(&self, request: &str) -> Result<String, JsValue> {
            self.engine.browse_words_json(request)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = soundMapInputs)]
        pub fn sound_map_inputs(&self, word: &str, after: &str) -> Result<String, JsValue> {
            self.engine.sound_map_inputs(word, after)
                .and_then(|page| serde_json::to_string(&page).map_err(|error| crate::invalid(&error.to_string())))
                .map_err(|error| JsValue::from_str(&error.to_string()))
        }
        #[wasm_bindgen(js_name = analyzeComponents)]
        pub fn analyze_components(&self, word: &str) -> Result<String, JsValue> {
            self.engine.analyze_components_json(word).map_err(|error|JsValue::from_str(&error.to_string()))
        }
        #[wasm_bindgen(js_name = analyzeComponentsWithExtension)]
        pub fn analyze_components_with_extension(&self, extension: &WasmEngine, word: &str) -> Result<String, JsValue> {
            self.engine.analyze_components_with_extension_json(word,&extension.engine).map_err(|error|JsValue::from_str(&error.to_string()))
        }
        #[wasm_bindgen(js_name = clearComponentExtensionCache)]
        pub fn clear_component_extension_cache(&self) { self.engine.clear_component_extension_cache(); }
        #[wasm_bindgen(js_name = loadWordMetadata)]
        pub fn load_word_metadata(&mut self, khmer: &str, english: &str) -> Result<(), JsValue> {
            self.engine.load_word_metadata(khmer, english).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = loadWordRanking)]
        pub fn load_word_ranking(&mut self, json: &str) -> Result<(), JsValue> {
            self.engine.load_word_ranking_json(json).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = lookupWordMetadata)]
        pub fn lookup_word_metadata(&self, language: &str, word: &str) -> Result<String, JsValue> {
            self.engine.lookup_word_metadata_json(language, word).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = browseWordMetadata)]
        pub fn browse_word_metadata(&self, request: &str) -> Result<String, JsValue> {
            self.engine.browse_word_metadata_json(request).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = lookupWordTranslations)]
        pub fn lookup_word_translations(&self, language: &str, word: &str) -> Result<String, JsValue> {
            self.engine.lookup_word_translations_json(language, word).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        pub fn convert(&self, input: &str) -> Result<String, JsValue> {
            self.engine
                .convert_json(input)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = convertWithExtension)]
        pub fn convert_with_extension(&self, extension: &WasmEngine, input: &str) -> Result<String, JsValue> {
            self.engine.convert_with_extension_json(&extension.engine, input)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        pub fn suggest(&self, input: &str) -> Result<String, JsValue> {
            self.engine
                .suggest_json(input)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = suggestWithContext)]
        pub fn suggest_with_context(&self, input: &str, features_json: &str, context_json: &str) -> Result<String, JsValue> {
            let features: SuggestionFeatures = parse_request_object(features_json, "suggestion features").map_err(|e| JsValue::from_str(&e.to_string()))?;
            let context: TypingContext = parse_request_object(context_json, "typing context").map_err(|e| JsValue::from_str(&e.to_string()))?;
            self.engine.suggest_with_context_json(input, features, &context).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = suggestWithRanking)]
        pub fn suggest_with_ranking(&self, input: &str, features_json: &str, context_json: &str,
            ranking_json: &str) -> Result<String, JsValue> {
            let features = parse_request_object(features_json, "suggestion features").map_err(|e| JsValue::from_str(&e.to_string()))?;
            let context = parse_request_object(context_json, "typing context").map_err(|e| JsValue::from_str(&e.to_string()))?;
            let ranking = parse_request_object(ranking_json, "ranking request").map_err(|e| JsValue::from_str(&e.to_string()))?;
            self.engine.suggest_with_ranking_json(input, features, &context, &ranking).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = rankingVocabulary)]
        pub fn ranking_vocabulary(&self, language: &str) -> Result<String, JsValue> {
            self.engine.ranking_vocabulary_json(language).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = suggestWithExtension)]
        pub fn suggest_with_extension(&self, extension: &WasmEngine, input: &str, features_json: &str,
            context_json: &str) -> Result<String, JsValue> {
            let features = parse_request_object(features_json, "suggestion features").map_err(|e| JsValue::from_str(&e.to_string()))?;
            let context = parse_request_object(context_json, "typing context").map_err(|e| JsValue::from_str(&e.to_string()))?;
            self.engine.suggest_with_extension_json(&extension.engine, input, features, &context)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
        #[wasm_bindgen(js_name = suggestWithFeatures)]
        pub fn suggest_with_features(
            &self,
            input: &str,
            features_json: &str,
        ) -> Result<String, JsValue> {
            let features = parse_request_object(features_json, "suggestion features")
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            self.engine
                .suggest_with_features_json(input, features)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        }
    }
}
