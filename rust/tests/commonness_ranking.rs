use khmer_engine::{Engine, SuggestionCandidate, SuggestionFeatures, TypingContext, TypingMode};
use serde_json::{json, Value};

fn metadata(language: &str, levels: &[(&str, Option<u8>)]) -> Value {
    json!({"format":"kmeow-word-metadata-v2", "language":language, "snapshotId":"a".repeat(64),
        "words":levels.iter().enumerate().map(|(id, (word, level))| json!({
            "id":id.to_string(), "word":word, "context":null, "commonnessLevel":level,
            "partsOfSpeech":[], "meanings":[], "translations":[], "sources":[]
        })).collect::<Vec<_>>()})
}
fn load(engine: &mut Engine, khmer: &[(&str, Option<u8>)], english: &[(&str, Option<u8>)]) {
    engine
        .load_word_metadata(
            &metadata("khmer", khmer).to_string(),
            &metadata("english", english).to_string(),
        )
        .unwrap();
}
fn outputs(engine: &Engine, input: &str, features: SuggestionFeatures) -> Vec<String> {
    engine
        .suggest_with_features(input, features)
        .unwrap()
        .candidates
        .iter()
        .map(|hit| match &hit.candidate {
            SuggestionCandidate::Khmer(c) => c.khmer.clone(),
            SuggestionCandidate::English(c) => c.text.clone(),
        })
        .collect()
}
fn output(hit: &khmer_engine::Suggestion) -> &str {
    match &hit.candidate {
        SuggestionCandidate::Khmer(c) => &c.khmer,
        SuggestionCandidate::English(c) => &c.text,
    }
}

#[test]
fn merged_saved_and_generated_prefixes_use_rank_and_key_ties_before_the_limit() {
    let mut mappings = Vec::new();
    let mut words = vec![json!({"id":1,"khmer":"កាល","rank":0,"status":"approved"})];
    let mut levels = vec![json!({"word":"កាល","level":3})];
    for i in 0..25 {
        let word = format!("ខារ{}", char::from_u32(0x1780 + i).unwrap());
        mappings.push(json!({"id":i+2,"romanized":format!("kz{}",char::from(b'a'+i as u8)),"khmer":word,"rank":500,"status":"approved"}));
        words.push(json!({"id":i+2,"khmer":word,"rank":500,"status":"approved"}));
        levels.push(json!({"word":word,"level":3}));
    }
    let mut engine = Engine::from_json(&json!({"schemaVersion":2,"mappings":mappings,"words":words}).to_string()).unwrap();
    engine.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","khmer":levels,"english":[]}).to_string()).unwrap();
    let hits = outputs(&engine, "k", SuggestionFeatures::default());
    assert_eq!(hits.len(), 20);
    assert_eq!(hits[0], "កាល");

    // A tied saved/generated pair must compare its completion key and ID too.
    for (key, expected) in [("kaaa", vec!["ខារ", "កាល"]), ("kal", vec!["កាល", "ខារ"])] {
        let mut engine = Engine::from_json(&json!({"schemaVersion":2,
            "mappings":[{"id":2,"romanized":key,"khmer":"ខារ","rank":0,"status":"approved"}],
            "words":[{"id":1,"khmer":"កាល","rank":0,"status":"approved"},{"id":2,"khmer":"ខារ","rank":0,"status":"approved"}]
        }).to_string()).unwrap();
        engine.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","khmer":[{"word":"កាល","level":3},{"word":"ខារ","level":3}],"english":[]}).to_string()).unwrap();
        assert_eq!(outputs(&engine, "k", SuggestionFeatures::default()), expected);
    }
}

