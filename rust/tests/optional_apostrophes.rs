use khmer_engine::{Engine, Pronunciation, SuggestionCandidate, SuggestionFeatures, TypingMode};
use serde_json::json;

fn fixture() -> Engine {
    Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":(["ពូន","ប្អូន","ផ្អែម","ស្អាត","ចង្អុល","ក្អក","កូន","មិត្ត","ភាព","មិត្តភាព"].iter().enumerate()
            .map(|(i,khmer)|json!({"id":100+i,"khmer":khmer,"rank":i,"status":"approved"})).collect::<Vec<_>>()),
        "mappings":[{"id":900,"romanized":"q'foo","khmer":"កូន","rank":0,"status":"approved"}]
    }).to_string()).unwrap()
}

#[test]
fn omitted_apostrophes_are_completions_only_and_keep_exact_words_aliases_and_dictionary_bytes() {
    let mut core=fixture();let bytes=core.to_binary();
    let canonical=core.sound_map_inputs("ប្អូន","").unwrap();
    for (input,word) in [("poun","ប្អូន"),("pon","ប្អូន"),("po","ប្អូន"),("phaem","ផ្អែម"),
        ("paem","ផ្អែម"),("sat","ស្អាត"),("jongol","ចង្អុល"),("jongul","ចង្អុល"),("kark","ក្អក"),
        ("ppoun","ប្អូន"),("ppou","ប្អូន"),("phhaem","ផ្អែម"),("jonggol","ចង្អុល")] {
        let result=core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
        let hit=result.result.candidates.iter().find(|hit|matches!(&hit.candidate,SuggestionCandidate::Khmer(c) if c.khmer==word)).unwrap_or_else(||panic!("{input}: {word}"));
        assert!(hit.completion.as_ref().is_some_and(|key|key.contains('\'')),"{input}");
        assert!(hit.correction.is_none());
        assert!(!result.exact_candidates.iter().any(|c|c.khmer==word));
    }
    let result=core.suggest_with_context("poun",Default::default(),&Default::default()).unwrap();
    assert_eq!(result.exact_candidates[0].khmer,"ពូន");
    assert!(matches!(&result.result.candidates[0].candidate,SuggestionCandidate::Khmer(c) if c.khmer=="ពូន"));
    let explicit=core.suggest_with_context("p'oun",Default::default(),&Default::default()).unwrap();
    assert!(explicit.exact_candidates.iter().any(|c|c.khmer=="ប្អូន"));
    assert!(explicit.result.candidates.iter().any(|hit|hit.completion.is_none()&&matches!(&hit.candidate,SuggestionCandidate::Khmer(c) if c.khmer=="ប្អូន")));
    for input in ["qfoo","'poun","p''oun","poun ","pppoun","pooun"] {
        let result=core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
        assert!(!result.result.candidates.iter().any(|hit|hit.correction.is_none()&&matches!(&hit.candidate,SuggestionCandidate::Khmer(c) if c.khmer=="ប្អូន"||c.khmer=="កូន")),"{input}");
    }
    assert_eq!(core.sound_map_inputs("ប្អូន","").unwrap().inputs,canonical.inputs);assert_eq!(core.to_binary(),bytes);
    core.set_shorthands_json(r#"[{"word":"មិត្ត","inputs":["s'key"]}]"#).unwrap();
    assert!(!core.suggest("skey").unwrap().candidates.iter().any(|hit|matches!(&hit.candidate,SuggestionCandidate::Khmer(c) if c.khmer=="មិត្ត")));
    core.set_input_exclusions_json(r#"[{"word":"ប្អូន","inputs":["poun"]}]"#).unwrap();
    assert!(!core.suggest("poun").unwrap().candidates.iter().any(|hit|matches!(&hit.candidate,SuggestionCandidate::Khmer(c) if c.khmer=="ប្អូន")));
}

#[test]
fn personal_omission_completions_preserve_ordinary_prefixes_ranks_and_duplicate_witnesses() {
    let base=Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":[{"id":100,"khmer":"កូន","rank":20,"status":"approved"},{"id":101,"khmer":"ប្អូន","rank":0,"status":"approved"}],
        "mappings":[{"id":900,"romanized":"pounx","khmer":"កូន","rank":20,"status":"approved"}]
    }).to_string()).unwrap();
    let personal=Engine::from_personal_words_json(r#"[{"word":"ប្អូន","spellings":["my-sibling"]}]"#).unwrap();
    let bytes=[base.to_binary(),personal.to_binary()];
    let ranked=personal.with_typing_overrides_json(r#"[{"language":"khmer","word":"ប្អូន","rank":-100}]"#).unwrap();
    for extra in [&personal,&ranked] {for (first,second) in [(&base,extra),(extra,&base)] {
        let result=first.suggest_with_extension(second,"poun",Default::default(),&Default::default()).unwrap();
        assert_eq!(result.result.candidates.iter().filter_map(|hit|match &hit.candidate {SuggestionCandidate::Khmer(c)=>Some(c.khmer.as_str()),_=>None}).collect::<Vec<_>>(),["កូន","ប្អូន"]);
        assert!(result.exact_candidates.is_empty());
        assert_eq!(result.result.candidates[1].completion.as_deref(),Some("p'oun"));
    }}
    assert_eq!(base.to_binary(),bytes[0]);assert_eq!(personal.to_binary(),bytes[1]);
    let duplicate=Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":[{"id":101,"khmer":"ប្អូន","rank":0,"status":"approved"}],
        "mappings":[{"id":901,"romanized":"pounx","khmer":"ប្អូន","rank":0,"status":"approved"}]
    }).to_string()).unwrap();
    let result=duplicate.suggest_with_extension(&personal,"poun",Default::default(),&Default::default()).unwrap();
    assert_eq!(result.result.candidates.len(),1);assert_eq!(result.result.candidates[0].completion.as_deref(),Some("pounx"));
    assert!(matches!(&result.result.candidates[0].candidate,SuggestionCandidate::Khmer(c) if c.mapping_id==Some(901)));
}

