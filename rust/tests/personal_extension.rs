use khmer_engine::{Engine, SuggestionCandidate, SuggestionFeatures, TypingContext, TypingMode};
use serde_json::json;

fn engines() -> (Engine, Engine) {
    let base = Engine::from_json(&json!({"schemaVersion":3,
        "mappings":[{"id":1,"romanized":"sok","khmer":"សុខ","rank":1,"status":"approved"}],
        "words":[{"id":1,"khmer":"សុខ","rank":1,"status":"approved"}],
        "englishWords":[{"id":2,"text":"peach","rank":1,"status":"approved"}]}).to_string()).unwrap();
    let extra = Engine::from_json(&json!({"schemaVersion":3,
        "mappings":[{"id":8000000000000001_u64,"romanized":"sok sabay","khmer":"សុខសប្បាយ","rank":0,"status":"approved"},
            {"id":8000000000000002_u64,"romanized":"sok","khmer":"សុខ","rank":0,"status":"approved"}],
        "words":[{"id":8000000000000001_u64,"khmer":"សុខសប្បាយ","rank":0,"status":"approved"}],
        "englishWords":[{"id":8000000000000003_u64,"text":"pear","rank":0,"status":"approved"}]}).to_string()).unwrap();
    (base, extra)
}

#[test]
fn new_vowel_alternatives_do_not_displace_saved_exact_spellings_in_either_layer() {
    let generated = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"englishWords":[],
        "words":[{"id":1,"khmer":"ផ្ទះ","rank":0,"status":"approved"},
            {"id":2,"khmer":"ចុះហត្ថលេខា","rank":0,"status":"approved"}]}).to_string()).unwrap();
    let saved = Engine::from_personal_words_json(&json!([
        {"word":"សៀវភៅ","spellings":["phteas","joshatlekha"]}
    ]).to_string()).unwrap();
    let before = [generated.to_binary(), saved.to_binary()];
    for (base, extension) in [(&generated, &saved), (&saved, &generated)] {
        for (input, alternative) in [("phteas", "ផ្ទះ"), ("joshatlekha", "ចុះហត្ថលេខា")] {
            let result = base.suggest_with_extension(extension, input, SuggestionFeatures::default(), &TypingContext::default()).unwrap();
            let exact: Vec<_> = result.exact_candidates.iter().map(|word| word.khmer.as_str()).collect();
            assert_eq!(exact, ["សៀវភៅ", alternative], "{input}");
            assert!(matches!(&result.result.candidates[0].candidate, SuggestionCandidate::Khmer(word) if word.khmer == "សៀវភៅ"));
        }
    }
    assert_eq!([generated.to_binary(), saved.to_binary()], before);
}

#[test]
fn additive_endings_preserve_existing_count_matches_with_context_and_custom_ranks() {
    // A synthetic word isolates an existing doubled-consonant reading jjos.
    let old_word = "ច្ចុស";
    let words = ["ចុះ", "សុខ", "សប្បាយ", "សុខសប្បាយ"];
    let base = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"englishWords":[],
        "words":words.iter().enumerate().map(|(i,word)|json!({
            "id":i+1,"khmer":word,"rank":0,"status":"approved"
        })).collect::<Vec<_>>()}).to_string()).unwrap();
    let personal = Engine::from_personal_words_json(&json!([
        {"word":old_word,"spellings":["my-jjos"]}
    ]).to_string()).unwrap();
    let before = [base.to_binary(), personal.to_binary()];
    for rank in [None, Some(0), Some(20)] {
        let personal = match rank {
            Some(rank) => personal.with_typing_overrides_json(&json!([
                {"language":"khmer","word":old_word,"rank":rank}
            ]).to_string()).unwrap(),
            None => Engine::from_personal_words_json(&json!([
                {"word":old_word,"spellings":["my-jjos"]}
            ]).to_string()).unwrap(),
        };
        for (base, extension) in [(&base,&personal),(&personal,&base)] {
            for previous_words in [vec![],vec!["សុខ".into()]] {
                let context = TypingContext {previous_words,..Default::default()};
                let result = base.suggest_with_extension(extension,"jos",Default::default(),&context).unwrap();
                let exact: Vec<_> = result.exact_candidates.iter().map(|word|word.khmer.as_str()).collect();
                assert_eq!(exact,[old_word,"ចុះ"],"rank {rank:?}, {context:?}");
                let displayed: Vec<_> = result.result.candidates.iter().filter_map(|hit|match &hit.candidate {
                    SuggestionCandidate::Khmer(word) if hit.completion.is_none() && hit.correction.is_none() => Some(word.khmer.as_str()),
                    _ => None,
                }).collect();
                assert_eq!(displayed,exact,"rank {rank:?}, {context:?}");
                let saved = base.suggest_with_extension(extension,"my-jjos",
                    SuggestionFeatures {sound_map:false,..Default::default()},&context).unwrap();
                assert_eq!(saved.exact_candidates[0].khmer,old_word);
            }
        }
    }
    assert_eq!([base.to_binary(),personal.to_binary()],before);
}

