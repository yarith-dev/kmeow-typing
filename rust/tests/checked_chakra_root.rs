#[path="support/retired_word_rules.rs"]
mod retired_word_rules;
use khmer_engine::{ApprovedDataset, ApprovedMapping, ApprovedStatus, Engine, SuggestionFeatures, TypingMode};

#[test]
fn checked_chakra_root_keeps_independent_readings_and_db_only_exceptions() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/checked-chakra-root.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [], "words": words.iter().enumerate().map(|(i,w)| {
            serde_json::json!({"id": 98400 + i, "khmer": w, "rank": i, "status": "approved"})
        }).collect::<Vec<_>>()
    })).unwrap();
    let core = Engine::new(data.clone()).unwrap();
    retired_word_rules::assert_retired(&core,&proof);
    let bytes = core.to_binary();
    for group in ["soundYes", "endingOnly", "compatibility"] {
        for row in proof[group].as_array().unwrap() {
            let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
            let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let hit = query.exact_candidates.iter().find(|c| c.khmer == word)
                .unwrap_or_else(|| panic!("{input}: {word}"));
            let index = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
            assert_eq!(hit.mapping_id, Some(98400 + index as u64));
            assert_eq!(hit.rank, index as i32);
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
        // Later reviewed សំបុត្រ reuse supplies this formerly alias-only input.
        let recurring: serde_json::Value = serde_json::from_str(include_str!(
            "../../test/fixtures/recurring-component-word-units-20261009.json"
        )).unwrap();
        let added = recurring["newPairs"].as_array().unwrap().contains(row);
        assert_eq!(core.suggest_with_context(input, Default::default(), &Default::default()).unwrap()
            .exact_candidates.iter().any(|c| c.khmer == word), added, "{input}: {word}");
    }
    assert_eq!(core.to_binary(), bytes);
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("rongjak").unwrap(), core.suggest("rongjak").unwrap());
    for (i,row) in proof["aliasYes"].as_array().unwrap().iter().enumerate() {
        data.mappings.push(ApprovedMapping {id: 99500+i as u64, romanized: row[0].as_str().unwrap().into(),
            khmer: row[1].as_str().unwrap().into(), rank: 300+i as i32, status: ApprovedStatus::Approved});
    }
    let saved = Engine::new(data).unwrap();
    for (i,row) in proof["aliasYes"].as_array().unwrap().iter().enumerate() {
        let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
        for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
            let query = saved.suggest_with_context(input, features, &Default::default()).unwrap();
            let hit = query.exact_candidates.iter().find(|c| c.khmer == word).unwrap();
            assert_eq!(hit.mapping_id, Some(99500+i as u64)); assert_eq!(hit.rank, 300+i as i32);
        }
        for features in [SuggestionFeatures {romanized: false, ..Default::default()},
            SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
            assert!(!saved.suggest_with_context(input, features, &Default::default()).unwrap()
                .exact_candidates.iter().any(|c| c.khmer == word));
        }
    }
}
