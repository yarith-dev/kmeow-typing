#[test]
fn initial_carrier_nasal_preserves_later_keys_guards_and_saved_inputs() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/initial-carrier-nasal-o.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset(); data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i,w)| ApprovedWord {
        id: 98000+i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32, status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap(); let bytes = core.to_binary();
    for row in proof["yes"].as_array().unwrap().iter().chain(proof["compatibility"].as_array().unwrap()) {
        let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let found = query.exact_candidates.iter().find(|c| c.khmer == word).expect(input);
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(found.mapping_id, Some(98000+position as u64)); assert_eq!(found.rank,position as u32);
    }
    for row in proof["no"].as_array().unwrap() {
        let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
        assert!(!core.suggest_with_context(input,Default::default(),&Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer==word),"{input}: {word}");
    }
    for features in [SuggestionFeatures {sound_map:false,..Default::default()},SuggestionFeatures {romanized:false,..Default::default()},SuggestionFeatures {mode:TypingMode::English,..Default::default()}] {
        assert!(!core.suggest_with_context("onsomjek",features,&Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer=="ពេជ្រ"));
    }
    assert_eq!(core.to_binary(),bytes);
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("onsomjek").unwrap(),core.suggest("onsomjek").unwrap());
    data.mappings.push(ApprovedMapping{id:99120,romanized:"ansomjek".into(),khmer:"អន្សមចេក".into(),rank:77,status:ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(),SuggestionFeatures {sound_map:false,..Default::default()}] {
        let query = saved.suggest_with_context("ansomjek",features,&Default::default()).unwrap();
        let found = query.exact_candidates.iter().find(|c| c.mapping_id==Some(99120)).unwrap();assert_eq!(found.rank,77);
    }
}

#[test]
fn exact_pech_replaces_old_generated_paths_without_alias_or_kor_changes() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/pech-word-replacement.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset(); data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i,w)| ApprovedWord {
        id: 98000+i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32, status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap(); let bytes = core.to_binary();
    for row in proof["yes"].as_array().unwrap().iter().chain(proof["compatibility"].as_array().unwrap()) {
        let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let found = query.exact_candidates.iter().find(|c| c.khmer == word).expect(input);
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(found.mapping_id, Some(98000+position as u64)); assert_eq!(found.rank,position as u32);
    }
    for row in proof["no"].as_array().unwrap() {
        let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
        assert!(!core.suggest_with_context(input,Default::default(),&Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer==word),"{input}: {word}");
    }
    for features in [SuggestionFeatures {sound_map:false,..Default::default()},SuggestionFeatures {romanized:false,..Default::default()},SuggestionFeatures {mode:TypingMode::English,..Default::default()}] {
        assert!(!core.suggest_with_context("pech",features,&Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer=="ពេជ្រ"));
    }
    assert_eq!(core.to_binary(),bytes);
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("pech").unwrap(),core.suggest("pech").unwrap());
    data.mappings.push(ApprovedMapping{id:99120,romanized:"pechro".into(),khmer:"ពេជ្រ".into(),rank:77,status:ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(),SuggestionFeatures {sound_map:false,..Default::default()}] {
        let query = saved.suggest_with_context("pechro",features,&Default::default()).unwrap();
        let found = query.exact_candidates.iter().find(|c| c.mapping_id==Some(99120)).unwrap();assert_eq!(found.rank,77);
    }
}

#[test]
fn terminal_nasal_compound_keeps_keys_guards_identity_and_saved_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/terminal-nasal-compound-boundary.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 6);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
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
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("tupngkok").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("tupngkok").unwrap(), core.suggest("tupngkok").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "tupngkok".into(), khmer: "ទូបង្កក".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("tupngkok", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}




#[test]
fn plain_a_terminal_sign_keeps_scoped_keys_identity_guards_and_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/plain-a-terminal-sign.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 14);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
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
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("khana").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("khana").unwrap(), core.suggest("khana").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "khana".into(), khmer: "ខណៈ".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("khana", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}




#[test]
fn explicit_glottal_bantoc_keeps_scoped_keys_guards_and_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/explicit-glottal-bantoc.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 3);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
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
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("bong'ak").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("bong'ak").unwrap(), core.suggest("bong'ak").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "bong'ak".into(), khmer: "បង្អាក់".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("bong'ak", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}


#[test]
fn written_nasal_glottal_boundary_keeps_later_keys_guards_and_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/written-nasal-glottal-boundary.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 29);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for row in proof["yes"].as_array().unwrap().iter().chain(proof["compatibility"].as_array().unwrap()).chain(proof["boundaryOnly"].as_array().unwrap()) {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let candidate = query.exact_candidates.iter().find(|c| c.khmer == word)
            .unwrap_or_else(|| panic!("{input}: {word}"));
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(candidate.mapping_id, Some(97500 + position as u64));
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("bong'aem").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("bong'aem").unwrap(), core.suggest("bong'aem").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "bang'aem".into(), khmer: "បង្អែម".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("bang'aem", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}


#[test]
fn written_ndr_boundary_keeps_later_keys_guards_and_collisions() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/written-ndr-boundary.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 8);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for row in proof["yes"].as_array().unwrap().iter().chain(proof["compatibility"].as_array().unwrap()).chain(proof["boundaryOnly"].as_array().unwrap()) {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let candidate = query.exact_candidates.iter().find(|c| c.khmer == word)
            .unwrap_or_else(|| panic!("{input}: {word}"));
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(candidate.mapping_id, Some(97500 + position as u64));
        assert_eq!(candidate.rank, position as u32);
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
    let collision = core.suggest_with_context("kontrok", Default::default(), &Default::default()).unwrap();
    for word in ["កន្ត្រក", "កន្ទ្រោក"] {
        assert!(collision.exact_candidates.iter().any(|c| c.khmer == word), "collision: {word}");
    }
    assert_eq!(core.to_binary(), bytes);
    assert!(core.convert("bontret").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("bontret").unwrap(), core.suggest("bontret").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "bantret".into(), khmer: "បន្ទ្រេត".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("bantret", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}


#[test]
fn unwritten_nasal_stop_boundary_preserves_later_vowels_and_saved_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/unwritten-nasal-stop-boundary.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 37);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for row in proof["yes"].as_array().unwrap().iter().chain(proof["compatibility"].as_array().unwrap()).chain(proof["boundaryOnly"].as_array().unwrap()) {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let candidate = query.exact_candidates.iter().find(|c| c.khmer == word)
            .unwrap_or_else(|| panic!("{input}: {word}"));
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(candidate.mapping_id, Some(97500 + position as u64));
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("bontup").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("bontup").unwrap(), core.suggest("bontup").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "bantup".into(), khmer: "បន្ទប់".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("bantup", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}


#[test]
fn written_velar_r_boundary_keeps_pronounced_subscripts_closed_syllables_and_guards() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/written-velar-r-boundary.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 7);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
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
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("songkroh").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("songkroh").unwrap(), core.suggest("songkroh").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "sangkroh".into(), khmer: "សង្គ្រោះ".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("sangkroh", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}


#[test]
fn unwritten_nasal_r_boundary_controls_only_the_reviewed_complete_frame() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/unwritten-nasal-r-boundary.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 11);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
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
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("kontrok").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("kontrok").unwrap(), core.suggest("kontrok").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "kantrok".into(), khmer: "កន្ត្រក".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("kantrok", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}


#[test]
fn written_nh_r_boundary_keeps_two_pronounced_onset_subscripts_and_guards() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/written-nh-r-boundary.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 17);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
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
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("konhjraeng").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("konhjraeng").unwrap(), core.suggest("konhjraeng").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "kanhjraeng".into(), khmer: "កញ្ច្រែង".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("kanhjraeng", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}


#[test]
fn written_nh_boundary_uses_coda_keys_preserves_initials_and_saved_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/written-nh-boundary.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 6);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
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
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("jinhjeum").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("jinhjeum").unwrap(), core.suggest("jinhjeum").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "jinyjeum".into(), khmer: "ចិញ្ចឹម".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("jinyjeum", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}


#[test]
fn written_ntr_boundary_keeps_coda_onset_roles_guards_and_saved_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/written-ntr-boundary.json")).unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 12);
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
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
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("montrei").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("montrei").unwrap(), core.suggest("montrei").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "montri".into(), khmer: "មន្ត្រី".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("montri", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}

#[test]
fn checked_root_continuations_require_named_boundaries_and_keep_saved_exceptions() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/confirmed-narrow-readings.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 5);
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 98000 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for row in proof["soundYes"].as_array().unwrap().iter()
        .chain(proof["compatibility"].as_array().unwrap()) {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let candidate = query.exact_candidates.iter().find(|c| c.khmer == word)
            .unwrap_or_else(|| panic!("{input}: {word}"));
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(candidate.mapping_id, Some(98000 + position as u64));
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("sereipheap").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("sereipheap").unwrap(), core.suggest("sereipheap").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "seripheap".into(), khmer: "សេរីភាព".into(), rank: 77, status: ApprovedStatus::Approved});
    for (i, row) in proof["aliasYes"].as_array().unwrap().iter().enumerate() {
        data.mappings.push(ApprovedMapping {id: 99101 + i as u64, romanized: row[0].as_str().unwrap().into(),
            khmer: row[1].as_str().unwrap().into(), rank: 88 + i as u32, status: ApprovedStatus::Approved});
    }
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("seripheap", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
    for (i, row) in proof["aliasYes"].as_array().unwrap().iter().enumerate() {
        for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
            let query = saved.suggest_with_context(row[0].as_str().unwrap(), features, &Default::default()).unwrap();
            let candidate = query.exact_candidates.iter().find(|c| c.khmer == row[1].as_str().unwrap()).unwrap();
            assert_eq!(candidate.mapping_id, Some(99101 + i as u64));assert_eq!(candidate.rank, 88 + i as u32);
        }
        for features in [SuggestionFeatures {romanized: false, ..Default::default()},
            SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
            assert!(!saved.suggest_with_context(row[0].as_str().unwrap(), features, &Default::default()).unwrap()
                .exact_candidates.iter().any(|c| c.khmer == row[1].as_str().unwrap()));
        }
    }
}

