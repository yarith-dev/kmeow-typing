use khmer_engine::{ApprovedDataset, ApprovedMapping, ApprovedStatus, Engine, SuggestionCandidate, SuggestionFeatures, TypingMode};

#[test]
fn independent_aoy_matches_the_selected_khmer_reading_without_changing_related_letters() {
    assert_eq!(Engine::pronunciation_inputs("ឲ").unwrap(), Engine::pronunciation_inputs("អោយ").unwrap());
    assert_eq!(Engine::pronunciation_inputs("ឲ").unwrap(), vec!["aoy"]);
    assert_eq!(Engine::pronunciation_inputs("ឱ").unwrap(), vec!["ao"]);
    for word in ["ឲ្យ", "ឱ្យ"] {
        assert_eq!(Engine::pronunciation_inputs(word).unwrap(), vec!["aoy", "oy"]);
    }
    let core = Engine::from_json(&serde_json::json!({"schemaVersion":2,
        "words":[{"id":1,"khmer":"ឲ","rank":7,"status":"approved"},
            {"id":3,"khmer":"អោយ","rank":1,"status":"approved"}],
        "mappings":[{"id":2,"romanized":"ao","khmer":"ឲ","rank":3,"status":"approved"},
            {"id":4,"romanized":"oy","khmer":"អោយ","rank":1,"status":"approved"}]
    }).to_string()).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map:false,..Default::default()}] {
        let result = core.suggest_with_context("ao",features,&Default::default()).unwrap();
        assert!(result.exact_candidates.iter().any(|row|row.khmer=="ឲ" && row.mapping_id==Some(2) && row.rank==3));
        let result = core.suggest_with_context("oy",features,&Default::default()).unwrap();
        assert!(result.exact_candidates.iter().any(|row|row.khmer=="អោយ" && row.mapping_id==Some(4)));
    }
    assert!(core.suggest_with_context("aoy",Default::default(),&Default::default()).unwrap()
        .exact_candidates.iter().any(|row|row.khmer=="អោយ" && row.mapping_id==Some(3)));
}

#[test]
fn independent_vowel_choices_keep_other_keys_and_explicit_saved_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/independent-vowel-choices.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [], "words": words.iter().enumerate().map(|(i, word)| {
            serde_json::json!({"id": 996500 + i, "khmer": word, "rank": i, "status": "approved"})
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
                assert_eq!(hit.mapping_id, Some(996500 + index as u64));
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
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        assert!(!query.exact_candidates.iter().any(|c| c.khmer == word));
        assert!(query.result.candidates.iter().any(|c| matches!(&c.candidate, SuggestionCandidate::Khmer(k) if k.khmer == word)
            ), "{input}: {word}");
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
