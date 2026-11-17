use khmer_engine::{Engine, SuggestionFeatures, TypingContext, TypingMode};
use serde_json::json;

fn context(words: &[&str]) -> TypingContext {
    TypingContext { previous_words: words.iter().map(|word| (*word).into()).collect(), ..Default::default() }
}
fn output(hit: &khmer_engine::Suggestion) -> &str {
    match &hit.candidate { khmer_engine::SuggestionCandidate::Khmer(c) => &c.khmer, _ => panic!() }
}
fn dataset() -> serde_json::Value {
    let pairs = [("sabay", "សប្បាយ"), ("sok", "សុខ"), ("sok sabay", "សុខសប្បាយ"),
        ("bat", "បាទ"), ("bat", "បត្រ"), ("sanha", "សញ្ញា"), ("sanha bat", "សញ្ញាបត្រ")];
    json!({"schemaVersion":2,
        "mappings":pairs.iter().enumerate().map(|(i,(key,word))| json!({"id":i+1,"romanized":key,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>(),
        "words":pairs.iter().enumerate().map(|(i,(_,word))| json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>(),
        "structures":[{"wordId":3,"componentIds":[2,1]},{"wordId":7,"componentIds":[6,5]}]})
}

#[test]
fn one_edit_recovery_is_manual_preserves_identity_and_does_not_change_conversion() {
    let source = Engine::from_json(&dataset().to_string()).unwrap();
    let bytes = source.to_binary();
    for engine in [&source, &Engine::from_binary(bytes.clone()).unwrap()] {
        for input in ["saby", "sabaay", "sabby", "sabya", " SABBY"] {
            let result = engine.suggest_with_context(input, SuggestionFeatures { sound_map:false, ..Default::default() }, &context(&[])).unwrap();
            let hit = result.result.candidates.iter().find(|hit| output(hit) == "សប្បាយ").unwrap();
            assert_eq!(hit.correction.as_deref(), Some("sabay"), "{input}");
            assert_eq!(hit.candidate.mapping_id(), Some(1));
            assert_eq!(hit.completion.as_deref(), Some("sabay"));
            assert!(result.exact_candidates.is_empty());
            assert!(engine.convert(input).unwrap().candidates.is_empty());
        }
        for input in ["sab", "sabxx", "sabby ", "sab by", "", &"x".repeat(100)] {
            assert!(engine.suggest(input).unwrap().candidates.iter().all(|hit| hit.correction.is_none()), "{input}");
        }
        let exact = engine.suggest_with_context("sabay", SuggestionFeatures::default(), &context(&[])).unwrap();
        assert!(exact.result.candidates.iter().filter(|hit| output(hit) == "សប្បាយ").all(|hit| hit.correction.is_none()));
        assert!(exact.exact_candidates.iter().any(|hit| hit.khmer == "សប្បាយ"));
        assert_eq!(engine.to_binary(), bytes);
    }
}

#[test]
fn recovery_bucket_follows_super_and_very_common_and_precedes_all_lower_tiers() {
    let entries = [("testa","ក",1), ("testb","ខ",2), ("testc","គ",3), ("testd","ឃ",4), ("teste","ង",5), ("tent","ច",5)];
    let mut engine = Engine::from_json(&json!({"schemaVersion":2,
        "mappings":entries.iter().enumerate().map(|(i,(key,word,_))| json!({"id":i+1,"romanized":key,"khmer":word,"rank":0,"status":"approved"})).collect::<Vec<_>>(),
        "words":entries.iter().enumerate().map(|(i,(_,word,_))| json!({"id":i+1,"khmer":word,"rank":0,"status":"approved"})).collect::<Vec<_>>()
    }).to_string()).unwrap();
    engine.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","english":[],
        "khmer":entries.iter().map(|(_,word,level)|json!({"word":word,"level":level})).collect::<Vec<_>>()}).to_string()).unwrap();
    for previous in [context(&[]), context(&["ក"])] {
        let result = engine.suggest_with_context("test", SuggestionFeatures { sound_map:false, ..Default::default() }, &previous).unwrap();
        assert_eq!(result.result.candidates.iter().map(output).collect::<Vec<_>>(), ["ក","ខ","ច","គ","ឃ","ង"]);
        assert_eq!(result.result.candidates[2].correction.as_deref(), Some("tent"));
        assert!(result.exact_candidates.is_empty());
    }
}

#[test]
fn generated_recovery_respects_sound_map_and_exact_word_disabled_policies() {
    let data = json!({"schemaVersion":2,"mappings":[],"words":[{"id":1,"khmer":"ស្ពាន","rank":0,"status":"approved"}]});
    let engine = Engine::from_json(&data.to_string()).unwrap();
    let hit = engine.suggest_with_context("speab", SuggestionFeatures::default(), &context(&[])).unwrap();
    assert!(hit.result.candidates.iter().any(|c| output(c)=="ស្ពាន" && c.correction.as_deref()==Some("spean")));
    assert!(hit.exact_candidates.is_empty());
    assert!(engine.suggest_with_features("speab", SuggestionFeatures { sound_map:false, ..Default::default() }).unwrap().candidates.is_empty());
    let mut disabled = data;
    disabled["soundMapDisabledWords"] = json!(["ស្ពាន"]);
    let engine = Engine::from_json(&disabled.to_string()).unwrap();
    assert!(engine.suggest("speab").unwrap().candidates.is_empty());
}

#[test]
fn predictions_use_reviewed_complete_words_and_reset_at_unknown_context() {
    let source = Engine::from_json(&dataset().to_string()).unwrap();
    for engine in [&source, &Engine::from_binary(source.to_binary()).unwrap()] {
        for (previous, expected) in [(vec!["សុខ"], vec!["សប្បាយ"]), (vec!["សញ្ញា"], vec!["បត្រ"]),
            (vec!["សញ្ញា","មិនមាន"], vec![]), (vec!["សញ្ញា","សុខ"], vec!["សប្បាយ"]), (vec![], vec![])] {
            let result = engine.suggest_with_context("", SuggestionFeatures::default(), &context(&previous)).unwrap();
            assert_eq!(result.result.candidates.iter().map(output).collect::<Vec<_>>(), expected);
            assert!(result.exact_candidates.is_empty());
        }
        for features in [SuggestionFeatures { khmer_completion:false, ..Default::default() }, SuggestionFeatures { mode:TypingMode::English, ..Default::default() }] {
            assert!(engine.suggest_with_context("",features,&context(&["សុខ"])).unwrap().result.candidates.is_empty());
        }
        assert_eq!(engine.suggest_with_context("bat", SuggestionFeatures::default(), &context(&["សញ្ញា"])).unwrap().exact_candidates[0].khmer,"បត្រ");
        assert!(engine.suggest("").unwrap().candidates.is_empty());
    }
}

#[test]
fn a_complete_personal_input_suppresses_base_recovery() {
    let base = Engine::from_json(&dataset().to_string()).unwrap();
    let personal = Engine::from_json(&json!({"schemaVersion":2,
        "mappings":[{"id":91,"romanized":"sabby","khmer":"សុខ","rank":0,"status":"approved"}],
        "words":[{"id":91,"khmer":"សុខ","rank":0,"status":"approved"}]
    }).to_string()).unwrap();
    let result = base.suggest_with_extension(&personal, "sabby",
        SuggestionFeatures { sound_map:false, ..Default::default() }, &context(&[])).unwrap();
    assert_eq!(result.exact_candidates[0].khmer, "សុខ");
    assert!(result.result.candidates.iter().all(|hit| hit.correction.is_none()));
}

#[test]
fn merged_recovery_tiers_precede_the_cap_and_preserve_personal_commonness() {
    let mut base = Engine::from_json(&json!({"schemaVersion":2,
        "mappings":[
            {"id":1,"romanized":"testy","khmer":"ខ","rank":0,"status":"approved"},
            {"id":2,"romanized":"testz","khmer":"គ","rank":0,"status":"approved"},
            {"id":3,"romanized":"tent","khmer":"ច","rank":0,"status":"approved"}],
        "words":[
            {"id":1,"khmer":"ខ","rank":0,"status":"approved"},
            {"id":2,"khmer":"គ","rank":0,"status":"approved"},
            {"id":3,"khmer":"ច","rank":0,"status":"approved"}]
    }).to_string()).unwrap();
    base.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","english":[],
        "khmer":[{"word":"ខ","level":2},{"word":"គ","level":1},{"word":"ច","level":5}]
    }).to_string()).unwrap();
    let words: Vec<_> = "កខគឃងចឆជឈញដឋឌឍណតថទធន".chars().map(|letter| format!("ក{letter}")).collect();
    let mut personal = Engine::from_json(&json!({"schemaVersion":2,
        "mappings":words.iter().enumerate().map(|(i,word)| json!({"id":i+100,
            "romanized":format!("test{}",char::from(b'a'+i as u8)),"khmer":word,"rank":0,"status":"approved"})).collect::<Vec<_>>(),
        "words":words.iter().enumerate().map(|(i,word)| json!({"id":i+100,"khmer":word,"rank":0,"status":"approved"})).collect::<Vec<_>>()
    }).to_string()).unwrap();
    let features = SuggestionFeatures { sound_map:false, ..Default::default() };
    let query = base.suggest_with_extension(&personal, "test", features, &context(&[])).unwrap();
    assert_eq!(query.result.candidates.len(),20);
    assert_eq!(query.result.candidates.iter().take(3).map(output).collect::<Vec<_>>(), ["គ","ខ","ច"]);
    assert_eq!(query.result.candidates[2].correction.as_deref(),Some("tent"));
    personal.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","english":[],
        "khmer":words.iter().enumerate().map(|(i,word)|json!({"word":word,"level":if i<2 {i+1} else {3}})).collect::<Vec<_>>()
    }).to_string()).unwrap();
    let before = (base.to_binary(),personal.to_binary());
    let query = base.suggest_with_extension(&personal, "test", features, &context(&[])).unwrap();
    assert_eq!(query.result.candidates.len(),20);
    assert_eq!(query.result.candidates.iter().take(5).map(output).collect::<Vec<_>>(), ["កក","កខ","គ","ខ","ច"]);
    assert_eq!(query.result.candidates[4].correction.as_deref(),Some("tent"));
    assert!(query.exact_candidates.is_empty());
    assert_eq!((base.to_binary(),personal.to_binary()),before);
}