#[test]
fn commonness_precedes_saved_exact_preference_without_overriding_committed_context() {
    for saved_key in ["kal", "ka l"] {
        let dataset = json!({"schemaVersion":2,
            "mappings":[
                {"id":2,"romanized":saved_key,"khmer":"ខារ","rank":500,"status":"approved"},
                {"id":3,"romanized":saved_key,"khmer":"គារ","rank":700,"status":"approved"}
            ],
            "words":[
                {"id":1,"khmer":"កាល","rank":0,"status":"approved"},
                {"id":2,"khmer":"ខារ","rank":500,"status":"approved"},
                {"id":3,"khmer":"គារ","rank":700,"status":"approved"},
                {"id":4,"khmer":"សុខ","rank":0,"status":"approved"},
                {"id":5,"khmer":"សុខកាល","rank":0,"status":"approved"}
            ],
            "structures":[{"wordId":5,"componentIds":[4,1]}]
        });
        let source = Engine::from_json(&dataset.to_string()).unwrap();
        for mut engine in [Engine::from_json(&dataset.to_string()).unwrap(), Engine::from_binary(source.to_binary()).unwrap()] {
            engine.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1",
                "khmer":[{"word":"កាល","level":1},{"word":"ខារ","level":5},{"word":"គារ","level":4}],
                "english":[]}).to_string()).unwrap();
            assert_eq!(outputs(&engine, "kal", SuggestionFeatures::default()), ["កាល", "គារ", "ខារ"]);
            for (previous, expected) in [(vec![], vec!["កាល", "គារ", "ខារ"]),
                (vec!["ស".into()], vec!["កាល", "គារ", "ខារ"]),
                (vec!["សុខ".into()], vec!["កាល", "គារ", "ខារ"])] {
                let result = engine.suggest_with_context("kal", SuggestionFeatures::default(), &TypingContext { previous_words: previous, ..Default::default() }).unwrap();
                assert_eq!(result.exact_candidates.iter().map(|c| c.khmer.as_str()).collect::<Vec<_>>(), expected);
                assert_eq!(result.result.candidates.iter().map(output).collect::<Vec<_>>(), expected);
            }
            // Prefixes continue to share commonness ranking across both sources.
            assert_eq!(outputs(&engine, "k", SuggestionFeatures::default())[0], "កាល");
            // Saved spellings still break equal-commonness ties; no identities,
            // stored ranks or compact data are changed by this display order.
            engine.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1",
                "khmer":[{"word":"កាល","level":1},{"word":"ខារ","level":1},{"word":"គារ","level":1}],
                "english":[]}).to_string()).unwrap();
            assert_eq!(outputs(&engine, "kal", SuggestionFeatures::default()), ["ខារ", "គារ", "កាល"]);
            assert_eq!(engine.to_binary(), source.to_binary());
        }
    }
}

#[test]
fn portable_ranking_is_atomic_preserves_compact_bytes_and_leaves_lexical_metadata_intact() {
    let mut engine = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],"englishWords":[
        {"id":1,"text":"hello","rank":0,"status":"approved"},
        {"id":2,"text":"help","rank":0,"status":"approved"}
    ]}).to_string()).unwrap();
    load(&mut engine, &[], &[("hello", Some(5)), ("help", Some(5))]);
    let bytes = engine.to_binary();
    let features = SuggestionFeatures { mode: TypingMode::English, ..Default::default() };
    let ranking = json!({"format":"kmeow-word-ranking-v1","khmer":[],"english":[
        {"word":"Hello","level":1},{"word":"hello","level":5},{"word":"help","level":2}
    ]});
    engine.load_word_ranking_json(&ranking.to_string()).unwrap();
    assert_eq!(outputs(&engine, "hel", features), vec!["hello", "help"]);
    assert_eq!(engine.to_binary(), bytes);
    let metadata: Value = serde_json::from_str(&engine.lookup_word_metadata_json("english", "hello").unwrap()).unwrap();
    assert_eq!(metadata[0]["commonnessLevel"], 5);
    for invalid in [
        json!({"format":"wrong","khmer":[],"english":[]}),
        json!({"format":"kmeow-word-ranking-v1","khmer":[],"english":[{"word":"help","level":0}]}),
        json!({"format":"kmeow-word-ranking-v1","khmer":[],"english":[{"word":"help","level":6}]}),
        json!({"format":"kmeow-word-ranking-v1","khmer":[],"english":[{"word":"help","level":1},{"word":"help","level":2}]}),
        json!({"format":"kmeow-word-ranking-v1","khmer":[],"english":[],"unexpected":true}),
        json!({"format":"kmeow-word-ranking-v1","khmer":[],"english":[{"word":"bad\nword","level":1}]}),
    ] {
        assert!(engine.load_word_ranking_json(&invalid.to_string()).is_err());
        assert_eq!(outputs(&engine, "hel", features), vec!["hello", "help"]);
    }
    engine.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","khmer":[],"english":[]}).to_string()).unwrap();
    assert_eq!(outputs(&engine, "hel", features), vec!["help", "hello"]);
}

