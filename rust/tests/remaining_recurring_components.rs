#[path="support/retired_word_rules.rs"]
mod retired_word_rules;
use khmer_engine::{ApprovedDataset, Engine};

#[test]
fn remaining_components_preserve_previous_readings_and_checked_original_scope() {
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../test/fixtures/remaining-recurring-components-20261010.json"
    )).unwrap();
    let mut words: Vec<&str> = proof["rows"].as_array().unwrap().iter().map(|r| r["word"].as_str().unwrap()).collect();
    words.extend(proof["guardReadings"].as_array().unwrap().iter().map(|r| r["word"].as_str().unwrap()));
    words.sort_unstable(); words.dedup();
    let dataset: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 3, "mappings": [], "englishWords": [], "words": words.iter().enumerate().map(|(i, w)|
            serde_json::json!({"id": 950000 + i, "khmer": w, "rank": i % 7, "status": "approved"})
        ).collect::<Vec<_>>()
    })).unwrap();
    let engine = Engine::new(dataset.clone()).unwrap();
    retired_word_rules::assert_retired(&engine,&proof);
    for group in ["proposedPairs", "compatibility"] {
        for pair in proof[group].as_array().unwrap() {
            let input = pair[0].as_str().unwrap(); let word = pair[1].as_str().unwrap();
            let result = engine.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let hit = result.exact_candidates.iter().find(|c| c.khmer == word).unwrap_or_else(|| panic!("{input}: {word}"));
            let index = words.iter().position(|&w| w == word).unwrap();
            assert_eq!(hit.mapping_id, Some((950000 + index) as u64)); assert_eq!(hit.rank, (index % 7) as u32);
        }
    }
    for pair in proof["negativePairs"].as_array().unwrap() {
        let result = engine.suggest_with_context(pair[0].as_str().unwrap(), Default::default(), &Default::default()).unwrap();
        assert!(!result.exact_candidates.iter().any(|c| c.khmer == pair[1].as_str().unwrap()), "{pair}");
    }
    for repeated in proof["repetitionControls"].as_array().unwrap() {
        for input in repeated["rejectedDoubledInputs"].as_array().unwrap() {
            let result = engine.suggest_with_context(input.as_str().unwrap(), Default::default(), &Default::default()).unwrap();
            assert!(!result.exact_candidates.iter().any(|c| c.khmer == repeated["word"].as_str().unwrap()), "{input}");
        }
    }
    let mut disabled = dataset;
    disabled.sound_map_disabled_words = Some(proof["rows"].as_array().unwrap().iter()
        .map(|r| r["word"].as_str().unwrap().to_owned()).collect::<std::collections::BTreeSet<_>>().into_iter().collect());
    let disabled = Engine::new(disabled).unwrap();
    for pair in proof["proposedPairs"].as_array().unwrap() {
        let result = disabled.suggest_with_context(pair[0].as_str().unwrap(), Default::default(), &Default::default()).unwrap();
        assert!(!result.exact_candidates.iter().any(|c| c.khmer == pair[1].as_str().unwrap()));
    }
}
