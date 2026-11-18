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
    #[serde(default)]
    pub words: Vec<String>,
    #[serde(default)]
    pub positions: Vec<WordReadingPosition>,
    #[serde(default)]
    pub reusable_as_component: Option<bool>,
}
impl Definition {
    fn reuses_component(&self) -> bool {
        self.reusable_as_component.unwrap_or(true)
            && (self.positions.is_empty()
                || self.positions.iter().any(|position| *position != WordReadingPosition::Standalone))
    }
}
#[derive(Default)]
pub(super) struct Prepared {
    pub syllables:Vec<FixedSyllable>,
    pub components:Vec<component_words::Reading>,
    pub word_parts:Vec<component_words::Reading>,
}
/// Scoped Read as definitions keep their original parser cases. The keys are
/// derived lazily after the immutable sound map has finished loading.
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum Keys {
    Literal(Vec<Key>),
    Pronunciation {
        #[serde(rename = "readAs")]
        read_as: String,
        #[serde(skip)]
        prepared: OnceLock<Vec<Key>>,
    },
}
impl From<Vec<Key>> for Keys {
    fn from(keys: Vec<Key>) -> Self { Self::Literal(keys) }
}
impl std::ops::Deref for Keys {
    type Target = [Key];
    fn deref(&self) -> &[Key] {
        match self {
            Self::Literal(keys) => keys,
            Self::Pronunciation { read_as, prepared } => prepared.get_or_init(|| {
                derived_keys(read_as, stock_syllables()).expect("validated built-in Read as spelling")
            }),
        }
    }
}
impl<'a> IntoIterator for &'a Keys {
    type Item = &'a Key;
    type IntoIter = std::slice::Iter<'a, Key>;
    fn into_iter(self) -> Self::IntoIter { self.iter() }
}

