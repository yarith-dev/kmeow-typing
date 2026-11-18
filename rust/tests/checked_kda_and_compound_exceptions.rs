#[path="support/retired_word_rules.rs"]
mod retired_word_rules;
use khmer_engine::{ApprovedDataset, Engine, SuggestionFeatures, TypingMode};

#[test]
fn checked_kda_additions_keep_old_paths_and_exact_exception_identities() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/checked-kda-and-compound-exceptions.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [], "words": words.iter().enumerate().map(|(i,w)| {
            serde_json::json!({"id": 93900 + i, "khmer": w, "rank": i, "status": "approved"})
        }).collect::<Vec<_>>()
    })).unwrap();
    let core = Engine::new(data.clone()).unwrap();
    retired_word_rules::assert_retired(&core,&proof);
    for group in ["yes", "compatibility"] {
        for row in proof[group].as_array().unwrap() {
            let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
            let result = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let hit = result.exact_candidates.iter().find(|c| c.khmer == word).unwrap_or_else(|| panic!("{input}: {word}"));
            let index = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
            assert_eq!(hit.mapping_id, Some(93900 + index as u64));
            assert_eq!(hit.rank, index as i32);
            for features in [SuggestionFeatures {sound_map: false, ..Default::default()},
                SuggestionFeatures {romanized: false, ..Default::default()},
                SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
                assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word));
            }
        }
    }
    for row in proof["no"].as_array().unwrap() {
        assert!(!core.suggest_with_context(row[0].as_str().unwrap(), Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == row[1].as_str().unwrap()), "{row}");
    }
    let binary = core.to_binary();
    assert_eq!(Engine::from_binary(binary).unwrap().suggest("kdakhean").unwrap(), core.suggest("kdakhean").unwrap());
    assert!(core.convert("kdakhean").unwrap().candidates.is_empty());
    data.mappings = serde_json::from_value(serde_json::json!([
        {"id": 99720, "romanized": "khsaekrovat", "khmer": "ខ្សែក្រវាត់", "rank": 0, "status": "approved"},
        {"id": 99721, "romanized": "kaevpongrik", "khmer": "កែវពង្រីក", "rank": 0, "status": "approved"},
        {"id": 99722, "romanized": "robohrobor", "khmer": "របស់របរ", "rank": 0, "status": "approved"},
        {"id": 99723, "romanized": "jah", "khmer": "ចា៎ស", "rank": 0, "status": "approved"}
    ])).unwrap();
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        for (input,id) in [("khsaekrovat",99720),("kaevpongrik",99721),("robohrobor",99722)] {
            let query = saved.suggest_with_context(input, features, &Default::default()).unwrap();
            assert_eq!(query.exact_candidates[0].mapping_id, Some(id));
            assert_eq!(query.exact_candidates[0].rank, 0);
        }
        assert_eq!(saved.suggest_with_context("jah", features, &Default::default()).unwrap().exact_candidates[0].khmer, "ចា៎ស");
    }
    for input in ["khsaekrovatka","kaevpongrikka","robohroborka"] {
        assert!(saved.suggest_with_context(input, Default::default(), &Default::default()).unwrap().exact_candidates.is_empty());
    }
}