fn merge_fixture(pairs: &[(String, String)], first_id: u64) -> Engine {
    Engine::from_json(&json!({"schemaVersion":2,
        "mappings":pairs.iter().enumerate().map(|(i,(key,word))|json!({
            "id":first_id+i as u64,"romanized":key,"khmer":word,"rank":0,"status":"approved"})).collect::<Vec<_>>(),
        "words":pairs.iter().enumerate().map(|(i,(_,word))|json!({
            "id":first_id+i as u64,"khmer":word,"rank":0,"status":"approved"})).collect::<Vec<_>>()
    }).to_string()).unwrap()
}

#[test]
fn merged_ordinary_completions_win_duplicate_recoveries_in_either_dictionary() {
    let features = SuggestionFeatures { sound_map:false, ..Default::default() };
    for (base_key, personal_key, ordinary_id) in [("testlong","tent",1), ("tent","testlong",101)] {
        let base = merge_fixture(&[(base_key.into(),"ក".into()), ("tost".into(),"ខ".into())],1);
        let personal = merge_fixture(&[(personal_key.into(),"ក".into())],101);
        let before = (base.to_binary(),personal.to_binary());
        let restored = (Engine::from_binary(before.0.clone()).unwrap(),Engine::from_binary(before.1.clone()).unwrap());
        for (base,personal) in [(&base,&personal),(&restored.0,&restored.1)] {
            let result = base.suggest_with_extension(&personal,"test",features,&context(&[])).unwrap();
            let ordinary = result.result.candidates.iter().find(|hit|output(hit)=="ក").unwrap();
            assert_eq!(ordinary.completion.as_deref(),Some("testlong"));
            assert!(ordinary.correction.is_none(),"{base_key} / {personal_key}");
            assert_eq!(ordinary.candidate.mapping_id(),Some(ordinary_id));
            assert_eq!(result.result.candidates.iter().filter(|hit|output(hit)=="ក").count(),1);
            assert!(result.result.candidates.iter().any(|hit|output(hit)=="ខ" && hit.correction.is_some()));
            assert!(result.exact_candidates.is_empty());
            assert!(base.convert("test").unwrap().candidates.is_empty());
            assert_eq!((base.to_binary(),personal.to_binary()),before);
        }
    }
}

