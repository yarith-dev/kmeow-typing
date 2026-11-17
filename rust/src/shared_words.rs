//! Explicit migrated preferences travel with the dictionary pair. Querying uses
//! the same prepared personal dictionary as before migration, without I/O.
use crate::{invalid, Engine, EngineError, PersonalWord, TypingMode};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

pub(crate) struct PersonalSource {
    pub words: Vec<PersonalWord>,
    fingerprint: [u8; 32],
}
impl PersonalSource {
    pub fn new(words: Vec<PersonalWord>) -> Self {
        let fingerprint = Sha256::digest(serde_json::to_vec(&words).expect("serializable personal words")).into();
        Self { words, fingerprint }
    }
}
pub(crate) struct SharedWords {
    source: Vec<PersonalWord>,
    engine: Box<Engine>,
    // One prepared union, replaced when a caller changes its personal words.
    merged: Mutex<Option<([u8; 32], Box<Engine>)>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedFile { format: String, words: Vec<PersonalWord> }

impl Engine {
    pub(crate) fn load_shared_words(&mut self, provenance: &serde_json::Value) -> Result<(), EngineError> {
        let Some(value) = provenance.get("sharedWords") else { return Ok(()); };
        let file: SharedFile = serde_json::from_value(value.clone()).map_err(|error| invalid(&error.to_string()))?;
        if file.format != "kmeow-shared-words-v1" { return Err(invalid("Unsupported shared word preferences.")); }
        let extension = Self::from_personal_words(file.words.clone())?;
        for word in &file.words {
            let present = if word.language == TypingMode::English {
                self.dictionary.english.contains(&word.word.to_ascii_lowercase())
            } else { self.dictionary.khmer.contains(&word.word) };
            if !present { return Err(invalid("Shared preferences require an existing dictionary word.")); }
            for spelling in &word.spellings {
                let index = &self.dictionary.romanized;
                let mut cursor = index.cursor(index.bound(spelling, false));
                let mut found = false;
                while let Some(row) = cursor.next()? {
                    if row.key != spelling { break; }
                    if self.dictionary.pool.get(row.target) == word.word { found = true; break; }
                }
                if !found { return Err(invalid("Shared preferences require an existing saved spelling.")); }
            }
        }
        if !file.words.is_empty() {
            self.shared_words = Some(Arc::new(SharedWords { source: file.words, engine: Box::new(extension), merged: Mutex::new(None) }));
        }
        Ok(())
    }

    pub(crate) fn has_shared_english_exact(&self, key: &str) -> bool {
        self.shared_words.as_ref().is_some_and(|shared| shared.engine.dictionary.english.contains(key))
    }

    pub(crate) fn shared_query<T>(&self, extension: Option<&Engine>,
        query: impl FnOnce(&Engine, bool) -> Result<T, EngineError>) -> Option<Result<T, EngineError>> {
        match (&self.shared_words, extension) {
            (None, None) => None,
            (None, Some(extension)) => Some(query(extension, false)),
            (Some(shared), None) => Some(query(&shared.engine, false)),
            (Some(shared), Some(extension)) => {
                let Some(personal) = &extension.personal_source else {
                    // General low-level extensions retain their independent
                    // policy settings; the base query includes the shared view.
                    return Some(query(extension, true));
                };
                let Ok(mut cached) = shared.merged.lock() else { return Some(Err(invalid("Shared word cache unavailable."))); };
                if cached.as_ref().is_none_or(|(key, _)| key != &personal.fingerprint) {
                    let mut words = shared.source.clone();
                    for local in &personal.words {
                        if let Some(old) = words.iter_mut().find(|word| word.language == local.language && word.word == local.word) {
                            old.rank = local.rank;
                            for spelling in &local.spellings { if !old.spellings.contains(spelling) { old.spellings.push(spelling.clone()); } }
                        } else { words.push(local.clone()); }
                    }
                    match Self::from_personal_words_bounded(words, 20_000, 200) {
                        Ok(engine) => *cached = Some((personal.fingerprint, Box::new(engine))),
                        Err(error) => return Some(Err(error)),
                    }
                }
                Some(query(&cached.as_ref().expect("prepared shared union").1, false))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EnglishWordContext, SuggestionFeatures, TypingContext, suggestion_output};
    use serde_json::json;

    fn dictionary() -> Engine {
        Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],"englishWords":[
            {"id":1,"text":"let","rank":0,"status":"approved"},
            {"id":2,"text":"book","rank":0,"status":"approved"},
            {"id":3,"text":"lets","rank":0,"status":"approved"}
        ]}).to_string()).unwrap()
    }
    fn context() -> TypingContext {
        TypingContext { english_word_context: Some(EnglishWordContext {before:String::new(),after:String::new()}), ..Default::default() }
    }
    #[test]
    fn shared_literal_choice_survives_context_and_local_extensions() {
        let mut engine=dictionary();
        engine.load_shared_words(&json!({"sharedWords":{"format":"kmeow-shared-words-v1","words":[{"word":"lets","language":"english"}]}})).unwrap();
        let features=SuggestionFeatures {mode:TypingMode::English,english_autocorrection:true,..Default::default()};
        assert_eq!(suggestion_output(&engine.suggest_with_context("lets",features,&context()).unwrap().result.candidates[0]),"lets");
        for word in ["localfixture","localother"] {
            let extension=Engine::from_personal_words_json(&json!([{"word":word,"language":"english"}]).to_string()).unwrap();
            let result=engine.suggest_with_extension(&extension,"local",features,&context()).unwrap();
            assert_eq!(suggestion_output(&result.result.candidates[0]),word);
            assert_eq!(suggestion_output(&engine.suggest_with_extension(&extension,"lets",features,&context()).unwrap().result.candidates[0]),"lets");
        }
    }
    #[test]
    fn preferences_cannot_admit_words_outside_the_actual_dictionary() {
        let mut engine=dictionary();
        assert!(engine.load_shared_words(&json!({"sharedWords":{"format":"other","words":[]}})).is_err());
        assert!(engine.load_shared_words(&json!({"sharedWords":{"format":"kmeow-shared-words-v1","words":[{"word":"notindictionary","language":"english"}]}})).is_err());
        engine.load_shared_words(&json!({"sharedWords":{"format":"kmeow-shared-words-v1","words":[{"word":"Book","language":"english"}]}})).unwrap();
    }
}
