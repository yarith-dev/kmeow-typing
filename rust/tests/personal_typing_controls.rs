use khmer_engine::{Engine, SuggestionCandidate, SuggestionFeatures, TypingContext, TypingMode};
use serde_json::json;

fn empty() -> Engine {
    Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],"englishWords":[]}).to_string()).unwrap()
}
fn outputs(base: &Engine, extra: &Engine, input: &str, mode: TypingMode) -> Vec<String> {
    base.suggest_with_extension(extra, input, SuggestionFeatures { mode, ..Default::default() }, &TypingContext::default())
        .unwrap().result.candidates.iter().map(|hit| match &hit.candidate {
            khmer_engine::SuggestionCandidate::Khmer(word) => word.khmer.clone(),
            khmer_engine::SuggestionCandidate::English(word) => word.text.clone(),
        }).collect()
}

#[test]
fn local_sound_policy_preserves_aliases_completion_bytes_and_scope() {
    let base = Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":[{"id":1,"khmer":"កា","rank":0,"status":"approved"},
            {"id":2,"khmer":"កាៗ","rank":0,"status":"approved"},
            {"id":3,"khmer":"កាខា","rank":0,"status":"approved"}],
        "mappings":[{"id":1,"romanized":"saved alias","khmer":"កា","rank":0,"status":"approved"}]}).to_string()).unwrap();
    let before = base.to_binary();
    let policy = json!([{"language":"khmer","word":"កា","soundMapEnabled":false}]).to_string();
    let local = base.with_typing_overrides_json(&policy).unwrap();
    let extra = Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],"mappings":[],
        "words":[{"id":11,"khmer":"កា","rank":0,"status":"approved"},
            {"id":12,"khmer":"កាៗ","rank":0,"status":"approved"}]}).to_string()).unwrap()
        .with_typing_overrides_json(&policy).unwrap();
    for input in ["k", "ka", "kaka", "kak"] {
        let words = outputs(&local, &extra, input, TypingMode::Khmer);
        assert!(!words.contains(&"កា".into()) && !words.contains(&"កាៗ".into()), "{input}");
    }
    for input in ["saved alias", "savedalias", "កា"] {
        assert!(outputs(&local, &extra, input, TypingMode::Khmer).contains(&"កា".into()), "{input}");
    }
    assert!(outputs(&local, &extra, "kakha", TypingMode::Khmer).contains(&"កាខា".into()));
    assert!(outputs(&base, &empty(), "ka", TypingMode::Khmer).contains(&"កា".into()));
    assert_eq!(local.to_binary(), before);
    assert_eq!(base.to_binary(), before);
    assert_eq!(serde_json::from_str::<serde_json::Value>(&local.word_typing_settings_json("khmer","កា").unwrap()).unwrap()["soundMapEnabled"], false);
}

#[test]
fn explicit_enable_reopens_base_disabled_generation_and_reset_reinstates_it() {
    let base = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"englishWords":[],
        "words":[{"id":1,"khmer":"កា","rank":0,"status":"approved"}],"soundMapDisabledWords":["កា"]}).to_string()).unwrap();
    let policy = json!([{"language":"khmer","word":"កា","soundMapEnabled":true}]).to_string();
    let local = base.with_typing_overrides_json(&policy).unwrap();
    let extra = empty().with_typing_overrides_json(&policy).unwrap();
    assert!(!outputs(&base,&extra,"ka",TypingMode::Khmer).contains(&"កា".into()));
    assert!(outputs(&local,&extra,"ka",TypingMode::Khmer).contains(&"កា".into()));
    let reset = local.with_typing_overrides_json("[]").unwrap();
    assert!(!outputs(&reset,&empty(),"ka",TypingMode::Khmer).contains(&"កា".into()));
    assert_eq!(local.to_binary(),base.to_binary());
}

