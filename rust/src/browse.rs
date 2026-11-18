//! Read-only, bounded pages of Khmer words, saved aliases and Sound map inputs.
use super::{invalid, normalize, Engine, EngineError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DictionaryWord {
    pub khmer: String,
    pub romanizations: Vec<String>,
    #[serde(default, rename = "soundMapInputs", skip_serializing_if = "Vec::is_empty")]
    pub sound_map_inputs: Vec<String>,
    #[serde(default, rename = "soundMapInputsTruncated", skip_serializing_if = "std::ops::Not::not")]
    pub sound_map_inputs_truncated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metadata: Vec<crate::WordMetadata>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryPage {
    pub words: Vec<DictionaryWord>,
    pub next_after: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SoundMapInputPage {
    pub inputs: Vec<String>,
    pub next_after: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    query: String,
    after: Option<String>,
    limit: usize,
}

impl Engine {
    /// Read generated spellings for one bundled word, including disabled or excluded inputs.
    /// Saved aliases are separate. Each page contains at most 64 unique spellings.
    pub fn sound_map_inputs(&self, word: &str, after: &str) -> Result<SoundMapInputPage, EngineError> {
        if !crate::valid_khmer(word) || word.chars().count() > 240 || after.len() > 512
            || !after.bytes().all(|byte| byte.is_ascii_lowercase() || byte == b'\'') {
            return Err(invalid("Invalid Sound map word or spelling cursor."));
        }
        let (inputs, next_after) = if self.dictionary.sound_map_disabled(word) {
            let preview = self.dictionary.with_sound_map_enabled(word);
            crate::sound_map::Index::new(&preview).input_page(word, after, 64)
        } else {
            let sounds = self.sound_index.get_or_init(|| crate::sound_map::Index::new(&self.dictionary));
            sounds.input_page(word, after, 64)
        };
        Ok(SoundMapInputPage { inputs, next_after })
    }

    pub fn browse_words_json(&self, request: &str) -> Result<String, EngineError> {
        let request: Request = serde_json::from_str(request).map_err(|e| invalid(&e.to_string()))?;
        let page = self.browse_words(&request.query, request.after.as_deref(), request.limit)?;
        serde_json::to_string(&page).map_err(|e| invalid(&e.to_string()))
    }

    pub fn browse_words(&self, query: &str, after: Option<&str>, limit: usize) -> Result<DictionaryPage, EngineError> {
        if !(1..=100).contains(&limit) || after.is_some_and(|value| value.chars().count() > 240) {
            return Err(invalid("Invalid dictionary page bounds."));
        }
        let query = normalize(query)?;
        let lexicon = self.word_lexicon()?;
        let sounds = self.sound_index.get_or_init(|| crate::sound_map::Index::new(&self.dictionary));
        let joined_query = query.replace(' ', "");
        let mut selected = BTreeSet::new();
        if lexicon.loaded {
            for word in lexicon.entries("khmer")?.values() {
                if after.is_some_and(|value| word.word.as_str() <= value) || !word.word.contains(&query)
                    || crate::profanity::contains(&word.word, crate::profanity::ProfanityLanguage::Khmer) { continue; }
                selected.insert(word.word.clone());
                if selected.len() > limit + 1 { selected.pop_last(); }
            }
        }
        let mut cursor = self.dictionary.romanized.cursor(0);
        while let Some(row) = cursor.next()? {
            if row.source.is_some() || self.blocked_mapping(&row) { continue; }
            let khmer = self.dictionary.pool.get(row.target);
            if after.is_some_and(|value| khmer <= value)
                || (if lexicon.loaded { lexicon.lookup("khmer", khmer)?.is_empty() } else { !self.dictionary.khmer.contains(khmer) }) { continue; }
            if !khmer.contains(&query) && !row.key.contains(&query)
                && !row.key.replace(' ', "").contains(&joined_query) { continue; }
            selected.insert(khmer.to_owned());
            if selected.len() > limit + 1 { selected.pop_last(); }
        }
        for khmer in sounds.browse_matches(&query) {
            if after.is_some_and(|value| khmer <= value)
                || (lexicon.loaded && lexicon.lookup("khmer", khmer)?.is_empty()) { continue; }
            selected.insert(khmer.to_owned());
            if selected.len() > limit + 1 { selected.pop_last(); }
        }
        let has_more = selected.len() > limit;
        if has_more { selected.pop_last(); }
        let next_after = if has_more { selected.last().cloned() } else { None };
        let mut aliases: BTreeMap<_, BTreeSet<String>> = selected.into_iter().map(|key| (key, BTreeSet::new())).collect();
        // Complete each selected word's aliases, even when the search matched
        // only one spelling. Derived joined lookup keys never become new aliases.
        let mut cursor = self.dictionary.romanized.cursor(0);
        while let Some(row) = cursor.next()? {
            if row.source.is_some() || self.blocked_mapping(&row) { continue; }
            if let Some(values) = aliases.get_mut(self.dictionary.pool.get(row.target)) {
                if !self.blocked_mapping(&row) { values.insert(row.key.to_owned()); }
            }
        }
        Ok(DictionaryPage {
            words: aliases.into_iter().map(|(khmer, values)| {
                let (sound_map_inputs, sound_map_inputs_truncated) = sounds.browse_inputs(&khmer);
                DictionaryWord {
                    metadata: lexicon.lookup("khmer", &khmer).expect("valid metadata language"),
                    khmer, romanizations: values.into_iter().collect(), sound_map_inputs, sound_map_inputs_truncated,
                }
            }).collect(),
            next_after,
        })
    }
}
