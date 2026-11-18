use khmer_engine::{ApprovedDataset, ApprovedStatus, ApprovedWord, ApprovedMapping,
    Engine, SuggestionFeatures, SuggestionCandidate, TypingMode};

fn dataset() -> ApprovedDataset {
    ApprovedDataset {schema_version: 2, mappings: vec![], words: None,
        english_words: None, structures: None, sound_map_disabled_words: None}
}

#[test]
fn unwritten_nasal_aspirated_boundary_keeps_keys_guards_and_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/unwritten-nasal-aspirated-boundary.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 12);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as i32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for row in proof["yes"].as_array().unwrap().iter().chain(proof["compatibility"].as_array().unwrap()) {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let candidate = query.exact_candidates.iter().find(|c| c.khmer == word)
            .unwrap_or_else(|| panic!("{input}: {word}"));
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(candidate.mapping_id, Some(97500 + position as u64));
        assert_eq!(candidate.rank, position as i32);
        for features in [SuggestionFeatures {sound_map: false, ..Default::default()},
            SuggestionFeatures {romanized: false, ..Default::default()},
            SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
            assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap()
                .exact_candidates.iter().any(|c| c.khmer == word));
        }
    }
    for row in proof["no"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        assert!(!core.suggest_with_context(input, Default::default(), &Default::default()).unwrap()
            .exact_candidates.iter().any(|c| c.khmer == word), "{input}: {word}");
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
    assert!(core.convert("bonsol").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("bonsol").unwrap(), core.suggest("bonsol").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "bonsol".into(), khmer: "បន្សល់".into(), rank: 77, status: ApprovedStatus::Approved});
    data.mappings.push(ApprovedMapping {id: 99101, romanized: "bansol".into(), khmer: "បន្សល់".into(), rank: 78, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("bonsol", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
        let old = saved.suggest_with_context("bansol", features, &Default::default()).unwrap();
        assert_eq!(old.exact_candidates[0].mapping_id, Some(99101));
        assert_eq!(old.exact_candidates[0].rank, 78);
    }
}
