use khmer_engine::{ApprovedDataset, ApprovedStatus, ApprovedWord, Engine};

#[test]
fn khmer_completion_is_bounded_deduplicated_and_independent_of_romanization() {
    let mut words: Vec<_> = (0..10_000_u64)
        .map(|i| {
            let suffix: String = (0..4)
                .rev()
                .map(|p| char::from_u32(0x1780 + ((i / 26_u64.pow(p)) % 26) as u32).unwrap())
                .collect();
            ApprovedWord {
                id: i + 1,
                khmer: format!("ក{suffix}"),
                rank: 9,
                status: ApprovedStatus::Approved,
            }
        })
        .collect();
    let first = words[0].khmer.clone();
    words.push(ApprovedWord {
        id: 20_000,
        khmer: first.clone(),
        rank: 0,
        status: ApprovedStatus::Approved,
    });
    let core = Engine::new(ApprovedDataset {
        structures: None, sound_map_disabled_words: None,
        schema_version: 2,
        mappings: vec![],
        words: Some(words.clone()),
        english_words: None,
    })
    .unwrap();
    words.reverse();
    let reversed = Engine::new(ApprovedDataset {
        structures: None, sound_map_disabled_words: None,
        schema_version: 2,
        mappings: vec![],
        words: Some(words),
        english_words: None,
    })
    .unwrap();
    for prefix in ["ក", "កក", &first, "ក ", "ក ខ", "sur", ""] {
        assert_eq!(
            core.suggest(prefix).unwrap(),
            reversed.suggest(prefix).unwrap()
        );
    }
    let result = core.suggest("ក").unwrap();
    assert_eq!(result.candidates.len(), 20);
    assert_eq!(result.candidates[0].candidate.mapping_id(), Some(20_000));
    assert_eq!(core.suggest(&first).unwrap().candidates.len(), 1);
    assert!(core.suggest("ក ").unwrap().candidates.is_empty());
    assert!(core.convert(&first).unwrap().candidates.is_empty());
    assert!(core.suggest(&"ក".repeat(513)).is_err());
    assert!(Engine::from_json(r#"{"schemaVersion":1,"mappings":[],"words":[]}"#).is_err());
    let legacy = Engine::from_json(r#"{"schemaVersion":1,"mappings":[]}"#).unwrap();
    assert!(legacy.suggest("ក").unwrap().candidates.is_empty());
}
