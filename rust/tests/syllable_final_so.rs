use khmer_engine::{ApprovedDataset, Engine, SuggestionFeatures, TypingMode};

#[test]
fn syllable_final_so_accepts_h_and_s_inside_compounds_without_changing_onsets_or_aliases() {
    // Artificial spellings exercise parser boundaries only; no dictionary admissions.
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/syllable-final-so.json")).unwrap();
    let rows: Vec<_> = proof["cases"].as_array().unwrap().iter()
        .chain(proof["onsets"].as_array().unwrap()).collect();
    let words: Vec<_> = rows.iter().map(|row|row["word"].as_str().unwrap())
        .chain(proof["parts"].as_array().unwrap().iter().map(|part|part.as_str().unwrap())).collect();
    let mut data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion":2,
        "words":words.iter().enumerate().map(|(i,word)|serde_json::json!({
            "id":98000+i,"khmer":word,"rank":i,"status":"approved"
        })).collect::<Vec<_>>(),
        "mappings":[{"id":99000,"romanized":"mycoda","khmer":"កាសកា","rank":77,"status":"approved"}]
    })).unwrap();
    let core = Engine::new(data.clone()).unwrap();
    let binary = core.to_binary();
    let restored = Engine::from_binary(binary.clone()).unwrap();
    for (i,row) in rows.iter().enumerate() {
        let word = row["word"].as_str().unwrap();
        for input in row["inputs"].as_array().unwrap() {
            let input = input.as_str().unwrap();
            let result = core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
            let hit = result.exact_candidates.iter().find(|candidate|candidate.khmer == word)
                .unwrap_or_else(||panic!("{input}: {word}"));
            assert_eq!(hit.mapping_id,Some(98000+i as u64));
            assert_eq!(hit.rank,i as u32);
            assert_eq!(restored.suggest(input).unwrap(),core.suggest(input).unwrap());
            for features in [SuggestionFeatures {sound_map:false,..Default::default()},
                SuggestionFeatures {romanized:false,..Default::default()},
                SuggestionFeatures {mode:TypingMode::English,..Default::default()}] {
                assert!(!core.suggest_with_context(input,features,&Default::default()).unwrap()
                    .exact_candidates.iter().any(|candidate|candidate.khmer == word));
            }
        }
        if let Some(inputs) = row["rejected"].as_array() {
            for input in inputs {
                assert!(!core.suggest_with_context(input.as_str().unwrap(),Default::default(),&Default::default()).unwrap()
                    .exact_candidates.iter().any(|candidate|candidate.khmer == word),"{input}: {word}");
            }
        }
    }
    for sound_map in [true,false] {
        let result = core.suggest_with_context("mycoda",SuggestionFeatures {sound_map,..Default::default()},&Default::default()).unwrap();
        let hit = result.exact_candidates.iter().find(|candidate|candidate.mapping_id == Some(99000)).unwrap();
        assert_eq!(hit.khmer,"កាសកា");
        assert_eq!(hit.rank,77);
    }
    assert_eq!(core.to_binary(),binary);
    let local = Engine::from_personal_words_json(r#"[{"word":"សុខសប្បាយ","spellings":["mygreeting"]}]"#).unwrap();
    for input in ["sokhohbbay","sukhohbbay"] {
        assert!(!core.suggest_with_extension(&local,input,Default::default(),&Default::default()).unwrap()
            .exact_candidates.iter().any(|candidate|candidate.khmer == "សុខសប្បាយ"),"personal duplicate {input}");
    }
    // Generated-onset guards must never filter an explicitly saved h spelling.
    data.mappings.push(serde_json::from_value(serde_json::json!({
        "id":99001,"romanized":"sokhohbbay","khmer":"សុខសប្បាយ","rank":77,"status":"approved"
    })).unwrap());
    let saved = Engine::new(data).unwrap();
    let personal = Engine::from_personal_words_json(r#"[{"word":"សុខសប្បាយ","spellings":["sukhohbbay","mygreeting"]}]"#).unwrap();
    let saved_bytes = (saved.to_binary(),personal.to_binary());
    for sound_map in [true,false] {
        for input in ["sokhohbbay","sukhohbbay","mygreeting"] {
            assert!(saved.suggest_with_extension(&personal,input,SuggestionFeatures {sound_map,..Default::default()},&Default::default()).unwrap()
                .exact_candidates.iter().any(|candidate|candidate.khmer == "សុខសប្បាយ"),"saved {input}");
        }
        let hit = saved.suggest_with_context("sokhohbbay",SuggestionFeatures {sound_map,..Default::default()},&Default::default()).unwrap();
        assert!(hit.exact_candidates.iter().any(|candidate|candidate.mapping_id == Some(99001) && candidate.rank == 77));
    }
    assert_eq!((saved.to_binary(),personal.to_binary()),saved_bytes);
}