#[test]
fn commonness_orders_exact_matches_without_rewriting_ranks_or_lexical_identities() {
    let words = ["ក", "ខ", "គ", "ឃ", "ង", "ច"];
    let mappings: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, word)| {
            json!({
                "id":i+1, "romanized":"qa", "khmer":word, "rank":i, "status":"approved"
            })
        })
        .collect();
    let mut engine =
        Engine::from_json(&json!({"schemaVersion":1,"mappings":mappings}).to_string()).unwrap();
    let original = engine.to_binary();
    load(
        &mut engine,
        &[
            ("ក", None),
            ("ខ", Some(5)),
            ("គ", Some(4)),
            ("ឃ", Some(3)),
            ("ង", Some(2)),
            ("ច", Some(1)),
        ],
        &[],
    );
    let features = SuggestionFeatures {
        sound_map: false,
        ..Default::default()
    };
    assert_eq!(
        outputs(&engine, "qa", features),
        ["ច", "ង", "ឃ", "គ", "ខ", "ក"]
    );
    let result = engine
        .suggest_with_context("qa", features, &TypingContext::default())
        .unwrap();
    assert_eq!(
        result
            .exact_candidates
            .iter()
            .map(|c| c.rank)
            .collect::<Vec<_>>(),
        [5, 4, 3, 2, 1, 0]
    );
    assert_eq!(
        result
            .result
            .candidates
            .iter()
            .map(|c| c.candidate.mapping_id())
            .collect::<Vec<_>>(),
        [Some(6), Some(5), Some(4), Some(3), Some(2), Some(1)]
    );
    assert_eq!(
        engine.convert("qa").unwrap().candidates,
        result.exact_candidates
    );
    assert_eq!(engine.to_binary(), original);

    // A replacement updates ranking atomically; a bad pair retains the old one.
    load(&mut engine, &[("ក", Some(1)), ("ច", Some(5))], &[]);
    assert_eq!(outputs(&engine, "qa", features)[0], "ក");
    let mut bad = metadata("english", &[]);
    bad["snapshotId"] = json!("b".repeat(64));
    assert!(engine
        .load_word_metadata(
            &metadata("khmer", &[("ច", Some(1))]).to_string(),
            &bad.to_string()
        )
        .is_err());
    assert_eq!(outputs(&engine, "qa", features)[0], "ក");

    // Different senses share an output; use its most common reviewed sense.
    let mut senses = metadata("khmer", &[("ក", Some(5)), ("ច", Some(3))]);
    let mut common = senses["words"][0].clone();
    common["id"] = json!("other");
    common["context"] = json!("Other meaning");
    common["commonnessLevel"] = json!(1);
    senses["words"].as_array_mut().unwrap().push(common);
    engine
        .load_word_metadata(&senses.to_string(), &metadata("english", &[]).to_string())
        .unwrap();
    assert_eq!(outputs(&engine, "qa", features)[0], "ក");
}