#[test]
fn initial_open_samaa_changes_only_the_parsed_vowel_and_preserves_identities() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/initial-open-samaa.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 5);
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 98000 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for row in proof["yes"].as_array().unwrap().iter()
        .chain(proof["partialMappedPaths"].as_array().unwrap())
        .chain(proof["compatibility"].as_array().unwrap()) {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let candidate = query.exact_candidates.iter().find(|c| c.khmer == word)
            .unwrap_or_else(|| panic!("{input}: {word}"));
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(candidate.mapping_id, Some(98000 + position as u64));
        assert_eq!(candidate.rank, position as u32);
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
    assert!(core.convert("samakom").unwrap().candidates.is_empty());
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("samakom").unwrap(), core.suggest("samakom").unwrap());
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "sameakom".into(), khmer: "សមាគម".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("sameakom", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}

#[test]
fn local_written_clusters_keep_parsed_scope_identities_and_feature_gates() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/local-written-clusters.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    assert_eq!(proof["pairs"].as_array().unwrap().len(), 19);
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 37);
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 96000 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for row in proof["yes"].as_array().unwrap().iter()
        .chain(proof["partialMappedPaths"].as_array().unwrap())
        .chain(proof["compatibility"].as_array().unwrap()) {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let candidate = query.exact_candidates.iter().find(|c| c.khmer == word)
            .unwrap_or_else(|| panic!("{input}: {word}"));
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(candidate.mapping_id, Some(96000 + position as u64));
        assert_eq!(candidate.rank, position as u32);
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
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "khleakhmum".into(), khmer: "ខ្លាឃ្មុំ".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("khleakhmum", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}

#[test]
fn original_gaps_keep_scoped_derivations_boundaries_and_saved_identities() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/original-gaps.json")).unwrap();
    let baseline: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/mapped-baseline.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, word)| ApprovedWord {
        id: 8000 + i as u64, khmer: word.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    for row in proof["yes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let found = query.exact_candidates.iter().find(|c| c.khmer == word).unwrap_or_else(|| panic!("{input}: {word}"));
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(found.mapping_id, Some(8000 + position as u64));
        assert_eq!(found.rank, position as u32);
        assert_eq!(restored.suggest(input).unwrap(), core.suggest(input).unwrap());
        for features in [SuggestionFeatures { sound_map: false, ..Default::default() },
            SuggestionFeatures { romanized: false, ..Default::default() },
            SuggestionFeatures { mode: TypingMode::English, ..Default::default() }] {
            assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word));
        }
    }
    for row in proof["no"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let now_mapped = baseline["historicalNowMapped"].as_array().unwrap().contains(row)
            && !baseline["overriddenByOnsetGovernance"].as_array().unwrap().contains(row)
            && !baseline["overriddenBySelectedVowelMapping"].as_array().unwrap().contains(row);
        assert_eq!(core.suggest_with_context(input, Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word), now_mapped, "{input}: default/rule scope for {word}");
    }
    for row in proof["prefixes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let completion = row[2].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        assert!(!query.exact_candidates.iter().any(|c| c.khmer == word));
        assert!(query.result.candidates.iter().any(|c| matches!(&c.candidate, SuggestionCandidate::Khmer(k) if k.khmer == word) && c.completion.as_deref() == Some(completion)), "{input}: {completion}");
    }
    assert_eq!(core.to_binary(), bytes);
    data.mappings.push(ApprovedMapping {id: 9996, romanized: "masin".into(), khmer: "ម៉ាស៊ីន".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("masin", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(9996));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}

use khmer_engine::{
    ApprovedDataset, ApprovedMapping, ApprovedStatus, ApprovedWord, Engine, SuggestionCandidate,
    SuggestionFeatures, TypingContext, TypingMode,
};

#[test]
fn reviewed_onset_series_covers_the_source_cohort_and_preserves_saved_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/onset-series.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 41000 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for row in proof["yes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let exact = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let candidate = exact.exact_candidates.iter().find(|c| c.khmer == word)
            .unwrap_or_else(|| panic!("{input}: {word}"));
        let index = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(candidate.mapping_id, Some(41000 + index as u64));
        assert_eq!(candidate.rank, index as u32);
        for features in [SuggestionFeatures {sound_map: false, ..Default::default()},
            SuggestionFeatures {romanized: false, ..Default::default()},
            SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
            assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap()
                .exact_candidates.iter().any(|c| c.khmer == word), "disabled: {input}: {word}");
        }
    }
    for row in proof["no"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        assert!(!core.suggest_with_context(input, Default::default(), &Default::default()).unwrap()
            .exact_candidates.iter().any(|c| c.khmer == word), "prior default: {input}: {word}");
    }
    for row in proof["prefixes"].as_array().unwrap() {
        let query = core.suggest_with_context(row[0].as_str().unwrap(), Default::default(), &Default::default()).unwrap();
        let word = row[1].as_str().unwrap();
        assert!(!query.exact_candidates.iter().any(|c| c.khmer == word));
        assert!(query.result.candidates.iter().any(|s| matches!(&s.candidate,
            SuggestionCandidate::Khmer(c) if c.khmer == word) && s.completion.as_deref() == row[2].as_str()));
    }
    assert_eq!(core.to_binary(), bytes);
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("trey").unwrap(), core.suggest("trey").unwrap());
    assert!(core.convert("trey").unwrap().candidates.is_empty());
    data.mappings.push(ApprovedMapping {id: 51000, romanized: "tri".into(), khmer: "ត្រី".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let result = saved.suggest_with_context("tri", features, &Default::default()).unwrap();
        assert_eq!(result.exact_candidates[0].mapping_id, Some(51000));
        assert_eq!(result.exact_candidates[0].rank, 77);
    }
}

