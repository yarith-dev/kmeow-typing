use khmer_engine::{ApprovedDataset, Engine, SuggestionFeatures};

#[test]
fn paired_trop_sambat_preserves_old_paths_and_rejects_cross_variant_inputs() {
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../test/fixtures/checked-trop-sambat-components-20261010.json"
    )).unwrap();
    let mut words = vec!["ទ្រព្យសម្បត្តិ"];
    words.extend(proof["guardReadings"].as_array().unwrap().iter().map(|r| r["word"].as_str().unwrap()));
    words.sort_unstable(); words.dedup();
    let dataset: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 3, "mappings": [], "englishWords": [], "words": words.iter().enumerate().map(|(i, w)|
            serde_json::json!({"id": 860000 + i, "khmer": w, "rank": i % 7, "status": "approved"})
        ).collect::<Vec<_>>()
    })).unwrap();
    let engine = Engine::new(dataset.clone()).unwrap();
    for group in ["newPairs", "compatibility"] {
        for pair in proof[group].as_array().unwrap() {
            let input = pair[0].as_str().unwrap(); let word = pair[1].as_str().unwrap();
            let result = engine.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let hit = result.exact_candidates.iter().find(|c| c.khmer == word)
                .unwrap_or_else(|| panic!("{input}: {word}"));
            let index = words.iter().position(|&w| w == word).unwrap();
            assert_eq!(hit.mapping_id, Some((860000 + index) as u64));
            assert_eq!(hit.rank, (index % 7) as u32);
        }
    }
    for pair in proof["negativePairs"].as_array().unwrap() {
        let result = engine.suggest_with_context(pair[0].as_str().unwrap(), Default::default(), &Default::default()).unwrap();
        assert!(!result.exact_candidates.iter().any(|c| c.khmer == pair[1].as_str().unwrap()), "{pair}");
    }
    let result = engine.suggest_with_context("tropsambat", Default::default(), &Default::default()).unwrap();
    for guard in proof["guardReadings"].as_array().unwrap() {
        assert!(!result.exact_candidates.iter().any(|c| c.khmer == guard["word"].as_str().unwrap()), "{guard}");
    }
    for features in [SuggestionFeatures { sound_map: false, ..Default::default() },
        SuggestionFeatures { romanized: false, ..Default::default() }] {
        assert!(!engine.suggest_with_context("tropsambat", features, &Default::default()).unwrap()
            .exact_candidates.iter().any(|c| c.khmer == "ទ្រព្យសម្បត្តិ"));
    }
    let mut disabled = dataset;
    disabled.sound_map_disabled_words = Some(vec!["ទ្រព្យសម្បត្តិ".to_owned()]);
    assert!(!Engine::new(disabled).unwrap().suggest_with_context("tropsambat", Default::default(), &Default::default()).unwrap()
        .exact_candidates.iter().any(|c| c.khmer == "ទ្រព្យសម្បត្តិ"));
}
