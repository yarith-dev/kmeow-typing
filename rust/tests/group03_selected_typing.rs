//! Exact native selections and artificial collisions; no wider pronunciation approval.
use khmer_engine::{
    ApprovedDataset, ApprovedMapping, ApprovedStatus, Engine, Suggestion, SuggestionCandidate,
    SuggestionFeatures, TypingContext, TypingMode,
};

fn proof() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../test/fixtures/group03-selected-typing.json"
    ))
    .unwrap()
}
fn rank_dataset() -> ApprovedDataset {
    serde_json::from_value(proof()["rankControlDataset"].clone()).unwrap()
}
fn khmer(hit: &Suggestion) -> &str {
    match &hit.candidate {
        SuggestionCandidate::Khmer(c) => &c.khmer,
        _ => panic!("Khmer fixture"),
    }
}
fn saved(hit: &Suggestion) -> &khmer_engine::Candidate {
    match &hit.candidate {
        SuggestionCandidate::Khmer(c) => c,
        _ => panic!("Khmer fixture"),
    }
}

#[test]
fn default_sur_reading_preserves_words_aliases_and_feature_controls() {
    let proof = proof();
    let data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [],
        "words": proof["unitWords"].as_array().unwrap().iter().enumerate().map(|(i, word)| {
            serde_json::json!({"id":74000+i,"khmer":word,"rank":i,"status":"approved"})
        }).collect::<Vec<_>>()
    }))
    .unwrap();
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for row in proof["unitYes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core
            .suggest_with_context(input, Default::default(), &Default::default())
            .unwrap();
        let hit = query
            .exact_candidates
            .iter()
            .find(|c| c.khmer == word)
            .expect(input);
        let position = proof["unitWords"]
            .as_array()
            .unwrap()
            .iter()
            .position(|w| w.as_str() == Some(word))
            .unwrap();
        assert_eq!(hit.mapping_id, Some(74000 + position as u64));
        assert_eq!(hit.rank, position as u32);
        assert!(
            core.convert(input).unwrap().candidates.is_empty(),
            "Generated unit is not a saved alias"
        );
        for features in [
            SuggestionFeatures {
                sound_map: false,
                ..Default::default()
            },
            SuggestionFeatures {
                romanized: false,
                ..Default::default()
            },
            SuggestionFeatures {
                mode: TypingMode::English,
                ..Default::default()
            },
        ] {
            assert!(!core
                .suggest_with_context(input, features, &Default::default())
                .unwrap()
                .exact_candidates
                .iter()
                .any(|c| c.khmer == word));
        }
    }
    for row in proof["unitRemoved"].as_array().unwrap() {
        let query = core.suggest(row[0].as_str().unwrap()).unwrap();
        assert!(
            !query
                .candidates
                .iter()
                .any(|c| khmer(c) == row[1].as_str().unwrap()),
            "{row}"
        );
    }
    for row in proof["unitCorrectionOnly"].as_array().unwrap() {
        let query = core.suggest_with_context(row[0].as_str().unwrap(), Default::default(), &Default::default()).unwrap();
        assert!(!query.exact_candidates.iter().any(|c| c.khmer == row[1].as_str().unwrap()));
        let hit = query.result.candidates.iter().find(|c| khmer(c) == row[1].as_str().unwrap()).unwrap();
        assert_eq!(hit.correction.as_deref(), row[2].as_str());
    }
    for row in proof["unitScopeNo"].as_array().unwrap() {
        let query = core
            .suggest_with_context(
                row[0].as_str().unwrap(),
                Default::default(),
                &Default::default(),
            )
            .unwrap();
        assert!(
            !query
                .exact_candidates
                .iter()
                .any(|c| c.khmer == row[1].as_str().unwrap()),
            "{row}"
        );
    }
    // Count tolerance may match the ordinary longer reading outside the
    // selected override. It must not expand that override's canonical scope.
    for row in proof["unitCountAlternatives"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let preferred = row[2].as_str().unwrap();
        let query = core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
        let hit = query.result.candidates.iter().find(|hit|khmer(hit) == word).unwrap();
        assert_eq!(hit.preferred_input.as_deref(),Some(preferred));
        let page = core.browse_words(word,None,100).unwrap();
        let inputs = &page.words.iter().find(|entry|entry.khmer == word).unwrap().sound_map_inputs;
        assert!(inputs.contains(&preferred.to_owned()));
        assert!(!inputs.contains(&input.to_owned()));
    }
    assert_eq!(core.suggest_with_context("surnorm",Default::default(),&Default::default()).unwrap()
        .exact_candidates[0].khmer,"សួរនាំ");
    let prefix = &proof["unitPrefix"];
    for input in [prefix[0].as_str().unwrap(),"surr"] {
    let query = core
        .suggest_with_context(
            input,
            Default::default(),
            &Default::default(),
        )
        .unwrap();
    assert!(!query
        .exact_candidates
        .iter()
        .any(|c| c.khmer == prefix[1].as_str().unwrap()));
    assert_eq!(
        query
            .result
            .candidates
            .iter()
            .find(|c| khmer(c) == prefix[1].as_str().unwrap())
            .unwrap()
            .completion
            .as_deref(),
        prefix[2].as_str()
    );
    }
    assert_eq!(core.to_binary(), bytes);
    let restored = Engine::from_binary(bytes).unwrap();
    assert_eq!(
        restored.suggest("surnorm").unwrap(),
        core.suggest("surnorm").unwrap()
    );
    // A previously explicit alias survives a generated replacement unchanged.
    let mut saved_data = data;
    saved_data.mappings.push(ApprovedMapping {
        id: 75000,
        romanized: "surrnorm".into(),
        khmer: "សួរនាំ".into(),
        rank: 77,
        status: ApprovedStatus::Approved,
    });
    let explicit = Engine::new(saved_data).unwrap();
    for features in [
        Default::default(),
        SuggestionFeatures {
            sound_map: false,
            ..Default::default()
        },
    ] {
        let query = explicit
            .suggest_with_context("surrnorm", features, &Default::default())
            .unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(75000));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}

