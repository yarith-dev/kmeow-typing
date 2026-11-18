//! Khmer respellings affect generated readings, never saved aliases or output.
use crate::{Engine, EngineError, invalid};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Pronunciation { pub word: String, pub read_as: ReadAs }

/// Single values retain their original JSON shape. Lists contain independent,
/// ordered spellings, with duplicate spellings removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadAs(Vec<String>);
impl ReadAs {
    pub fn spellings(&self) -> &[String] { &self.0 }
}
impl From<String> for ReadAs {
    fn from(spelling: String) -> Self { Self(vec![spelling]) }
}
impl From<&str> for ReadAs {
    fn from(spelling: &str) -> Self { spelling.to_owned().into() }
}
impl From<Vec<String>> for ReadAs {
    fn from(mut spellings: Vec<String>) -> Self {
        let mut seen = BTreeSet::new();
        spellings.retain(|spelling| seen.insert(spelling.clone()));
        Self(spellings)
    }
}
impl<'de> Deserialize<'de> for ReadAs {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self,D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Input { Single(String), Multiple(Vec<String>) }
        Ok(match Input::deserialize(deserializer)? {
            Input::Single(spelling) => spelling.into(),
            Input::Multiple(spellings) => spellings.into(),
        })
    }
}
impl Serialize for ReadAs {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok,S::Error> {
        match self.0.as_slice() {
            [spelling] => spelling.serialize(serializer),
            spellings => spellings.serialize(serializer),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File { format: String, rules: Vec<Pronunciation> }
impl Engine {
    /// Replace the complete pronunciation list. Empty restores normal readings.
    /// Each alternate is read once through the normal sound map (no chaining).
    pub fn set_pronunciations(&mut self, rules: Vec<Pronunciation>) -> Result<(),EngineError> {
        if rules.iter().map(|rule|rule.read_as.spellings().len()).sum::<usize>()>1000 {
            return Err(invalid("Use at most 1,000 pronunciation spellings."));
        }
        let mut seen=BTreeSet::new();
        for rule in &rules {
            if (!self.dictionary.khmer.contains(&rule.word) && !crate::sound_map::has_pronunciation_default(&rule.word))
                || self.dictionary.excluded_khmer.contains(&rule.word)
                || !seen.insert(&rule.word) || rule.read_as.spellings().is_empty()
                || rule.read_as.spellings().iter().all(|spelling|spelling==&rule.word) {
                return Err(invalid("Choose one existing Khmer word or built-in Read as part per rule and at least one different alternate spelling."));
            }
            for spelling in rule.read_as.spellings() { Self::pronunciation_inputs(spelling)?; }
        }
        crate::sound_map::validate_pronunciation_boundaries(&rules).map_err(|error|invalid(&error))?;
        self.dictionary.pronunciations=Arc::new(rules);
        self.component_index=Default::default();
        let mut index=crate::sound_map::Index::new(&self.dictionary);
        index.set_commonness(&self.commonness);
        self.sound_index=std::sync::OnceLock::from(index);
        Ok(())
    }
    pub fn pronunciation_inputs(read_as:&str)->Result<Vec<String>,EngineError> {
        if !crate::valid_khmer(read_as) || read_as.chars().count()>120 || read_as.contains(' ') {
            return Err(invalid("Enter a Khmer pronunciation spelling of up to 120 characters."));
        }
        let inputs=crate::sound_map::pronunciation_inputs(read_as);
        if inputs.is_empty(){return Err(invalid("This alternate spelling has no sound map reading."));}
        Ok(inputs)
    }
    pub(crate) fn load_pronunciations(&mut self, provenance:&serde_json::Value)->Result<(),EngineError> {
        let Some(value)=provenance.get("pronunciations") else {return Ok(());};
        let file:File=serde_json::from_value(value.clone()).map_err(|e|invalid(&e.to_string()))?;
        if file.format!="kmeow-pronunciations-v1" {return Err(invalid("Unsupported pronunciation format."));}
        self.set_pronunciations(file.rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApprovedDataset,SuggestionFeatures};
    use serde_json::json;
    fn engine()->Engine {
        let data:ApprovedDataset=serde_json::from_value(json!({"schemaVersion":3,"englishWords":[],
            "mappings":[{"id":70,"romanized":"friend","khmer":"មិត្ត","rank":4,"status":"approved"}],
            "words":(["មិត្ត","ភាព","មិត្តភាព","មិត្ត៍ភាព","កូន"].iter().enumerate().map(|(i,w)|
                json!({"id":i+1,"khmer":w,"rank":i,"status":"approved"})).collect::<Vec<_>>()) })).unwrap();
        Engine::new(data).unwrap()
    }
    fn rule(word:&str,read_as:&str)->Pronunciation {Pronunciation{word:word.into(),read_as:read_as.into()}}

    #[test]
    fn multiple_spellings_keep_whole_alternatives_compounds_aliases_and_reset() {
        let words=["ដែល","កូន","ដែលកូន","កូនដែល"];
        let mut core=Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
            "mappings":[{"id":90,"romanized":"saved-del","khmer":"ដែល","rank":7,"status":"approved"}],
            "words":words.iter().enumerate().map(|(i,word)|json!({
                "id":i+1,"khmer":word,"rank":i,"status":"approved"
            })).collect::<Vec<_>>()
        }).to_string()).unwrap();
        let bytes=core.to_binary();
        let saved=core.convert("saved-del").unwrap();
        let original=core.suggest("dael").unwrap();
        let multiple:Vec<Pronunciation>=serde_json::from_value(json!([
            {"word":"ដែល","readAs":["ដេល","ដែល","ដេល"]},
            {"word":"កូន","readAs":["កូន","គូន"]}
        ])).unwrap();
        assert_eq!(serde_json::to_value(&multiple).unwrap()[0]["readAs"],json!(["ដេល","ដែល"]));
        core.set_pronunciations(multiple).unwrap();
        for input in ["del","dael"] {assert!(exact(&core,input,"ដែល"),"{input}");}
        for input in ["delkon","delkun","delkoun","daelkon","daelkun","daelkoun"] {
            assert!(exact(&core,input,"ដែលកូន"),"{input}");
        }
        for input in ["kondel","kondael","kundel","kundael"] {
            assert!(exact(&core,input,"កូនដែល"),"{input}");
        }
        assert!(!exact(&core,"kon-saved-del","កូនដែល"));
        let page=core.browse_words("ដែល",None,100).unwrap();
        assert_eq!(page.words.iter().find(|word|word.khmer=="ដែល").unwrap().sound_map_inputs,vec!["dael","del"]);
        let before=core.suggest("del").unwrap();
        for invalid in [json!([]),json!(["ដែល"]),json!(["ដេល","latin"])] {
            let rules:Vec<Pronunciation>=serde_json::from_value(json!([{"word":"ដែល","readAs":invalid}])).unwrap();
            assert!(core.set_pronunciations(rules).is_err());
            assert_eq!(core.suggest("del").unwrap(),before);
        }
        assert_eq!(core.convert("saved-del").unwrap(),saved);
        assert_eq!(core.to_binary(),bytes);
        let single:Pronunciation=serde_json::from_value(json!({"word":"ដែល","readAs":["ដេល"]})).unwrap();
        assert_eq!(serde_json::to_value(&single).unwrap()["readAs"],"ដេល");
        core.set_pronunciations(vec![single]).unwrap();
        assert!(exact(&core,"del","ដែល"));assert!(!exact(&core,"dael","ដែល"));
        core.set_pronunciations(vec![]).unwrap();
        assert_eq!(core.suggest("dael").unwrap(),original);
        assert_eq!(core.to_binary(),bytes);
    }

