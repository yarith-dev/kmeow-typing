use khmer_engine::{ApprovedDataset, Engine, SuggestionFeatures};

#[test]
fn selected_omitted_components_preserve_prior_readings_and_finite_boundaries() {
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../test/fixtures/selected-omitted-component-readings-20261009.json"
    )).unwrap();
    let targets: Vec<&str> = proof["rows"].as_array().unwrap().iter()
        .map(|r| r["word"].as_str().unwrap()).collect();
    let mut words = targets.clone();
    words.extend(proof["guardReadings"].as_array().unwrap().iter().map(|r| r["word"].as_str().unwrap()));
    words.sort_unstable(); words.dedup();
    let dataset: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 3, "mappings": [], "englishWords": [], "words": words.iter().enumerate().map(|(i, w)|
            serde_json::json!({"id": 820000 + i, "khmer": w, "rank": i % 7, "status": "approved"})
        ).collect::<Vec<_>>()
    })).unwrap();
    let engine = Engine::new(dataset.clone()).unwrap();
    assert_eq!(proof["newPairs"].as_array().unwrap().len(), 13);
    for group in ["newPairs", "compatibility"] {
        for pair in proof[group].as_array().unwrap() {
            let input = pair[0].as_str().unwrap(); let word = pair[1].as_str().unwrap();
            let result = engine.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let hit = result.exact_candidates.iter().find(|c| c.khmer == word)
                .unwrap_or_else(|| panic!("{input}: {word}"));
            let index = words.iter().position(|&w| w == word).unwrap();
            assert_eq!(hit.mapping_id, Some((820000 + index) as u64));
            assert_eq!(hit.rank, (index % 7) as i32);
            assert!(!engine.suggest_with_context(input, SuggestionFeatures { sound_map: false,
                ..Default::default() }, &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word));
        }
    }
    for pair in proof["newPairs"].as_array().unwrap() {
        let input = pair[0].as_str().unwrap();
        let result = engine.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        for guard in proof["guardReadings"].as_array().unwrap() {
            let word = guard["word"].as_str().unwrap();
            let expected = guard["inputs"].as_array().unwrap().iter().any(|v| v.as_str() == Some(input));
            assert_eq!(result.exact_candidates.iter().any(|c| c.khmer == word), expected, "{input}: {word}");
        }
    }
    let mut disabled = dataset;
    disabled.sound_map_disabled_words = Some(targets.iter().map(|w| (*w).to_owned()).collect());
    let disabled = Engine::new(disabled).unwrap();
    for pair in proof["newPairs"].as_array().unwrap() {
        assert!(!disabled.suggest_with_context(pair[0].as_str().unwrap(), Default::default(), &Default::default())
            .unwrap().exact_candidates.iter().any(|c| c.khmer == pair[1].as_str().unwrap()));
    }
}
