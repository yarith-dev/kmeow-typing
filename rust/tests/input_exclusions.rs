use khmer_engine::{Engine, Pronunciation, SuggestionFeatures, TypingContext, WordInputExclusion};
use serde_json::json;

fn engine() -> Engine {
    Engine::from_json(&json!({"schemaVersion":3,"englishWords":[
        {"id":80,"text":"book","rank":0,"status":"approved"}],
        "mappings":[
            {"id":90,"romanized":"ke","khmer":"កេរ្តិ៍","rank":0,"status":"approved"},
            {"id":91,"romanized":"kid","khmer":"កូន","rank":0,"status":"approved"},
            {"id":92,"romanized":"kon","khmer":"កូន","rank":0,"status":"approved"},
            {"id":93,"romanized":"met","khmer":"មិត្ត","rank":0,"status":"approved"}],
        "words":(["កូន","មិត្ត","កូនមិត្ត","កេរ្តិ៍","គេ","ខ្ញុំ"].iter().enumerate().map(|(id, word)|
            json!({"id":id+1,"khmer":word,"rank":id,"status":"approved"})).collect::<Vec<_>>())
    }).to_string()).unwrap()
}
fn rule(word: &str, inputs: &[&str]) -> WordInputExclusion {
    WordInputExclusion { word: word.into(), inputs: inputs.iter().map(|key| (*key).into()).collect() }
}
fn has(core: &Engine, input: &str, word: &str) -> bool {
    core.suggest(input).unwrap().candidates.iter().any(|hit| match &hit.candidate {
        khmer_engine::SuggestionCandidate::Khmer(candidate) => candidate.khmer == word, _ => false,
    })
}

#[test]
fn exclusions_override_saved_generated_and_shorthand_matches_without_deleting_them() {
    let mut core = engine();
    for (key,word) in [("ke","កេរ្តិ៍"),("ke","គេ"),("kon","កូន"),("nh","ខ្ញុំ")] { assert!(has(&core,key,word),"{key} {word}"); }
    let binary = core.to_binary();
    let before: Vec<_> = ["ke","kon","ko","kid","nh","ម","book"].iter().map(|input| core.suggest(input).unwrap()).collect();
    core.set_input_exclusions(vec![rule("កេរ្តិ៍", &[" KE "]),rule("កូន", &["kon"]),rule("ខ្ញុំ", &["nh"])]).unwrap();
    for (key,word) in [("ke","កេរ្តិ៍"),(" KE ","កេរ្តិ៍"),("kon","កូន"),("nh","ខ្ញុំ")] {
        assert!(!has(&core,key,word),"{key} {word}");
        assert!(!core.convert(key).unwrap().candidates.iter().any(|hit| hit.khmer==word));
        let context=core.suggest_with_context(key,SuggestionFeatures::default(),&TypingContext::default()).unwrap();
        assert!(!context.exact_candidates.iter().any(|hit|hit.khmer==word));
        assert!(!context.result.candidates.iter().any(|hit|match &hit.candidate {
            khmer_engine::SuggestionCandidate::Khmer(hit)=>hit.khmer==word,_=>false
        }));
    }
    assert!(has(&core,"ke","គេ"));
    assert!(has(&core,"ko","កូន"));
    assert!(has(&core,"kid","កូន"));
    assert_eq!(core.to_binary(),binary);
    core.set_input_exclusions(vec![]).unwrap();
    for (input, result) in ["ke","kon","ko","kid","nh","ម","book"].iter().zip(before) { assert_eq!(core.suggest(input).unwrap(),result); }
}

