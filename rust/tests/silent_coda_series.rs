use khmer_engine::{ApprovedDataset, ApprovedMapping, ApprovedStatus, Engine, SuggestionFeatures};

#[test]
fn consumed_ss_coda_governs_plain_naa_without_a_saved_word_alias() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/silent-coda-series.json")).unwrap();
    let rows: Vec<_> = fixture["cases"].as_array().unwrap().iter()
        .chain(fixture["unrelated"].as_array().unwrap()).collect();
    let mut data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [],
        "words": rows.iter().enumerate().map(|(i,row)| serde_json::json!({
            "id": 97000+i, "khmer": row["word"], "rank": i, "status": "approved"
        })).collect::<Vec<_>>()
    })).unwrap();
    let core = Engine::new(data.clone()).unwrap();
    let binary = core.to_binary();
    let restored = Engine::from_binary(binary.clone()).unwrap();
    let page: serde_json::Value = serde_json::from_str(&core.browse_words_json(r#"{"query":"","limit":100}"#).unwrap()).unwrap();
    for (i, row) in rows.iter().enumerate() {
        let word = row["word"].as_str().unwrap();
        let displayed = page["words"].as_array().unwrap().iter().find(|entry| entry["khmer"] == word)
            .map(|entry| entry["soundMapInputs"].clone()).unwrap_or(serde_json::json!([]));
        assert_eq!(displayed, row["inputs"], "{word}");
        for input in row["inputs"].as_array().unwrap() {
            let input = input.as_str().unwrap();
            let result = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let hit = result.exact_candidates.iter().find(|candidate| candidate.khmer == word).unwrap();
            assert_eq!(hit.mapping_id, Some(97000+i as u64));
            assert_eq!(hit.rank, i as u32);
            assert_eq!(restored.suggest(input).unwrap(), core.suggest(input).unwrap());
        }
        if let Some(removed) = row["removed"].as_array() {
            for input in removed {
                assert!(!core.suggest_with_context(input.as_str().unwrap(), Default::default(), &Default::default()).unwrap()
                    .exact_candidates.iter().any(|candidate| candidate.khmer == word), "{input}: {word}");
            }
        }
    }
    assert_eq!(core.to_binary(), binary);
    let disabled = SuggestionFeatures { sound_map: false, ..Default::default() };
    assert!(!core.suggest_with_context("tusna", disabled.clone(), &Default::default()).unwrap()
        .exact_candidates.iter().any(|candidate| candidate.khmer == "ទស្សនា"));
    data.mappings.push(ApprovedMapping { id: 99002, romanized: "tosnea".into(), khmer: "ទស្សនា".into(), rank: 77, status: ApprovedStatus::Approved });
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), disabled] {
        let result = saved.suggest_with_context("tosnea", features, &Default::default()).unwrap();
        let hit = result.exact_candidates.iter().find(|candidate| candidate.mapping_id == Some(99002)).unwrap();
        assert_eq!(hit.khmer, "ទស្សនា");
        assert_eq!(hit.rank, 77);
    }
}