#[test]
fn mapped_defaults_and_reviewed_endings_apply_locally_without_changing_saved_data() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/mapped-baseline.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, word)| ApprovedWord {
        id: 20000 + i as u64, khmer: word.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    data.mappings.push(ApprovedMapping {id: 30000, romanized: "jah".into(), khmer: "ចា៎ស".into(), rank: 80, status: ApprovedStatus::Approved});
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    for row in proof["yes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let matches: Vec<_> = query.exact_candidates.iter().filter(|c| c.khmer == word).collect();
        assert_eq!(matches.len(), 1, "{input}: {word}");
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(matches[0].mapping_id, Some(20000 + position as u64));
        assert_eq!(matches[0].rank, position as u32);
        assert_eq!(restored.suggest(input).unwrap(), core.suggest(input).unwrap());
        for features in [SuggestionFeatures {sound_map: false, ..Default::default()},
            SuggestionFeatures {romanized: false, ..Default::default()},
            SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
            assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word), "{input}: disabled");
        }
    }
    for row in proof["no"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        assert!(!core.suggest_with_context(input, Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word), "{input}: guarded {word}");
    }
    for row in proof["prefixes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let completion = row[2].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        assert!(!query.exact_candidates.iter().any(|c| c.khmer == word));
        assert!(query.result.candidates.iter().any(|c| matches!(&c.candidate, SuggestionCandidate::Khmer(k) if k.khmer == word) && c.completion.as_deref() == Some(completion)), "{input}: prefix {word}");
    }
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let jah = core.suggest_with_context("jah", features, &Default::default()).unwrap();
        assert_eq!(jah.exact_candidates[0].khmer, "ចា៎ស");
        assert_eq!(jah.exact_candidates[0].mapping_id, Some(30000));
        assert_eq!(jah.exact_candidates[0].rank, 80);
    }
    assert_eq!(core.to_binary(), bytes);
    // Generating a Sound map reading creates no saved alias. Low-level
    // conversion and an engine without that approved word cannot synthesize it.
    assert!(core.convert("puth").unwrap().candidates.is_empty());
    data.words.as_mut().unwrap().retain(|w| w.khmer != "ពុធ");
    let absent = Engine::new(data.clone()).unwrap();
    for input in ["put", "puth"] {
        assert!(!absent.suggest_with_context(input, Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == "ពុធ"));
    }
    data.words.as_mut().unwrap().push(ApprovedWord {id: 31000, khmer: "ពុធ".into(), rank: 30, status: ApprovedStatus::Approved});
    data.mappings.push(ApprovedMapping {id: 31001, romanized: "put".into(), khmer: "ពុធ".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("put", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(31001));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}

#[test]
fn narrow_closing_stems_and_ry_keep_alternatives_identities_and_boundaries() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/narrow-closing.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    assert_eq!(proof["proofWords"].as_array().unwrap().len(), 27);
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, word)| ApprovedWord {
        id: 8000 + i as u64, khmer: word.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    for row in proof["yes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let found = query.exact_candidates.iter().find(|c| c.khmer == word).unwrap_or_else(|| panic!("{input}: {word}"));
        let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
        assert_eq!(found.mapping_id, Some(8000 + position as u64));
        assert_eq!(found.rank, position as u32);
        assert_eq!(restored.suggest(input).unwrap(), core.suggest(input).unwrap());
        for features in [SuggestionFeatures { sound_map: false, ..Default::default() },
            SuggestionFeatures { romanized: false, ..Default::default() },
            SuggestionFeatures { mode: TypingMode::English, ..Default::default() }] {
            assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word));
        }
    }
    for row in proof["no"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        assert!(!core.suggest_with_context(input, Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word), "{input}: guard for {word}");
    }
    for row in proof["prefixes"].as_array().unwrap() {
        let input = row[0].as_str().unwrap();
        let word = row[1].as_str().unwrap();
        let completion = row[2].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        assert!(!query.exact_candidates.iter().any(|c| c.khmer == word));
        assert!(query.result.candidates.iter().any(|c| matches!(&c.candidate, SuggestionCandidate::Khmer(k) if k.khmer == word) && c.completion.as_deref() == Some(completion)), "{input}: {completion}");
    }
    assert_eq!(core.to_binary(), bytes);
    data.mappings.push(ApprovedMapping {id: 9996, romanized: "sut".into(), khmer: "សុទ្ធ".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let query = saved.suggest_with_context("sut", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(9996));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}

#[test]
fn accepted_pair_mark_and_final_rules_preserve_words_and_exact_order() {
    let words = ["ចាស់", "ចាស", "ណាស់", "ចម្រៀង", "ចម្ការ", "សណ្ដែក", "ពោធិ៍",
        "ន័យ", "វ័យ", "ជ័យ", "ញ័រ", "ជ័រ", "សប្បាយ", "កញ្ញា", "សញ្ញា",
        "ពោធិ", "ពោធិ៍ក", "ធិ៍", "ពោធុ៍", "កេរ្តិ៍", "ស័យ", "ក័រ", "ញ៉័រ",
        "ន័យក", "សា", "សាលា", "ចម្លើយ", "ស្ពាន", "ពិសោធន៍", "សាសនា", "ម៉ន្ដា"];
    let mut data = dataset();
    data.words = Some(words.iter().enumerate().map(|(i, word)| ApprovedWord {
        id: 1000 + i as u64, khmer: word.to_string(), rank: i as u32, status: ApprovedStatus::Approved,
    }).collect());
    data.mappings.clear();
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    for (input, word) in [("jomreang", "ចម្រៀង"), ("jomka", "ចម្ការ"), ("sondaek", "សណ្ដែក"),
        ("po", "ពោធិ៍"), ("ney", "ន័យ"), ("vey", "វ័យ"), ("wey", "វ័យ"),
        ("jey", "ជ័យ"), ("chey", "ជ័យ"), ("nhoa", "ញ័រ"), ("nyoa", "ញ័រ"),
        ("joa", "ជ័រ"), ("choa", "ជ័រ"), ("jah", "ចាស"), ("jah", "ចាស់"),
        ("nah", "ណាស់"), ("jas", "ចាស"), ("jas", "ចាស់"), ("nas", "ណាស់"),
        ("sabbay", "សប្បាយ"), ("pothi", "ពោធិ"), ("sa", "សា"), ("sala", "សាលា"),
        ("sahnea", "សាសនា"), ("sasnea", "សាសនា")] {
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let found = query.exact_candidates.iter().find(|c| c.khmer == word).expect(input);
        let position = words.iter().position(|w| *w == word).unwrap();
        assert_eq!(found.mapping_id, Some(1000 + position as u64), "{input}: identity");
        assert_eq!(found.rank, position as u32, "{input}: saved rank");
        assert_eq!(restored.suggest(input).unwrap(), core.suggest(input).unwrap());
        for features in [SuggestionFeatures { sound_map: false, ..Default::default() },
            SuggestionFeatures { romanized: false, ..Default::default() },
            SuggestionFeatures { mode: TypingMode::English, ..Default::default() }] {
            assert!(core.suggest_with_context(input, features, &Default::default()).unwrap().exact_candidates.is_empty(), "{input}: feature gate");
        }
    }
    let jah = core.suggest_with_context("jah", Default::default(), &Default::default()).unwrap();
    assert_eq!(jah.exact_candidates.iter().map(|c| c.khmer.as_str()).collect::<Vec<_>>(), ["ចាស់", "ចាស"]);
    assert_eq!(jah.exact_candidates.iter().map(|c| c.rank).collect::<Vec<_>>(), [0, 1]);
    for (input, word) in [("jamreang", "ចម្រៀង"), ("sapbay", "សប្បាយ"), ("sopbay", "សប្បាយ"),
        ("kanha", "កញ្ញា"), ("sanha", "សញ្ញា"), ("po", "ពោធិ៍ក"), ("po", "ធិ៍"),
        ("po", "ពោធុ៍"), ("ke", "កេរ្តិ៍"), ("sey", "ស័យ"), ("koa", "ក័រ"),
        ("nhoa", "ញ៉័រ"), ("neyk", "ន័យក"), ("jomhleuy", "ចម្លើយ"),
        ("span", "ស្ពាន"), ("pisaot", "ពិសោធន៍"), ("monda", "ម៉ន្ដា")] {
        assert!(!core.suggest_with_context(input, Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word), "{input}: scope guard for {word}");
    }
    for input in ["jom", "ne", "nh", "ja"] {
        assert!(core.suggest_with_context(input, Default::default(), &Default::default()).unwrap().exact_candidates.is_empty(), "{input}: prefix is not exact");
    }
    assert_eq!(core.to_binary(), bytes);
    // A saved spelling remains first and retains its own alias identity.
    data.mappings.push(ApprovedMapping { id: 3000, romanized: "jah".into(), khmer: "ចាស់".into(), rank: 77, status: ApprovedStatus::Approved });
    data.mappings.push(ApprovedMapping { id: 3001, romanized: "jamreang".into(), khmer: "ចម្រៀង".into(), rank: 0, status: ApprovedStatus::Approved });
    let saved = Engine::new(data).unwrap();
    let query = saved.suggest_with_context("jah", Default::default(), &Default::default()).unwrap();
    assert_eq!(query.exact_candidates[0].mapping_id, Some(3000));
    assert_eq!(query.exact_candidates[0].rank, 77);
    for features in [Default::default(), SuggestionFeatures { sound_map: false, ..Default::default() }] {
        assert_eq!(saved.suggest_with_context("jamreang", features, &Default::default()).unwrap().exact_candidates[0].mapping_id, Some(3001));
    }
}
fn dataset() -> ApprovedDataset {
    ApprovedDataset {
        schema_version: 2,
        structures: None, sound_map_disabled_words: None,
        english_words: None,
        mappings: vec![ApprovedMapping {
            id: 99,
            romanized: "jg".into(),
            khmer: "ចង់".into(),
            rank: 0,
            status: ApprovedStatus::Approved,
        }],
        words: Some(
            [
                "ចង់",
                "ចង",
                "សុខ",
                "សៀវភៅ",
                "ស៊ី",
                "ម៉ែ",
                "ឈើ",
                "អូន",
                "កំ",
                "កុំ",
                "ពាក់",
                "ព្រឹក",
                "ទឹក",
                "ទុក",
                "ទូក",
                "ទុក្ខ",
            ]
            .iter()
            .enumerate()
            .map(|(i, w)| ApprovedWord {
                id: i as u64 + 1,
                khmer: w.to_string(),
                rank: i as u32,
                status: ApprovedStatus::Approved,
            })
            .collect(),
        ),
    }
}
#[test]
fn closing_mp_and_nasal_y_keep_base_finals_and_written_boundaries() {
    // Attached/marked strings are isolated orthographic fixtures, not corpus
    // additions. Source-only roots verify reuse beyond the everyday examples.
    let words = ["គុម្ព", "បោះពុម្ព", "រោងពុម្ព", "សូន្យ", "បុណ្យ", "ពិម្ព", "តុម្ព", "សែន្យ",
        "អម្ពិល", "សន្យា", "គាំទ្រ", "ទ្រាំទ្រ", "សុទ្ធ", "ព្យាង្គ", "អាចារ្យ", "មិត្តភាព",
        "គម្ព", "គុម្ពា", "គុម្ពកា", "គុម្ព់", "គុម្ព៍", "គុម្ព៊", "គុម្ព្រ", "គុំម្ព", "គាំម្ព",
        "សូន្យា", "សូន្យកា", "បណ្យ", "បុណ្យា", "បុណ្យ់"];
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, word)| ApprovedWord {
        id: 7000 + i as u64, khmer: word.to_string(), rank: i as u32, status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    for (input, word) in [("kum", "គុម្ព"), ("kom", "គុម្ព"), ("bospum", "បោះពុម្ព"),
        ("bospom", "បោះពុម្ព"), ("rongpum", "រោងពុម្ព"), ("rongpom", "រោងពុម្ព"),
        ("son", "សូន្យ"), ("bon", "បុណ្យ"), ("bun", "បុណ្យ"), ("pim", "ពិម្ព"),
        ("pem", "ពិម្ព"), ("tom", "តុម្ព"), ("tum", "តុម្ព"), ("saen", "សែន្យ"),
        ("ompil", "អម្ពិល"), ("ompel", "អម្ពិល"), ("kormtro", "គាំទ្រ"),
        ("trormtro", "ទ្រាំទ្រ"), ("suttho", "សុទ្ធ"), ("pyeang", "ព្យាង្គ"),
        ("aja", "អាចារ្យ"), ("mittorpheap", "មិត្តភាព")] {
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let found = query.exact_candidates.iter().find(|c| c.khmer == word).unwrap_or_else(|| panic!("{input}: {word}"));
        let position = words.iter().position(|w| *w == word).unwrap();
        assert_eq!(found.mapping_id, Some(7000 + position as u64));
        assert_eq!(found.rank, position as u32);
        assert_eq!(restored.suggest(input).unwrap(), core.suggest(input).unwrap());
        for features in [SuggestionFeatures { sound_map: false, ..Default::default() },
            SuggestionFeatures { romanized: false, ..Default::default() },
            SuggestionFeatures { mode: TypingMode::English, ..Default::default() }] {
            assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word));
        }
    }
    for (input, word) in [("mitpheap", "មិត្តភាព"), ("kumpo", "គុម្ព"), ("kompo", "គុម្ព"), ("bohpumpo", "បោះពុម្ព"),
        ("rongpumpo", "រោងពុម្ព"), ("sonyo", "សូន្យ"), ("bonyo", "បុណ្យ"),
        ("omil", "អម្ពិល"), ("son", "សន្យា"), ("kormt", "គាំទ្រ"), ("trormt", "ទ្រាំទ្រ"),
        ("pyeangko", "ព្យាង្គ"), ("ajaryo", "អាចារ្យ"),
        ("kom", "គម្ព"), ("kum", "គុម្ពា"), ("kumka", "គុម្ពកា"), ("kum", "គុម្ព់"),
        ("kum", "គុម្ព៍"), ("kum", "គុម្ព៊"), ("kum", "គុម្ព្រ"), ("kumm", "គុំម្ព"),
        ("kormm", "គាំម្ព"), ("son", "សូន្យា"), ("sonka", "សូន្យកា"), ("bon", "បណ្យ"),
        ("bon", "បុណ្យា"), ("bon", "បុណ្យ់")] {
        assert!(!core.suggest_with_context(input, Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word), "{input}: guard for {word}");
    }
    let prefix = core.suggest_with_context("ku", Default::default(), &Default::default()).unwrap();
    assert!(!prefix.exact_candidates.iter().any(|c| c.khmer == "គុម្ព"));
    assert!(prefix.result.candidates.iter().any(|c| matches!(&c.candidate, SuggestionCandidate::Khmer(k) if k.khmer == "គុម្ព") && c.completion.as_deref() == Some("kum")));
    assert_eq!(core.to_binary(), bytes);
    data.mappings.push(ApprovedMapping { id: 9997, romanized: "kum".into(), khmer: "គុម្ព".into(), rank: 77, status: ApprovedStatus::Approved });
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures { sound_map: false, ..Default::default() }] {
        let query = saved.suggest_with_context("kum", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(9997));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}
