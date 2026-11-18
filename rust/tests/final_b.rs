use khmer_engine::{Engine, SuggestionFeatures, TypingMode};

#[test]
fn final_ba_adds_b_without_replacing_p_or_other_letters_and_explicit_spellings() {
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../test/fixtures/final-b.json"
    )).unwrap();
    let rows: Vec<_> = proof["readings"].as_array().unwrap().iter()
        .chain(proof["unchanged"].as_array().unwrap()).collect();
    let mut data = serde_json::json!({"schemaVersion":2,
        "words":rows.iter().enumerate().map(|(i,row)|serde_json::json!({
            "id":99300+i,"khmer":row[0],"rank":i,"status":"approved"
        })).collect::<Vec<_>>(),"mappings":[]});
    let core = Engine::from_json(&data.to_string()).unwrap();
    let before = core.to_binary();
    let restored = Engine::from_binary(before.clone()).unwrap();
    for (field, added) in [("readings",true),("unchanged",false)] {
        for row in proof[field].as_array().unwrap() {
            let word = row[0].as_str().unwrap();
            for (group, expected) in [(1,true),(2,added)] {
                for input in row[group].as_array().unwrap() {
                    let input = input.as_str().unwrap();
                    let result = core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
                    assert_eq!(result.exact_candidates.iter().any(|c|c.khmer==word),expected,"{input}: {word}");
                    assert_eq!(core.suggest(input).unwrap(),restored.suggest(input).unwrap());
                }
            }
        }
    }
    for features in [SuggestionFeatures {sound_map:false,..Default::default()},
        SuggestionFeatures {romanized:false,..Default::default()},
        SuggestionFeatures {mode:TypingMode::English,..Default::default()}] {
        assert!(!core.suggest_with_context("brab",features,&Default::default()).unwrap()
            .exact_candidates.iter().any(|c|c.khmer=="ប្រាប់"));
    }
    data["soundMapDisabledWords"] = serde_json::json!(["ប្រាប់"]);
    data["mappings"] = serde_json::json!([
        {"id":99910,"romanized":"customp","khmer":"ប្រាប់","rank":77,"status":"approved"}
    ]);
    let saved = Engine::from_json(&data.to_string()).unwrap();
    for input in ["brap","brab"] {
        assert!(!saved.suggest_with_context(input,Default::default(),&Default::default()).unwrap()
            .exact_candidates.iter().any(|c|c.khmer=="ប្រាប់"));
    }
    let personal = Engine::from_personal_words_json(
        r#"[{"word":"ប្រាប់","spellings":["mybpword","customp"],"rank":23}]"#
    ).unwrap();
    let saved_before = saved.to_binary();
    let personal_before = personal.to_binary();
    for sound_map in [true,false] {
        let features = SuggestionFeatures {sound_map,..Default::default()};
        assert!(saved.suggest_with_context("customp",features,&Default::default()).unwrap()
            .exact_candidates.iter().any(|c|c.mapping_id==Some(99910)&&c.rank==77));
        for input in ["mybpword","customp"] {
            assert!(core.suggest_with_extension(&personal,input,features,&Default::default()).unwrap()
                .exact_candidates.iter().any(|c|c.khmer=="ប្រាប់"));
        }
        assert!(!core.suggest_with_extension(&personal,"customb",features,&Default::default()).unwrap()
            .exact_candidates.iter().any(|c|c.khmer=="ប្រាប់"));
    }
    assert_eq!(core.to_binary(),before);
    assert_eq!(saved.to_binary(),saved_before);
    assert_eq!(personal.to_binary(),personal_before);
}