#[test]
fn rank_reorders_saved_exact_matches_preserves_ids_and_does_not_promote_prefixes() {
    let base = Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":[{"id":1,"khmer":"កា","rank":0,"status":"approved"},
            {"id":2,"khmer":"ខា","rank":10,"status":"approved"}],
        "mappings":[{"id":1,"romanized":"shared","khmer":"កា","rank":0,"status":"approved"},
            {"id":2,"romanized":"shared","khmer":"ខា","rank":10,"status":"approved"}]}).to_string()).unwrap();
    let extra = Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":[{"id":11,"khmer":"គា","rank":0,"status":"approved"}],
        "mappings":[{"id":11,"romanized":"sharedlong","khmer":"គា","rank":0,"status":"approved"}]}).to_string()).unwrap();
    let local = base.with_typing_overrides_json(&json!([{"language":"khmer","word":"កា","rank":100}]).to_string()).unwrap();
    let query = local.suggest_with_extension(&extra,"shared",SuggestionFeatures::default(),&TypingContext::default()).unwrap();
    assert_eq!(query.exact_candidates.iter().map(|word| word.mapping_id).collect::<Vec<_>>(), [Some(2),Some(1)]);
    assert_eq!(outputs(&local,&extra,"shared",TypingMode::Khmer), ["ខា","កា","គា"]);
    assert_eq!(query.result.candidates[0].completion, None);
    assert_eq!(outputs(&base,&extra,"shared",TypingMode::Khmer), ["កា","ខា","គា"]);
    assert_eq!(local.to_binary(),base.to_binary());
}

#[test]
fn rank_is_applied_before_completion_limits_and_across_both_dictionaries() {
    let words = (0..70).map(|i| json!({"id":i+1,"text":format!("probe{}{}",char::from(b'a'+i/26),char::from(b'a'+i%26)),
        "rank":50,"status":"approved"})).collect::<Vec<_>>();
    let base = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],"englishWords":words}).to_string()).unwrap();
    let extra = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],
        "englishWords":[{"id":101,"text":"probepersonal","rank":0,"status":"approved"}]}).to_string()).unwrap();
    let policy = json!([{"language":"english","word":"probecr","rank":0},
        {"language":"english","word":"probepersonal","rank":100}]).to_string();
    let local = base.with_typing_overrides_json(&policy).unwrap();
    let personal = extra.with_typing_overrides_json(&policy).unwrap();
    let words = outputs(&local,&personal,"probe",TypingMode::English);
    assert_eq!(words.len(),20);
    assert_eq!(words[0],"probecr");
    assert!(!words.contains(&"probepersonal".into()));
    assert_eq!(outputs(&local,&personal,"probepersonal",TypingMode::English)[0],"probepersonal");
    let before = outputs(&base,&extra,"probe",TypingMode::English);
    assert_eq!(before[0],"probepersonal");
    assert_eq!(local.to_binary(),base.to_binary());
}

#[test]
fn commonness_still_precedes_local_rank_and_unconfigured_queries_are_identical() {
    let mut base = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],
        "englishWords":[{"id":1,"text":"testone","rank":0,"status":"approved"},
            {"id":2,"text":"testtwo","rank":0,"status":"approved"}]}).to_string()).unwrap();
    base.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","khmer":[],
        "english":[{"word":"testone","level":2},{"word":"testtwo","level":4}]}).to_string()).unwrap();
    let settings = json!([{"language":"english","word":"testone","rank":1_000_000}]).to_string();
    let view = base.with_typing_overrides_json(&settings).unwrap();
    assert_eq!(outputs(&view,&empty(),"test",TypingMode::English),["testone","testtwo"]);
    let default = base.with_typing_overrides_json("[]").unwrap();
    for input in ["test","testone","testtwo","missing",""] {
        assert_eq!(outputs(&default,&empty(),input,TypingMode::English),outputs(&base,&empty(),input,TypingMode::English));
    }
}

#[test]
fn malformed_settings_never_change_the_original_dictionary() {
    let base = empty(); let before = base.to_binary();
    for value in [json!({}),json!([{"language":"khmer","word":"កា","rank":1_000_001}]),
        json!([{"language":"english","word":"test","soundMapEnabled":false}]),
        json!([{"language":"english","word":"TEST","rank":0}]),
        json!([{"language":"khmer","word":"កា"}]),
        json!([{"language":"khmer","word":"កា","rank":0},{"language":"khmer","word":"កា","rank":2}])] {
        assert!(base.with_typing_overrides_json(&value.to_string()).is_err(),"{value}");
    }
    assert_eq!(base.to_binary(),before);
}

