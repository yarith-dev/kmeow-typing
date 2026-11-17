use khmer_engine::{Engine, ComponentStatus, SuggestionFeatures};

fn engine(words:&[&str],structures:serde_json::Value)->Engine {
    Engine::from_json(&serde_json::json!({"schemaVersion":2,"mappings":[
        {"id":99900,"romanized":"myalias","khmer":words.last().unwrap(),"rank":77,"status":"approved"}],
        "words":words.iter().enumerate().map(|(i,word)|serde_json::json!({
            "id":99000+i,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>(),
        "structures":structures
    }).to_string()).unwrap()
}
fn has(core:&Engine,input:&str,word:&str)->bool {
    core.suggest_with_context(input,Default::default(),&Default::default()).unwrap()
        .exact_candidates.iter().any(|c|c.khmer==word)
}

#[test]
fn rules_reuse_complete_components_at_start_middle_and_end_without_alias_inheritance() {
    for (word,inputs) in [("កិច្ចកា",["kichka","kechka"]),
        ("កាកិច្ច",["kakich","kakech"]),("កាកិច្ចខា",["kakichkha","kakechkha"])] {
        let core=engine(&["កា","ខា","កិច្ច",word],serde_json::json!([]));
        assert_eq!(core.analyze_components(word).unwrap().status,ComponentStatus::Complete);
        let bytes=core.to_binary();let restored=Engine::from_binary(bytes.clone()).unwrap();
        for core in [&core,&restored] {
            for input in inputs {assert!(has(core,input,word),"{input} → {word}");}
            assert!(has(core,"myalias",word));
            assert!(!has(core,"myaliaska",word));
            for features in [SuggestionFeatures{sound_map:false,..Default::default()},
                SuggestionFeatures{romanized:false,..Default::default()}] {
                assert!(!core.suggest_with_context(inputs[0],features,&Default::default()).unwrap()
                    .exact_candidates.iter().any(|c|c.khmer==word));
            }
            assert_eq!(core.to_binary(),bytes);
        }
    }
}

#[test]
fn unresolved_splits_attached_marks_and_subscripts_do_not_establish_boundaries() {
    for word in ["ខាកិច្ច","កិច្ចា","កាក្កិច្ច"] {
        let core=engine(&["កា","កិច្ច",word],serde_json::json!([]));
        for input in ["khakich","kicha","kakich"] {assert!(!has(&core,input,word),"{input} → {word}");}
    }
    let words=["កា","ខា","កិច្ច","កាកិច្ច","កិច្ចខា","កាកិច្ចខា"];
    let ambiguous=engine(&words,serde_json::json!([]));
    assert_eq!(ambiguous.analyze_components(words[5]).unwrap().status,ComponentStatus::Ambiguous);
    assert!(!has(&ambiguous,"kakichkha",words[5]));
    let saved=engine(&words,serde_json::json!([{"wordId":99005,"componentIds":[99000,99002,99001]}]));
    assert!(has(&saved,"kakichkha",words[5]));
}

#[test]
fn standalone_shorthands_never_propagate_as_components() {
    let core=engine(&["កិច្ច","ខ្ញុំ","ខ្ញុំកិច្ច"],serde_json::json!([]));
    assert!(has(&core,"nh","ខ្ញុំ"));
    assert!(!has(&core,"nhkich","ខ្ញុំកិច្ច"));
    assert!(!has(&core,"nhkech","ខ្ញុំកិច្ច"));
    let result=core.suggest_with_context("nh",Default::default(),&Default::default()).unwrap();
    assert!(result.result.candidates.iter().any(|s|s.tap_only
        && matches!(&s.candidate,khmer_engine::SuggestionCandidate::Khmer(c) if c.khmer=="ខ្ញុំ")));
    let special=engine(&["ស","កម្ម","សកម្ម"],serde_json::json!([]));
    assert!(has(&special,"sakkam","សកម្ម"));
    assert!(!has(&special,"sorkam","សកម្ម"));
}

#[test]
fn existing_consonant_count_matches_stay_ahead_of_new_component_readings() {
    let core=engine(&["កា","កិច្ច","កាកិច្ច","កាក្កិច"],serde_json::json!([]));
    let result=core.suggest_with_context("kakich",Default::default(),&Default::default()).unwrap();
    assert_eq!(result.exact_candidates[0].khmer,"កាក្កិច");
    assert!(result.exact_candidates.iter().any(|c|c.khmer=="កាកិច្ច"));
    assert_eq!(result.result.candidates[0].preferred_input.as_deref(),Some("kakkich"));
}

#[test]
fn nested_components_reuse_word_readings_and_keep_previous_component_paths() {
    let core=engine(&["កា","កិច្ច","ខា","កាកិច្ច","កាកិច្ចខា","កាកិច្ចកិច្ច"],serde_json::json!([]));
    assert_eq!(core.analyze_components("កាកិច្ចខា").unwrap().status,ComponentStatus::Complete);
    for (word,inputs) in [("កាកិច្ចខា",vec!["kakichkha","kakechkha"]),
        ("កាកិច្ចកិច្ច",vec!["kakichkich","kakijjorkich"])] {
        for input in inputs {assert!(has(&core,input,word),"{input} → {word}");}
    }
}