#[test]
fn recovery_and_commonness_do_not_promote_omission_over_ordinary_completions() {
    let mut base=Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":(["កូន","មិត្ត","ផ្អែម"].iter().enumerate().map(|(i,khmer)|json!({"id":100+i,"khmer":khmer,"rank":i,"status":"approved"})).collect::<Vec<_>>()),
        "mappings":[{"id":900,"romanized":"phaemx","khmer":"កូន","rank":0,"status":"approved"},
            {"id":901,"romanized":"xhaem","khmer":"មិត្ត","rank":0,"status":"approved"}]
    }).to_string()).unwrap();
    let metadata=|language:&str,words:serde_json::Value|json!({"format":"kmeow-word-metadata-v2","language":language,"snapshotId":"a".repeat(64),"words":words}).to_string();
    let km=json!([{"id":"1","word":"កូន","commonnessLevel":3,"context":null,"partsOfSpeech":[],"meanings":[],"translations":[],"sources":[]},
        {"id":"2","word":"ផ្អែម","commonnessLevel":1,"context":null,"partsOfSpeech":[],"meanings":[],"translations":[],"sources":[]}]);
    base.load_word_metadata(&metadata("khmer",km),&metadata("english",json!([]))).unwrap();
    let extra=Engine::from_personal_words_json(r#"[{"word":"ផ្អែម","spellings":["my-sweet"]}]"#).unwrap();
    for result in [base.suggest_with_context("phaem",Default::default(),&Default::default()).unwrap(),
        base.suggest_with_extension(&extra,"phaem",Default::default(),&Default::default()).unwrap()] {
        assert!(matches!(&result.result.candidates[0].candidate,SuggestionCandidate::Khmer(c) if c.khmer=="កូន"));
        assert!(result.result.candidates.iter().any(|hit|hit.correction.is_some()));
        let optional=result.result.candidates.iter().position(|hit|matches!(&hit.candidate,SuggestionCandidate::Khmer(c) if c.khmer=="ផ្អែម")).unwrap();
        let correction=result.result.candidates.iter().position(|hit|hit.correction.is_some()).unwrap();assert!(optional<correction);
    }
}

#[test]
fn optional_apostrophe_completions_follow_read_as_boundaries_and_feature_controls() {
    let mut core=fixture();let bytes=core.to_binary();
    for features in [SuggestionFeatures{sound_map:false,..Default::default()},
        SuggestionFeatures{romanized:false,..Default::default()},SuggestionFeatures{mode:TypingMode::English,..Default::default()}] {
        assert!(!core.suggest_with_context("poun",features,&Default::default()).unwrap().result.candidates.iter()
            .any(|hit|matches!(&hit.candidate,SuggestionCandidate::Khmer(c) if c.khmer=="ប្អូន")));
    }
    core.set_pronunciations(vec![Pronunciation {word:"មិត្ត".into(),read_as:"ប្អូន".into()}]).unwrap();
    for (input,word) in [("poun","មិត្ត"),("pounpeap","មិត្តភាព")] {
        let result=core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
        assert!(result.result.candidates.iter().any(|hit|hit.completion.is_some()&&matches!(&hit.candidate,SuggestionCandidate::Khmer(c) if c.khmer==word)));
        assert!(!result.exact_candidates.iter().any(|c|c.khmer==word));
    }
    core.set_pronunciations(vec![Pronunciation {word:"មិត្ត".into(),read_as:"ប្អូន".repeat(12).into()}]).unwrap();
    let canonical=core.sound_map_inputs("មិត្ត","").unwrap().inputs[0].clone();
    assert!(core.suggest_with_context(&canonical,Default::default(),&Default::default()).unwrap().exact_candidates.iter().any(|c|c.khmer=="មិត្ត"));
    for input in [canonical.replace("'",""),canonical.replacen("'","",1),
        canonical.rsplit_once('\'').map(|(prefix,suffix)|format!("{prefix}{suffix}")).unwrap()] {
        assert!(core.suggest(&input).unwrap().candidates.iter().any(|hit|hit.completion.is_some()&&matches!(&hit.candidate,SuggestionCandidate::Khmer(c) if c.khmer=="មិត្ត")),"{input}");
    }
    core.set_pronunciations(vec![]).unwrap();assert_eq!(core.to_binary(),bytes);
}
