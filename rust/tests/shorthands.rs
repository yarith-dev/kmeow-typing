use khmer_engine::{Engine,SuggestionCandidate,SuggestionFeatures,TypingContext};
use serde_json::json;
fn engine()->Engine {
    Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":(["ខ្ញុំ","កូន","មិត្ត","កូនមិត្ត"].iter().enumerate().map(|(id,word)|json!({"id":id+1,"khmer":word,"rank":id,"status":"approved"})).collect::<Vec<_>>()),
        "mappings":[{"id":20,"romanized":"kid","khmer":"កូន","rank":2,"status":"approved"}]
    }).to_string()).unwrap()
}
fn selected(core:&Engine,input:&str,word:&str)->bool {
    let result=core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
    result.result.candidates.iter().any(|hit|matches!(&hit.candidate,SuggestionCandidate::Khmer(hit) if hit.khmer==word))
}
#[test]
fn built_in_shorthand_is_editable_separate_and_restorable() {
    let mut core=engine();let before=core.suggest("nh").unwrap();let binary=core.to_binary();
    assert_eq!(core.word_shorthands("ខ្ញុំ"),["nh"]);
    assert!(!core.sound_map_inputs("ខ្ញុំ","").unwrap().inputs.contains(&"nh".into()));
    core.set_shorthands_json(r#"[{"word":"ខ្ញុំ","inputs":["meow"]}]"#).unwrap();
    assert!(!selected(&core,"nh","ខ្ញុំ"));assert!(selected(&core,"meow","ខ្ញុំ"));
    assert!(selected(&core,"khnhom","ខ្ញុំ"));
    core.set_shorthands_json(r#"[{"word":"ខ្ញុំ","inputs":[]}]"#).unwrap();
    assert!(!selected(&core,"nh","ខ្ញុំ"));assert!(!selected(&core,"meow","ខ្ញុំ"));
    core.set_shorthands_json("[]").unwrap();assert_eq!(core.suggest("nh").unwrap(),before);assert_eq!(core.to_binary(),binary);
}
#[test]
fn overlapping_shorthands_keep_every_generated_spelling_visible() {
    let mut core=Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"englishWords":[],
        "words":[{"id":1,"khmer":"កម្ម","rank":0,"status":"approved"}]
    }).to_string()).unwrap();
    let baseline=core.suggest("kam").unwrap();let bytes=core.to_binary();
    for inputs in [vec!["kam","shortcut"],vec!["kam"],vec![]] {
        core.set_shorthands_json(&json!([{"word":"កម្ម","inputs":inputs}]).to_string()).unwrap();
        let before=core.suggest("kam").unwrap();
        assert_eq!(core.sound_map_inputs("កម្ម","").unwrap().inputs,["kam"]);
        let page=core.browse_words("កម្ម",None,100).unwrap();
        assert_eq!(page.words[0].sound_map_inputs,["kam"]);
        assert_eq!(core.suggest("kam").unwrap(),before);
        assert_eq!(before,baseline);
    }
    core.set_shorthands_json("[]").unwrap();assert_eq!(core.to_binary(),bytes);
}
#[test]
fn shorthand_uses_normal_acceptance_and_never_propagates() {
    let mut core=engine();let baseline=core.suggest("kon").unwrap();
    core.set_shorthands_json(r#"[{"word":"កូន","inputs":["bb","kid"]}]"#).unwrap();
    for input in ["b","bb","kid"] {
        let result=core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
        let hit=result.result.candidates.iter().find(|hit|matches!(&hit.candidate,SuggestionCandidate::Khmer(hit) if hit.khmer=="កូន")).unwrap();
        assert!(!hit.tap_only,"{input}");
    }
    assert!(core.convert("bb").unwrap().candidates.is_empty());
    assert!(!selected(&core,"bbmet","កូនមិត្ត"));
    assert!(!core.convert("bb kid").unwrap().candidates.iter().any(|hit|hit.khmer.contains("កូនកូន")));
    assert_eq!(core.suggest("kon").unwrap(),baseline);
    assert!(core.convert("kid").unwrap().candidates.iter().any(|hit|hit.khmer=="កូន"));
    core.set_input_exclusions_json(r#"[{"word":"កូន","inputs":["bb"]}]"#).unwrap();assert!(!selected(&core,"bb","កូន"));
}
#[test]
fn personal_duplicates_keep_base_shorthand_choices_and_invalid_edits_are_atomic() {
    let mut core=engine();let personal=Engine::from_personal_words_json(r#"[{"word":"ខ្ញុំ","spellings":[]},{"word":"មិត្តកូន","spellings":["personalword"]}]"#).unwrap();
    core.set_shorthands_with_extension_json(&personal,r#"[{"word":"ខ្ញុំ","inputs":[]},{"word":"មិត្តកូន","inputs":["mm"]}]"#).unwrap();
    let query=|core:&Engine,input|core.suggest_with_extension(&personal,input,SuggestionFeatures::default(),&TypingContext::default()).unwrap();
    assert!(!query(&core,"nh").exact_candidates.iter().any(|hit|hit.khmer=="ខ្ញុំ"));
    let before=query(&core,"mm");assert!(before.result.candidates.iter().any(|hit|!hit.tap_only&&matches!(&hit.candidate,SuggestionCandidate::Khmer(hit) if hit.khmer=="មិត្តកូន")));
    for json in [r#"[{"word":"កូន","inputs":["!"]}]"#,r#"[{"word":"កូន","inputs":["a b"]}]"#,r#"[{"word":"មិនមាន","inputs":["x"]}]"#,r#"[["កូន",["x"]]]"#] {
        assert!(core.set_shorthands_with_extension_json(&personal,json).is_err());assert_eq!(query(&core,"mm"),before);
    }
}

#[test]
fn read_as_keeps_builtin_shorthand_unless_explicitly_removed_or_excluded() {
    let mut core=engine();let bytes=core.to_binary();
    core.set_pronunciations(vec![khmer_engine::Pronunciation{word:"ខ្ញុំ".into(),read_as:"មិត".into()}]).unwrap();
    for input in ["nh","met","mit"] {
        let result=core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
        assert!(result.exact_candidates.iter().any(|hit|hit.khmer=="ខ្ញុំ"),"{input}");
        assert!(result.result.candidates.iter().any(|hit|!hit.tap_only&&matches!(&hit.candidate,SuggestionCandidate::Khmer(hit) if hit.khmer=="ខ្ញុំ")),"{input}");
    }
    let personal=Engine::from_personal_words_json(r#"[{"word":"ខ្ញុំ","spellings":[]}]"#).unwrap();
    let merged=core.suggest_with_extension(&personal,"nh",Default::default(),&Default::default()).unwrap();
    assert!(merged.exact_candidates.iter().any(|hit|hit.khmer=="ខ្ញុំ"));
    core.set_input_exclusions_json(r#"[{"word":"ខ្ញុំ","inputs":["nh"]}]"#).unwrap();assert!(!selected(&core,"nh","ខ្ញុំ"));
    core.set_input_exclusions_json("[]").unwrap();assert!(selected(&core,"nh","ខ្ញុំ"));
    core.set_shorthands_json(r#"[{"word":"ខ្ញុំ","inputs":[]}]"#).unwrap();
    assert!(!selected(&core,"nh","ខ្ញុំ"));assert!(selected(&core,"met","ខ្ញុំ"));
    core.set_shorthands_json(r#"[{"word":"ខ្ញុំ","inputs":["meow"]}]"#).unwrap();
    assert!(!selected(&core,"nh","ខ្ញុំ"));assert!(selected(&core,"meow","ខ្ញុំ"));
    core.set_shorthands_json("[]").unwrap();assert!(selected(&core,"nh","ខ្ញុំ"));assert_eq!(core.to_binary(),bytes);
}

#[test]
fn normal_acceptance_of_shorthands_preserves_rare_word_protection() {
    let mut core=engine();
    core.set_shorthands_json(r#"[{"word":"កូន","inputs":["bb"]}]"#).unwrap();
    let personal=Engine::from_personal_words_json(r#"[{"word":"កូន","spellings":["bb"]}]"#).unwrap();
    for level in [2,4,5] {
        core.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","khmer":[{"word":"កូន","level":level}],"english":[]}).to_string()).unwrap();
        let result=core.suggest_with_extension(&personal,"bb",Default::default(),&Default::default()).unwrap();
        let hit=result.result.candidates.iter().find(|hit|matches!(&hit.candidate,SuggestionCandidate::Khmer(hit) if hit.khmer=="កូន")).unwrap();
        assert_eq!(hit.tap_only,level>=4,"{level}");
    }
}
