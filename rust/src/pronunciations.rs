//! Khmer respellings affect generated readings, never saved aliases or output.
use crate::{Engine, EngineError, invalid};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Pronunciation { pub word: String, pub read_as: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File { format: String, rules: Vec<Pronunciation> }
impl Engine {
    /// Replace the complete pronunciation list. Empty restores normal readings.
    /// Each alternate is read once through the normal sound map (no chaining).
    pub fn set_pronunciations(&mut self, rules: Vec<Pronunciation>) -> Result<(),EngineError> {
        if rules.len()>1000 {return Err(invalid("Use at most 1,000 pronunciation rules."));}
        let mut seen=BTreeSet::new();
        for rule in &rules {
            if !self.dictionary.khmer.contains(&rule.word) || self.dictionary.excluded_khmer.contains(&rule.word)
                || !seen.insert(&rule.word) || rule.word==rule.read_as {
                return Err(invalid("Choose one existing Khmer word per pronunciation rule and a different alternate spelling."));
            }
            Self::pronunciation_inputs(&rule.read_as)?;
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

}
