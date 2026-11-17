use khmer_engine::Engine;

// The owner removed default-equivalent component overrides. Keep each old
// fixture claim accounted for, and prove retired generated inputs stay absent.
pub fn assert_retired(engine:&Engine,proof:&serde_json::Value) {
    if let Some(groups)=proof["retiredGenerated"].as_object() {
        for pairs in groups.values() {
            for pair in pairs.as_array().unwrap() {
                let input=pair[0].as_str().unwrap();let word=pair[1].as_str().unwrap();
                assert!(!engine.suggest_with_context(input,Default::default(),&Default::default()).unwrap()
                    .exact_candidates.iter().any(|candidate|candidate.khmer==word),"retired {input}: {word}");
                assert!(engine.suggest(word).unwrap().candidates.iter().any(|suggestion|
                    matches!(&suggestion.candidate,khmer_engine::SuggestionCandidate::Khmer(candidate) if candidate.khmer==word)),
                    "retired reading must not remove {word}");
            }
        }
    }
}
