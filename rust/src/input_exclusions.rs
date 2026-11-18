//! Exact query/output exclusions. Stored readings and component rules stay intact.
use crate::{compact, invalid, Engine, EngineError, Suggestion, SuggestionCandidate};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, sync::Arc};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WordInputExclusion { pub word: String, pub inputs: Vec<String> }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File { format: String, words: Vec<crate::JsonObject<WordInputExclusion>> }

impl compact::Dictionary {
    pub(crate) fn input_excluded(&self, input: &str, word: &str) -> bool {
        self.input_exclusions.get(input).is_some_and(|words| words.contains(word))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_preferences_preserve_an_extensions_exclusion_policy_and_clear_exactly() {
        let mut core = Engine::from_json(r#"{"schemaVersion":2,"words":[
            {"id":1,"khmer":"កូន","rank":1,"status":"approved"}],"mappings":[
            {"id":1,"romanized":"kon","khmer":"កូន","rank":1,"status":"approved"}]}"#).unwrap();
        core.load_shared_words(&serde_json::json!({"sharedWords":{
            "format":"kmeow-shared-words-v1","words":[{"word":"កូន","spellings":["kon"],"rank":1}]
        }})).unwrap();
        let mut personal = Engine::from_personal_words_json(r#"[{"word":"កូន","spellings":["kon"],"rank":0}]"#).unwrap();
        let query = |personal: &Engine| core.suggest_with_extension(personal,"kon",Default::default(),&Default::default()).unwrap();
        let before = query(&personal);
        assert!(!before.exact_candidates.is_empty());
        personal.set_input_exclusions(vec![WordInputExclusion{word:"កូន".into(),inputs:vec!["kon".into()]}]).unwrap();
        let blocked = query(&personal);
        assert!(blocked.exact_candidates.is_empty());
        assert!(!blocked.result.candidates.iter().any(|hit|matches!(&hit.candidate,
            SuggestionCandidate::Khmer(word) if word.khmer=="កូន")));
        personal.set_input_exclusions(vec![]).unwrap();
        assert_eq!(query(&personal),before);
    }
}

impl Engine {
    /// Replace the complete list. An empty list restores all stored readings.
    /// Exclusions apply to the complete normalized query, never to components.
    pub fn set_input_exclusions(&mut self, words: Vec<WordInputExclusion>) -> Result<(), EngineError> {
        self.set_input_exclusions_using(words, None)
    }

    /// Validate against both vocabularies while keeping one policy on the base.
    /// Replacing or clearing personal words never discards this policy; adding
    /// the same personal word later applies its saved exclusions again.
    pub fn set_input_exclusions_with_extension(&mut self, extension: &Engine, words: Vec<WordInputExclusion>) -> Result<(), EngineError> {
        self.set_input_exclusions_using(words, Some(&extension.dictionary))
    }

    fn set_input_exclusions_using(&mut self, words: Vec<WordInputExclusion>, extension: Option<&compact::Dictionary>) -> Result<(), EngineError> {
        if words.len() > 10_000 { return Err(invalid("Use at most 10,000 word input exclusions.")); }
        let mut seen = BTreeSet::new();
        let mut index: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for entry in words {
            let present = self.dictionary.khmer.contains(&entry.word)
                || extension.is_some_and(|dictionary| dictionary.khmer.contains(&entry.word));
            if !present || !seen.insert(entry.word.clone()) || entry.inputs.len() > 1000 {
                return Err(invalid("Choose distinct existing Khmer words with at most 1,000 excluded inputs each."));
            }
            for input in entry.inputs {
                let input = crate::normalize(&input)?;
                if !crate::valid_romanized(&input) { return Err(invalid("Use Romanized letters, spaces, apostrophes or hyphens for excluded inputs (at most 120 characters).")); }
                index.entry(input).or_default().insert(entry.word.clone());
            }
        }
        self.dictionary.input_exclusions = Arc::new(index);
        // Sound map queries filter before ranking and limits, including personal views.
        let mut sounds = crate::sound_map::Index::new(&self.dictionary);
        sounds.set_commonness(&self.commonness);
        self.sound_index = std::sync::OnceLock::from(sounds);
        Ok(())
    }

    pub fn set_input_exclusions_json(&mut self, json: &str) -> Result<(), EngineError> {
        self.set_input_exclusions(parse_words(json)?)
    }

    pub fn set_input_exclusions_with_extension_json(&mut self, extension: &Engine, json: &str) -> Result<(), EngineError> {
        self.set_input_exclusions_with_extension(extension, parse_words(json)?)
    }

    pub(crate) fn load_input_exclusions(&mut self, provenance: &serde_json::Value) -> Result<(), EngineError> {
        let Some(value) = provenance.get("inputExclusions") else { return Ok(()); };
        let file: File = serde_json::from_value(value.clone()).map_err(|e| invalid(&e.to_string()))?;
        if file.format != "kmeow-input-exclusions-v1" { return Err(invalid("Unsupported input exclusions format.")); }
        self.set_input_exclusions(file.words.into_iter().map(|word| word.0).collect())
    }

    pub(crate) fn input_excluded(&self, input: &str, word: &str, policy: Option<&compact::Dictionary>) -> bool {
        self.dictionary.input_excluded(input, word) || policy.is_some_and(|policy| policy.input_excluded(input, word))
    }

    pub(crate) fn filter_excluded_inputs(&self, candidates: &mut Vec<Suggestion>, input: &str, policy: Option<&compact::Dictionary>) {
        candidates.retain(|hit| match &hit.candidate {
            SuggestionCandidate::Khmer(word) => !self.input_excluded(input, &word.khmer, policy),
            SuggestionCandidate::English(_) => true,
        });
    }
}

fn parse_words(json: &str) -> Result<Vec<WordInputExclusion>, EngineError> {
    if json.len() > 4 * 1024 * 1024 { return Err(invalid("Input exclusions are too large.")); }
    let words: Vec<crate::JsonObject<WordInputExclusion>> = serde_json::from_str(json).map_err(|e| invalid(&e.to_string()))?;
    Ok(words.into_iter().map(|word| word.0).collect())
}
