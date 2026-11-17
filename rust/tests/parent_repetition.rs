use khmer_engine::{Engine, SuggestionCandidate, SuggestionFeatures};
use serde_json::json;

#[test]
fn parent_doubling_is_removed_without_removing_words_or_saved_spellings() {
    let mut source=json!({"schemaVersion":2,"mappings":[
        {"id":9001,"romanized":"customrepeat","khmer":"កាៗ","rank":77,"status":"approved"}],
        "words":[{"id":1,"khmer":"កា","rank":0,"status":"approved"},
            {"id":2,"khmer":"កាៗ","rank":4,"status":"approved"}]});
    let core=Engine::from_json(&source.to_string()).unwrap();
    let bytes=core.to_binary();
    let restored=Engine::from_binary(bytes.clone()).unwrap();
    for engine in [&core,&restored] {
        for input in ["kak","kaka"] {
            let result=engine.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
            assert!(!result.exact_candidates.iter().any(|candidate|candidate.khmer=="កាៗ"));
            assert!(!result.result.candidates.iter().any(|candidate|matches!(&candidate.candidate,
                SuggestionCandidate::Khmer(word) if word.khmer=="កាៗ")));
        }
        assert!(engine.suggest("កាៗ").unwrap().candidates.iter().any(|candidate|matches!(&candidate.candidate,
            SuggestionCandidate::Khmer(word) if word.khmer=="កាៗ")));
        let saved=engine.convert("customrepeat").unwrap();
        assert_eq!(saved.candidates[0].mapping_id,Some(9001));
        assert_eq!(saved.candidates[0].rank,77);
        assert_eq!(engine.to_binary(),bytes);
    }
    source["mappings"].as_array_mut().unwrap().push(json!({"id":9002,"romanized":"kaka","khmer":"កាៗ","rank":5,"status":"approved"}));
    let saved=Engine::from_json(&source.to_string()).unwrap();
    for features in [Default::default(),SuggestionFeatures{sound_map:false,..Default::default()}] {
        let result=saved.suggest_with_context("kaka",features,&Default::default()).unwrap();
        assert_eq!(result.exact_candidates[0].mapping_id,Some(9002));
    }
}
