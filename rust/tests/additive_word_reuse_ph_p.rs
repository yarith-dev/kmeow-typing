use khmer_engine::{ApprovedDataset, Engine, SuggestionFeatures, TypingMode};

#[test]
fn additive_words_and_ph_p_preserve_full_paths_guards_and_saved_identities() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/additive-word-reuse-ph-p.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [], "words": words.iter().enumerate().map(|(i,w)| {
            serde_json::json!({"id": 92900 + i, "khmer": w, "rank": i, "status": "approved"})
        }).collect::<Vec<_>>()
    })).unwrap();
    let core = Engine::new(data.clone()).unwrap();
    let binary = core.to_binary();
    for group in ["yes", "compatibility", "phP"] {
        for row in proof[group].as_array().unwrap() {
            let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
            let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let hit = query.exact_candidates.iter().find(|c| c.khmer == word)
                .unwrap_or_else(|| panic!("{input}: {word}"));
            let index = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
            assert_eq!(hit.mapping_id, Some(92900 + index as u64));
            assert_eq!(hit.rank, index as i32);
            for features in [SuggestionFeatures {sound_map: false, ..Default::default()},
                SuggestionFeatures {romanized: false, ..Default::default()},
                SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
                assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap()
                    .exact_candidates.iter().any(|c| c.khmer == word));
            }
        }
    }
    // Repetition no longer inherits generated Latin readings; retain these negative cases.
    for row in proof["no"].as_array().unwrap().iter().chain(proof["repetition"].as_array().unwrap()) {
        let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
        assert!(!core.suggest_with_context(input, Default::default(), &Default::default()).unwrap()
            .exact_candidates.iter().any(|c| c.khmer == word), "{input}: {word}");
    }
    assert_eq!(core.to_binary(), binary);
    assert_eq!(Engine::from_binary(binary).unwrap().suggest("mjahpteah").unwrap(), core.suggest("mjahpteah").unwrap());
    assert!(core.convert("mjahpteah").unwrap().candidates.is_empty());
    data.mappings = serde_json::from_value(serde_json::json!([
        {"id": 99620, "romanized": "bontupteuk", "khmer": "បន្ទប់ទឹក", "rank": 77, "status": "approved"},
        {"id": 99622, "romanized": "khurkbal", "khmer": "ខួរក្បាល", "rank": 0, "status": "approved"},
        {"id": 99621, "romanized": "jah", "khmer": "ចា៎ស", "rank": 0, "status": "approved"}
    ])).unwrap();
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("bontupteuk", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99620));
        assert_eq!(query.exact_candidates[0].rank, 77);
        assert_eq!(saved.suggest_with_context("jah", features, &Default::default()).unwrap().exact_candidates[0].khmer, "ចា៎ស");
    }
    assert_eq!(proof["checkedUnits"][0]["words"].as_array().unwrap().len(), 3);
    assert_eq!(proof["checkedUnits"][1]["words"].as_array().unwrap().len(), 4);
    assert!(saved.suggest_with_context("khurkbal", Default::default(), &Default::default()).unwrap()
        .exact_candidates.iter().any(|c| c.mapping_id == Some(99622)));
}