#[test]
fn poh_saved_match_priority_preserves_identity_rank_context_and_binary() {
    let core = Engine::new(rank_dataset()).unwrap();
    let bytes = core.to_binary();
    let context = TypingContext { previous_words: vec!["អ".into()], ..Default::default() };
    for input in ["poh", "POH", "poh "] {
        let converted = core.convert(input).unwrap();
        assert_eq!(
            converted
                .candidates
                .iter()
                .map(|c| c.khmer.as_str())
                .collect::<Vec<_>>(),
            ["ពោះ", "ប៉ុស្តិ៍"]
        );
        assert_eq!(converted.candidates[1].mapping_id, Some(72001));
        assert_eq!(converted.candidates[1].rank, 0);
        for features in [
            Default::default(),
            SuggestionFeatures {
                sound_map: false,
                ..Default::default()
            },
        ] {
            let plain = core.suggest_with_features(input, features).unwrap();
            assert_eq!(
                plain.candidates[..2].iter().map(khmer).collect::<Vec<_>>(),
                ["ពោះ", "ប៉ុស្តិ៍"]
            );
            assert!(plain.candidates[..2].iter().all(|c| c.completion.is_none()));
            assert_eq!(saved(&plain.candidates[1]).mapping_id, Some(72001));
            assert_eq!(saved(&plain.candidates[1]).rank, 0);
            let contextual = core
                .suggest_with_context(input, features, &context)
                .unwrap();
            assert_eq!(
                contextual
                    .exact_candidates
                    .iter()
                    .map(|c| c.khmer.as_str())
                    .collect::<Vec<_>>(),
                ["ពោះ", "ប៉ុស្តិ៍"]
            );
            assert_eq!(contextual.exact_candidates[1].mapping_id, Some(72001));
            assert_eq!(contextual.exact_candidates[1].rank, 0);
        }
    }
    // A neutral control proves that this context would otherwise prefer the post.
    let mut neutral = rank_dataset();
    for mapping in &mut neutral.mappings {
        if mapping.romanized == "poh" {
            mapping.romanized = "qa".into();
            mapping.rank = if mapping.khmer == "ប៉ុស្តិ៍" {
                100
            } else {
                0
            };
        }
    }
    let neutral = Engine::new(neutral).unwrap();
    assert_eq!(neutral.convert("qa").unwrap().candidates[0].khmer, "ពោះ");
    assert_eq!(
        neutral
            .suggest_with_context("qa", Default::default(), &context)
            .unwrap()
            .exact_candidates[0]
            .khmer,
        "ប៉ុស្តិ៍"
    );
    assert_eq!(core.to_binary(), bytes);
    let restored = Engine::from_binary(bytes).unwrap();
    for features in [
        Default::default(),
        SuggestionFeatures {
            sound_map: false,
            ..Default::default()
        },
    ] {
        assert_eq!(
            restored
                .suggest_with_context("poh", features, &context)
                .unwrap(),
            core.suggest_with_context("poh", features, &context)
                .unwrap()
        );
    }
}

