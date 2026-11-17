use khmer_engine::{Engine, SuggestionFeatures, TypingMode};

fn fixture(saved: bool) -> Engine {
    let words = ["ក", "គ", "មក", "កក", "កក់", "កកាយ", "បបរ", "ល្អ", "ក្អក",
        "សាលា", "កម្ម", "សកម្ម", "សកម្មភាព", "សកម្មា", "សព្ទ", "កិច្ច"];
    Engine::from_json(&serde_json::json!({"schemaVersion":2,
        "words":words.iter().enumerate().map(|(i,word)|serde_json::json!({
            "id":99100+i,"khmer":word,"rank":i,"status":"approved"
        })).collect::<Vec<_>>(),
        "mappings":if saved {serde_json::json!([
            {"id":99500,"romanized":"sorkam","khmer":"សកម្ម","rank":9,"status":"approved"},
            {"id":99501,"romanized":"myorword","khmer":"កម្ម","rank":7,"status":"approved"}
        ])} else {serde_json::json!([])}
    }).to_string()).unwrap()
}

fn exact(core: &Engine, input: &str, word: &str) -> bool {
    core.suggest_with_context(input,Default::default(),&Default::default()).unwrap()
        .exact_candidates.iter().any(|candidate|candidate.khmer == word)
}

#[test]
fn ar_and_or_are_per_syllable_alternatives_without_changing_other_vowels() {
    let core = fixture(false);
    let binary = core.to_binary();
    let restored = Engine::from_binary(binary.clone()).unwrap();
    for (word, inputs) in [
        ("ក",vec!["kor","kar"]), ("គ",vec!["ko"]),
        ("មក",vec!["mok"]), ("កក",vec!["kok"]), ("កក់",vec!["kok"]),
        ("កកាយ",vec!["korkay","karkay"]),
        ("បបរ",vec!["borbor","barbor","borbar","barbar"]),
        ("ល្អ",vec!["l'ar","l'or"]), ("ក្អក",vec!["k'ark","k'ork"]),
        ("សាលា",vec!["sala"]), ("កម្ម",vec!["kam"]),
        ("សកម្ម",vec!["sakkam"]), ("សកម្មភាព",vec!["sorkampheap","sarkampheap"]),
        ("សព្ទ",vec!["sap","sorpto","sarpto"]),
        ("កិច្ច",vec!["kech","kich","kejjor","kejjar","kijjor","kijjar"]),
    ] {
        for input in inputs {
            assert!(exact(&core,input,word),"{input}: {word}");
            assert_eq!(restored.suggest(input).unwrap(),core.suggest(input).unwrap());
            for features in [SuggestionFeatures {sound_map:false,..Default::default()},
                SuggestionFeatures {romanized:false,..Default::default()},
                SuggestionFeatures {mode:TypingMode::English,..Default::default()}] {
                assert!(!core.suggest_with_context(input,features,&Default::default()).unwrap()
                    .exact_candidates.iter().any(|candidate|candidate.khmer == word));
            }
        }
    }
    for (word, inputs) in [
        ("គ",vec!["kar","kor"]), ("មក",vec!["mark","mork"]),
        ("កក",vec!["kark","kork"]), ("កក់",vec!["kark","kork"]),
        ("សាលា",vec!["sarla","sorla"]), ("កម្ម",vec!["karm","korm"]),
        ("សកម្ម",vec!["sorkam","sarkam"]),
        ("សកម្មភាព",vec!["sakkampheap"]), ("សកម្មា",vec!["sakkam"]),
    ] {
        for input in inputs { assert!(!exact(&core,input,word),"{input}: {word}"); }
    }
    let result = core.suggest_with_context("sakkam",Default::default(),&Default::default()).unwrap();
    let hit = result.exact_candidates.iter().find(|candidate|candidate.khmer == "សកម្ម").unwrap();
    assert_eq!(hit.mapping_id,Some(99111));
    assert_eq!(hit.rank,11);
    assert_eq!(core.to_binary(),binary);
}

#[test]
fn selected_sakkam_exception_preserves_saved_and_personal_spellings_exactly() {
    let core = fixture(true);
    let personal = Engine::from_personal_words_json(r#"[
        {"word":"សកម្ម","spellings":["sarkam","myactive"]},
        {"word":"កម្ម","spellings":["myarword"]}
    ]"#).unwrap();
    let before = (core.to_binary(),personal.to_binary());
    for sound_map in [true,false] {
        for (input,word) in [("sorkam","សកម្ម"),("sarkam","សកម្ម"),("myactive","សកម្ម"),
            ("myorword","កម្ម"),("myarword","កម្ម")] {
            assert!(core.suggest_with_extension(&personal,input,
                SuggestionFeatures {sound_map,..Default::default()},&Default::default()).unwrap()
                .exact_candidates.iter().any(|candidate|candidate.khmer == word),"{input}: {word}");
        }
    }
    assert!(!exact(&core,"myarword","កម្ម"));
    assert!(!exact(&core,"myarward","កម្ម"));
    let saved = core.suggest_with_context("sorkam",Default::default(),&Default::default()).unwrap();
    assert!(saved.exact_candidates.iter().any(|candidate|candidate.mapping_id == Some(99500) && candidate.rank == 9));
    assert_eq!((core.to_binary(),personal.to_binary()),before);
}
