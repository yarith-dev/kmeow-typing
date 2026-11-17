use khmer_engine::{
    ApprovedDataset, ApprovedEnglishWord, ApprovedStatus, Engine, SuggestionFeatures, TypingMode,
};
fn dataset() -> ApprovedDataset {
    let mut data: ApprovedDataset = serde_json::from_value(
        serde_json::from_str::<serde_json::Value>(include_str!(
            "../../test/fixtures/conversion.json"
        ))
        .unwrap()["dataset"]
            .clone(),
    )
    .unwrap();
    data.schema_version = 3;
    data.english_words = Some(vec![
        ApprovedEnglishWord {
            id: 9007199254740991,
            text: "A".repeat(120),
            rank: 1_000_000,
            status: ApprovedStatus::Approved,
        },
        ApprovedEnglishWord {
            id: 2,
            text: "Hello".into(),
            rank: 10,
            status: ApprovedStatus::Approved,
        },
        ApprovedEnglishWord {
            id: 3,
            text: "hello".into(),
            rank: 0,
            status: ApprovedStatus::Approved,
        },
        ApprovedEnglishWord {
            id: 4,
            text: "HELP".into(),
            rank: 0,
            status: ApprovedStatus::Approved,
        },
    ]);
    data
}
fn rehash(bytes: &mut [u8]) {
    let end = bytes.len() - 8;
    let hash = bytes[..end].iter().fold(0xcbf29ce484222325u64, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    bytes[end..].copy_from_slice(&hash.to_le_bytes());
}
#[test]
fn compact_round_trip_preserves_all_modes_ids_case_unicode_and_deterministic_bytes() {
    let source = dataset();
    let engine = Engine::new(source.clone()).unwrap();
    let bytes = engine.to_binary();
    let loaded = Engine::from_binary(bytes.clone()).unwrap();
    for input in [
        "sur",
        "sur sdey",
        "s",
        "ou",
        "oun nv na",
        "ស",
        "សួ",
        "ក",
        "hel",
        "HEL",
        "hello",
        &"a".repeat(120),
        "?",
        "",
    ] {
        assert_eq!(
            engine.convert(input).unwrap(),
            loaded.convert(input).unwrap()
        );
        for features in [
            SuggestionFeatures::default(),
            SuggestionFeatures {
                romanized: false,
                ..Default::default()
            },
            SuggestionFeatures {
                mode: TypingMode::English,
                ..Default::default()
            },
            SuggestionFeatures {
                mode: TypingMode::English,
                english_completion: false,
                ..Default::default()
            },
        ] {
            assert_eq!(
                engine.suggest_with_features(input, features).unwrap(),
                loaded.suggest_with_features(input, features).unwrap()
            );
        }
    }
    let mut reordered = source;
    reordered.mappings.reverse();
    reordered.words.as_mut().unwrap().reverse();
    reordered.english_words.as_mut().unwrap().reverse();
    assert_eq!(Engine::new(reordered).unwrap().to_binary(), bytes);
}
#[test]
fn compact_rejects_corruption_truncation_and_forged_offsets_without_panicking() {
    let bytes = Engine::new(dataset()).unwrap().to_binary();
    for position in 0..bytes.len() {
        let mut corrupt = bytes.clone();
        corrupt[position] ^= 0x80;
        assert!(Engine::from_binary(corrupt).is_err());
        assert!(Engine::from_binary(bytes[..position].to_vec()).is_err());
    }
    // Recomputed checksums must not allow malformed directories or row data.
    for position in (8..bytes.len() - 8).step_by(3) {
        let mut corrupt = bytes.clone();
        corrupt[position] ^= 0xff;
        rehash(&mut corrupt);
        let result = std::panic::catch_unwind(|| Engine::from_binary(corrupt));
        assert!(result.is_ok(), "parser panicked at {position}");
    }
    let mut appended = bytes.clone();
    appended.push(0);
    assert!(Engine::from_binary(appended).is_err());
}
#[test]
fn streaming_compilation_matches_regular_compilation_and_rejects_duplicates() {
    let words: Vec<_> = (0..2000)
        .map(|i| ApprovedEnglishWord {
            id: i + 1,
            text: format!(
                "x{}{}{}",
                char::from(b'a' + (i / 676) as u8),
                char::from(b'a' + ((i / 26) % 26) as u8),
                char::from(b'a' + (i % 26) as u8)
            ),
            rank: (i % 701) as u32,
            status: ApprovedStatus::Approved,
        })
        .collect();
    let plain = Engine::new(ApprovedDataset {
        structures: None, sound_map_disabled_words: None,
        schema_version: 3,
        mappings: vec![],
        words: Some(vec![]),
        english_words: Some(words.clone()),
    })
    .unwrap();
    let streamed = Engine::compile_sorted_english(words.clone()).unwrap();
    assert_eq!(plain.compiled_bytes(), streamed.compiled_bytes());
    let mut duplicate = words;
    duplicate[100].id = duplicate[99].id;
    assert!(Engine::compile_sorted_english(duplicate).is_err());
}
