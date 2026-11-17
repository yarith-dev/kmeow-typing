use khmer_engine::{ApprovedDataset, Engine, SuggestionFeatures, TypingMode};

#[test]
fn matched_science_part_replaces_full_readings_without_per_word_allowlists() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/checked-science-ending.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [], "words": words.iter().enumerate().map(|(i,w)| {
            serde_json::json!({"id": 98100 + i, "khmer": w, "rank": i, "status": "approved"})
        }).collect::<Vec<_>>()
    })).unwrap();
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for group in ["soundYes", "compatibility", "yes"] {
        for row in proof[group].as_array().unwrap() {
            let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
            let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let hit = query.exact_candidates.iter().find(|c| c.khmer == word)
                .unwrap_or_else(|| panic!("{input}: {word}"));
            let index = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
            assert_eq!(hit.mapping_id, Some(98100 + index as u64));
            assert_eq!(hit.rank, index as u32);
            for features in [SuggestionFeatures {sound_map: false, ..Default::default()},
                SuggestionFeatures {romanized: false, ..Default::default()},
                SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
                assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap()
                    .exact_candidates.iter().any(|c| c.khmer == word));
            }
        }
    }
    for row in proof["no"].as_array().unwrap() {
        let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
        // Checked prefix choices remain independent. Changed or
        // cancellation-exposed spellings must not reveal the short part.
        assert!(!core.suggest_with_context(input, Default::default(), &Default::default()).unwrap()
            .exact_candidates.iter().any(|c| c.khmer == word), "{input}: {word}");
    }
    assert_eq!(core.to_binary(), bytes);
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("vithisah").unwrap(), core.suggest("vithisah").unwrap());
    assert_eq!(proof["aliasYes"].as_array().unwrap().len(), 0);
    assert_eq!(proof["proof"].as_array().unwrap().len(), 13);
    assert_eq!(proof["currentAppWords"].as_array().unwrap().len(), 6);
    assert_eq!(proof["sourceOnly"].as_array().unwrap().len(), 7);
}

#[test]
fn shared_part_override_preserves_saved_and_personal_whole_input_spellings() {
    let base = Engine::from_json(r#"{"schemaVersion":2,
        "words":[{"id":1,"khmer":"សាស្ត្រ","rank":0,"status":"approved"}],
        "mappings":[{"id":2,"romanized":"savedscience","khmer":"សាស្ត្រ","rank":7,"status":"approved"}] }"#).unwrap();
    let personal = Engine::from_personal_words_json(r#"[
        {"word":"សាស្ត្រ","spellings":["sastro","myscience"]},
        {"word":"កាសាស្ត្រ","spellings":["mycompound"]}
    ]"#).unwrap();
    let before = (base.to_binary(), personal.to_binary());
    assert!(!base.suggest_with_context("sastro", Default::default(), &Default::default()).unwrap()
        .exact_candidates.iter().any(|candidate| candidate.khmer == "សាស្ត្រ"));
    for sound_map in [true, false] {
        let features = SuggestionFeatures {sound_map, ..Default::default()};
        for (input, word) in [("savedscience", "សាស្ត្រ"), ("sastro", "សាស្ត្រ"),
            ("myscience", "សាស្ត្រ"), ("mycompound", "កាសាស្ត្រ")] {
            let query = base.suggest_with_extension(&personal, input, features, &Default::default()).unwrap();
            assert!(query.exact_candidates.iter().any(|candidate| candidate.khmer == word), "{input}");
            assert_eq!(base.convert_with_extension(&personal, input).unwrap().candidates[0].khmer, word);
        }
    }
    assert_eq!(base.convert("savedscience").unwrap().candidates[0].mapping_id, Some(2));
    assert_eq!((base.to_binary(), personal.to_binary()), before);
}
