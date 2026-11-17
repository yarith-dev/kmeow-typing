use khmer_engine::{ComponentSource, ComponentStatus, Engine};

fn engine(words: &[&str], saved: bool) -> Engine {
    Engine::from_json(&serde_json::json!({"schemaVersion":2,
        "words":words.iter().enumerate().map(|(i,word)|serde_json::json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>(),
        "mappings":[{"id":900,"romanized":"myword","khmer":words.last().unwrap(),"rank":77,"status":"approved"}],
        "structures":if saved {serde_json::json!([{"wordId":words.len(),"componentIds":[1,3]}])} else {serde_json::json!([])}
    }).to_string()).unwrap()
}

#[test]
fn partial_components_keep_unknown_prefixes_middles_and_suffixes() {
    for (known,texts,ids) in [
        (vec!["កា"],vec!["កា","ខាគា"],vec![true,false]),
        (vec!["ខា"],vec!["កា","ខា","គា"],vec![false,true,false]),
        (vec!["គា"],vec!["កាខា","គា"],vec![false,true]),
        (vec!["កា","គា"],vec!["កា","ខា","គា"],vec![true,false,true]),
    ] {
        let mut words = known;words.push("កាខាគា");
        let core = engine(&words,false);
        let bytes = core.to_binary();let before = core.suggest("myword").unwrap();
        let result = core.analyze_components("កាខាគា").unwrap();
        assert_eq!(result.status,ComponentStatus::Partial);
        assert_eq!(result.source,ComponentSource::Automatic);
        assert_eq!(result.parts.iter().map(|part|part.text.as_str()).collect::<Vec<_>>(),texts);
        assert_eq!(result.parts.iter().map(|part|part.word_id.is_some()).collect::<Vec<_>>(),ids);
        assert_eq!(result.parts.iter().map(|part|part.text.as_str()).collect::<String>(),"កាខាគា");
        assert_eq!(core.analyze_components("កាខាគា").unwrap(),result);
        assert_eq!(core.suggest("myword").unwrap(),before);
        assert_eq!(core.to_binary(),bytes);
    }
}

#[test]
fn ambiguity_stays_unselected_and_saved_components_resolve_it() {
    let words = ["កា","កាខា","ខាគា","គា","កាខាគា"];
    let core = engine(&words,false);
    let result = core.analyze_components("កាខាគា").unwrap();
    assert_eq!(result.status,ComponentStatus::Ambiguous);
    assert!(result.parts.is_empty());assert_eq!(result.alternatives.len(),2);
    assert!(!result.more_alternatives);
    let saved = engine(&words,true);
    let bytes = saved.to_binary();
    let result = saved.analyze_components("កាខាគា").unwrap();
    assert_eq!(result.source,ComponentSource::Saved);
    assert_eq!(result.status,ComponentStatus::Complete);
    assert_eq!(result.parts.iter().map(|part|part.text.as_str()).collect::<Vec<_>>(),["កា","ខាគា"]);
    assert_eq!(Engine::from_binary(bytes.clone()).unwrap().analyze_components("កាខាគា").unwrap(),result);
    assert_eq!(saved.to_binary(),bytes);
}

#[test]
fn marks_subscripts_whole_word_matches_and_large_requests_are_guarded() {
    let core = engine(&["ក","សា","កា","ក្សា"],false);
    for word in ["កា","ក្សា"] {
        let result = core.analyze_components(word).unwrap();
        assert_eq!(result.status,ComponentStatus::Unknown,"{word}");
        assert!(result.parts.iter().all(|part|part.word_id.is_none()));
    }
    for word in ["","books","កា ខា",&"ក".repeat(241)] {assert!(core.analyze_components(word).is_err());}
    // Three different two-part analyses still tie after component-count scoring.
    let core = engine(&["កា","កាខា","កាខាគា","ខាគាឃា","គាឃា","ឃា","កាខាគាឃា"],false);
    let result = core.analyze_components("កាខាគាឃា").unwrap();
    assert_eq!(result.status,ComponentStatus::Ambiguous);
    assert_eq!(result.alternatives.len(),2);assert!(result.more_alternatives);
}

#[test]
fn equal_coverage_prefers_fewer_components_but_more_coverage_still_wins() {
    let words = ["កា","ខា","គា","កាខា","កាខាគា"];
    let core = engine(&words,false);
    let result = core.analyze_components("កាខាគា").unwrap();
    assert_eq!(result.source,ComponentSource::Automatic);
    assert_eq!(result.status,ComponentStatus::Complete);
    assert_eq!(result.parts.iter().map(|part|part.text.as_str()).collect::<Vec<_>>(),["កាខា","គា"]);
    assert!(result.alternatives.is_empty());assert!(!result.more_alternatives);

    let core = engine(&["កា","ខា","គា","កាខាគា"],false);
    let result = core.analyze_components("កាខាគា").unwrap();
    assert_eq!(result.status,ComponentStatus::Complete);
    assert_eq!(result.parts.len(),3,"full coverage beats a shorter partial match");

    let core = engine(&["កា","ខា","កាខា","កាខាគាឃា"],false);
    let result = core.analyze_components("កាខាគាឃា").unwrap();
    assert_eq!(result.status,ComponentStatus::Partial);
    assert_eq!(result.parts.iter().map(|part|part.text.as_str()).collect::<Vec<_>>(),["កាខា","គាឃា"]);
    assert!(result.parts[0].word_id.is_some());assert!(result.parts[1].word_id.is_none());
}

#[test]
fn saved_definitions_override_fewer_component_preferences() {
    let words = ["កា","ខា","គា","កាខា","ខាគា","កាខាគា"];
    let source = |alternatives:serde_json::Value|serde_json::json!({"schemaVersion":2,"mappings":[],
        "words":words.iter().enumerate().map(|(i,word)|serde_json::json!({"id":i+1,"khmer":word,"rank":0,"status":"approved"})).collect::<Vec<_>>(),
        "structures":alternatives});
    let saved = Engine::from_json(&source(serde_json::json!([{"wordId":6,"componentIds":[1,2,3]}])).to_string()).unwrap();
    let result = saved.analyze_components("កាខាគា").unwrap();
    assert_eq!(result.source,ComponentSource::Saved);
    assert_eq!(result.parts.iter().map(|part|part.text.as_str()).collect::<Vec<_>>(),["កា","ខា","គា"]);
    let saved = Engine::from_json(&source(serde_json::json!([
        {"wordId":6,"componentIds":[1,2,3]},{"wordId":6,"componentIds":[4,3]}
    ])).to_string()).unwrap();
    let result = saved.analyze_components("កាខាគា").unwrap();
    assert_eq!(result.source,ComponentSource::Saved);
    assert_eq!(result.status,ComponentStatus::Ambiguous);
    assert_eq!(result.alternatives.len(),2,"explicit saved choices must not be pruned");
}

#[test]
fn personal_components_are_cached_per_generation_without_changing_spellings() {
    let core = engine(&["កាខា"],false);
    let before = core.to_binary();
    assert_eq!(core.analyze_components("កាខា").unwrap().status,ComponentStatus::Unknown);
    for (word,texts) in [("កា",vec!["កា","ខា"]),("ខា",vec!["កា","ខា"])] {
        let local = Engine::from_personal_words_json(&serde_json::json!([
            {"word":word,"spellings":["mypart"]},{"word":"កាខា","spellings":["ownword"]}
        ]).to_string()).unwrap();
        let bytes = local.to_binary();
        let result = core.analyze_components_with_extension("កាខា",&local).unwrap();
        assert_eq!(result.status,ComponentStatus::Partial);
        assert_eq!(result.parts.iter().map(|part|part.text.as_str()).collect::<Vec<_>>(),texts);
        assert_eq!(result.parts[usize::from(word=="ខា")].word_id.is_some(),true);
        assert_eq!(result.parts[usize::from(word=="កា")].word_id.is_none(),true);
        assert!(core.convert_with_extension(&local,"ownword").unwrap().candidates.iter().any(|candidate|candidate.khmer=="កាខា"));
        assert_eq!(local.to_binary(),bytes);
    }
    assert_eq!(core.analyze_components("កាខា").unwrap().status,ComponentStatus::Unknown);
    assert_eq!(core.to_binary(),before);
}

#[test]
fn component_queries_never_change_typing_results() {
    let core = engine(&["កា","ខា","គា","កាខា","ខាគា","កាខាគា"],false);
    let inputs = ["myword","ka","kakhea","s","books"];
    let before:Vec<_> = inputs.iter().map(|input|core.suggest(input).unwrap()).collect();
    for word in ["កាខាគា","កា","ខាគា","គាឃាងា"] {core.analyze_components(word).unwrap();}
    assert_eq!(inputs.iter().map(|input|core.suggest(input).unwrap()).collect::<Vec<_>>(),before);
}

#[test]
fn saved_components_report_unavailable_parts_without_discarding_the_definition() {
    let original = engine(&["កា","កាខា","ខាគា","គា","កាខាគា"],true);
    for hidden in [vec!["កា"],vec!["កា","ខាគា"]] {
        let settings:Vec<_> = hidden.iter().map(|word|serde_json::json!({"language":"khmer","word":word,"excluded":true})).collect();
        let core = original.with_typing_overrides_json(&serde_json::to_string(&settings).unwrap()).unwrap();
        let before = core.to_binary();
        let result = core.analyze_components("កាខាគា").unwrap();
        assert_eq!(result.source,ComponentSource::Saved);
        assert_eq!(result.status,ComponentStatus::Partial);
        assert_eq!(result.parts.iter().map(|part|part.text.as_str()).collect::<Vec<_>>(),["កា","ខាគា"]);
        assert!(result.parts.iter().filter(|part|hidden.contains(&part.text.as_str())).all(|part|part.word_id.is_none()));
        assert_eq!(core.to_binary(),before);
    }
}

#[test]
fn personal_component_lookup_respects_exclusions_in_both_dictionaries() {
    let base = engine(&["កា","ខា","កាខា"],false);
    let hidden = base.with_typing_overrides_json(r#"[{"language":"khmer","word":"កា","excluded":true}]"#).unwrap();
    let local = Engine::from_personal_words_json(r#"[{"word":"កា","spellings":["mypart"]}]"#).unwrap();
    let result = hidden.analyze_components_with_extension("កាខា",&local).unwrap();
    assert_eq!(result.status,ComponentStatus::Partial);
    assert_eq!(result.parts[0].word_id,None);
    assert!(result.parts[1].word_id.is_some());
    // A newer personal view can withhold a part that is still in the base.
    assert_eq!(base.analyze_components_with_extension("កាខា",&local).unwrap().status,ComponentStatus::Complete);
    let blocked = local.with_typing_overrides_json(r#"[{"language":"khmer","word":"ខា","excluded":true}]"#).unwrap();
    let result = base.analyze_components_with_extension("កាខា",&blocked).unwrap();
    assert_eq!(result.status,ComponentStatus::Partial);
    assert!(result.parts[0].word_id.is_some());assert_eq!(result.parts[1].word_id,None);
}

#[test]
fn all_saved_alternatives_survive_different_component_availability() {
    let words = ["កា","កាខា","ខាគា","គា","កាខាគា"];
    let core = Engine::from_json(&serde_json::json!({"schemaVersion":2,"mappings":[],
        "words":words.iter().enumerate().map(|(i,word)|serde_json::json!({"id":i+1,"khmer":word,"rank":0,"status":"approved"})).collect::<Vec<_>>(),
        "structures":[{"wordId":5,"componentIds":[1,3]},{"wordId":5,"componentIds":[2,4]}]
    }).to_string()).unwrap();
    for hidden in [vec!["កា"],vec!["កា","កាខា","ខាគា","គា"]] {
        let settings:Vec<_> = hidden.iter().map(|word|serde_json::json!({"language":"khmer","word":word,"excluded":true})).collect();
        let view = core.with_typing_overrides_json(&serde_json::to_string(&settings).unwrap()).unwrap();
        let result = view.analyze_components("កាខាគា").unwrap();
        assert_eq!(result.source,ComponentSource::Saved);
        assert_eq!(result.status,ComponentStatus::Ambiguous);
        assert_eq!(result.alternatives.len(),2);assert!(!result.more_alternatives);
    }
}
