use khmer_engine::{
    ApprovedDataset, ApprovedMapping, ApprovedStatus, ApprovedWord, Engine, SuggestionFeatures,
    TypingContext,
};

#[test]
fn romanized_aliases_show_each_word_once_before_the_completion_limit() {
    let mut mappings: Vec<_> = (b'a'..=b'z')
        .enumerate()
        .map(|(index, letter)| ApprovedMapping {
            id: index as u64 + 1,
            romanized: format!("s{} a", char::from(letter)),
            khmer: format!(
                "ក{}",
                char::from_u32(
                    0x1780
                        + if matches!(letter, b'b' | b'z') {
                            0
                        } else {
                            index as u32
                        }
                )
                .unwrap()
            ),
            rank: if letter == b'z' { 0 } else { 10 },
            status: ApprovedStatus::Approved,
        })
        .collect();
    let source = |mappings: Vec<ApprovedMapping>| ApprovedDataset {
        schema_version: 2,
        words: Some(
            mappings
                .iter()
                .map(|row| ApprovedWord {
                    id: row.id,
                    khmer: row.khmer.clone(),
                    rank: row.rank,
                    status: ApprovedStatus::Approved,
                })
                .collect(),
        ),
        mappings,
        english_words: None,
        structures: None,
        sound_map_disabled_words: None,
    };
    let engine = Engine::new(source(mappings.clone())).unwrap();
    let result = engine.suggest("s").unwrap();
    // Three aliases, including their joined lookups, compete for one word.
    // A better-ranked alias at the end of the index must replace the first.
    assert_eq!(
        result
            .candidates
            .iter()
            .map(|row| row.candidate.mapping_id().unwrap())
            .collect::<Vec<_>>(),
        [vec![26], (3..=21).collect()].concat()
    );
    assert_eq!(result.candidates[0].completion.as_deref(), Some("sz a"));
    assert_eq!(
        result,
        Engine::from_binary(engine.to_binary())
            .unwrap()
            .suggest("s")
            .unwrap()
    );
    assert_eq!(
        result,
        engine
            .suggest_with_context(
                "s",
                SuggestionFeatures::default(),
                &TypingContext::default()
            )
            .unwrap()
            .result
    );
    // Deduplicating chips must retain every saved spelling and exact identity.
    for (input, id) in [("sa a", 1), ("sb a", 2), ("sz a", 26), ("sba", 2)] {
        assert_eq!(
            engine.convert(input).unwrap().candidates[0].mapping_id,
            Some(id)
        );
    }
    mappings.reverse();
    assert_eq!(
        result,
        Engine::new(source(mappings.clone()))
            .unwrap()
            .suggest("s")
            .unwrap()
    );
    mappings.iter_mut().find(|row| row.id == 26).unwrap().rank = 20;
    let changed = Engine::new(source(mappings)).unwrap().suggest("s").unwrap();
    assert_eq!(
        changed
            .candidates
            .iter()
            .map(|row| row.candidate.mapping_id().unwrap())
            .collect::<Vec<_>>(),
        [vec![1], (3..=21).collect()].concat()
    );
}

#[test]
fn completions_are_bounded_and_deterministic_without_truncating_exact_alternatives() {
    let mut rows: Vec<_> = (0..25)
        .map(|i| ApprovedMapping {
            id: i + 1,
            romanized: "phrase".into(),
            khmer: char::from_u32(0x1780 + i as u32).unwrap().to_string(),
            rank: 25 - i as i32,
            status: ApprovedStatus::Approved,
        })
        .collect();
    // A large unrelated range must not change this prefix's results.
    for i in 0..10_000_u64 {
        let key: String = (0..4)
            .rev()
            .map(|p| char::from(b'a' + ((i / 26_u64.pow(p)) % 26) as u8))
            .collect();
        rows.push(ApprovedMapping {
            id: i + 100,
            romanized: format!("z{key}"),
            khmer: key
                .chars()
                .map(|ch| char::from_u32(0x1780 + u32::from(ch) - u32::from('a')).unwrap())
                .collect(),
            rank: 0,
            status: ApprovedStatus::Approved,
        });
    }
    let core = Engine::new(ApprovedDataset {
        structures: None,
        sound_map_disabled_words: None,
        schema_version: 1,
        mappings: rows.clone(),
        words: None,
        english_words: None,
    })
    .unwrap();
    rows.reverse();
    let reversed = Engine::new(ApprovedDataset {
        structures: None,
        sound_map_disabled_words: None,
        schema_version: 1,
        mappings: rows,
        words: None,
        english_words: None,
    })
    .unwrap();
    for prefix in ["p", "ph", "phra", "phrase", "z", "zzzz", "unknown"] {
        assert_eq!(
            core.suggest(prefix).unwrap(),
            reversed.suggest(prefix).unwrap()
        );
    }
    let partial = core.suggest("ph").unwrap();
    assert_eq!(partial.candidates.len(), 20);
    assert_eq!(partial.candidates[0].candidate.mapping_id(), Some(25));
    assert_eq!(partial.candidates[19].candidate.mapping_id(), Some(6));
    assert!(partial
        .candidates
        .iter()
        .all(|c| c.completion.as_deref() == Some("phrase")));
    assert_eq!(core.suggest("z").unwrap().candidates.len(), 20);
    let exact = core.suggest("phrase").unwrap();
    assert_eq!(exact.candidates.len(), 25);
    assert!(exact.candidates.iter().all(|c| c.completion.is_none()));
    assert!(core.convert("ph").unwrap().candidates.is_empty());
}