#[test]
fn reusable_coda_and_kam_units_and_nasal_governance_keep_boundaries() {
    // Some orthographic fixtures below test attached-vowel/initial boundaries;
    // they are isolated strings, never proposals or additions to the corpus.
    let words = ["មនុស្ស", "សិស្ស", "ទុក្ខ", "ទុក្ខសោក", "កម្ម", "កម្មវិធី", "សកម្ម", "សកម្មភាព",
        "ដំរី", "សំរាម", "កំញាន", "សាលី", "សាលា", "កំណើត", "ដំណើរ", "គីមី", "កុលាប",
        "ធម្មតា", "ស្សា", "ក្ខា", "សិស្សា", "ទុក្ខា", "កម្មា", "បន្ត", "តូច", "ទិញ", "ដំរីក"];
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, word)| ApprovedWord {
        id: 4000 + i as u64, khmer: word.to_string(), rank: i as u32, status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    let exact = |input: &str, word: &str| core.suggest_with_context(input, Default::default(), &Default::default()).unwrap()
        .exact_candidates.iter().any(|c| c.khmer == word);
    for (input, word) in [("monuh", "មនុស្ស"), ("monus", "មនុស្ស"), ("serh", "សិស្ស"), ("soeuh", "សិស្ស"),
        ("sers", "សិស្ស"), ("soeus", "សិស្ស"), ("tuk", "ទុក្ខ"), ("tok", "ទុក្ខ"), ("tuksaok", "ទុក្ខសោក"),
        ("kam", "កម្ម"), ("kamvithi", "កម្មវិធី"), ("sakkam", "សកម្ម"), ("sorkampheap", "សកម្មភាព"),
        ("domrey", "ដំរី"), ("domrei", "ដំរី"), ("dumrey", "ដំរី"), ("somram", "សំរាម"),
        ("komnhan", "កំញាន"), ("komnyan", "កំញាន"), ("sali", "សាលី"), ("sala", "សាលា"),
        ("komnert", "កំណើត"), ("domner", "ដំណើរ"), ("kimi", "គីមី"), ("kolap", "កុលាប"),
        ("thommota", "ធម្មតា"), ("ssa", "ស្សា"), ("kkha", "ក្ខា"), ("sissa", "សិស្សា"),
        ("tukkha", "ទុក្ខា"), ("kormmea", "កម្មា")] {
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let found = query.exact_candidates.iter().find(|c| c.khmer == word).unwrap_or_else(|| panic!("{input}: missing {word}"));
        assert_eq!(found.mapping_id, Some(4000 + words.iter().position(|w| *w == word).unwrap() as u64));
        assert_eq!(restored.suggest(input).unwrap(), core.suggest(input).unwrap());
        for features in [SuggestionFeatures { sound_map: false, ..Default::default() },
            SuggestionFeatures { romanized: false, ..Default::default() },
            SuggestionFeatures { mode: TypingMode::English, ..Default::default() }] {
            assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word));
        }
    }
    for (input, word) in [("monussa", "មនុស្ស"), ("sissa", "សិស្ស"), ("sessa", "សិស្ស"),
        ("tukkha", "ទុក្ខ"), ("tokkha", "ទុក្ខ"), ("tukkhasaok", "ទុក្ខសោក"),
        ("kommo", "កម្ម"), ("domri", "ដំរី"), ("dumri", "ដំរី"), ("somream", "សំរាម"),
        ("komnhean", "កំញាន"), ("komnyian", "កំញាន"), ("salei", "សាលី"),
        ("thomta", "ធម្មតា"), ("ban", "បន្ត"),
        ("domrey", "ដំរីក"), ("kam", "កម្មា"), ("sih", "សិស្សា"), ("tuk", "ទុក្ខា")] {
        assert!(!exact(input, word), "{input}: must not give {word}");
    }
    let prefix = core.suggest_with_context("domre", Default::default(), &Default::default()).unwrap();
    assert!(!prefix.exact_candidates.iter().any(|c| c.khmer == "ដំរី"));
    assert!(prefix.result.candidates.iter().any(|c| matches!(&c.candidate, SuggestionCandidate::Khmer(k) if k.khmer == "ដំរី") && c.completion.as_deref() == Some("domrei")));
    let kam = core.suggest_with_context("kam", Default::default(), &Default::default()).unwrap();
    assert!(kam.exact_candidates.iter().any(|c| c.khmer == "កម្ម"));
    assert!(!kam.exact_candidates.iter().any(|c| c.khmer == "កម្មវិធី"));
    assert!(kam.result.candidates.iter().any(|c| matches!(&c.candidate, SuggestionCandidate::Khmer(k) if k.khmer == "កម្មវិធី") && c.completion.as_deref() == Some("kamvithi")));
    assert_eq!(core.to_binary(), bytes);
    data.mappings.push(ApprovedMapping { id: 9999, romanized: "monuh".into(), khmer: "មនុស្ស".into(), rank: 77, status: ApprovedStatus::Approved });
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures { sound_map: false, ..Default::default() }] {
        let query = saved.suggest_with_context("monuh", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(9999));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}