#[test]
fn merged_recoveries_share_one_limit_and_leave_room_for_ordinary_completions() {
    let letters: Vec<_> = "កខគឃងចឆជឈញដឋឌឍណតថទធន".chars().collect();
    let base_pairs: Vec<_> = "abcdfghi".chars().enumerate()
        .map(|(i,letter)|(format!("t{letter}st"),format!("ក{}",letters[i])))
        .chain([("testy".into(),"គ".into()),("testz".into(),"ឃ".into())]).collect();
    let personal_pairs: Vec<_> = "jklmnopq".chars().enumerate()
        .map(|(i,letter)|(format!("t{letter}st"),format!("ក{}",letters[i+8])))
        .chain(letters.iter().enumerate().map(|(i,letter)|
            (format!("test{}",char::from(b'a'+i as u8)),format!("ខ{letter}")))).collect();
    let mut base = merge_fixture(&base_pairs,1);
    let personal = merge_fixture(&personal_pairs,101);
    base.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","english":[],
        "khmer":[{"word":"គ","level":1},{"word":"ឃ","level":2}]}).to_string()).unwrap();
    let features = SuggestionFeatures { sound_map:false, ..Default::default() };
    for engine in [&base,&personal] {
        assert_eq!(engine.suggest_with_context("test",features,&context(&[])).unwrap()
            .result.candidates.iter().filter(|hit|hit.correction.is_some()).count(),8);
    }
    let before = (base.to_binary(),personal.to_binary());
    let result = base.suggest_with_extension(&personal,"test",features,&context(&[])).unwrap();
    assert_eq!(result.result.candidates.len(),20);
    assert_eq!(result.result.candidates.iter().take(2).map(output).collect::<Vec<_>>(),["គ","ឃ"]);
    assert!(result.result.candidates[2..10].iter().all(|hit|hit.correction.is_some()));
    assert!(result.result.candidates[10..].iter().all(|hit|hit.correction.is_none()));
    assert_eq!(result.result.candidates.iter().filter(|hit|hit.correction.is_some()).count(),8);
    assert!(result.exact_candidates.is_empty());
    assert_eq!((base.to_binary(),personal.to_binary()),before);
}