#[test]
fn multiple_additive_endings_share_one_display_and_exact_order() {
    // Synthetic words isolate two new endings against a single/double match.
    let words = ["កះកះ", "កះកះកា", "សុខ", "សប្បាយ", "សុខសប្បាយ"];
    let base = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"englishWords":[],
        "words":words.iter().enumerate().map(|(i,word)|json!({
            "id":i+1,"khmer":word,"rank":0,"status":"approved"
        })).collect::<Vec<_>>()}).to_string()).unwrap();
    let personal = Engine::from_personal_words_json(r#"[{"word":"ក្កាសកះ","spellings":["personal-reading"]}]"#).unwrap();
    let before = [base.to_binary(),personal.to_binary()];
    for rank in [None,Some(0),Some(20)] {
        let extra = match rank {
            Some(rank) => personal.with_typing_overrides_json(&json!([
                {"language":"khmer","word":"ក្កាសកះ","rank":rank}
            ]).to_string()).unwrap(),
            None => Engine::from_personal_words_json(r#"[{"word":"ក្កាសកះ","spellings":["personal-reading"]}]"#).unwrap(),
        };
        for (base,extension) in [(&base,&extra),(&extra,&base)] {
            for previous_words in [vec![],vec!["សុខ".into()]] {
                let context = TypingContext {previous_words,..Default::default()};
                let result = base.suggest_with_extension(extension,"kaskas",Default::default(),&context).unwrap();
                let exact: Vec<_> = result.exact_candidates.iter().map(|word|word.khmer.as_str()).collect();
                assert_eq!(exact,["កះកះ","ក្កាសកះ"],"rank {rank:?}, {context:?}");
                let displayed: Vec<_> = result.result.candidates.iter().filter_map(|hit|match &hit.candidate {
                    SuggestionCandidate::Khmer(word) if hit.completion.is_none() && hit.correction.is_none() => Some(word.khmer.as_str()),
                    _ => None,
                }).collect();
                assert_eq!(displayed,exact,"rank {rank:?}, {context:?}");
                assert!(result.result.candidates.iter().any(|hit|hit.completion.as_deref()==Some("kaskaska")));
                let tolerated = result.result.candidates.iter().find(|hit|matches!(&hit.candidate,
                    SuggestionCandidate::Khmer(word) if word.khmer=="ក្កាសកះ")).unwrap();
                assert_eq!(tolerated.preferred_input.as_deref(),Some("kkaskas"));
                assert_eq!(base.suggest_with_extension(extension,"personal-reading",Default::default(),&context)
                    .unwrap().exact_candidates[0].khmer,"ក្កាសកះ");
            }
        }
    }
    assert_eq!([base.to_binary(),personal.to_binary()],before);
}

#[test]
fn older_signed_priorities_keep_their_merge_order_with_personal_aliases() {
    let words = ["រាំ", "គាំ", "នាំ", "ប៉ុស្តិ៍"];
    let base = Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "mappings":[{"id":5,"romanized":"poh","khmer":"ប៉ុស្តិ៍","rank":0,"status":"approved"}],
        "words":words.iter().enumerate().map(|(i,word)|json!({
            "id":i+1,"khmer":word,"rank":0,"status":"approved"
        })).collect::<Vec<_>>()}).to_string()).unwrap();
    let personal = Engine::from_personal_words_json(&json!([
        {"word":"សៀវភៅ","spellings":["roam","koam","noam","poh"]}
    ]).to_string()).unwrap();
    let before = [base.to_binary(), personal.to_binary()];
    for (input, word) in [("roam","រាំ"),("koam","គាំ"),("noam","នាំ"),("poh","ប៉ុស្តិ៍")] {
        for (base, extension, expected) in [(&base,&personal,vec![word,"សៀវភៅ"]),
            (&personal,&base,vec!["សៀវភៅ",word])] {
            let result = base.suggest_with_extension(extension,input,Default::default(),&Default::default()).unwrap();
            assert_eq!(result.exact_candidates.iter().map(|word|word.khmer.as_str()).collect::<Vec<_>>(),expected,"{input}");
            assert!(matches!(&result.result.candidates[0].candidate,
                SuggestionCandidate::Khmer(word) if word.khmer == expected[0]));
        }
    }
    assert_eq!([base.to_binary(), personal.to_binary()], before);
}