#[test]
fn final_cho_and_nyo_use_selected_ending_keys_without_changing_initials() {
    let words = ["តូច", "ដូច", "តិច", "ទិញ", "មិញ", "ចេញ", "ញី", "ចង"];
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, word)| ApprovedWord {
        id: 6000 + i as u64, khmer: word.to_string(), rank: i as u32, status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    for (input, word) in [("toch", "តូច"), ("doch", "ដូច"), ("tich", "តិច"), ("tech", "តិច"),
        ("tinh", "ទិញ"), ("tenh", "ទិញ"), ("minh", "មិញ"), ("menh", "មិញ"),
        ("jenh", "ចេញ"), ("nhi", "ញី"), ("nyi", "ញី"), ("jong", "ចង")] {
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        let found = query.exact_candidates.iter().find(|c| c.khmer == word).expect(input);
        assert_eq!(found.mapping_id, Some(6000 + words.iter().position(|w| *w == word).unwrap() as u64));
        assert_eq!(found.rank, words.iter().position(|w| *w == word).unwrap() as u32);
        assert_eq!(core.suggest(input).unwrap(), restored.suggest(input).unwrap());
        for features in [SuggestionFeatures { sound_map: false, ..Default::default() },
            SuggestionFeatures { romanized: false, ..Default::default() },
            SuggestionFeatures { mode: TypingMode::English, ..Default::default() }] {
            assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word));
        }
    }
    for (input, word) in [("toj", "តូច"), ("tocha", "តូច"), ("tiny", "ទិញ"), ("teny", "ទិញ"),
        ("miny", "មិញ"), ("chenh", "ចេញ"), ("chong", "ចង")] {
        assert!(!core.suggest_with_context(input, Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word), "{input}: {word}");
    }
    let prefix = core.suggest_with_context("to", Default::default(), &Default::default()).unwrap();
    assert!(prefix.exact_candidates.is_empty());
    assert!(prefix.result.candidates.iter().any(|c| matches!(&c.candidate, SuggestionCandidate::Khmer(k) if k.khmer == "តូច") && c.completion.as_deref() == Some("toch")));
    assert_eq!(core.to_binary(), bytes);
    data.mappings.push(ApprovedMapping { id: 9998, romanized: "tinh".into(), khmer: "ទិញ".into(), rank: 77, status: ApprovedStatus::Approved });
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures { sound_map: false, ..Default::default() }] {
        let query = saved.suggest_with_context("tinh", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates[0].mapping_id, Some(9998));
        assert_eq!(query.exact_candidates[0].rank, 77);
    }
}
#[test]
fn historical_rejections_do_not_filter_sound_matches() {
    let core = Engine::new(dataset()).unwrap();
    let bytes = core.to_binary();
    for (input, target) in [("prerk", "ព្រឹក"), ("on", "អូន"), ("tuk", "ទុក"), ("tuk", "ទូក")]
    {
        let result = core
            .suggest_with_context(input, Default::default(), &Default::default())
            .unwrap();
        assert!(
            result.exact_candidates.iter().any(|c| c.khmer == target),
            "{input}: {result:?}"
        );
        assert!(
            result
                .result
                .candidates
                .iter()
                .any(|s| s.completion.as_deref().unwrap_or(input) == input
                    && matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer == target)),
            "{input}: {result:?}"
        );
    }
    let prefix = core.suggest("pr").unwrap();
    let morning = prefix
        .candidates
        .iter()
        .find(|s| matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer == "ព្រឹក"))
        .unwrap();
    assert_eq!(morning.completion.as_deref(), Some("prerk"));
    let allowed = core
        .suggest_with_context("proeuk", Default::default(), &Default::default())
        .unwrap();
    assert!(allowed.exact_candidates.iter().any(|c| c.khmer == "ព្រឹក"));
    let boat = core
        .suggest_with_context("tuk", Default::default(), &Default::default())
        .unwrap();
    assert!(boat.exact_candidates.iter().any(|c| c.khmer == "ទូក"));
    assert!(!core.suggest("ព្រឹក").unwrap().candidates.is_empty());
    assert_eq!(core.to_binary(), bytes);
}
#[test]
fn glottal_apostrophes_and_scoped_f_keep_boundaries_and_saved_aliases() {
    let mut data = dataset();
    data.words.as_mut().unwrap().extend(
        ["ផ្អែម", "ក្អក", "ល្អ", "ស្អាត", "កាហ្វេ", "ហ្វឹកហាត់", "កាហ្វេក", "អាន", "អៀន", "សប្បាយ", "កញ្ញា", "សញ្ញា", "ក្អក់", "ក្អ្រា", "ក្អ៉ា"]
            .iter().enumerate().map(|(i, word)| ApprovedWord {
                id: 200 + i as u64, khmer: word.to_string(), rank: i as u32, status: ApprovedStatus::Approved,
            }),
    );
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for (input, word) in [("ph'aem", "ផ្អែម"), ("p'aem", "ផ្អែម"), ("k'ark", "ក្អក"), ("k'ork", "ក្អក"),
        ("l'ar", "ល្អ"), ("l'or", "ល្អ"), ("s'at", "ស្អាត"), ("kafe", "កាហ្វេ"),
        ("an", "អាន"), ("ean", "អៀន"), ("sabbay", "សប្បាយ"), ("sabay", "សប្បាយ")] {
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        assert!(query.exact_candidates.iter().any(|c| c.khmer == word), "{input}: {query:?}");
        for features in [SuggestionFeatures { sound_map: false, ..Default::default() },
            SuggestionFeatures { romanized: false, ..Default::default() },
            SuggestionFeatures { mode: TypingMode::English, ..Default::default() }] {
            assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word));
        }
    }
    let exact = |input: &str, word: &str| core.suggest_with_context(input, Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == word);
    for (input, word) in [("phaem", "ផ្អែម"), ("paem", "ផ្អែម"), ("kak", "ក្អក"), ("k'orko", "ក្អក"),
        ("ka'ak", "ក្អក"), ("k'ark", "ក្អក់"), ("k'ra", "ក្អ្រា"), ("k'a", "ក្អ៉ា"),
        ("kafek", "កាហ្វេក"), ("feukh at", "ហ្វឹកហាត់"), ("sabbaay", "សប្បាយ"),
        ("sapbay", "សប្បាយ"), ("kanha", "កញ្ញា"), ("sanha", "សញ្ញា")] {
        assert!(!exact(input, word), "{input}: scope guard for {word}");
    }
    for input in ["'ph'aem", "ph''aem", "ph’ aem", "ph' ", "ph-aem"] {
        assert!(!core.suggest(input).unwrap().candidates.iter().any(|s| matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer == "ផ្អែម")), "{input}");
    }
    let prefix = core.suggest_with_context("ph'", Default::default(), &Default::default()).unwrap();
    assert!(prefix.exact_candidates.is_empty());
    assert_eq!(prefix.result.candidates.iter().find(|s| matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer == "ផ្អែម")).unwrap().completion.as_deref(), Some("ph'aem"));
    assert_eq!(core.to_binary(), bytes);
    // A confirmed alias remains independent of Sound map and keeps precedence.
    data.mappings.push(ApprovedMapping { id: 300, romanized: "p'aem".into(), khmer: "ផ្អែម".into(), rank: 0, status: ApprovedStatus::Approved });
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures { sound_map: false, ..Default::default() }] {
        let query = saved.suggest_with_context("p'aem", features, &Default::default()).unwrap();
        assert_eq!(query.exact_candidates.len(), 1);
        assert_eq!(query.exact_candidates[0].mapping_id, Some(300));
    }
}
#[test]
fn sound_default_matches_approved_words_and_off_retains_saved_aliases() {
    let core = Engine::new(dataset()).unwrap();
    for (input, expected) in [
        ("jong", "ចង់"),
        ("jong", "ចង"),
        ("sok", "សុខ"),
        ("suk", "សុខ"),
        ("seavphov", "សៀវភៅ"),
        ("sievphov", "សៀវភៅ"),
        ("si", "ស៊ី"),
        ("mae", "ម៉ែ"),
        ("cher", "ឈើ"),
    ] {
        let query = core
            .suggest_with_context(
                input,
                SuggestionFeatures::default(),
                &TypingContext::default(),
            )
            .unwrap();
        assert!(
            query.exact_candidates.iter().any(|c| c.khmer == expected),
            "{input}: {query:?}"
        );
        assert!(core
            .suggest_with_features(
                input,
                SuggestionFeatures {
                    sound_map: false,
                    ..Default::default()
                }
            )
            .unwrap()
            .candidates
            .is_empty());
    }
    let joined = core
        .suggest_with_context("jong", Default::default(), &Default::default())
        .unwrap();
    assert_eq!(joined.exact_candidates.len(), 2);
    assert_ne!(
        joined.exact_candidates[0].mapping_id,
        joined.exact_candidates[1].mapping_id
    );
    let alias = core
        .suggest_with_features(
            "jg",
            SuggestionFeatures {
                sound_map: false,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(alias.candidates[0].candidate.mapping_id(), Some(99));
    assert!(core
        .suggest_with_features(
            "jong",
            SuggestionFeatures {
                romanized: false,
                ..Default::default()
            }
        )
        .unwrap()
        .candidates
        .is_empty());
    // Written ា plus bantoc selects the O-series velar reading.
    assert!(core
        .suggest_with_context("peak", Default::default(), &Default::default())
        .unwrap()
        .exact_candidates
        .iter()
        .any(|c| c.khmer == "ពាក់"));
    assert!(core.suggest("xyz").unwrap().candidates.is_empty());
}
#[test]
fn prefixes_are_not_exact_and_ambiguity_keeps_words_without_mutating_data() {
    let core = Engine::new(dataset()).unwrap();
    let bytes = core.to_binary();
    let prefix = core
        .suggest_with_context(
            "jo",
            SuggestionFeatures::default(),
            &TypingContext::default(),
        )
        .unwrap();
    assert!(prefix.exact_candidates.is_empty());
    assert_eq!(
        prefix.result.candidates[0].completion.as_deref(),
        Some("jong")
    );
    let ambiguous = core
        .suggest_with_context(
            "kom",
            SuggestionFeatures::default(),
            &TypingContext::default(),
        )
        .unwrap();
    assert_eq!(ambiguous.exact_candidates.len(), 2); // កំ and កុំ use the pinned overlapping keys.
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    assert_eq!(restored.suggest("jo").unwrap(), core.suggest("jo").unwrap());
    assert_eq!(core.to_binary(), bytes);
    assert!(core.suggest("jo ").unwrap().candidates.is_empty());
    assert!(core.suggest("jo-ng").unwrap().candidates.is_empty());
    assert_eq!(
        core.suggest("ស").unwrap(),
        core.suggest_with_features(
            "ស",
            SuggestionFeatures {
                sound_map: false,
                ..Default::default()
            }
        )
        .unwrap()
    );
}

#[test]
fn silent_endings_keep_word_identities_and_do_not_silence_initials() {
    let mut data = dataset();
    data.words.as_mut().unwrap().extend(
        ["សត្វ", "ពី", "ពីរ", "គូ", "គូរ", "កូរ", "រា", "វា"]
            .iter()
            .enumerate()
            .map(|(i, word)| ApprovedWord {
                id: 200 + i as u64,
                khmer: word.to_string(),
                rank: 200 + i as u32,
                status: ApprovedStatus::Approved,
            }),
    );
    let core = Engine::new(data).unwrap();
    let bytes = core.to_binary();
    let exact = |input: &str, features| {
        core.suggest_with_context(input, features, &Default::default())
            .unwrap()
            .exact_candidates
            .into_iter()
            .map(|c| c.khmer)
            .collect::<Vec<_>>()
    };
    for (input, target) in [("sat", "សត្វ"), ("pi", "ពីរ"), ("ku", "គូរ"), ("ko", "កូរ")]
    {
        assert!(exact(input, Default::default()).iter().any(|w| w == target));
        assert!(!exact(
            input,
            SuggestionFeatures {
                sound_map: false,
                ..Default::default()
            }
        )
        .iter()
        .any(|w| w == target));
        assert!(!exact(
            input,
            SuggestionFeatures {
                romanized: false,
                ..Default::default()
            }
        )
        .iter()
        .any(|w| w == target));
    }
    assert_eq!(exact("pi", Default::default()), ["ពី", "ពីរ"]);
    assert_eq!(exact("ku", Default::default()), ["គូ", "គូរ"]);
    assert!(exact("rea", Default::default()).iter().any(|w| w == "រា"));
    assert!(exact("vea", Default::default()).iter().any(|w| w == "វា"));
    assert!(!exact("a", Default::default()).iter().any(|w| w == "រា"));
    assert!(!exact("ea", Default::default()).iter().any(|w| w == "វា"));
    for wrong in ["sot", "satv", "satw", "pir", "kur", "kor"] {
        assert!(exact(wrong, Default::default()).is_empty(), "{wrong}");
    }
    // The subsequently accepted ក្ខ coda keeps ទុក្ខ alongside the other tuk words.
    assert!(exact("tuk", Default::default()).iter().any(|w| w == "ទុក្ខ"));
    let prefix = core
        .suggest_with_context("sa", Default::default(), &Default::default())
        .unwrap();
    assert!(!prefix.exact_candidates.iter().any(|c| c.khmer == "សត្វ"));
    assert!(prefix
        .result
        .candidates
        .iter()
        .any(|s| s.completion.as_deref() == Some("sat")
            && matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer == "សត្វ")));
    assert_eq!(core.to_binary(), bytes);
    assert_eq!(
        Engine::from_binary(bytes).unwrap().suggest("sat").unwrap(),
        core.suggest("sat").unwrap()
    );
    let empty = Engine::new(ApprovedDataset {
        mappings: vec![],
        words: Some(vec![]),
        ..dataset()
    })
    .unwrap();
    assert!(empty.suggest("sat").unwrap().candidates.is_empty());
}

#[test]
fn nikahit_before_final_ngo_uses_one_nasal_and_preserves_series_and_other_endings() {
    let mut data = dataset();
    data.words.as_mut().unwrap().extend(
        ["តាំង", "សាំង", "ទាំង", "តាង", "ចាំ", "រាំ", "ខ្លាំង"]
            .iter()
            .enumerate()
            .map(|(i, word)| ApprovedWord {
                id: 300 + i as u64,
                khmer: word.to_string(),
                rank: 300 + i as u32,
                status: ApprovedStatus::Approved,
            }),
    );
    let core = Engine::new(data).unwrap();
    let bytes = core.to_binary();
    let exact = |input: &str, features| {
        core.suggest_with_context(input, features, &Default::default())
            .unwrap()
            .exact_candidates
            .into_iter()
            .map(|c| c.khmer)
            .collect::<Vec<_>>()
    };
    for (input, target) in [
        ("tang", "តាំង"),
        ("sang", "សាំង"),
        ("teang", "ទាំង"),
        ("jam", "ចាំ"),
        ("rorm", "រាំ"),
    ] {
        assert!(
            exact(input, Default::default()).iter().any(|w| w == target),
            "{input}"
        );
        assert!(!exact(
            input,
            SuggestionFeatures {
                sound_map: false,
                ..Default::default()
            }
        )
        .iter()
        .any(|w| w == target));
        assert!(!exact(
            input,
            SuggestionFeatures {
                romanized: false,
                ..Default::default()
            }
        )
        .iter()
        .any(|w| w == target));
    }
    assert_eq!(exact("tang", Default::default()), ["តាំង", "តាង"]);
    for input in ["tamng", "samng", "tormng", "ta-ng", "ja", "roa"] {
        assert!(exact(input, Default::default()).is_empty(), "{input}");
    }
    // The separate reviewed onset selector now keeps ខ's A-series before ល.
    assert!(exact("khlang", Default::default())
        .iter()
        .any(|w| w == "ខ្លាំង"));
    let prefix = core
        .suggest_with_context("tan", Default::default(), &Default::default())
        .unwrap();
    assert!(prefix.exact_candidates.is_empty());
    assert!(prefix
        .result
        .candidates
        .iter()
        .any(|s| s.completion.as_deref() == Some("tang")
            && matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer=="តាំង")));
    assert_eq!(core.to_binary(), bytes);
    assert_eq!(
        Engine::from_binary(bytes).unwrap().suggest("tang").unwrap(),
        core.suggest("tang").unwrap()
    );
}

