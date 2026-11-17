use khmer_engine::{ApprovedDataset, ApprovedMapping, ApprovedStatus, Engine, SuggestionCandidate, SuggestionFeatures, TypingContext, TypingMode};

#[test]
fn terminal_composites_keep_original_end_scope_feature_gates_and_saved_identities() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/terminal-composite-vowels.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 32);
    let words = proof["words"].as_array().unwrap();
    let mut data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [],
        "words": words.iter().enumerate().map(|(i, word)| serde_json::json!({
            "id": 97500 + i, "khmer": word, "rank": i, "status": "approved"
        })).collect::<Vec<_>>()
    })).unwrap();
    let core = Engine::new(data.clone()).unwrap();
    let context = TypingContext::default();
    let binary = core.to_binary();
    for row in proof["yes"].as_array().unwrap().iter()
        .chain(proof["partialMappedPaths"].as_array().unwrap())
        .chain(proof["compatibility"].as_array().unwrap()) {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &context).unwrap();
        let candidate = query.exact_candidates.iter().find(|c| c.khmer == word)
            .unwrap_or_else(|| panic!("{input}: {word}"));
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(candidate.mapping_id, Some(97500 + position as u64));
        assert_eq!(candidate.rank, position as u32);
        for features in [SuggestionFeatures {sound_map: false, ..Default::default()},
            SuggestionFeatures {romanized: false, ..Default::default()},
            SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
            assert!(!core.suggest_with_context(input, features, &context).unwrap()
                .exact_candidates.iter().any(|c| c.khmer == word));
        }
    }
    for row in proof["no"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        assert!(!core.suggest_with_context(input, Default::default(), &context).unwrap()
            .exact_candidates.iter().any(|c| c.khmer == word), "{input}: {word}");
    }
    for row in proof["prefixes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let completion = row[2].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &context).unwrap();
        assert!(!query.exact_candidates.iter().any(|c| c.khmer == word));
        assert!(query.result.candidates.iter().any(|c| matches!(&c.candidate, SuggestionCandidate::Khmer(k) if k.khmer == word)
            && c.completion.as_deref() == Some(completion)), "{input}: {word}");
    }
    assert_eq!(core.to_binary(), binary);
    assert_eq!(Engine::from_binary(binary).unwrap().suggest("chkeuh").unwrap(), core.suggest("chkeuh").unwrap());
    assert!(core.convert("chkeuh").unwrap().candidates.is_empty());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "chkeuah".into(), khmer: "ឆ្កឹះ".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("chkeuah", features, &context).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}