#[test]
fn poh_policy_does_not_propagate_to_prefixes_compositions_source_joined_or_other_words() {
    let core = Engine::new(rank_dataset()).unwrap();
    let off = SuggestionFeatures {
        sound_map: false,
        ..Default::default()
    };
    let prefix = core.suggest_with_features("po", off).unwrap();
    assert_eq!(khmer(&prefix.candidates[0]), "ប៉ុស្តិ៍");
    assert_eq!(prefix.candidates[0].completion.as_deref(), Some("poh"));
    let composed = core.convert("a poh").unwrap();
    assert_eq!(
        composed
            .candidates
            .iter()
            .map(|c| c.khmer.as_str())
            .collect::<Vec<_>>(),
        ["អប៉ុស្តិ៍", "អពោះ"]
    );
    assert_eq!(composed.candidates[0].mapping_ids, vec![72004, 72001]);
    let repeated = core.convert("poh poh").unwrap();
    assert_eq!(repeated.candidates[0].khmer, "ប៉ុស្តិ៍ប៉ុស្តិ៍");
    assert_eq!(repeated.candidates[0].mapping_ids, vec![72001, 72001]);
    assert_eq!(
        core.suggest_with_context("a poh", off, &Default::default())
            .unwrap()
            .result
            .candidates[0]
            .candidate,
        SuggestionCandidate::Khmer(composed.candidates[0].clone())
    );
    let mut joined = rank_dataset();
    joined.mappings[0].romanized = "p oh".into();
    // Only a saved source with its verified word identity has a derived joined key.
    joined.words.as_mut().unwrap()[0].id = joined.mappings[0].id;
    joined.structures = None;
    let joined = Engine::new(joined).unwrap();
    assert_eq!(
        joined.convert("poh").unwrap().candidates[0].mapping_id,
        Some(72001)
    );
    for input in ["poh", "p oh"] {
        assert_eq!(
            saved(&joined.suggest_with_features(input, off).unwrap().candidates[0]).mapping_id,
            Some(72001)
        );
    }
    let mut other = rank_dataset();
    other.mappings[0].khmer = "ប៉ុស្តិ៍រ".into();
    let other = Engine::new(other).unwrap();
    assert_eq!(other.convert("poh").unwrap().candidates[0].khmer, "ប៉ុស្តិ៍រ");
}

#[test]
fn low_priority_exact_poh_survives_prefix_limit_and_does_not_generate_a_word() {
    let mut data = rank_dataset();
    for (i, ch) in "កខគឃងចឆជឈញដឋឌឍណតថទធនបផពភម".chars().enumerate()
    {
        data.mappings.push(ApprovedMapping {
            id: 73000 + i as u64,
            romanized: format!("poh{}", "a".repeat(i + 2)),
            khmer: format!("ក{ch}"),
            rank: 0,
            status: ApprovedStatus::Approved,
        });
    }
    let core = Engine::new(data).unwrap();
    let query = core
        .suggest_with_features(
            "poh",
            SuggestionFeatures {
                sound_map: false,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(query.candidates.len(), 20);
    assert_eq!(saved(&query.candidates[1]).mapping_id, Some(72001));
    assert!(query.candidates[..2].iter().all(|c| c.completion.is_none()));
    assert!(query.candidates[2..].iter().all(|c| c.completion.is_some()));
    let mut generated_only = rank_dataset();
    generated_only.mappings.clear();
    let generated_only = Engine::new(generated_only).unwrap();
    assert!(!generated_only
        .suggest_with_context("poh", Default::default(), &Default::default())
        .unwrap()
        .exact_candidates
        .iter()
        .any(|c| c.khmer == "ប៉ុស្តិ៍"));
}
