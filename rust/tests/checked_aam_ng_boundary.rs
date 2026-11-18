use khmer_engine::{ApprovedDataset, Engine, SuggestionFeatures, TypingMode};

#[test]
fn checked_aam_ng_words_replace_mechanical_paths_and_preserve_controls() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/checked-aam-ng-boundary.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [], "words": words.iter().enumerate().map(|(i,w)| {
            serde_json::json!({"id": 989000+i, "khmer": w, "rank": i, "status": "approved"})
        }).collect::<Vec<_>>()
    })).unwrap();
    let core = Engine::new(data).unwrap();
    let bytes = core.to_binary();
    for row in proof["yes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
        let result = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let hit = result.exact_candidates.iter().find(|c| c.khmer == word).unwrap_or_else(|| panic!("{input}: {word}"));
        let index = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(hit.mapping_id, Some(989000 + index as u64)); assert_eq!(hit.rank, index as i32);
        for features in [SuggestionFeatures {sound_map:false, ..Default::default()},
            SuggestionFeatures {romanized:false, ..Default::default()},
            SuggestionFeatures {mode:TypingMode::English, ..Default::default()}] {
            assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word));
        }
    }
    for row in proof["no"].as_array().unwrap() {
        assert!(!core.suggest_with_context(row[0].as_str().unwrap(), Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == row[1].as_str().unwrap()), "{row}");
    }
    for row in proof["unchangedControls"].as_array().unwrap() {
        assert!(core.suggest_with_context(row[0].as_str().unwrap(), Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == row[1].as_str().unwrap()), "{row}");
    }
    assert_eq!(core.to_binary(), bytes);
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("veangnon").unwrap(), core.suggest("veangnon").unwrap());
}
