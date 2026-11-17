use khmer_engine::{Engine, SuggestionFeatures, TypingMode};

#[test]
fn shortened_unwritten_vowels_use_o_without_replacing_explicit_spellings() {
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../test/fixtures/short-inherent-o.json"
    )).unwrap();
    let rows: Vec<_> = proof["readings"].as_array().unwrap().iter()
        .chain(proof["unchanged"].as_array().unwrap()).collect();
    let data = serde_json::json!({"schemaVersion":2,
        "words":rows.iter().enumerate().map(|(i,row)|serde_json::json!({
            "id":99100+i,"khmer":row[0],"rank":i,"status":"approved"
        })).collect::<Vec<_>>(),"mappings":[]});
    let core = Engine::from_json(&data.to_string()).unwrap();
    let before = core.to_binary();
    let restored = Engine::from_binary(before.clone()).unwrap();
    let exact = |engine: &Engine, input: &str, word: &str| engine
        .suggest_with_context(input, Default::default(), &Default::default()).unwrap()
        .exact_candidates.iter().any(|candidate| candidate.khmer == word);
    for row in rows {
        let word = row[0].as_str().unwrap();
        for input in row[1].as_array().unwrap() {
            let input = input.as_str().unwrap();
            assert!(exact(&core,input,word),"{input}: {word}");
            assert_eq!(core.suggest(input).unwrap(),restored.suggest(input).unwrap());
        }
        for input in row[2].as_array().unwrap() {
            assert!(!exact(&core,input.as_str().unwrap(),word),"{input}: {word}");
        }
    }
    for features in [SuggestionFeatures {sound_map:false,..Default::default()},
        SuggestionFeatures {romanized:false,..Default::default()},
        SuggestionFeatures {mode:TypingMode::English,..Default::default()}] {
        assert!(!core.suggest_with_context("rolot",features,&Default::default()).unwrap()
            .exact_candidates.iter().any(|candidate|candidate.khmer=="រលត់"));
    }
    let mut saved_data = data;
    saved_data["mappings"] = serde_json::json!([
        {"id":99900,"romanized":"rolut","khmer":"រលត់","rank":77,"status":"approved"}
    ]);
    let saved = Engine::from_json(&saved_data.to_string()).unwrap();
    let personal = Engine::from_personal_words_json(
        r#"[{"word":"រលត់","spellings":["myreading","rolut"]}]"#
    ).unwrap();
    let saved_before = saved.to_binary();
    let personal_before = personal.to_binary();
    for sound_map in [true,false] {
        let features = SuggestionFeatures {sound_map,..Default::default()};
        let result = saved.suggest_with_context("rolut",features,&Default::default()).unwrap();
        assert!(result.exact_candidates.iter().any(|c|c.mapping_id==Some(99900)&&c.rank==77));
        for input in ["myreading","rolut"] {
            assert!(core.suggest_with_extension(&personal,input,features,&Default::default()).unwrap()
                .exact_candidates.iter().any(|c|c.khmer=="រលត់"));
        }
    }
    assert_eq!(core.to_binary(),before);
    assert_eq!(saved.to_binary(),saved_before);
    assert_eq!(personal.to_binary(),personal_before);
}