#[test]
fn span_must_not_match_the_mixed_series_bridge_word() {
    let mut data = dataset();
    data.words.as_mut().unwrap().push(ApprovedWord {
        id: 400,
        khmer: "ស្ពាន".into(),
        rank: 0,
        status: ApprovedStatus::Approved,
    });
    let core = Engine::new(data).unwrap();
    assert!(core.suggest_with_context("spean", Default::default(), &Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer == "ស្ពាន"));
    for input in ["span", "SPAN", "span ", "spa"] {
        let query = core
            .suggest_with_context(input, Default::default(), &Default::default())
            .unwrap();
        assert!(
            !query.exact_candidates.iter().any(|c| c.khmer == "ស្ពាន"),
            "{input}"
        );
        assert!(
            !query
                .result
                .candidates
                .iter()
                .any(|s| s.correction.is_none() && matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer == "ស្ពាន")),
            "{input}"
        );
        if input == "span" || input == "SPAN" {
            // Recovery is a manual spelling suggestion, never a new reading.
            assert!(query.result.candidates.iter().any(|s| s.correction.as_deref() == Some("spean")
                && matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer == "ស្ពាន")));
        }
    }
    // The word remains eligible for direct Khmer completion.
    assert!(core
        .suggest("ស្ពាន")
        .unwrap()
        .candidates
        .iter()
        .any(|s| matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer == "ស្ពាន")));
}

#[test]
fn contextual_vowels_use_the_final_and_guarded_governing_series() {
    let mut data = dataset();
    data.words.as_mut().unwrap().extend(
        [
            "គាត់",
            "មាត់",
            "មាន់",
            "មាន",
            "លាក់",
            "ចាប់",
            "កាត់",
            "សាលា",
            "កីឡា",
            "បារី",
            "ការី",
            "កុលាប",
            "បុរាណ",
            "សេរី",
            "សាលី",
            "គីឡូ",
            "គីមី",
            "សុភាព",
            "សាលារៀន",
            "គូក់",
            "ប្រវត្តិ",
        ]
        .iter()
        .enumerate()
        .map(|(i, word)| ApprovedWord {
            id: 500 + i as u64,
            khmer: word.to_string(),
            rank: 500 + i as u32,
            status: ApprovedStatus::Approved,
        }),
    );
    let core = Engine::new(data).unwrap();
    let bytes = core.to_binary();
    let exact = |input: &str, features| {
        core.suggest_with_context(input, features, &Default::default())
            .unwrap()
            .exact_candidates
            .into_iter()
            .map(|c| c.khmer)
            .collect::<Vec<_>>()
    };
    for (input, target) in [
        ("koat", "គាត់"),
        ("moat", "មាត់"),
        ("moan", "មាន់"),
        ("mean", "មាន"),
        ("leak", "លាក់"),
        ("peak", "ពាក់"),
        ("jap", "ចាប់"),
        ("kat", "កាត់"),
        ("sala", "សាលា"),
        ("keila", "កីឡា"),
        ("keyla", "កីឡា"),
        ("barei", "បារី"),
        ("barey", "បារី"),
        ("karei", "ការី"),
        ("kolap", "កុលាប"),
        ("boran", "បុរាណ"),
        ("serei", "សេរី"),
        ("sali", "សាលី"),
        ("kilo", "គីឡូ"),
        ("kimi", "គីមី"),
        ("sopheap", "សុភាព"),
        ("salarean", "សាលារៀន"),
    ] {
        assert!(
            exact(input, Default::default()).iter().any(|w| w == target),
            "{input}"
        );
        assert!(
            !exact(
                input,
                SuggestionFeatures {
                    sound_map: false,
                    ..Default::default()
                }
            )
            .iter()
            .any(|w| w == target),
            "{input}: disabled"
        );
        assert!(
            !exact(
                input,
                SuggestionFeatures {
                    romanized: false,
                    ..Default::default()
                }
            )
            .iter()
            .any(|w| w == target),
            "{input}: disabled Romanized conversion"
        );
    }
    for (input, target) in [
        ("keat", "គាត់"),
        ("meat", "មាត់"),
        ("mean", "មាន់"),
        ("liak", "លាក់"),
        ("salea", "សាលា"),
        ("bari", "បារី"),
        ("keilea", "កីឡា"),
        ("salei", "សាលី"),
        ("kilu", "គីឡូ"),
        ("sophap", "សុភាព"),
        ("salearean", "សាលារៀន"),
        ("kuk", "គូក់"),
        ("pra vat", "ប្រវត្តិ"),
        ("sa la", "សាលា"),
    ] {
        assert!(
            !exact(input, Default::default()).iter().any(|w| w == target),
            "{input}"
        );
    }
    let prefix = core
        .suggest_with_context("koa", Default::default(), &Default::default())
        .unwrap();
    assert!(!prefix.exact_candidates.iter().any(|w| w.khmer == "គាត់"));
    assert!(prefix
        .result
        .candidates
        .iter()
        .any(|s| s.completion.as_deref() == Some("koat")
            && matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer == "គាត់")));
    assert_eq!(core.to_binary(), bytes);
    assert_eq!(
        Engine::from_binary(bytes).unwrap().suggest("koat").unwrap(),
        core.suggest("koat").unwrap()
    );
    let empty = Engine::new(ApprovedDataset {
        mappings: vec![],
        words: Some(vec![]),
        ..dataset()
    })
    .unwrap();
    assert!(empty.suggest("koat").unwrap().candidates.is_empty());
}