const MAX_KEYS:usize=256;
fn spelling_paths(text:&str,syllables:&[FixedSyllable])->Result<Vec<Vec<Vec<Key>>>,String> {
    // No caller overrides or chaining while deriving an alternate.
    let mut paths=word_readings_in_context(text,false,overrides::Context::bare(text).with_read_as(syllables).for_pronunciation())
        .ok_or_else(||"This alternate spelling has no sound map reading.".to_owned())?;
    ending_alternatives::append(&mut paths);
    Ok(paths)
}
fn flatten(paths:&[Vec<Vec<Key>>],max_stage:u8)->Result<Vec<Key>,String> {
    let mut keys=Vec::new();
    for reading in paths {
        let mut prefixes=vec![Key::from(String::new())];
        for group in reading {
            let mut next=Vec::new();
            let mut choices=Vec::new();
            if !prefixes.is_empty() {for key in group.iter().filter(|key|key.prediction_stage<=max_stage) {
                if let Some(paths)=&key.paths {
                    choices.extend(flatten(paths,max_stage)?.into_iter().map(|mut choice| {
                        choice.priority=choice.priority.saturating_add(key.priority);
                        choice.preserve_existing|=key.preserve_existing;
                        choice.prediction_stage=choice.prediction_stage.max(key.prediction_stage);
                        choice
                    }));
                } else {choices.push(key.clone());}
            }}
            for prefix in &prefixes {for choice in &choices {
                let joined=prefix.joined(choice);
                if !next.contains(&joined) {next.push(joined);}
                if next.len()>MAX_KEYS {return Err("This pronunciation has too many alternatives.".into());}
            }}
            prefixes=next;
        }
        for key in prefixes {if !keys.contains(&key) {keys.push(key);}}
        if keys.len()>MAX_KEYS {return Err("This pronunciation has too many alternatives.".into());}
    }
    Ok(keys)
}
fn keys_for_paths(paths:Vec<Vec<Vec<Key>>>)->Result<Vec<Key>,String> {
    match flatten(&paths,u8::MAX) {
        Ok(keys)=>Ok(keys),
        Err(error)=>{
            // Keep the established 256-choice limit on pre-onset spellings.
            // The newer onset alternatives remain a compact lattice, including
            // combinations far larger than the flat budget, rather than being
            // dropped or turning a previously valid saved rule into an error.
            flatten(&paths,1).map_err(|_|error)?;
            Ok(vec![Key::factored(paths)])
        }
    }
}
fn derived_keys(text:&str,syllables:&[FixedSyllable])->Result<Vec<Key>,String> {
    keys_for_paths(spelling_paths(text,syllables)?)
}
fn alternate_keys(read_as:&crate::ReadAs,syllables:&[FixedSyllable])->Result<Vec<Key>,String> {
    let mut paths=Vec::new();
    for spelling in read_as.spellings() {paths.extend(spelling_paths(spelling,syllables)?);}
    keys_for_paths(paths)
}
pub(super) fn pronunciation_paths(read_as:&crate::ReadAs)->Option<Vec<Vec<Vec<Key>>>> {
    match read_as.spellings() {
        [spelling] => word_readings_in_context(spelling,false,
            overrides::Context::new(spelling).for_pronunciation()),
        // Keep whole spellings independent, and combine components as a
        // lattice rather than expanding all combinations of their variants.
        _ => Some(vec![vec![alternate_keys(read_as,stock_syllables()).ok()?]]),
    }
}
fn prepare(overrides:&[crate::Pronunciation],syllables:&[FixedSyllable])->Result<Prepared,String> {
    let mut result=Prepared::default();
    for rule in &rules().pronunciation_syllables {
        // A deliberately standalone shorthand must never inherit a default
        // reusable pronunciation with the same identity.
        if has_shorthand_rule(&rule.word) {continue;}
        let keys = match overrides.iter().find(|entry|entry.word==rule.word) {
            Some(entry) => alternate_keys(&entry.read_as,syllables)?,
            None => derived_keys(&rule.read_as,syllables)?,
        };
        result.syllables.push(FixedSyllable {
            khmer: rule.word.clone(), keys: keys.clone().into(),
            original_spelling_only: rule.original_spelling_only,
            words: rule.words.clone(),
            positions: rule.positions.clone(),
        });
        if rule.reuses_component() {
            result.components.push(component_words::Reading { khmer: rule.word.clone(), keys: keys.into() });
        }
    }
    for rule in overrides {
        if owns(&rule.word) || has_shorthand_rule(&rule.word) {
            continue;
        }
        let runtime = &rules().runtime;
        let migrated_terminal = runtime.pronunciation_defaults.iter().any(|stock| stock.word == rule.word)
            && (runtime.checked_terminal_roots.iter().any(|unit| unit.root == rule.word)
                || runtime.terminal_lexical_units.iter().any(|unit| unit.khmer == rule.word));
        let has_boundary = runtime.fixed_syllables.iter().any(|unit| unit.khmer == rule.word)
            || runtime.word_readings.iter().any(|unit| unit.khmer == rule.word)
            || migrated_terminal;
        if has_boundary {
            let keys = alternate_keys(&rule.read_as, syllables)?;
            result.word_parts.push(component_words::Reading { khmer: rule.word.clone(), keys: keys.into() });
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
    let syllables=&defaults().syllables;
    for rule in overrides.iter().filter(|rule|rule.read_as.spellings().len()>1) {
        alternate_keys(&rule.read_as,syllables)?;
    }
    prepare(overrides,syllables)
}
pub(super) fn owns(word:&str)->bool {
    rules().pronunciation_syllables.iter().any(|rule|rule.word==word)
}

// Checked paths also run while defaults are being prepared. Never recursively
// initialize their OnceLock; the bootstrap parser deliberately has no defaults.
pub(super) fn stock_syllables()->&'static [FixedSyllable] {
    rules().prepared_pronunciations.get().map_or(&[],|prepared|prepared.syllables.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_two_dot_pronunciations_keep_series_and_syllable_boundaries() {
        for (word, expected) in [("គៈណៈ", vec!["keaknak", "keanak"]), ("កៈណៈ", vec!["kaknak", "kanak"]),
            ("គៈ", vec!["keak"]), ("ណៈ", vec!["nak"])] {
            assert_eq!(crate::Engine::pronunciation_inputs(word).unwrap(),expected,"{word}");
        }
        for word in ["គ៉ៈណៈ", "គាៈណៈ", "គៈៈ", "គៈា", "គៈ្", "គៈណៈ៍"] {
            assert!(crate::Engine::pronunciation_inputs(word).is_err(),"{word}");
        }
        let chars: Vec<_> = "គៈណៈ".chars().collect();
        assert!(two_dot::keys(&chars,0,1,"o",false,true).is_none());
    }

    #[test]
    #[cfg(feature="bundled-dictionaries")]
    fn saving_stock_pronunciations_keeps_every_bundled_lattice_and_priority() {
        let engine = crate::Engine::bundled().unwrap();
        let before = Index::new(&engine.dictionary);
        let compare = |dictionary: &compact::Dictionary| {
            let after = Index::new(dictionary);
            assert_eq!(after.words.len(), before.words.len());
            for (actual, expected) in after.words.iter().zip(&before.words) {
                assert_eq!(actual.candidate, expected.candidate);
                assert_eq!(actual.readings, expected.readings, "{}", expected.candidate.khmer);
                assert_eq!(actual.recovery_lengths, expected.recovery_lengths);
            }
            assert_eq!(after.prefixes, before.prefixes);
            assert_eq!(after.count_prefixes, before.count_prefixes);
            assert_eq!(after.has_rank_priorities, before.has_rank_priorities);
            assert_eq!(after.has_so, before.has_so);
        };
        for stock in &rules().runtime.pronunciation_defaults {
            let mut dictionary = engine.dictionary.clone();
            let mut saved: Vec<_> = dictionary.pronunciations.iter()
                .filter(|rule| rule.word != stock.word).cloned().collect();
            saved.push(stock.clone());
            dictionary.pronunciations = std::sync::Arc::new(saved);
            compare(&dictionary);
        }
        let mut dictionary = engine.dictionary.clone();
        let mut saved: Vec<_> = dictionary.pronunciations.iter().filter(|rule|
            !rules().runtime.pronunciation_defaults.iter().any(|stock|stock.word==rule.word)).cloned().collect();
        saved.extend(rules().runtime.pronunciation_defaults.iter().cloned());
        dictionary.pronunciations = std::sync::Arc::new(saved);
        compare(&dictionary);
    }
}
