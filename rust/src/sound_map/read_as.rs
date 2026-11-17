//! Pronunciation defaults retain their original parsed boundaries. Keys are
//! derived from Khmer respellings, never copied from saved Latin spellings.
use super::*;

#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(super) struct Definition {
    pub word:String,
    pub read_as:String,
    #[serde(default)]
    pub original_spelling_only:bool,
}
#[derive(Default)]
pub(super) struct Prepared {
    pub syllables:Vec<FixedSyllable>,
    pub components:Vec<component_words::Reading>,
    pub word_parts:Vec<component_words::Reading>,
}
const MAX_KEYS:usize=256;
fn derived_keys(text:&str,syllables:&[FixedSyllable])->Result<Vec<Key>,String> {
    // No caller overrides or chaining while deriving an alternate.
    let readings=word_readings_in_context(text,false,overrides::Context::bare(text).with_read_as(syllables).for_pronunciation())
        .ok_or("This alternate spelling has no sound map reading.")?;
    let mut keys=Vec::new();
    for reading in readings {
        let mut prefixes=vec![Key::from(String::new())];
        for group in reading {
            let mut next=Vec::new();
            for prefix in &prefixes {for key in &group {
                let joined=prefix.joined(key);
                if !next.contains(&joined) {next.push(joined);}
                if next.len()>MAX_KEYS {return Err("This syllable reading has too many alternatives.".into());}
            }}
            prefixes=next;
        }
        for key in prefixes {if !keys.contains(&key) {keys.push(key);}}
        if keys.len()>MAX_KEYS {return Err("This syllable reading has too many alternatives.".into());}
    }
    Ok(keys)
}
fn prepare(overrides:&[crate::Pronunciation],syllables:&[FixedSyllable])->Result<Prepared,String> {
    let mut result=Prepared::default();
    for rule in &rules().pronunciation_syllables {
        // A deliberately standalone shorthand must never inherit a default
        // reusable pronunciation with the same identity.
        if has_shorthand_rule(&rule.word) {continue;}
        let text=overrides.iter().find(|entry|entry.word==rule.word).map_or(rule.read_as.as_str(),|entry|entry.read_as.as_str());
        let keys = derived_keys(text, syllables)?;
        result.syllables.push(FixedSyllable {
            khmer: rule.word.clone(), keys: keys.clone(),
            original_spelling_only: rule.original_spelling_only, words: vec![], positions: vec![],
        });
        result.components.push(component_words::Reading { khmer: rule.word.clone(), keys });
    }
    for rule in overrides {
        if owns(&rule.word) || has_shorthand_rule(&rule.word) {
            continue;
        }
        let runtime = &rules().runtime;
        let has_boundary = runtime.fixed_syllables.iter().any(|unit| unit.khmer == rule.word)
            || runtime.word_readings.iter().any(|unit| unit.khmer == rule.word);
        if has_boundary {
            let keys = derived_keys(&rule.read_as, syllables)?;
            result.word_parts.push(component_words::Reading { khmer: rule.word.clone(), keys });
        }
    }
    Ok(result)
}
pub(super) fn defaults()->&'static Prepared {
    rules().prepared_pronunciations.get_or_init(||prepare(&[],&[]).expect("validated pronunciation defaults"))
}
pub(super) fn configured(overrides:&[crate::Pronunciation])->Result<Prepared,String> {
    // Initialize the stock view first. Helpers inside more complex alternate
    // spellings can then consult it without recursively initializing it.
    prepare(overrides,&defaults().syllables)
}
pub(super) fn owns(word:&str)->bool {
    rules().pronunciation_syllables.iter().any(|rule|rule.word==word)
}

// Checked paths also run while defaults are being prepared. Never recursively
// initialize their OnceLock; the bootstrap parser deliberately has no defaults.
pub(super) fn stock_syllables()->&'static [FixedSyllable] {
    rules().prepared_pronunciations.get().map_or(&[],|prepared|prepared.syllables.as_slice())
}