#[test]
fn verified_silent_endings_preserve_vowels_boundaries_and_saved_aliases() {
    let mut data = dataset();
    data.words.as_mut().unwrap().extend(
        [
            "ភូមិ",
            "ជាតិ",
            "ធាតុ",
            "ហេតុ",
            "សមុទ្រ",
            "សូត្រ",
            "សំបុត្រ",
            "លីត្រ",
            "ពេទ្យ",
            "មិត្ត",
            "ខេត្ត",
            "មតិ",
            "សតិ",
            "ពិន្ទុ",
            "គាំទ្រ",
            "ទ្រី",
            "វិទ្យា",
            "ភូមិសាស្ត្រ",
            "លក់",
            "ភក់",
            "ពពក",
            "បបរ",
        ]
        .iter()
        .enumerate()
        .map(|(i, word)| ApprovedWord {
            id: 700 + i as u64,
            khmer: word.to_string(),
            rank: 700 + i as u32,
            status: ApprovedStatus::Approved,
        }),
    );
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let exact = |input: &str, features| {
        core.suggest_with_context(input, features, &Default::default())
            .unwrap()
            .exact_candidates
            .into_iter()
            .map(|c| c.khmer)
            .collect::<Vec<_>>()
    };
    for (input, word) in [
        ("phum", "ភូមិ"),
        ("jeat", "ជាតិ"),
        ("cheat", "ជាតិ"),
        ("theat", "ធាតុ"),
        ("het", "ហេតុ"),
        ("sormot", "សមុទ្រ"),
        ("sormut", "សមុទ្រ"),
        ("sot", "សូត្រ"),
        ("sombot", "សំបុត្រ"),
        ("lit", "លីត្រ"),
        ("pet", "ពេទ្យ"),
        ("mit", "មិត្ត"),
        ("khet", "ខេត្ត"),
    ] {
        assert!(
            exact(input, Default::default()).iter().any(|w| w == word),
            "{input}"
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
        ] {
            assert!(
                !exact(input, features).iter().any(|w| w == word),
                "{input}: disabled"
            );
        }
        let restored = Engine::from_binary(bytes.clone()).unwrap();
        assert_eq!(
            restored.suggest(input).unwrap(),
            core.suggest(input).unwrap()
        );
    }
    // The same signs/subscripts can be pronounced in another word or position.
    for (input, word) in [
        ("moti", "មតិ"),
        ("sorti", "សតិ"),
        ("pinto", "ពិន្ទុ"),
        ("kormtro", "គាំទ្រ"),
        ("tri", "ទ្រី"),
        ("vityea", "វិទ្យា"),
        ("luk", "លក់"),
        ("phuk", "ភក់"),
    ] {
        assert!(
            exact(input, Default::default()).iter().any(|w| w == word),
            "{input}"
        );
    }
    for (input, word) in [
        ("phumi", "ភូមិ"),
        ("jeati", "ជាតិ"),
        ("theatu", "ធាតុ"),
        ("hetu", "ហេតុ"),
        ("samotro", "សមុទ្រ"),
        ("petyo", "ពេទ្យ"),
        ("mitta", "មិត្ត"),
        ("khetta", "ខេត្ត"),
        ("mot", "មតិ"),
        ("sat", "សតិ"),
        ("pint", "ពិន្ទុ"),
        ("kormt", "គាំទ្រ"),
        ("vi", "វិទ្យា"),
        ("phum", "ភូមិសាស្ត្រ"),
        ("haet", "ហេតុ"),
        ("khaet", "ខេត្ត"),
        ("babo", "បបរ"),
    ] {
        assert!(
            !exact(input, Default::default()).iter().any(|w| w == word),
            "{input}"
        );
    }
    let prefix = core
        .suggest_with_context("phu", Default::default(), &Default::default())
        .unwrap();
    assert!(prefix.exact_candidates.is_empty());
    assert!(prefix
        .result
        .candidates
        .iter()
        .any(|s| s.completion.as_deref() == Some("phum")
            && matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer == "ភូមិ")));
    assert_eq!(core.to_binary(), bytes);

    // An existing saved spelling keeps its identity and still works with Sound map off.
    data.mappings.push(ApprovedMapping {
        id: 100,
        romanized: "phumi".into(),
        khmer: "ភូមិ".into(),
        rank: 0,
        status: ApprovedStatus::Approved,
    });
    let aliases = Engine::new(data).unwrap();
    let saved = aliases
        .suggest_with_context(
            "phumi",
            SuggestionFeatures {
                sound_map: false,
                ..Default::default()
            },
            &Default::default(),
        )
        .unwrap();
    assert_eq!(saved.exact_candidates[0].mapping_id, Some(100));
    assert_eq!(saved.exact_candidates[0].khmer, "ភូមិ");
    let empty = Engine::new(ApprovedDataset {
        mappings: vec![],
        words: Some(vec![]),
        ..dataset()
    })
    .unwrap();
    assert!(empty.suggest("phum").unwrap().candidates.is_empty());
}

#[test]
fn reviewed_presyllables_and_short_velar_keys_preserve_scope_and_identity() {
    let mut data = dataset();
    let words = [
        "ពពក",
        "បបរ",
        "លក់",
        "ភក់",
        "កក",
        "កក់",
        "លក",
        "ទប់",
        "លលក",
        "លក់ដូរ",
    ];
    data.words
        .as_mut()
        .unwrap()
        .extend(words.iter().enumerate().map(|(i, word)| ApprovedWord {
            id: 800 + i as u64,
            khmer: word.to_string(),
            rank: 800 + i as u32,
            status: ApprovedStatus::Approved,
        }));
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    for (input, word) in [
        ("popok", "ពពក"),
        ("borbor", "បបរ"),
        ("luk", "លក់"),
        ("luak", "លក់"),
        ("luok", "លក់"),
        ("phuk", "ភក់"),
        ("phuak", "ភក់"),
        ("phuok", "ភក់"),
    ] {
        let query = core
            .suggest_with_context(input, Default::default(), &Default::default())
            .unwrap();
        let matched = query
            .exact_candidates
            .iter()
            .find(|c| c.khmer == word)
            .expect(input);
        let position = words.iter().position(|w| *w == word).unwrap();
        assert_eq!(matched.mapping_id, Some(800 + position as u64), "{input}");
        assert_eq!(matched.rank, 800 + position as u32, "{input}");
        assert_eq!(
            restored.suggest(input).unwrap(),
            core.suggest(input).unwrap()
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
            assert!(
                !core
                    .suggest_with_context(input, features, &Default::default())
                    .unwrap()
                    .exact_candidates
                    .iter()
                    .any(|c| c.khmer == word),
                "{input}: disabled"
            );
        }
    }
    for (input, word, completion) in [
        ("popo", "ពពក", "popok"),
        ("borb", "បបរ", "borbor"),
        ("lua", "លក់", "luak"),
        ("lu", "លក់", "luk"),
    ] {
        let query = core
            .suggest_with_context(input, Default::default(), &Default::default())
            .unwrap();
        assert!(
            !query.exact_candidates.iter().any(|c| c.khmer == word),
            "{input}"
        );
        assert!(
            query
                .result
                .candidates
                .iter()
                .any(|s| s.completion.as_deref() == Some(completion)
                    && matches!(&s.candidate, SuggestionCandidate::Khmer(c) if c.khmer == word)),
            "{input}"
        );
    }
    // Repeated letters, other codas/series, explicit vowels and medial finals
    // do not acquire the new reading just because some letters match.
    for (input, word) in [
        ("kok", "កក"),
        ("kok", "កក់"),
        ("lok", "លក"),
        ("top", "ទប់"),
        // Without the removed លក់ override, compounds use the ordinary map.
        ("lokdo", "លក់ដូរ"),
        // No repeat-first-syllable rule is needed for the mechanical mapped
        // default; this does not broaden the reviewed ពពក/បបរ pronunciation.
        ("lolok", "លលក"),
    ] {
        assert!(
            core.suggest_with_context(input, Default::default(), &Default::default())
                .unwrap()
                .exact_candidates
                .iter()
                .any(|c| c.khmer == word),
            "{input}: existing"
        );
    }
    for (input, word) in [
        ("lukdo", "លក់ដូរ"),
        ("luakdo", "លក់ដូរ"),
        ("luokdo", "លក់ដូរ"),
        ("kaka", "កក"),
        ("babo", "បបរ"),
        ("kuak", "កក់"),
        ("kuok", "កក់"),
        ("luak", "លក"),
        ("luok", "លក"),
        ("tuap", "ទប់"),
        ("tuop", "ទប់"),
        ("tuak", "ទុក"),
        ("tuok", "ទុក"),
        ("tuak", "ទូក"),
        ("tuok", "ទូក"),
        ("po pok", "ពពក"),
        ("ba ba", "បបរ"),
    ] {
        assert!(
            !core
                .suggest_with_context(input, Default::default(), &Default::default())
                .unwrap()
                .exact_candidates
                .iter()
                .any(|c| c.khmer == word),
            "{input}: guard"
        );
    }
    assert_eq!(core.to_binary(), bytes);
    data.mappings.push(ApprovedMapping {
        id: 100,
        romanized: "babor".into(),
        khmer: "បបរ".into(),
        rank: 0,
        status: ApprovedStatus::Approved,
    });
    let saved = Engine::new(data)
        .unwrap()
        .suggest_with_context(
            "babor",
            SuggestionFeatures {
                sound_map: false,
                ..Default::default()
            },
            &Default::default(),
        )
        .unwrap();
    assert_eq!(saved.exact_candidates[0].mapping_id, Some(100));
    assert_eq!(saved.exact_candidates[0].khmer, "បបរ");
    let empty = Engine::new(ApprovedDataset {
        mappings: vec![],
        words: Some(vec![]),
        ..dataset()
    })
    .unwrap();
    for input in ["popok", "baba", "luak", "phuok"] {
        assert!(empty.suggest(input).unwrap().candidates.is_empty());
    }
}