#[test]
fn common_words_beyond_the_twenty_slot_limit_survive_saved_khmer_and_english_prefixes() {
    let words: Vec<_> = (0..33)
        .map(|i| format!("ក{}", char::from_u32(0x1780 + i).unwrap()))
        .collect();
    let mappings: Vec<_> = words
        .iter()
        .enumerate()
        .map(|(i, word)| {
            json!({
                "id":i+1, "romanized":format!("qa{}",char::from(b'a'+(i/26) as u8)),
                "khmer":word, "rank":i, "status":"approved"
            })
        })
        .collect();
    let mut engine = Engine::from_json(&json!({"schemaVersion":3,"mappings":mappings,
        "words":words.iter().enumerate().map(|(i,word)| json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>(),
        "englishWords":[{"id":100,"text":"tea","rank":0,"status":"approved"},
            {"id":101,"text":"Teacher","rank":999,"status":"approved"},
            {"id":102,"text":"team","rank":0,"status":"approved"}]
    }).to_string()).unwrap();
    load(
        &mut engine,
        &[
            (words[32].as_str(), Some(1)),
            (words[31].as_str(), Some(2)),
            (words[30].as_str(), Some(5)),
        ],
        &[("tea", Some(5)), ("teacher", Some(1)), ("team", Some(3))],
    );
    for input in ["q", "ក"] {
        let hits = outputs(
            &engine,
            input,
            SuggestionFeatures {
                sound_map: false,
                ..Default::default()
            },
        );
        assert_eq!(hits.len(), 20);
        assert_eq!(
            &hits[..3],
            &words[30..].iter().rev().cloned().collect::<Vec<_>>()
        );
        assert_eq!(
            hits.iter().collect::<std::collections::BTreeSet<_>>().len(),
            20
        );
    }
    let english = SuggestionFeatures {
        mode: TypingMode::English,
        ..Default::default()
    };
    assert_eq!(outputs(&engine, "te", english), ["Teacher", "team", "tea"]);
    assert_eq!(outputs(&engine, "tea", english), ["tea", "Teacher", "team"]);
    let original = engine.to_binary();
    let mut legacy = metadata(
        "khmer",
        &[(words[0].as_str(), Some(1)), (words[1].as_str(), Some(3))],
    );
    legacy["format"] = json!("kmeow-word-metadata-v1");
    let mut empty = metadata("english", &[]);
    empty["format"] = json!("kmeow-word-metadata-v1");
    engine
        .load_word_metadata(&legacy.to_string(), &empty.to_string())
        .unwrap();
    let entry: Value = serde_json::from_str(
        &engine
            .lookup_word_metadata_json("khmer", &words[0])
            .unwrap(),
    )
    .unwrap();
    assert_eq!(entry[0]["commonnessLevel"], 3);
    assert_eq!(
        outputs(
            &engine,
            "q",
            SuggestionFeatures {
                sound_map: false,
                ..Default::default()
            }
        )[0],
        words[0]
    );
    assert_eq!(engine.to_binary(), original);
}

#[test]
fn uncommon_and_academic_words_stay_visible_but_never_become_automatic() {
    let mut engine = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],"englishWords":[
        {"id":1,"text":"ant","rank":0,"status":"approved"},
        {"id":2,"text":"anchor","rank":0,"status":"approved"},
        {"id":3,"text":"angle","rank":0,"status":"approved"},
        {"id":4,"text":"ancient","rank":0,"status":"approved"},
        {"id":5,"text":"anthology","rank":0,"status":"approved"}
    ]}).to_string()).unwrap();
    load(&mut engine, &[], &[("ant",Some(1)),("anchor",Some(2)),("angle",Some(3)),
        ("ancient",Some(4)),("anthology",Some(5))]);
    let before = engine.to_binary();
    let features = SuggestionFeatures { mode: TypingMode::English, ..Default::default() };
    let result = engine.suggest_with_features("an", features).unwrap();
    assert_eq!(result.candidates.len(), 5);
    for hit in &result.candidates {
        assert_eq!(hit.tap_only, ["ancient", "anthology"].contains(&output(hit)));
    }
    for input in ["ancient", "anth", "anthology"] {
        let result = engine.suggest_with_ranking(input, features, &TypingContext::default(), &Default::default()).unwrap();
        assert!(!result.baseline.result.candidates.is_empty());
        assert!(result.baseline.result.candidates[0].tap_only);
        assert!(result.ranking.baseline_automatic_candidate.is_none());
        assert!(!result.ranking.groups[0].reorderable);
    }
    let common = engine.suggest_with_ranking("an", features, &TypingContext::default(), &Default::default()).unwrap();
    assert_eq!(output(common.ranking.baseline_automatic_candidate.as_ref().unwrap()), "ant");
    let json: Value = serde_json::from_str(&engine.suggest_with_features_json("anth", features).unwrap()).unwrap();
    assert_eq!(json["candidates"][0]["tapOnly"], true);
    assert_eq!(engine.to_binary(), before, "The policy never rewrites dictionary bytes");
}

#[test]
fn rare_saved_and_generated_exact_khmer_readings_require_explicit_selection() {
    for saved in [false, true] {
        let mappings = if saved { json!([{"id":9,"romanized":"kal","khmer":"កាល","rank":0,"status":"approved"}]) }
            else { json!([]) };
        let mut engine = Engine::from_json(&json!({"schemaVersion":2,"mappings":mappings,
            "words":[{"id":1,"khmer":"កាល","rank":0,"status":"approved"}]
        }).to_string()).unwrap();
        for level in [3, 4, 5] {
            load(&mut engine, &[("កាល",Some(level))], &[]);
            let result = engine.suggest_with_ranking("kal", SuggestionFeatures::default(), &TypingContext::default(), &Default::default()).unwrap();
            assert_eq!(result.baseline.exact_candidates[0].khmer, "កាល");
            assert_eq!(output(&result.baseline.result.candidates[0]), "កាល");
            assert_eq!(result.baseline.result.candidates[0].tap_only, level > 3);
            assert_eq!(result.ranking.baseline_automatic_candidate.is_some(), level <= 3);
        }
    }
}

#[test]
fn personal_duplicates_cannot_weaken_or_strengthen_bundled_commonness() {
    fn english(words: &[&str]) -> Engine {
        Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],
            "englishWords":words.iter().enumerate().map(|(i,text)| json!({"id":i+1,"text":text,"rank":0,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap()
    }
    let mut base = english(&["abacus", "about"]);
    let mut personal = english(&["abacus", "about", "abundant", "able"]);
    load(&mut base, &[], &[("abacus",Some(5)),("about",Some(3))]);
    load(&mut personal, &[], &[("abacus",Some(1)),("about",Some(5)),("abundant",Some(4)),("able",Some(2))]);
    let result = base.suggest_with_extension(&personal,"ab", SuggestionFeatures { mode: TypingMode::English, ..Default::default() }, &TypingContext::default()).unwrap();
    for (word, expected) in [("abacus",true),("about",false),("abundant",true),("able",false)] {
        assert_eq!(result.result.candidates.iter().find(|hit| output(hit)==word).unwrap().tap_only, expected, "{word}");
    }
}
