use khmer_engine::{ApprovedDataset, ApprovedMapping, ApprovedStatus, Engine, SuggestionFeatures, TypingMode};

#[test]
fn owner_selected_vowel_keys_follow_effective_series_and_keep_saved_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/vowel-series.json")).unwrap();
    let rows: Vec<_> = proof["cases"].as_array().unwrap().iter()
        .chain(proof["unrelated"].as_array().unwrap()).collect();
    let mut data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [],
        "words": rows.iter().enumerate().map(|(i,row)| serde_json::json!({
            "id": 98000+i, "khmer": row["word"], "rank": i, "status": "approved"
        })).collect::<Vec<_>>()
    })).unwrap();
    let core = Engine::new(data.clone()).unwrap();
    let binary = core.to_binary();
    let restored = Engine::from_binary(binary.clone()).unwrap();
    for (i,row) in rows.iter().enumerate() {
        let word = row["word"].as_str().unwrap();
        for field in ["yes", "no"] {
            for input in row[field].as_array().unwrap() {
                let input = input.as_str().unwrap();
                let result = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
                let hit = result.exact_candidates.iter().find(|candidate| candidate.khmer == word);
                assert_eq!(hit.is_some(), field == "yes", "{field}: {input}: {word}");
                if let Some(hit) = hit {
                    assert_eq!(hit.mapping_id, Some(98000+i as u64));
                    assert_eq!(hit.rank, i as u32);
                    for features in [SuggestionFeatures { sound_map: false, ..Default::default() },
                        SuggestionFeatures { romanized: false, ..Default::default() },
                        SuggestionFeatures { mode: TypingMode::English, ..Default::default() }] {
                        assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap()
                            .exact_candidates.iter().any(|candidate| candidate.khmer == word));
                    }
                }
                assert_eq!(restored.suggest(input).unwrap(), core.suggest(input).unwrap());
            }
        }
    }
    assert_eq!(core.to_binary(), binary);
    data.mappings.push(ApprovedMapping { id: 99001, romanized: "koh".into(), khmer: "កោះ".into(), rank: 77, status: ApprovedStatus::Approved });
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures { sound_map: false, ..Default::default() }] {
        let result = saved.suggest_with_context("koh", features, &Default::default()).unwrap();
        let alias = result.exact_candidates.iter().find(|candidate| candidate.mapping_id == Some(99001)).unwrap();
        assert_eq!(alias.khmer, "កោះ");
        assert_eq!(alias.rank, 77);
    }
}