#[test]
fn personal_lookup_preserves_base_and_exact_identities_and_never_promotes_prefixes() {
    let (base, extra) = engines();
    let before = base.to_binary();
    let query = base.suggest_with_extension(&extra, "sok", SuggestionFeatures::default(), &TypingContext::default()).unwrap();
    assert_eq!(query.exact_candidates.len(), 1);
    assert_eq!(query.exact_candidates[0].mapping_id, Some(1));
    assert_eq!(query.result.candidates.len(), 2);
    assert!(query.result.candidates[0].completion.is_none());
    assert_eq!(query.result.candidates[1].completion.as_deref(), Some("sok sabay"));
    let joined = base.suggest_with_extension(&extra, "soksabay", SuggestionFeatures::default(), &TypingContext::default()).unwrap();
    assert_eq!(joined.exact_candidates[0].khmer, "សុខសប្បាយ");
    assert_eq!(base.to_binary(), before);
    assert_eq!(base.suggest_with_context("soksabay", SuggestionFeatures::default(), &TypingContext::default()).unwrap().result.candidates.len(), 0);
}

#[test]
fn english_and_khmer_personal_completions_keep_feature_gates_and_outputs() {
    let (base, extra) = engines();
    let features = SuggestionFeatures { mode: TypingMode::English, ..Default::default() };
    let query = base.suggest_with_extension(&extra, "pe", features, &TypingContext::default()).unwrap();
    let words: Vec<_> = query.result.candidates.iter().map(|s| match &s.candidate {
        SuggestionCandidate::English(word) => word.text.as_str(), _ => panic!("English-only query"),
    }).collect();
    assert_eq!(words, ["pear", "peach"]);
    assert!(query.exact_candidates.is_empty());
    assert!(base.suggest_with_extension(&extra, "pe", SuggestionFeatures { english_completion: false, ..features },
        &TypingContext::default()).unwrap().result.candidates.is_empty());
    assert!(base.suggest_with_extension(&extra, "sok", SuggestionFeatures { romanized: false, ..Default::default() },
        &TypingContext::default()).unwrap().result.candidates.is_empty());
    assert!(base.suggest_with_extension(&extra, "សុខ", SuggestionFeatures { khmer_completion: false, ..Default::default() },
        &TypingContext::default()).unwrap().result.candidates.is_empty());
}

