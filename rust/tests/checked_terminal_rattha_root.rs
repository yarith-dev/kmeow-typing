use khmer_engine::{ApprovedDataset, Engine, SuggestionFeatures, TypingMode};

#[test]
fn checked_terminal_rattha_root_preserve_full_readings_and_guard_scope() {
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../test/fixtures/checked-terminal-rattha-root.json"
    )).unwrap();
    let words = proof["words"].as_array().unwrap();
    assert_eq!(proof["proof"].as_array().unwrap().len(), 3);
    assert_eq!(proof["currentAppWords"].as_array().unwrap().len(), 3);
    assert_eq!(proof["referenceOnly"].as_array().unwrap().len(), 0);
    let data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [],
        "words": words.iter().enumerate().map(|(i, word)| serde_json::json!({
            "id": 101400 + i, "khmer": word, "rank": i, "status": "approved"
        })).collect::<Vec<_>>()
    })).unwrap();
    let engine = Engine::new(data).unwrap();
    let bytes = engine.to_binary();
    for group in ["soundYes", "compatibility"] {
        for row in proof[group].as_array().unwrap() {
            let input = row[0].as_str().unwrap();
            let word = row[1].as_str().unwrap();
            let result = engine.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let hit = result.exact_candidates.iter().find(|c| c.khmer == word)
                .unwrap_or_else(|| panic!("{input}: {word}"));
            let index = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
            assert_eq!(hit.mapping_id, Some(101400 + index as u64));
            assert_eq!(hit.rank, index as i32);
            for features in [
                SuggestionFeatures { sound_map: false, ..Default::default() },
                SuggestionFeatures { romanized: false, ..Default::default() },
                SuggestionFeatures { mode: TypingMode::English, ..Default::default() },
            ] {
                assert!(!engine.suggest_with_context(input, features, &Default::default()).unwrap()
                    .exact_candidates.iter().any(|c| c.khmer == word), "inactive: {input}: {word}");
            }
        }
    }
    for row in proof["no"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        assert!(!engine.suggest_with_context(input, Default::default(), &Default::default()).unwrap()
            .exact_candidates.iter().any(|c| c.khmer == word), "scope: {input}: {word}");
    }
    assert_eq!(engine.to_binary(), bytes);
    let restored = Engine::from_binary(bytes).unwrap();
    for row in proof["soundYes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        assert_eq!(restored.suggest(input).unwrap(), engine.suggest(input).unwrap());
    }
}