#[test]
fn read_as_recoveries_and_personal_duplicates_cannot_revive_a_blocked_word() {
    let mut core=engine();
    core.set_pronunciations(vec![Pronunciation{word:"កូន".into(),read_as:"មិត".into()}]).unwrap();
    assert!(has(&core,"met","កូន"));
    let personal=Engine::from_personal_words_json(r#"[{"word":"កូន","spellings":["met","mett","other"]}]"#).unwrap();
    core.set_input_exclusions(vec![rule("កូន", &["met","mett"])]).unwrap();
    assert!(!has(&core,"met","កូន"));
    assert!(!has(&core,"mett","កូន"));
    let local=core.with_typing_overrides_json(r#"[{"language":"khmer","word":"កូន","rank":0,"soundMapEnabled":true}]"#).unwrap();
    for input in ["met","mett"] {
        let result=local.suggest_with_extension(&personal,input,SuggestionFeatures::default(),&TypingContext::default()).unwrap();
        assert!(!result.exact_candidates.iter().any(|hit| hit.khmer=="កូន"));
        assert!(!result.result.candidates.iter().any(|hit|match &hit.candidate {
            khmer_engine::SuggestionCandidate::Khmer(hit)=>hit.khmer=="កូន",_=>false
        }));
        assert!(!local.convert_with_extension(&personal,input).unwrap().candidates.iter().any(|hit|hit.khmer=="កូន"));
    }
    assert!(local.convert_with_extension(&personal,"other").unwrap().candidates.iter().any(|hit|hit.khmer=="កូន"));
}

#[test]
fn only_complete_input_is_excluded_not_components_khmer_or_english() {
    let mut core=engine();
    let composed=core.convert("kon met").unwrap();
    assert!(!composed.candidates.is_empty());
    let khmer=core.suggest("កូន").unwrap();
    let features=SuggestionFeatures{mode:khmer_engine::TypingMode::English,..Default::default()};
    let english=core.suggest_with_features("book",features).unwrap();
    core.set_input_exclusions(vec![rule("កូន", &["kon"])]).unwrap();
    assert_eq!(core.convert("kon met").unwrap(),composed);
    assert_eq!(core.suggest("កូន").unwrap(),khmer);
    assert_eq!(core.suggest_with_features("book",features).unwrap(),english);
}

#[test]
fn invalid_changes_are_atomic_and_exclusions_survive_other_runtime_edits() {
    let mut core=engine();
    core.set_input_exclusions(vec![rule("កូន", &["kon"])]).unwrap();
    for rules in [vec![rule("មិនមាន", &["test"])],vec![rule("កូន", &["!"])],vec![rule("កូន", &[""])],
        vec![rule("កូន", &["a".repeat(121).as_str()])],vec![rule("កូន", &["kon"]),rule("កូន", &["kid"])]] {
        assert!(core.set_input_exclusions(rules).is_err());assert!(!has(&core,"kon","កូន"));
    }
    assert!(core.set_input_exclusions_json(r#"[["កូន",["kon"]]]"#).is_err());
    core.set_pronunciations(vec![Pronunciation{word:"មិត្ត".into(),read_as:"កូន".into()}]).unwrap();
    assert!(!has(&core,"kon","កូន"));
    assert!(has(&core,"kon","មិត្ត"));
}

#[test]
fn base_policy_can_target_a_personal_only_word_and_survive_extension_replacement() {
    let mut core = engine();
    let personal = Engine::from_personal_words_json(r#"[{"word":"មិត្តកូន","spellings":["fixtureword"],"rank":7}]"#).unwrap();
    let features = SuggestionFeatures::default();
    let context = TypingContext::default();
    let before = core.suggest_with_extension(&personal,"fixtureword",features,&context).unwrap();
    assert!(before.exact_candidates.iter().any(|hit|hit.khmer=="មិត្តកូន"));
    let bytes = (core.to_binary(),personal.to_binary());
    assert!(core.set_input_exclusions(vec![rule("មិត្តកូន", &["fixtureword"])]).is_err());
    core.set_input_exclusions_with_extension(&personal,vec![rule("មិត្តកូន", &[" FIXTUREWORD "]),rule("កូន", &["kon"])]).unwrap();
    for (input,word) in [("fixtureword","មិត្តកូន"),("kon","កូន")] {
        let result = core.suggest_with_extension(&personal,input,features,&context).unwrap();
        assert!(!result.exact_candidates.iter().any(|hit|hit.khmer==word));
        assert!(!result.result.candidates.iter().any(|hit|matches!(&hit.candidate,
            khmer_engine::SuggestionCandidate::Khmer(hit) if hit.khmer==word)));
        assert!(!core.convert_with_extension(&personal,input).unwrap().candidates.iter().any(|hit|hit.khmer==word));
    }
    assert!(core.set_input_exclusions_with_extension_json(&personal,r#"[{"word":"មិត្តកូន","inputs":["!"]}]"#).is_err());
    assert!(core.set_input_exclusions_with_extension_json(&personal,r#"[["មិត្តកូន",["fixtureword"]]]"#).is_err());
    assert!(core.set_input_exclusions_with_extension(&personal,vec![rule("មិនមាន", &["test"])]).is_err());
    let replacement = Engine::from_personal_words_json(r#"[{"word":"មិត្តកូន","spellings":["fixtureword","other"],"rank":7}]"#).unwrap();
    assert!(!core.convert_with_extension(&replacement,"fixtureword").unwrap().candidates.iter().any(|hit|hit.khmer=="មិត្តកូន"));
    assert!(core.convert_with_extension(&replacement,"other").unwrap().candidates.iter().any(|hit|hit.khmer=="មិត្តកូន"));
    assert_eq!((core.to_binary(),personal.to_binary()),bytes);
    core.set_input_exclusions(vec![]).unwrap();
    assert_eq!(core.suggest_with_extension(&personal,"fixtureword",features,&context).unwrap(),before);
}