    #[test]
    fn keanak_read_as_keeps_original_words_aliases_and_compound_scope() {
        let words=["គណៈ","គណៈរដ្ឋមន្ត្រី","គណៈរដ្ឋមន្ត្រីក","កគណៈរដ្ឋមន្ត្រី","កូន"];
        let mut core=Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
            "mappings":[{"id":900,"romanized":"savedkona","khmer":"គណៈ","rank":7,"status":"approved"}],
            "words":words.iter().enumerate().map(|(i,word)|
                json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        let bytes=core.to_binary();let alias=core.convert("savedkona").unwrap();
        assert!(exact(&core,"keanak","គណៈ"));
        assert!(exact(&core,"keaknak","គណៈ"));
        assert!(exact(&core,"keaknakroatmontri","គណៈរដ្ឋមន្ត្រី"));
        assert!(exact(&core,"keanakroatmontri","គណៈរដ្ឋមន្ត្រី"));
        for outside in &words[2..4] {assert!(!exact(&core,"keanakroatmontri",outside));}
        let before=core.sound_index.get().unwrap().browse_inputs("គណៈរដ្ឋមន្ត្រី");
        core.set_pronunciations(vec![rule("គណៈ","គៈណៈ")]).unwrap();
        assert_eq!(core.sound_index.get().unwrap().browse_inputs("គណៈរដ្ឋមន្ត្រី"),before);
        core.set_pronunciations(vec![rule("គណៈ","មិត")]).unwrap();
        assert!(exact(&core,"met","គណៈ"));assert!(!exact(&core,"keanak","គណៈ"));
        core.set_pronunciations(vec![]).unwrap();
        assert!(exact(&core,"keanak","គណៈ"));
        assert_eq!(core.sound_index.get().unwrap().browse_inputs("គណៈរដ្ឋមន្ត្រី"),before);
        assert_eq!(core.convert("savedkona").unwrap(),alias);
        assert_eq!(core.to_binary(),bytes);
    }
    fn exact(core:&Engine,input:&str,word:&str)->bool {
        core.suggest_with_context(input,Default::default(),&Default::default()).unwrap().exact_candidates.iter().any(|c|c.khmer==word)
    }
    #[test]
    fn alternate_spelling_changes_only_generated_readings_and_resets_completely() {
        let mut core=engine();let compact=core.to_binary();let alias=core.convert("friend").unwrap();
        let queries=["mettar","met","mit","metpeap","mittarpheap","friend","មិត្ត","kon"];
        let before:Vec<_>=queries.iter().map(|q|core.suggest(q).unwrap()).collect();
        let inputs=Engine::pronunciation_inputs("មិត").unwrap();assert_eq!(inputs,vec!["met","mit"]);
        core.set_pronunciations(vec![rule("មិត្ត","មិត")]).unwrap();
        for input in &inputs {assert!(exact(&core,input,"មិត្ត"));}
        assert!(exact(&core,"metpeap","មិត្តភាព"));assert!(!exact(&core,"mettar","មិត្ត"));
        assert!(!exact(&core,"metpeap","មិត្ត៍ភាព"));
        assert!(core.suggest_with_context("met",SuggestionFeatures{sound_map:false,..Default::default()},&Default::default()).unwrap().exact_candidates.is_empty());
        assert_eq!(core.convert("friend").unwrap(),alias);assert_eq!(core.to_binary(),compact);
        let hit=core.suggest_with_context("met",Default::default(),&Default::default()).unwrap().exact_candidates.remove(0);
        assert_eq!(hit.khmer,"មិត្ត");assert_eq!(hit.mapping_id,Some(1));
        core.set_pronunciations(vec![]).unwrap();
        for (input,before) in queries.into_iter().zip(before){assert_eq!(core.suggest(input).unwrap(),before,"{input}");}
    }
    #[test]
    fn marked_read_as_keeps_the_closure_before_an_independent_vowel_and_inside_compounds() {
        let mut core = engine();
        let bytes = core.to_binary();
        let saved = core.convert("friend").unwrap();
        let before = core.suggest("met").unwrap();
        core.set_pronunciations(vec![rule("មិត្ត","គៈឥ")]).unwrap();
        assert!(exact(&core,"keake","មិត្ត"));
        assert!(exact(&core,"keakepeap","មិត្តភាព"));
        assert!(!exact(&core,"keae","មិត្ត"),"an independent vowel did not previously allow an omitted closure");
        let changed = core.suggest("keake").unwrap();
        assert!(core.set_pronunciations(vec![rule("មិត្ត","គៈឨ")]).is_err());
        assert_eq!(core.suggest("keake").unwrap(),changed);
        assert_eq!(core.convert("friend").unwrap(),saved);
        core.set_pronunciations(vec![]).unwrap();
        assert_eq!(core.suggest("met").unwrap(),before);
        assert_eq!(core.to_binary(),bytes);
    }
    #[test]
    fn builtin_read_as_parts_are_editable_without_adding_their_source_to_a_small_dictionary() {
        let words = ["កម្មករ","កុម្ភៈកូន","កូន"];
        let mut core = Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
            "mappings":[{"id":70,"romanized":"savedcompound","khmer":"កម្មករ","rank":4,"status":"approved"}],
            "words":words.iter().enumerate().map(|(i,word)|
                json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        let bytes = core.to_binary();
        let alias = core.convert("savedcompound").unwrap();
        let probes = ["kamkor","keakekor","metkon","savedcompound"];
        let before: Vec<_> = probes.iter().map(|input|core.suggest(input).unwrap()).collect();
        assert!(exact(&core,"kamkor","កម្មករ"));
        core.set_pronunciations(vec![rule("កម្ម","គៈឥ"),rule("កុម្ភៈ","មិត")]).unwrap();
        assert!(exact(&core,"keakekor","កម្មករ"));
        assert!(!exact(&core,"keake","កម្ម"),"editing a part must not add a lexical word");
        assert!(!exact(&core,"metkon","កុម្ភៈកូន"),"a standalone default must retain its scope");
        assert_eq!(core.to_binary(),bytes);
        assert_eq!(core.convert("savedcompound").unwrap(),alias);
        let changed = core.suggest("keakekor").unwrap();
        assert!(core.set_pronunciations(vec![rule("មិនស្គាល់","មិត")]).is_err());
        assert_eq!(core.suggest("keakekor").unwrap(),changed);
        core.set_pronunciations(vec![
            Pronunciation{word:"កម្ម".into(),read_as:vec!["គៈឥ".into(),"មិត".into()].into()},
            Pronunciation{word:"កុម្ភៈ".into(),read_as:vec!["មិត".into(),"ដេល".into()].into()},
        ]).unwrap();
        for input in ["keakekor","metkor","mitkor"] {assert!(exact(&core,input,"កម្មករ"),"{input}");}
        for input in ["metkon","mitkon","delkon"] {
            assert!(!exact(&core,input,"កុម្ភៈកូន"),"multiple spellings retain the standalone scope: {input}");
        }
        core.set_pronunciations(vec![]).unwrap();
        for (input,before) in probes.iter().zip(before) { assert_eq!(core.suggest(input).unwrap(),before); }
        assert_eq!(core.to_binary(),bytes);
    }
    #[test]
    fn rules_do_not_chain_and_invalid_updates_are_atomic() {
        let mut core=engine();core.set_pronunciations(vec![rule("មិត្ត","កូន"),rule("កូន","មិត")]).unwrap();
        assert!(exact(&core,"kon","មិត្ត"));assert!(!exact(&core,"met","មិត្ត"));
        let before=core.suggest("met").unwrap();
        for rules in [vec![rule("មិត្ត","latin")],vec![rule("missing","មិត")],vec![rule("មិត្ត","មិត្ត")],
            vec![rule("មិត្ត","មិត"),rule("មិត្ត","មិត")],vec![rule("មិត្ត","")]] {
            assert!(core.set_pronunciations(rules).is_err());assert_eq!(core.suggest("met").unwrap(),before);
        }
    }
    #[test]
    fn alternate_shorthand_stays_with_its_original_word_and_standalone_parents_are_protected() {
        let mut core=engine();
        core.set_pronunciations(vec![rule("កូន","ខ្ញុំ")]).unwrap();
        assert!(!exact(&core, "nh", "កូន"));
        assert!(exact(&core, "khnhom", "កូន"));
        core.load_word_components(&json!({"wordComponents":{"format":"kmeow-word-components-v1", "words":[{"word":"មិត្តភាព","components":[]}]}})).unwrap();
        let before=core.suggest("metpeap").unwrap();
        core.set_pronunciations(vec![rule("មិត្ត","មិត")]).unwrap();
        assert_eq!(core.suggest("metpeap").unwrap().candidates.iter().any(|hit|crate::suggestion_output(hit)=="មិត្តភាព"),
            before.candidates.iter().any(|hit|crate::suggestion_output(hit)=="មិត្តភាព"));
    }

    #[test]
    fn component_alternates_keep_the_previews_vowels_instead_of_reparsing_joined_khmer() {
        let words=["គន្លឹះ","ទ្វារ","គន្លឹះទ្វារ","កូន","ភាព","កូនភាព"];
        let mut core=Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"englishWords":[],
            "words":words.iter().enumerate().map(|(i,word)|json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        core.set_pronunciations(vec![rule("គន្លឹះ","គន់លឹះ"),rule("កូន","ខ្ញុំ")]).unwrap();
        assert!(exact(&core,"konleuhtvea","គន្លឹះទ្វារ"));
        assert!(!exact(&core,"konlereahtvea","គន្លឹះទ្វារ"));
        assert!(!exact(&core,"nhpeap","កូនភាព"));
        assert!(exact(&core,"khnhompeap","កូនភាព"));
    }

    #[test]
    fn parsed_read_as_keeps_partial_compounds_scoped_paths_and_refreshes_personal_cache() {
        let words=["កម្ម","ក","កូន","កម្មករ","សកម្ម","គណិត","សាស្ត្រ","គណិតសាស្ត្រ","អេកូសាស្ត្រ"];
        let mut core=Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"englishWords":[],
            "words":words.iter().enumerate().map(|(i,word)|json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        let compact=core.to_binary();
        let personal=Engine::from_personal_words_json(r#"[{"word":"កម្មកូន","spellings":[]}]"#).unwrap();
        let personal_exact=|core:&Engine,input:&str|core.suggest_with_extension(&personal,input,Default::default(),&Default::default()).unwrap()
            .exact_candidates.iter().any(|candidate|candidate.khmer=="កម្មកូន");
        assert!(exact(&core,"kamkor","កម្មករ"),"a partial dictionary split retains its parsed boundary");
        assert!(personal_exact(&core,"kamkon"));
        core.set_pronunciations(vec![rule("កម្ម","មិត"),rule("សាស្ត្រ","សាស"),rule("គណិត","កូន")]).unwrap();
        for input in ["metkor","mitkor"] {assert!(exact(&core,input,"កម្មករ"),"{input}");}
        assert!(!exact(&core,"kamkor","កម្មករ"));
        assert!(personal_exact(&core,"metkon"));assert!(!personal_exact(&core,"kamkon"));
        assert!(exact(&core,"sakkam","សកម្ម"),"the longer standalone rule wins");
        assert!(exact(&core,"kaeknitsah","គណិតសាស្ត្រ"),"checked math prefix cannot be rebuilt from a conflicting component edit");
        assert!(exact(&core,"kon","គណិត"),"the same edit still controls the standalone word");
        let before=core.suggest("metkor").unwrap();
        assert!(core.set_pronunciations(vec![rule("កម្ម",&"មិត".repeat(12))]).is_err());
        assert_eq!(core.suggest("metkor").unwrap(),before,"an oversized alternate fails before mutating caches");
        core.set_pronunciations(vec![rule("កម្ម","សាស្ត្រ"),rule("សាស្ត្រ","កូន")]).unwrap();
        assert!(exact(&core,"sahkor","កម្មករ"));assert!(!exact(&core,"konkor","កម្មករ"),"Read as does not chain caller overrides");
        core.set_pronunciations(vec![rule("កម្ម","ខ្ញុំ")]).unwrap();
        assert!(exact(&core,"khnhomkor","កម្មករ"));assert!(!exact(&core,"nhkor","កម្មករ"));
        assert!(!exact(&core, "nh", "កម្ម"));
        assert!(exact(&core, "khnhom", "កម្ម"));
        core.set_pronunciations(vec![]).unwrap();
        assert!(exact(&core,"kamkor","កម្មករ"));assert!(personal_exact(&core,"kamkon"));
        assert!(!personal_exact(&core,"metkon"));assert_eq!(core.to_binary(),compact);
    }

    #[test]
    fn general_read_as_edits_keep_checked_compound_keys() {
        let words=["សាស្ត្រ","គណិតសាស្ត្រ","វេជ្ជសាស្ត្រ","សេដ្ឋសាស្ត្រ","អេកូសាស្ត្រ"];
        let mut core=Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"englishWords":[],
            "words":words.iter().enumerate().map(|(i,word)|json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        let checked=[("kaeknitsah","គណិតសាស្ត្រ"),("kaeknitsas","គណិតសាស្ត្រ"),
            ("vechchosah","វេជ្ជសាស្ត្រ"),("sedthasah","សេដ្ឋសាស្ត្រ")];
        for (input,word) in checked {assert!(exact(&core,input,word),"before: {input}");}
        let before=core.to_binary();
        core.set_pronunciations(vec![rule("សាស្ត្រ","កូន")]).unwrap();
        for (input,word) in checked {assert!(exact(&core,input,word),"lost checked reading: {input}");}
        assert!(exact(&core,"kon","សាស្ត្រ"));
        assert!(exact(&core,"ekokon","អេកូសាស្ត្រ"),"unscoped compounds follow the edit");
        assert!(!exact(&core,"kaeknitkon","គណិតសាស្ត្រ"),"checked keys cannot mix with the general edit");
        core.set_pronunciations(vec![rule("គណិតសាស្ត្រ","កូន")]).unwrap();
        assert!(exact(&core,"kon","គណិតសាស្ត្រ"),"an explicit whole-word edit still controls that word");
        assert_eq!(core.to_binary(),before);
    }

    #[test]
    fn read_as_component_splits_do_not_inherit_original_word_only_checked_paths() {
        let words=["គណិតសាស្ត្រ","កូន","គណិតសាស្ត្រកូន"];
        let mut core=Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"englishWords":[],
            "words":words.iter().enumerate().map(|(i,word)|json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        core.set_pronunciations(vec![rule("កូន","មិត")]).unwrap();
        assert!(exact(&core,"kaeknitsah","គណិតសាស្ត្រ"));
        assert!(exact(&core,"konetsahmet","គណិតសាស្ត្រកូន"));
        assert!(!exact(&core,"kaeknitsahmet","គណិតសាស្ត្រកូន"),"a checked whole-word prefix must not escape through a component split");
    }

    #[test]
    fn respellings_reuse_reviewed_boundaries_without_the_old_latin_keys() {
        let words = ["អ្នក", "អ្នកអាន", "អ្នកអាង", "អក្សរ", "អក្សរឈរ", "អក្សរចរ",
            "ពេជ្រ", "កូន", "ពេជ្រកូន"];
        let mut core = Engine::from_json(&json!({
            "schemaVersion": 3, "mappings": [], "englishWords": [],
            "words": words.iter().enumerate().map(|(i, word)|
                json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        let unlisted = ["អ្នកអាង", "អក្សរចរ", "ពេជ្រកូន"].map(|word| core.sound_index.get().unwrap().browse_inputs(word));
        assert!(exact(&core, "neakan", "អ្នកអាន"));
        core.set_pronunciations(vec![rule("អ្នក", "មិត"), rule("អក្សរ", "មិត"), rule("ពេជ្រ", "មិត")]).unwrap();
        assert!(exact(&core, "metan", "អ្នកអាន"));
        assert!(!exact(&core, "neakan", "អ្នកអាន"));
        assert!(exact(&core, "met", "ពេជ្រ"));
        let scoped = core.sound_index.get().unwrap().browse_inputs("អក្សរឈរ").0;
        assert!(scoped.iter().any(|key| key.starts_with("met")), "{scoped:?}");
        for (word, before) in ["អ្នកអាង", "អក្សរចរ", "ពេជ្រកូន"].into_iter().zip(unlisted) {
            assert_eq!(core.sound_index.get().unwrap().browse_inputs(word), before, "{word}");
        }
    }

    #[test]
    fn pronunciation_respects_component_opt_outs_and_checked_ending_scope() {
        let words = ["បណ្ឌិត្យ", "បណ្ឌិត្យកូន", "បណ្ឌិត្យសភា", "កូន", "រោងចក្រ", "រោងចក្រកូន"];
        let mut core = Engine::from_json(&json!({
            "schemaVersion": 3, "mappings": [], "englishWords": [],
            "words": words.iter().enumerate().map(|(i, word)|
                json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        core.set_pronunciations(vec![rule("បណ្ឌិត្យ", "មិត")]).unwrap();
        assert!(exact(&core, "met", "បណ្ឌិត្យ"));
        assert!(exact(&core, "bondetyokon", "បណ្ឌិត្យកូន"));
        assert!(!exact(&core, "metkon", "បណ្ឌិត្យកូន"));
        assert!(exact(&core, "bondetsapea", "បណ្ឌិត្យសភា"));
        core.set_pronunciations(vec![rule("កូន", "មិត")]).unwrap();
        assert!(exact(&core, "rongjak", "រោងចក្រ"));
        assert!(exact(&core, "rongochkromet", "រោងចក្រកូន"));
        assert!(!exact(&core, "rongjakmet", "រោងចក្រកូន"));
    }

    #[test]
    fn pronunciation_preview_and_typed_reading_ignore_a_targets_latin_override() {
        let inputs = Engine::pronunciation_inputs("ពេជ្រ").unwrap();
        assert!(!inputs.iter().any(|input| input == "pech"));
        let mut core = engine();
        core.set_pronunciations(vec![rule("មិត្ត", "ពេជ្រ")]).unwrap();
        for input in inputs {
            assert!(exact(&core, &input, "មិត្ត"), "{input}");
        }
        assert!(!exact(&core, "pech", "មិត្ត"));
    }

    #[test]
    fn disabled_parts_and_standalone_parents_suspend_inherited_respelling() {
        let words = ["អ្នក", "អ្នកអាន", "កម្ម", "កម្មករ"];
        let mut core = Engine::from_json(&json!({
            "schemaVersion": 3, "mappings": [], "englishWords": [], "soundMapDisabledWords": ["អ្នក"],
            "words": words.iter().enumerate().map(|(i, word)|
                json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        core.set_pronunciations(vec![rule("អ្នក", "មិត"), rule("កម្ម", "មិត")]).unwrap();
        assert!(exact(&core, "neakan", "អ្នកអាន"));
        assert!(!exact(&core, "metan", "អ្នកអាន"));
        assert!(exact(&core, "metkor", "កម្មករ"));
        core.load_word_components(&json!({"wordComponents": {
            "format": "kmeow-word-components-v1",
            "words": [{"word":"កម្មករ","components":[]}]
        }})).unwrap();
        assert!(exact(&core, "kamkor", "កម្មករ"));
        assert!(!exact(&core, "metkor", "កម្មករ"));
        core.set_pronunciations(vec![rule("កម្មករ", "មិត")]).unwrap();
        assert!(exact(&core, "met", "កម្មករ"));
    }

    #[test]
    fn scoped_part_and_retained_ending_do_not_escape_through_component_splits() {
        let words = ["អក្សរឈរ", "អក្សរឈរកូន", "ទស្សន៍", "ទស្សន៍កូន", "កូន"];
        let mut core = Engine::from_json(&json!({
            "schemaVersion": 3, "mappings": [], "englishWords": [],
            "words": words.iter().enumerate().map(|(i, word)|
                json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        core.set_pronunciations(vec![rule("កូន", "មិត")]).unwrap();
        assert!(exact(&core, "aksorchor", "អក្សរឈរ"));
        assert!(exact(&core, "arksarrochormet", "អក្សរឈរកូន"));
        assert!(!exact(&core, "aksorchormet", "អក្សរឈរកូន"));
        assert!(exact(&core, "tuh", "ទស្សន៍"));
        assert!(!exact(&core, "tuhmet", "ទស្សន៍កូន"));
    }

    #[test]
    fn pronunciation_split_cannot_cut_a_reviewed_word_part() {
        let words = ["អ្ន", "កអាន", "អ្នកអាន", "អក្ស", "រឈរ", "អក្សរឈរ", "រោងច", "ក្រ", "រោងចក្រ",
            "ទូរទស្ស", "ន៍", "ទូរទស្សន៍"];
        let mut core = Engine::from_json(&json!({
            "schemaVersion": 3, "mappings": [], "englishWords": [],
            "words": words.iter().enumerate().map(|(i, word)|
                json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        core.set_pronunciations(vec![rule("កអាន", "កូន"), rule("រឈរ", "កូន"), rule("ក្រ", "កូន"),
            rule("ន៍", "កូន")]).unwrap();
        assert!(exact(&core, "kon", "កអាន"));
        assert!(exact(&core, "neakan", "អ្នកអាន"));
        assert!(!exact(&core, "nokon", "អ្នកអាន"));
        assert!(exact(&core, "kon", "រឈរ"));
        assert!(exact(&core, "aksorchor", "អក្សរឈរ"));
        assert!(!exact(&core, "arksarkon", "អក្សរឈរ"));
        assert!(exact(&core, "rongjak", "រោងចក្រ"));
        assert!(!exact(&core, "rongochkon", "រោងចក្រ"));
        assert!(exact(&core, "turotuh", "ទូរទស្សន៍"));
        assert!(!exact(&core, "turotssarkon", "ទូរទស្សន៍"));
    }

    #[test]
    fn standalone_child_keeps_stock_pronunciation_when_a_sibling_is_edited() {
        let words = ["កម្ម", "កម្មករ", "កូន", "កម្មករកូន"];
        let mut core = Engine::from_json(&json!({
            "schemaVersion": 3, "mappings": [], "englishWords": [],
            "words": words.iter().enumerate().map(|(i, word)|
                json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        core.load_word_components(&json!({"wordComponents": {
            "format": "kmeow-word-components-v1",
            "words": [{"word":"កម្មករ","components":[]},
                {"word":"កម្មករកូន","components":[["កម្មករ","កូន"]]}]
        }})).unwrap();
        core.set_pronunciations(vec![rule("កម្ម", "មិត"), rule("កូន", "កា")]).unwrap();
        assert!(exact(&core, "kamkor", "កម្មករ"));
        assert!(!exact(&core, "metkorka", "កម្មករកូន"));
        assert!(exact(&core, "kamkorka", "កម្មករកូន"));
        core.set_pronunciations(vec![rule("កម្មករ", "មិត"), rule("កូន", "កា")]).unwrap();
        assert!(exact(&core, "metka", "កម្មករកូន"));
    }

    #[test]
    fn compound_checks_use_applied_edits_and_keep_the_approved_terminal_path() {
        let words = ["ការ", "កិច្ច", "ការកិច្ច", "កា", "កូន", "រោង", "ចក្រ", "រោងចក្រ"];
        let mut core = Engine::from_json(&json!({
            "schemaVersion": 3, "mappings": [], "englishWords": [],
            "words": words.iter().enumerate().map(|(i, word)|
                json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        core.set_pronunciations(vec![rule("ការ", "កា"), rule("រោង", "កូន")]).unwrap();
        let before = core.sound_index.get().unwrap().browse_inputs("ការកិច្ច");
        assert!(exact(&core, "kakejjar", "ការកិច្ច"));
        assert!(exact(&core, "rongjak", "រោងចក្រ"));
        assert!(exact(&core, "konjarkro", "រោងចក្រ"));
        core.set_pronunciations(vec![rule("ការ", "កា"), rule("រោង", "កូន"), rule("កា", "កូន")]).unwrap();
        assert_eq!(core.sound_index.get().unwrap().browse_inputs("ការកិច្ច"), before);
        core.set_pronunciations(vec![rule("ការ", "កូន")]).unwrap();
        assert!(exact(&core, "kakech", "ការកិច្ច"));
        assert!(!exact(&core, "konkejjar", "ការកិច្ច"));
    }

    #[test]
    fn sibling_respellings_keep_rules_that_are_scoped_to_the_original_compound() {
        let words = ["ទូរ", "ទស្សន៍", "ទូរទស្សន៍", "អក្សរ", "ឈរ", "អក្សរឈរ"];
        let mut core = Engine::from_json(&json!({
            "schemaVersion": 3, "mappings": [], "englishWords": [],
            "words": words.iter().enumerate().map(|(i, word)|
                json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        core.set_pronunciations(vec![rule("ទូរ", "កូន"), rule("ឈរ", "កូន")]).unwrap();
        assert!(exact(&core, "kontuh", "ទូរទស្សន៍"));
        assert!(exact(&core, "kontus", "ទូរទស្សន៍"));
        assert!(!exact(&core, "kontoh", "ទូរទស្សន៍"));
        assert!(exact(&core, "aksorkon", "អក្សរឈរ"));
        assert!(exact(&core, "arksorkon", "អក្សរឈរ"));
    }

    #[test]
    fn terminal_respelling_uses_scoped_boundaries_without_dictionary_splits() {
        let words = ["អាណាចក្រ", "អាណាចក្រា", "អាណាចក្រ៍", "អាណាចក្រកូន",
            "យុត្តិធម៌", "ក្សេមក្សាន្ត", "កាតព្វកិច្ច", "ប្រអប់ប្រៃសណីយ៍", "ពាក់ព័ន្ធ", "ប្រព័ន្ធព្រះអាទិត្យ"];
        let mut core = Engine::from_json(&json!({
            "schemaVersion":3,"englishWords":[],
            "words":words.iter().enumerate().map(|(i,word)|
                json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>(),
            "mappings":[{"id":900,"romanized":"savedchakra","khmer":"អាណាចក្រ","rank":0,"status":"approved"}]
        }).to_string()).unwrap();
        let bytes = core.to_binary();
        let alias = core.convert("savedchakra").unwrap();
        let system = core.sound_index.get().unwrap().browse_inputs(words[9]);
        for (word, input) in [("ចក្រ", "anamet"), ("ធម៌", "yottimet"),
            ("ក្សាន្ត", "ksemmet"), ("កិច្ច", "katpvomet"), ("ប្រៃសណីយ៍", "broopmet"), ("ព័ន្ធ", "peakmet")] {
            let parent = match word {
                "ចក្រ" => words[0], "ធម៌" => words[4], "ក្សាន្ត" => words[5],
                "កិច្ច" => words[6], "ប្រៃសណីយ៍" => words[7], _ => words[8],
            };
            let before = core.sound_index.get().unwrap().browse_inputs(parent);
            assert!(!exact(&core,input,parent));
            core.set_pronunciations(vec![rule(word,"មិត")]).unwrap();
            assert!(exact(&core,input,parent),"{input} → {parent}");
            assert_eq!(core.convert("savedchakra").unwrap(),alias);
            assert_eq!(core.sound_index.get().unwrap().browse_inputs(words[9]),system);
            for outside in &words[1..4] {
                assert!(!exact(&core,"anamet",outside),"{outside}");
            }
            assert_eq!(core.to_binary(),bytes);
            core.set_pronunciations(vec![]).unwrap();
            assert_eq!(core.sound_index.get().unwrap().browse_inputs(parent),before,"reset {word}");
        }
    }

}