#[test]
fn local_quarantine_hides_aliases_joined_forms_completion_sounds_and_corrections() {
    let base = Engine::from_json(&json!({"schemaVersion":3,
        "mappings":[{"id":1,"romanized":"ka ka","khmer":"កា","rank":0,"status":"approved"},
                    {"id":2,"romanized":"kha kha","khmer":"ខា","rank":0,"status":"approved"}],
        "words":[{"id":1,"khmer":"កា","rank":0,"status":"approved"},
                 {"id":2,"khmer":"ខា","rank":0,"status":"approved"}],
        "englishWords":[{"id":3,"text":"peach","rank":0,"status":"approved"},
                        {"id":4,"text":"pear","rank":0,"status":"approved"}]}).to_string()).unwrap();
    let before = base.to_binary();
    let policy = json!([{"language":"khmer","word":"កា","excluded":true},
                       {"language":"english","word":"peach","excluded":true}]).to_string();
    let local = base.with_typing_overrides_json(&policy).unwrap();
    for input in ["ka ka","kaka","ka","ក"] {
        assert!(!outputs(&local,&empty(),input,TypingMode::Khmer).contains(&"កា".into()),"{input}");
    }
    for input in ["kha kha","khakha"] {
        assert!(outputs(&local,&empty(),input,TypingMode::Khmer).contains(&"ខា".into()),"{input}");
    }
    assert!(!outputs(&local,&empty(),"pea",TypingMode::English).contains(&"peach".into()));
    let correction = local.suggest_with_features("paech", SuggestionFeatures {
        mode: TypingMode::English, english_autocorrection: true, ..SuggestionFeatures::default()
    }).unwrap();
    assert!(!correction.candidates.iter().any(|s| matches!(&s.candidate,
        SuggestionCandidate::English(word) if word.text == "peach")));
    assert!(outputs(&local,&empty(),"pea",TypingMode::English).contains(&"pear".into()));
    let reset = local.with_typing_overrides_json("[]").unwrap();
    assert!(outputs(&reset,&empty(),"kaka",TypingMode::Khmer).contains(&"កា".into()));
    assert!(outputs(&reset,&empty(),"peach",TypingMode::English).contains(&"peach".into()));
    assert_eq!(local.to_binary(),before);
    assert_eq!(base.to_binary(),before);
}

#[test]
fn local_quarantine_cannot_be_recreated_by_composing_unrelated_words() {
    let base = Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":[{"id":1,"khmer":"កា","rank":0,"status":"approved"},
                 {"id":2,"khmer":"ខា","rank":0,"status":"approved"}],
        "mappings":[{"id":1,"romanized":"first","khmer":"កា","rank":0,"status":"approved"},
                    {"id":2,"romanized":"second","khmer":"ខា","rank":0,"status":"approved"}]}).to_string()).unwrap();
    assert!(base.convert("first second").unwrap().candidates.iter().any(|c| c.khmer == "កាខា"));
    let view = base.with_typing_overrides_json(r#"[{"language":"khmer","word":"កាខា","excluded":true}]"#).unwrap();
    assert!(!view.convert("first second").unwrap().candidates.iter().any(|c| c.khmer == "កាខា"));
    assert!(!view.convert("first").unwrap().candidates.is_empty());
}

#[test]
fn a_rank_setting_does_not_reorder_queries_for_other_words() {
    let mut base = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],
        "englishWords":[{"id":1,"text":"peach","rank":0,"status":"approved"},
            {"id":2,"text":"testone","rank":0,"status":"approved"}]}).to_string()).unwrap();
    base.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","khmer":[],
        "english":[{"word":"peach","level":3}]}).to_string()).unwrap();
    let extra = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],
        "englishWords":[{"id":11,"text":"pear","rank":0,"status":"approved"}]}).to_string()).unwrap();
    let settings = json!([{"language":"english","word":"testone","rank":100}]).to_string();
    let local = base.with_typing_overrides_json(&settings).unwrap();
    let personal = extra.with_typing_overrides_json(&settings).unwrap();
    assert_eq!(outputs(&base,&extra,"pe",TypingMode::English),["pear","peach"]);
    assert_eq!(outputs(&local,&personal,"pe",TypingMode::English),outputs(&base,&extra,"pe",TypingMode::English));
    let khmer = Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":[{"id":1,"khmer":"កា","rank":100,"status":"approved"},
            {"id":2,"khmer":"ខា","rank":0,"status":"approved"},
            {"id":3,"khmer":"គា","rank":0,"status":"approved"}],
        "mappings":[{"id":1,"romanized":"khprefix","khmer":"កា","rank":100,"status":"approved"}]}).to_string()).unwrap();
    let policy = json!([{"language":"khmer","word":"គា","rank":99}]).to_string();
    let local = khmer.with_typing_overrides_json(&policy).unwrap();
    assert_eq!(outputs(&local,&empty(),"kh",TypingMode::Khmer),outputs(&khmer,&empty(),"kh",TypingMode::Khmer));
}
