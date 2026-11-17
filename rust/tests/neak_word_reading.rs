use khmer_engine::{ApprovedDataset, Engine, SuggestionFeatures, TypingMode};

#[test]
fn neak_word_reading_composes_without_aliases_or_tail_rule_leaks() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/neak-word-reading.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [], "words": words.iter().enumerate().map(|(i,w)| {
            serde_json::json!({"id": 92700 + i, "khmer": w, "rank": i, "status": "approved"})
        }).collect::<Vec<_>>()
    })).unwrap();
    let core = Engine::new(data.clone()).unwrap();
    let binary = core.to_binary();
    for group in ["yes", "compatibility"] {
        for row in proof[group].as_array().unwrap() {
            let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
            let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let hit = query.exact_candidates.iter().find(|c| c.khmer == word)
                .unwrap_or_else(|| panic!("{input}: {word}"));
            let index = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
            assert_eq!(hit.mapping_id, Some(92700 + index as u64));
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
        assert!(!core.suggest_with_context(input, Default::default(), &Default::default()).unwrap()
            .exact_candidates.iter().any(|c| c.khmer == word), "{input}: {word}");
    }
    assert_eq!(core.to_binary(), binary);
    assert_eq!(Engine::from_binary(binary).unwrap().suggest("neakluk").unwrap(), core.suggest("neakluk").unwrap());
    assert!(core.convert("neakluk").unwrap().candidates.is_empty());
    data.mappings = serde_json::from_value(serde_json::json!([
        {"id": 99620, "romanized": "nok", "khmer": "អ្នក", "rank": 77, "status": "approved"},
        {"id": 99621, "romanized": "jah", "khmer": "ចា៎ស", "rank": 0, "status": "approved"}
    ])).unwrap();
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("nok", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99620));
        assert_eq!(query.exact_candidates[0].rank, 77);
        assert_eq!(saved.suggest_with_context("jah", features, &Default::default()).unwrap().exact_candidates[0].khmer, "ចា៎ស");
    }
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 38);
}