#[test]
fn complete_english_words_precede_personal_prefixes_and_keep_base_identities() {
    let base = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],
        "englishWords":[{"id":1,"text":"cat","rank":10,"status":"approved"},
            {"id":2,"text":"catalogue","rank":0,"status":"approved"}]}).to_string()).unwrap();
    let extra = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],
        "englishWords":[{"id":8000000000000001_u64,"text":"catalog","rank":0,"status":"approved"},
            {"id":8000000000000002_u64,"text":"cat","rank":0,"status":"approved"}]}).to_string()).unwrap();
    let features = SuggestionFeatures { mode: TypingMode::English, ..Default::default() };
    for input in ["cat", "CAT"] {
        let query = base.suggest_with_extension(&extra, input, features, &TypingContext::default()).unwrap();
        let words: Vec<_> = query.result.candidates.iter().map(|s| match &s.candidate {
            SuggestionCandidate::English(word) => (word.text.as_str(), word.mapping_id), _ => panic!("English-only query"),
        }).collect();
        assert_eq!(words, [("cat", 1), ("catalog", 8000000000000001), ("catalogue", 2)]);
        assert!(query.exact_candidates.is_empty(), "English matches must not become Khmer conversions");
    }
    let query = base.suggest_with_extension(&extra, "catalog", features, &TypingContext::default()).unwrap();
    assert_eq!(query.result.candidates[0].completion.as_deref(), Some("catalog"));
    assert_eq!(query.result.candidates[1].completion.as_deref(), Some("catalogue"));
}

#[test]
fn personal_metadata_cannot_restore_disabled_generated_words_or_repetitions() {
    let base = Engine::from_json(&json!({"schemaVersion":3,
        "words":[{"id":1,"khmer":"កា","rank":0,"status":"approved"},
            {"id":2,"khmer":"កាៗ","rank":0,"status":"approved"},
            {"id":3,"khmer":"ខា","rank":0,"status":"approved"}],
        "mappings":[{"id":4,"romanized":"saved","khmer":"កា","rank":0,"status":"approved"}],
        "soundMapDisabledWords":["កា"],"englishWords":[]}).to_string()).unwrap();
    let extra = Engine::from_json(&json!({"schemaVersion":3,
        "words":[{"id":11,"khmer":"កា","rank":0,"status":"approved"},
            {"id":12,"khmer":"កាៗ","rank":0,"status":"approved"},
            {"id":13,"khmer":"ខា","rank":0,"status":"approved"},
            {"id":14,"khmer":"គា","rank":0,"status":"approved"},
            {"id":17,"khmer":"គាៗ","rank":0,"status":"approved"},
            {"id":18,"khmer":"កាខា","rank":0,"status":"approved"}],
        "mappings":[{"id":15,"romanized":"personal","khmer":"កា","rank":0,"status":"approved"},
            {"id":16,"romanized":"repeat","khmer":"កាៗ","rank":0,"status":"approved"}],"englishWords":[]}).to_string()).unwrap();
    let before = (base.to_binary(), extra.to_binary());
    let query = |input| base.suggest_with_extension(&extra, input, SuggestionFeatures::default(), &TypingContext::default()).unwrap();
    for input in ["k", "ka", "kak", "kaka"] {
        let result = query(input);
        assert!(result.result.candidates.iter().all(|s| !matches!(&s.candidate,
            SuggestionCandidate::Khmer(c) if c.khmer == "កា" || c.khmer == "កាៗ")), "{input}");
    }
    for (input, id) in [("saved",4), ("personal",15), ("repeat",16), ("kha",3), ("kea",14), ("kakha",18), ("កា",1)] {
        assert!(query(input).exact_candidates.iter().any(|c| c.mapping_id == Some(id)), "{input}");
    }
    assert!(query("pers").result.candidates.iter().any(|s| s.completion.as_deref() == Some("personal")));
    assert!(query("គ").result.candidates.iter().any(|s| matches!(&s.candidate,
        SuggestionCandidate::Khmer(c) if c.khmer == "គា")));
    assert_eq!((base.to_binary(), extra.to_binary()), before);
    // Constraints are query-local; reusing the same extension with another
    // base must not retain the first base's policy.
    assert!(!extra.suggest_with_context("ka", SuggestionFeatures::default(), &TypingContext::default()).unwrap().exact_candidates.is_empty());
    let enabled = Engine::from_json(&json!({"schemaVersion":1,"mappings":[]}).to_string()).unwrap();
    for input in ["kaka","keakea"] {
        assert!(!enabled.suggest_with_extension(&extra,input,SuggestionFeatures::default(),&TypingContext::default()).unwrap()
            .exact_candidates.iter().any(|candidate| candidate.khmer.ends_with('ៗ')));
    }

    let explicit = Engine::from_json(&json!({"schemaVersion":2,
        "words":[{"id":21,"khmer":"កា","rank":0,"status":"approved"},
            {"id":22,"khmer":"កាៗ","rank":0,"status":"approved"}],
        "mappings":[{"id":21,"romanized":"ka","khmer":"កា","rank":0,"status":"approved"},
            {"id":22,"romanized":"kaka","khmer":"កាៗ","rank":0,"status":"approved"}]}).to_string()).unwrap();
    for sound_map in [true,false] {
        for (input,id) in [("ka",21),("kaka",22)] {
            assert_eq!(base.suggest_with_extension(&explicit,input,SuggestionFeatures {sound_map,..Default::default()},
                &TypingContext::default()).unwrap().exact_candidates[0].mapping_id,Some(id));
        }
    }
    assert_eq!(base.suggest_with_extension(&explicit,"kak",SuggestionFeatures::default(),&TypingContext::default())
        .unwrap().result.candidates[0].completion.as_deref(),Some("kaka"));
}

