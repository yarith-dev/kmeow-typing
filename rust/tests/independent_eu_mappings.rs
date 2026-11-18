use khmer_engine::{ApprovedDataset, ApprovedMapping, ApprovedStatus, Engine, SuggestionCandidate, SuggestionFeatures, TypingMode};

#[test]
fn independent_eu_replacements_keep_other_keys_and_explicit_old_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/independent-eu-mappings.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [], "words": words.iter().enumerate().map(|(i, word)| {
            serde_json::json!({"id": 98500 + i, "khmer": word, "rank": i, "status": "approved"})
        }).collect::<Vec<_>>()
    })).unwrap();
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    for field in ["yes", "compatibility", "mappingOnly", "letterUnits", "no"] {
        for row in proof[field].as_array().unwrap() {
            let input = row[0].as_str().unwrap();
            let word = row[1].as_str().unwrap();
            let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let hit = query.exact_candidates.iter().find(|c| c.khmer == word);
            assert_eq!(hit.is_some(), field != "no", "{field}: {input}: {word}");
            if let Some(hit) = hit {
                let index = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
                assert_eq!(hit.mapping_id, Some(98500 + index as u64));
                assert_eq!(hit.rank, index as i32);
                for features in [SuggestionFeatures {sound_map: false, ..Default::default()},
                    SuggestionFeatures {romanized: false, ..Default::default()},
                    SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
                    assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap()
                        .exact_candidates.iter().any(|c| c.khmer == word));
                }
            }
            assert_eq!(restored.suggest(input).unwrap(), core.suggest(input).unwrap());
        }
    }
    for row in proof["prefixes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let completion = row[2].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        assert!(!query.exact_candidates.iter().any(|c| c.khmer == word));
        assert!(query.result.candidates.iter().any(|c| matches!(&c.candidate, SuggestionCandidate::Khmer(k) if k.khmer == word)
            && c.completion.as_deref() == Some(completion)), "{input}: {word}");
    }
    assert_eq!(core.to_binary(), bytes);
    for row in proof["savedAliasControls"].as_array().unwrap() {
        data.mappings.push(ApprovedMapping {
            id: row["id"].as_u64().unwrap(), romanized: row["input"].as_str().unwrap().into(),
            khmer: row["word"].as_str().unwrap().into(), rank: row["rank"].as_u64().unwrap() as i32,
            status: ApprovedStatus::Approved,
        });
    }
    let saved = Engine::new(data).unwrap();
    for row in proof["savedAliasControls"].as_array().unwrap() {
        for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
            let query = saved.suggest_with_context(row["input"].as_str().unwrap(), features, &Default::default()).unwrap();
            assert_eq!(query.exact_candidates[0].khmer, row["word"].as_str().unwrap());
            assert_eq!(query.exact_candidates[0].mapping_id, row["id"].as_u64());
            assert_eq!(query.exact_candidates[0].rank, row["rank"].as_u64().unwrap() as i32);
        }
    }
}
