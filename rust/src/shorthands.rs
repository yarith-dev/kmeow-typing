//! Whole-word, suggestion-only inputs. Never used for composition or components.
use crate::{compact, invalid, Engine, EngineError};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, sync::Arc};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WordShorthand { pub word: String, pub inputs: Vec<String> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File { format: String, words: Vec<crate::JsonObject<WordShorthand>> }

impl compact::Dictionary {
    pub(crate) fn shorthand_inputs(&self, word: &str, base: Option<&Self>) -> Vec<String> {
        base.and_then(|base|base.shorthands.get(word)).or_else(||self.shorthands.get(word))
            .cloned().unwrap_or_else(||crate::sound_map::default_shorthands(word))
    }
}
impl Engine {
    pub fn word_shorthands(&self, word: &str) -> Vec<String> { self.dictionary.shorthand_inputs(word,None) }
    /// Replace overrides, with omitted words inheriting built-in shorthands.
    /// An explicit empty inputs list disables that word's built-in shorthand.
    pub fn set_shorthands_json(&mut self, json: &str) -> Result<(),EngineError> {
        self.set_shorthands_using(json,None)
    }
    pub fn set_shorthands_with_extension_json(&mut self, extension: &Engine, json: &str) -> Result<(),EngineError> {
        self.set_shorthands_using(json,Some(&extension.dictionary))
    }
    fn set_shorthands_using(&mut self, json: &str, extension: Option<&compact::Dictionary>) -> Result<(),EngineError> {
        if json.len()>4*1024*1024 {return Err(invalid("Shorthands are too large."));}
        let entries: Vec<crate::JsonObject<WordShorthand>>=serde_json::from_str(json).map_err(|e|invalid(&e.to_string()))?;
        if entries.len()>10_000 {return Err(invalid("Use at most 10,000 shorthand words."));}
        let mut map=BTreeMap::new();
        for crate::JsonObject(entry) in entries {
            if (!self.dictionary.khmer.contains(&entry.word)&&!extension.is_some_and(|d|d.khmer.contains(&entry.word)))
                || map.contains_key(&entry.word) || entry.inputs.len()>1000 {
                return Err(invalid("Choose distinct existing Khmer words with at most 1,000 shorthands each."));
            }
            let mut inputs=BTreeSet::new();
            for input in entry.inputs {
                let input=crate::normalize(&input)?;
                if input.len()>120 || input.is_empty() || input.starts_with('\'') || input.ends_with('\'') || input.contains("''")
                    || !input.bytes().all(|b|b.is_ascii_lowercase()||b==b'\'') {
                    return Err(invalid("Use Romanized letters or a single internal apostrophe for shorthands (at most 120 characters)."));
                }
                inputs.insert(input);
            }
            map.insert(entry.word,inputs.into_iter().collect());
        }
        self.dictionary.shorthands=Arc::new(map);
        let mut sounds=crate::sound_map::Index::new(&self.dictionary);
        sounds.set_commonness(&self.commonness);
        self.sound_index=std::sync::OnceLock::from(sounds);
        Ok(())
    }
    pub(crate) fn load_shorthands(&mut self, provenance:&serde_json::Value)->Result<(),EngineError> {
        let Some(value)=provenance.get("shorthands") else{return Ok(());};
        let file:File=serde_json::from_value(value.clone()).map_err(|e|invalid(&e.to_string()))?;
        if file.format!="kmeow-shorthands-v1" {return Err(invalid("Unsupported shorthands format."));}
        self.set_shorthands_json(&serde_json::to_string(&file.words.into_iter().map(|entry|entry.0).collect::<Vec<_>>()).unwrap())
    }
}
