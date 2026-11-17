use khmer_engine::{
    ApprovedDataset, ApprovedEnglishWord, ApprovedStatus, Engine, SuggestionFeatures, TypingMode,
};

#[test]
fn english_completes_one_word_with_bounded_results_and_never_transliterates() {
    let mut words: Vec<_> = (0..10_000_u64)
        .map(|i| {
            let suffix: String = (0..4)
                .rev()
                .map(|p| char::from(b'a' + ((i / 26_u64.pow(p)) % 26) as u8))
                .collect();
            ApprovedEnglishWord {
                id: i + 1,
                text: format!("word{suffix}"),
                rank: 9,
                status: ApprovedStatus::Approved,
            }
        })
        .collect();
    words.push(ApprovedEnglishWord {
        id: 20_000,
        text: "wordaaaa".into(),
        rank: 0,
        status: ApprovedStatus::Approved,
    });
    // Far beyond the first 20 alphabetical rows; still the best completion.
    words.push(ApprovedEnglishWord {
        id: 20_001,
        text: "wordzzzz".into(),
        rank: 0,
        status: ApprovedStatus::Approved,
    });
    let dataset = ApprovedDataset {
        structures: None, sound_map_disabled_words: None,
        schema_version: 3,
        mappings: vec![],
        words: Some(vec![]),
        english_words: Some(words.clone()),
    };
    let core = Engine::new(dataset.clone()).unwrap();
    words.reverse();
    let reversed = Engine::new(ApprovedDataset {
        structures: None, sound_map_disabled_words: None,
        english_words: Some(words),
        ..dataset
    })
    .unwrap();
    let features = SuggestionFeatures {
        mode: TypingMode::English,
        ..Default::default()
    };
    for prefix in ["w", "WoRd", "wordaaaa", "word ", "two words", "សួ", ""] {
        assert_eq!(
            core.suggest_with_features(prefix, features).unwrap(),
            reversed.suggest_with_features(prefix, features).unwrap()
        );
    }
    let result = core.suggest_with_features("WORD", features).unwrap();
    assert_eq!(result.candidates.len(), 20);
    assert_eq!(result.candidates[0].candidate.mapping_id(), Some(20_000));
    assert_eq!(result.candidates[1].candidate.mapping_id(), Some(20_001));
    // A fully typed word stays first even when another match has a better rank.
    let exact = Engine::from_json(
        r#"{"schemaVersion":3,"mappings":[],"words":[],"englishWords":[
        {"id":1,"text":"work","rank":90,"status":"approved"},
        {"id":2,"text":"worker","rank":1,"status":"approved"}] }"#,
    )
    .unwrap();
    assert_eq!(
        exact
            .suggest_with_features("work", features)
            .unwrap()
            .candidates[0]
            .candidate
            .mapping_id(),
        Some(1)
    );
    assert!(core.suggest("word").unwrap().candidates.is_empty());
    assert!(core.convert("wordaaaa").unwrap().candidates.is_empty());
    assert!(core
        .suggest_with_features("word ", features)
        .unwrap()
        .candidates
        .is_empty());
    assert!(core
        .suggest_with_features(
            "word",
            SuggestionFeatures {
                english_completion: false,
                ..features
            }
        )
        .unwrap()
        .candidates
        .is_empty());
    assert!(core
        .suggest_with_features(&"a".repeat(513), features)
        .is_err());
    for text in ["", "two words", "hello!", "-bad", "bad-", "hello2", "ខ្មែរ"] {
        let value = serde_json::json!({"schemaVersion":3,"mappings":[],"words":[],"englishWords":[{"id":1,"text":text,"rank":0,"status":"approved"}]});
        assert!(Engine::from_json(&value.to_string()).is_err());
    }
}