#[test]
fn terminal_sap_unit_and_named_y_aliases_keep_scope_and_identity() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/terminal-sap-y-readings.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, w)| ApprovedWord {
        id: 97500 + i as u64, khmer: w.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    for group in ["soundYes", "compatibility", "endingOnly"] {
        for row in proof[group].as_array().unwrap() {
            let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
            let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let candidate = query.exact_candidates.iter().find(|c| c.khmer == word)
                .unwrap_or_else(|| panic!("{input}: {word}"));
            let position = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
            assert_eq!(candidate.mapping_id, Some(97500 + position as u64));
            assert_eq!(candidate.rank, position as u32);
            for features in [SuggestionFeatures {sound_map: false, ..Default::default()},
                SuggestionFeatures {romanized: false, ..Default::default()},
                SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
                assert!(!core.suggest_with_context(input, features, &Default::default()).unwrap()
                    .exact_candidates.iter().any(|c| c.khmer == word));
            }
        }
    }
    for group in ["no", "aliasYes"] {
        for row in proof[group].as_array().unwrap() {
            let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
            // Later explicitly selected medical scopes supersede only their
            // exact historical negatives; saved-alias precedence still applies.
            let medical: serde_json::Value = serde_json::from_str(include_str!(
                "../../test/fixtures/checked-medical-gain-components-20261009.json"
            )).unwrap();
            let remaining: serde_json::Value = serde_json::from_str(include_str!(
                "../../test/fixtures/remaining-recurring-components-20261010.json"
            )).unwrap();
            // Compound fixtures with more available component words may still
            // derive a key; retired overrides must not supply it on their own.
            let retired = [&medical, &remaining].iter().any(|fixture|
                fixture["retiredGenerated"].as_object().is_some_and(|groups|
                    groups.values().any(|rows| rows.as_array().unwrap().contains(row))));
            let selected = !retired && medical["newPairs"].as_array().unwrap().iter()
                .chain(remaining["newPairs"].as_array().unwrap()).any(|pair|
                pair[0].as_str() == Some(input) && pair[1].as_str() == Some(word));
            assert_eq!(core.suggest_with_context(input, Default::default(), &Default::default()).unwrap()
                .exact_candidates.iter().any(|c| c.khmer == word), selected, "{input}: {word}");
        }
    }
    for row in proof["prefixes"].as_array().unwrap().iter().take(2) {
        let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
        let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
        assert!(!query.exact_candidates.iter().any(|c| c.khmer == word));
        assert!(query.result.candidates.iter().any(|c| matches!(&c.candidate, SuggestionCandidate::Khmer(k) if k.khmer == word)
            && c.completion.as_deref() == row[2].as_str()), "{input}: {word}");
    }
    assert_eq!(core.to_binary(), bytes);
    assert_eq!(Engine::from_binary(bytes).unwrap().suggest("sap").unwrap(), core.suggest("sap").unwrap());
    for (i,row) in proof["aliasYes"].as_array().unwrap().iter().enumerate() {
        data.mappings.push(ApprovedMapping {id: 99100+i as u64, romanized: row[0].as_str().unwrap().into(),
            khmer: row[1].as_str().unwrap().into(), rank: 300+i as u32, status: ApprovedStatus::Approved});
    }
    let saved = Engine::new(data).unwrap();
    for (i,row) in proof["aliasYes"].as_array().unwrap().iter().enumerate() {
        let input = row[0].as_str().unwrap(); let word = row[1].as_str().unwrap();
        for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
            let query = saved.suggest_with_context(input, features, &Default::default()).unwrap();
            let candidate = query.exact_candidates.iter().find(|c| c.khmer==word).unwrap();
            assert_eq!(candidate.mapping_id, Some(99100+i as u64)); assert_eq!(candidate.rank,300+i as u32);
        }
        for features in [SuggestionFeatures {romanized: false, ..Default::default()},
            SuggestionFeatures {mode: TypingMode::English, ..Default::default()}] {
            assert!(!saved.suggest_with_context(input,features,&Default::default()).unwrap().exact_candidates.iter().any(|c| c.khmer==word));
        }
    }
}
#[test]
fn reviewed_map_combinations_keep_shared_series_original_end_and_identity() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/reviewed-map-combinations.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, word)| ApprovedWord {
        id: 98000 + i as u64, khmer: word.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    for field in ["yes", "compatibility", "no"] {
        for pair in proof[field].as_array().unwrap() {
            let input = pair[0].as_str().unwrap();
            let word = pair[1].as_str().unwrap();
            let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let found = query.exact_candidates.iter().find(|c| c.khmer == word);
            assert_eq!(found.is_some(), field != "no", "{field}: {input}: {word}");
            if let Some(candidate) = found {
                let index = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
                assert_eq!(candidate.mapping_id, Some(98000 + index as u64));
                assert_eq!(candidate.rank, index as u32);
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
    assert_eq!(core.to_binary(), bytes);
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "aoy".into(), khmer: "ឱ្យ".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let result = saved.suggest_with_context("aoy", features, &Default::default()).unwrap();
        assert_eq!(result.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(result.exact_candidates[0].rank, 77);
    }
}

#[test]
fn plain_o_eu_replaces_generated_ueu_and_preserves_combinations_and_aliases() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/plain-o-eu-replacement.json")).unwrap();
    let words = proof["words"].as_array().unwrap();
    let mut data = dataset();
    data.mappings.clear();
    data.words = Some(words.iter().enumerate().map(|(i, word)| ApprovedWord {
        id: 98000 + i as u64, khmer: word.as_str().unwrap().into(), rank: i as u32,
        status: ApprovedStatus::Approved,
    }).collect());
    let core = Engine::new(data.clone()).unwrap();
    let bytes = core.to_binary();
    let restored = Engine::from_binary(bytes.clone()).unwrap();
    for field in ["yes", "compatibility", "no"] {
        for pair in proof[field].as_array().unwrap() {
            let input = pair[0].as_str().unwrap();
            let word = pair[1].as_str().unwrap();
            let query = core.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
            let found = query.exact_candidates.iter().find(|c| c.khmer == word);
            assert_eq!(found.is_some(), field != "no", "{field}: {input}: {word}");
            if let Some(candidate) = found {
                let index = words.iter().position(|w| w.as_str() == Some(word)).unwrap();
                assert_eq!(candidate.mapping_id, Some(98000 + index as u64));
                assert_eq!(candidate.rank, index as u32);
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
    assert_eq!(core.to_binary(), bytes);
    data.mappings.push(ApprovedMapping {id: 99100, romanized: "phlueu".into(), khmer: "ភ្លឺ".into(), rank: 77, status: ApprovedStatus::Approved});
    let saved = Engine::new(data).unwrap();
    for features in [Default::default(), SuggestionFeatures {sound_map: false, ..Default::default()}] {
        let result = saved.suggest_with_context("phlueu", features, &Default::default()).unwrap();
        assert_eq!(result.exact_candidates[0].mapping_id, Some(99100));
        assert_eq!(result.exact_candidates[0].rank, 77);
    }
}
