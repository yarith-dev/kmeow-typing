use khmer_engine::{Engine, SuggestionCandidate, SuggestionFeatures, TypingContext};
use serde_json::{json, Value};

fn source() -> Value {
    let words = ["ពាក្យ", "សុំ", "សូម", "សម", "ពាក្យសុំ"].iter().enumerate().map(|(i, word)|
            json!({"id":i+1,"khmer":word,"rank":if *word=="សុំ" {999} else {0},"status":"approved"})
        ).collect::<Vec<_>>();
    json!({"schemaVersion":2,"mappings":[],"words":words})
}
fn context(previous: &[&str]) -> TypingContext {
    TypingContext { previous_words: previous.iter().map(|word| (*word).into()).collect(), ..Default::default() }
}
fn output(hit: &khmer_engine::Suggestion) -> &str {
    match &hit.candidate { SuggestionCandidate::Khmer(c) => &c.khmer, _ => panic!() }
}

#[test]
fn reviewed_starting_rule_ranks_from_one_letter_without_making_prefixes_exact() {
    let engine = Engine::from_json(&source().to_string()).unwrap();
    let bytes = engine.to_binary();
    // Compact loaders derive the same rule context without changing the file.
    for engine in [&engine, &Engine::from_binary(bytes.clone()).unwrap()] {
        for input in ["s", "S", "so", "ស"] {
            let plain = engine.suggest(input).unwrap();
            assert_ne!(output(&plain.candidates[0]), "សុំ", "{input}");
            let result = engine.suggest_with_context(input, SuggestionFeatures::default(), &context(&["ពាក្យ"])).unwrap();
            assert_eq!(output(&result.result.candidates[0]), "សុំ", "{input}");
            assert!(result.result.candidates[0].completion.is_some());
            assert!(result.exact_candidates.is_empty());
        }
        let exact = engine.suggest_with_context("som", SuggestionFeatures::default(), &context(&["ពាក្យ"])).unwrap();
        assert_eq!(exact.exact_candidates[0].khmer, "សុំ");
        assert!(exact.result.candidates[0].completion.is_none());
        assert_eq!(exact.exact_candidates[0].rank, 999);
        let many = engine.suggest_with_context("s", SuggestionFeatures::default(), &context(&["សម", "ពាក្យ"])).unwrap();
        assert_eq!(output(&many.result.candidates[0]), "សុំ");
        for previous in [&["ពាក្យ", "សម"][..], &["មិនមាន"][..], &[][..]] {
            assert_eq!(engine.suggest_with_context("s", SuggestionFeatures::default(), &context(previous)).unwrap().result,
                engine.suggest("s").unwrap());
        }
        for input in ["s "] {
            let result = engine.suggest_with_context(input, SuggestionFeatures::default(), &context(&["ពាក្យ"])).unwrap();
            assert!(result.result.candidates.is_empty());
        }
        let next = engine.suggest_with_context("", SuggestionFeatures::default(), &context(&["ពាក្យ"])).unwrap();
        assert_eq!(next.result.candidates.iter().map(output).collect::<Vec<_>>(), ["សុំ"]);
        assert!(next.exact_candidates.is_empty());
        assert_eq!(engine.to_binary(), bytes);
    }
}

#[test]
fn context_requires_available_whole_words_and_uses_clear_new_compounds() {
    for omitted in ["ពាក្យ", "ពាក្យសុំ"] {
        let mut data = source();
        data["words"].as_array_mut().unwrap().retain(|word| word["khmer"] != omitted);
        let engine = Engine::from_json(&data.to_string()).unwrap();
        assert_eq!(engine.suggest_with_context("s", SuggestionFeatures::default(), &context(&["ពាក្យ"])).unwrap().result,
            engine.suggest("s").unwrap());
    }
    let mut data = source();
    data["words"].as_array_mut().unwrap().retain(|word| word["khmer"] != "ពាក្យសុំ");
    data["words"].as_array_mut().unwrap().push(json!({"id":20,"khmer":"ពាក្យសម","rank":0,"status":"approved"}));
    let engine = Engine::from_json(&data.to_string()).unwrap();
    assert_eq!(engine.analyze_components("ពាក្យសម").unwrap().status,khmer_engine::ComponentStatus::Complete);
    let plain = engine.suggest("s").unwrap();
    let ranked = engine.suggest_with_context("s", SuggestionFeatures::default(), &context(&["ពាក្យ"])).unwrap();
    assert_eq!(output(&ranked.result.candidates[0]),"សម");
    assert!(ranked.exact_candidates.is_empty());
    assert_eq!(plain.candidates.iter().map(output).collect::<std::collections::BTreeSet<_>>(),
        ranked.result.candidates.iter().map(output).collect::<std::collections::BTreeSet<_>>());
}

#[test]
fn saved_prefix_context_survives_twenty_slots_and_preserves_alias_identity() {
    let mut data = source();
    let words = data["words"].as_array_mut().unwrap();
    let mut mappings = Vec::new();
    // Artificial alias keys isolate the completion heap, not pronunciation.
    for (i, ch) in ('ក'..='ល').enumerate() {
        let word = ch.to_string();
        words.push(json!({"id":i+20,"khmer":word,"rank":0,"status":"approved"}));
        mappings.push(json!({"id":i+20,"romanized":format!("sa{}{}", (b'a'+(i/26) as u8) as char, (b'a'+(i%26) as u8) as char),
            "khmer":word,"rank":0,"status":"approved"}));
    }
    mappings.push(json!({"id":100,"romanized":"som","khmer":"សុំ","rank":999,"status":"approved"}));
    mappings.push(json!({"id":101,"romanized":"somx","khmer":"សុំ","rank":999,"status":"approved"}));
    data["mappings"] = json!(mappings);
    let engine = Engine::from_json(&data.to_string()).unwrap();
    let bytes = engine.to_binary();
    let features = SuggestionFeatures { sound_map:false, ..Default::default() };
    let plain = engine.suggest_with_features("s", features).unwrap();
    assert_eq!(plain.candidates.len(),20);
    assert!(!plain.candidates.iter().any(|hit| output(hit)=="សុំ"));
    for features in [features, SuggestionFeatures::default()] {
        let result = engine.suggest_with_context("s",features,&context(&["ពាក្យ"])).unwrap();
        assert_eq!(result.result.candidates.len(),20);
        assert_eq!(output(&result.result.candidates[0]),"សុំ");
        assert_eq!(result.result.candidates[0].candidate.mapping_id(),Some(100));
        assert_eq!(result.result.candidates[0].completion.as_deref(),Some("som"));
        assert!(result.exact_candidates.is_empty());
        assert_eq!(result.result.candidates.iter().map(output).collect::<std::collections::BTreeSet<_>>().len(),20);
    }
    assert_eq!(engine.to_binary(),bytes);
}