#[test]
fn extension_policy_checks_the_child_and_fills_limits_after_filtering() {
    let child_off = Engine::from_json(&json!({"schemaVersion":2,"mappings":[],
        "words":[{"id":1,"khmer":"កា","rank":0,"status":"approved"},
            {"id":2,"khmer":"កាៗ","rank":0,"status":"approved"}],
        "soundMapDisabledWords":["កាៗ"]}).to_string()).unwrap();
    let extra = Engine::from_json(&json!({"schemaVersion":2,"mappings":[],
        "words":[{"id":11,"khmer":"កា","rank":0,"status":"approved"},
            {"id":12,"khmer":"កាៗ","rank":0,"status":"approved"}]}).to_string()).unwrap();
    assert_eq!(child_off.suggest_with_extension(&extra,"ka",SuggestionFeatures::default(),&TypingContext::default())
        .unwrap().exact_candidates[0].mapping_id,Some(1));
    assert!(child_off.suggest_with_extension(&extra,"kaka",SuggestionFeatures::default(),&TypingContext::default())
        .unwrap().result.candidates.is_empty());

    let words: Vec<_> = ["ា","ិ","ុ","េ"].into_iter().flat_map(|vowel|
        ["ក","ង","ច","ត","ន","ប","ម","យ","ល","ស"].into_iter().map(move |last|format!("ក{vowel}{last}")))
        .enumerate().map(|(i,text)|json!({"id":100+i,"khmer":text,"rank":0,"status":"approved"})).collect();
    let disabled = words[..20].to_vec();
    let enabled = words[20..].to_vec();
    let base = Engine::from_json(&json!({"schemaVersion":2,"mappings":[],"words":disabled,
        "soundMapDisabledWords":disabled.iter().map(|w|w["khmer"].clone()).collect::<Vec<_>>()} ).to_string()).unwrap();
    let extra = Engine::from_json(&json!({"schemaVersion":2,"mappings":[],"words":words}).to_string()).unwrap();
    let control = Engine::from_json(&json!({"schemaVersion":2,"mappings":[],"words":enabled}).to_string()).unwrap();
    let actual = base.suggest_with_extension(&extra,"k",SuggestionFeatures::default(),&TypingContext::default()).unwrap();
    let expected = control.suggest_with_context("k",SuggestionFeatures::default(),&TypingContext::default()).unwrap();
    assert_eq!(expected.result.candidates.len(),20);
    assert_eq!(actual,expected);
}

#[test]
fn personal_prefixes_cannot_hide_completed_base_compositions() {
    let base = Engine::from_json(&json!({"schemaVersion":1,"mappings":[
        {"id":1,"romanized":"one","khmer":"ក","rank":0,"status":"approved"},
        {"id":2,"romanized":"two","khmer":"ខ","rank":0,"status":"approved"}
    ]}).to_string()).unwrap();
    let features = SuggestionFeatures { sound_map: false, ..Default::default() };
    let endings = ["ក","ខ","គ","ឃ","ង","ច","ឆ","ជ","ឈ","ញ","ដ","ឋ","ឌ","ឍ","ណ","ត","ថ","ទ","ធ","ន"];
    for count in [0, 1, 20] {
        for exact in [false, true] {
            let mut mappings: Vec<_> = endings[..count].iter().enumerate().map(|(i, last)| json!({
                "id":100+i,"romanized":format!("one two {}",char::from(b'a'+i as u8)),
                "khmer":format!("គ{last}"),"rank":0,"status":"approved"
            })).collect();
            if exact { mappings.push(json!({"id":200,"romanized":"one two","khmer":"ឃគ","rank":0,"status":"approved"})); }
            let extra = Engine::from_json(&json!({"schemaVersion":1,"mappings":mappings}).to_string()).unwrap();
            let before = (base.to_binary(), extra.to_binary());
            let result = base.suggest_with_extension(&extra,"one two",features,&TypingContext::default()).unwrap();
            assert_eq!(result.exact_candidates.len(),usize::from(exact));
            if exact { assert_eq!(result.exact_candidates[0].mapping_id,Some(200)); }
            let composed = &result.result.candidates[usize::from(exact)];
            assert!(composed.completion.is_none());
            match &composed.candidate {
                SuggestionCandidate::Khmer(candidate) => {
                    assert_eq!(candidate.khmer,"កខ");
                    assert_eq!(candidate.mapping_id,None);
                    assert_eq!(candidate.mapping_ids,vec![1,2]);
                },
                _ => panic!("Expected complete Khmer composition"),
            }
            assert_eq!(result.result.candidates.len(),(count+1+usize::from(exact)).min(20));
            assert_eq!((base.to_binary(), extra.to_binary()),before);
        }
    }
}

#[test]
fn completed_personal_compositions_precede_base_prefixes_and_keep_base_duplicate_identity() {
    let extra = Engine::from_json(&json!({"schemaVersion":1,"mappings":[
        {"id":101,"romanized":"one","khmer":"ក","rank":0,"status":"approved"},
        {"id":102,"romanized":"two","khmer":"ខ","rank":0,"status":"approved"}
    ]}).to_string()).unwrap();
    let prefix = json!({"id":3,"romanized":"one two more","khmer":"គ","rank":0,"status":"approved"});
    let features = SuggestionFeatures { sound_map: false, ..Default::default() };
    for base_components in [false,true] {
        let mut mappings = vec![prefix.clone()];
        if base_components {
            mappings.extend([
                json!({"id":1,"romanized":"one","khmer":"ក","rank":0,"status":"approved"}),
                json!({"id":2,"romanized":"two","khmer":"ខ","rank":0,"status":"approved"}),
            ]);
        }
        let base = Engine::from_json(&json!({"schemaVersion":1,"mappings":mappings}).to_string()).unwrap();
        let result = base.suggest_with_extension(&extra,"one two",features,&TypingContext::default()).unwrap();
        assert!(result.exact_candidates.is_empty());
        assert_eq!(result.result.candidates.len(),2);
        assert!(result.result.candidates[0].completion.is_none());
        assert_eq!(result.result.candidates[1].completion.as_deref(),Some("one two more"));
        match &result.result.candidates[0].candidate {
            SuggestionCandidate::Khmer(candidate) => assert_eq!(candidate.mapping_ids,
                if base_components { vec![1,2] } else { vec![101,102] }),
            _ => panic!("Expected complete Khmer composition"),
        }
    }
}
