use khmer_engine::{ComponentStatus, Engine, SuggestionCandidate, SuggestionFeatures, TypingContext};
use serde_json::{json,Value};

fn source(words: &[&str], matches: &[&str], structures: Value) -> Value {
    json!({"schemaVersion":2,
        "words":words.iter().enumerate().map(|(i,word)|json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>(),
        "mappings":matches.iter().enumerate().map(|(i,word)|json!({"id":900+i,"romanized":"same","khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>(),
        "structures":structures})
}
fn context(previous: &[&str]) -> TypingContext { TypingContext {previous_words:previous.iter().map(|word|(*word).into()).collect(),..Default::default()} }
fn exact(engine: &Engine, previous: &[&str]) -> Vec<String> {
    engine.suggest_with_context("same",Default::default(),&context(previous)).unwrap().exact_candidates
        .into_iter().map(|candidate|candidate.khmer).collect()
}

#[test]
fn clear_components_rank_existing_exact_prefix_and_next_word_choices() {
    let value = source(&["សួរ","សុខ","សប្បាយ","សុខសប្បាយ"],&["សួរ","សប្បាយ"],json!([]));
    let core = Engine::from_json(&value.to_string()).unwrap();let bytes = core.to_binary();
    assert_eq!(core.analyze_components("សុខសប្បាយ").unwrap().status,ComponentStatus::Complete);
    assert_eq!(exact(&core,&[]),["សួរ","សប្បាយ"]);
    assert_eq!(exact(&core,&["សុខ"]),["សប្បាយ","សួរ"]);
    assert_eq!(exact(&core,&["សួរ"]),exact(&core,&[]));
    let prefix = core.suggest_with_context("sa",Default::default(),&context(&["សុខ"])).unwrap();
    assert_eq!(prefix.result.candidates[0].candidate,SuggestionCandidate::Khmer(
        core.suggest_with_context("same",Default::default(),&context(&["សុខ"])).unwrap().exact_candidates[0].clone()));
    assert!(prefix.exact_candidates.is_empty());
    let next = core.suggest_with_context("",Default::default(),&context(&["សុខ"])).unwrap();
    assert!(next.exact_candidates.is_empty());
    assert!(next.result.candidates.iter().any(|hit|matches!(&hit.candidate,SuggestionCandidate::Khmer(c) if c.khmer == "សប្បាយ")));
    assert_eq!(core.convert("same").unwrap().candidates[0].khmer,"សួរ");
    assert_eq!(core.to_binary(),bytes);
    assert_eq!(exact(&Engine::from_binary(bytes).unwrap(),&["សុខ"]),exact(&core,&["សុខ"]));
}

#[test]
fn ambiguous_partial_and_multiple_saved_splits_do_not_guess() {
    let words = ["សួរ","កា","កាខា","ខាគា","គា","កាខាគា"];
    for structures in [json!([]),json!([{"wordId":6,"componentIds":[2,4]},{"wordId":6,"componentIds":[3,5]}])] {
        let core = Engine::from_json(&source(&words,&["សួរ","ខាគា"],structures).to_string()).unwrap();
        assert_eq!(core.analyze_components("កាខាគា").unwrap().status,ComponentStatus::Ambiguous);
        assert_eq!(exact(&core,&["កា"]),exact(&core,&[]));
    }
    let core = Engine::from_json(&source(&["កា","កាខាគា"],&["កា"],json!([])).to_string()).unwrap();
    assert_eq!(core.analyze_components("កាខាគា").unwrap().status,ComponentStatus::Partial);
    assert!(core.suggest_with_context("",Default::default(),&context(&["កា"])).unwrap().result.candidates.is_empty());
}

#[test]
fn saved_components_outrank_automatic_and_keep_longer_authoritative_choices() {
    let words = ["កា","ខា","គា","កាខា","កាខាគា","កាគា"];
    let core = Engine::from_json(&source(&words,&["ខា","គា"],json!([{"wordId":6,"componentIds":[1,3]}])).to_string()).unwrap();
    assert_eq!(exact(&core,&["កា"]),["គា","ខា"]);
    let core = Engine::from_json(&source(&words,&["កាខា","ខា"],json!([{"wordId":5,"componentIds":[1,2,3]}])).to_string()).unwrap();
    assert_eq!(exact(&core,&["កា"]),["ខា","កាខា"]);
    let next = core.suggest_with_context("",Default::default(),&context(&["កា","ខា"])).unwrap();
    assert!(next.result.candidates.iter().any(|hit|matches!(&hit.candidate,SuggestionCandidate::Khmer(c) if c.khmer == "គា")));
}

#[test]
fn inferred_context_keeps_rare_words_explicit_and_never_promotes_prefixes_to_exact() {
    let mut core = Engine::from_json(&source(&["សួរ","សុខ","សប្បាយ","សុខសប្បាយ"],
        &["សួរ","សប្បាយ"],json!([])).to_string()).unwrap();
    core.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","khmer":[
        {"word":"សួរ","level":1},{"word":"សប្បាយ","level":5}],"english":[]}).to_string()).unwrap();
    let result = core.suggest_with_context("same",Default::default(),&context(&["សុខ"])).unwrap();
    assert_eq!(result.exact_candidates[0].khmer,"សប្បាយ");
    assert!(result.result.candidates[0].tap_only);
    assert!(result.result.candidates[0].completion.is_none());
    let prefix = core.suggest_with_context("sa",Default::default(),&context(&["សុខ"])).unwrap();
    assert!(prefix.exact_candidates.is_empty());
    let target = prefix.result.candidates.iter().find(|hit|matches!(&hit.candidate,
        SuggestionCandidate::Khmer(candidate) if candidate.khmer == "សប្បាយ")).unwrap();
    assert!(target.tap_only && target.completion.is_some());
}

#[test]
fn personal_generation_resolves_or_introduces_ambiguity_without_editing_spellings() {
    let base = Engine::from_json(&source(&["កា","គា","កាខា"],&["គា"],json!([])).to_string()).unwrap();
    let local = Engine::from_personal_words_json(r#"[{"word":"ខា","spellings":["same","my-part"]}]"#).unwrap();
    let bytes = (base.to_binary(),local.to_binary());
    assert_eq!(exact(&base,&["កា"]),["គា"]);
    for _ in 0..3 {
        let result = base.suggest_with_extension(&local,"same",Default::default(),&context(&["កា"])).unwrap();
        assert_eq!(result.exact_candidates[0].khmer,"ខា");
        assert_eq!(result.result.candidates[0].candidate,SuggestionCandidate::Khmer(result.exact_candidates[0].clone()));
    }
    base.clear_component_extension_cache();
    let replacement = Engine::from_personal_words_json(r#"[{"word":"សួរ","spellings":["same"]}]"#).unwrap();
    assert_eq!(base.suggest_with_extension(&replacement,"same",Default::default(),&context(&["កា"])).unwrap().exact_candidates[0].khmer,"គា");
    let hidden = local.with_typing_overrides_json(r#"[{"language":"khmer","word":"ខា","excluded":true}]"#).unwrap();
    assert!(!base.suggest_with_extension(&hidden,"same",Default::default(),&context(&["កា"])).unwrap().exact_candidates.iter().any(|candidate|candidate.khmer == "ខា"));
    assert_eq!((base.to_binary(),local.to_binary()),bytes);

    let base = Engine::from_json(&source(&["សួរ","កា","កាខា","ខាគា","កាខាគា"],&["សួរ","ខាគា"],json!([])).to_string()).unwrap();
    assert_eq!(exact(&base,&["កា"])[0],"ខាគា");
    let local = Engine::from_personal_words_json(r#"[{"word":"គា","spellings":["my-new-part"]}]"#).unwrap();
    assert_eq!(base.analyze_components_with_extension("កាខាគា",&local).unwrap().status,ComponentStatus::Ambiguous);
    assert_eq!(base.suggest_with_extension(&local,"same",Default::default(),&context(&["កា"])).unwrap().exact_candidates[0].khmer,"សួរ");
    assert_eq!(exact(&base,&["កា"])[0],"ខាគា");
    assert!(base.suggest_with_extension(&local,"my-new-part",SuggestionFeatures {sound_map:false,..Default::default()},&context(&["កា"])).unwrap().exact_candidates.iter().any(|candidate|candidate.khmer == "គា"));
}

#[test]
fn unrelated_custom_ranks_keep_context_active_for_merged_exact_and_prefix_choices() {
    for saved in [false,true] {
        let words = ["កា","គា","ខា","កាខា","ឃា"];
        let base = Engine::from_json(&source(&words,&["គា"],if saved {
            json!([{"wordId":4,"componentIds":[1,3]}])
        } else {json!([])}).to_string()).unwrap();
        let local = Engine::from_personal_words_json(r#"[
            {"word":"ខា","spellings":["same","my-part"]},
            {"word":"ឃា","spellings":["other-word"]}
        ]"#).unwrap();
        let base_rank = base.with_typing_overrides_json(r#"[{"language":"khmer","word":"កា","rank":123}]"#).unwrap();
        let local_rank = local.with_typing_overrides_json(r#"[{"language":"khmer","word":"ឃា","rank":456}]"#).unwrap();
        let binaries = (base.to_binary(),local.to_binary());
        for input in ["same","sa","my-part"] {
            for previous in [context(&[]),context(&["កា"])] {
                let expected = base.suggest_with_extension(&local,input,Default::default(),&previous).unwrap();
                for (core,extra) in [(&base_rank,&local),(&base,&local_rank),(&base_rank,&local_rank)] {
                    assert_eq!(core.suggest_with_extension(extra,input,Default::default(),&previous).unwrap(),
                        expected,"saved={saved}, input={input}, previous={:?}",previous.previous_words);
                }
                if !previous.previous_words.is_empty() {
                    if input == "sa" { assert!(expected.exact_candidates.is_empty()); }
                    else { assert_eq!(expected.exact_candidates[0].khmer,"ខា"); }
                    assert!(matches!(&expected.result.candidates[0].candidate,
                        SuggestionCandidate::Khmer(word) if word.khmer == "ខា"));
                }
            }
        }
        assert_eq!((base.to_binary(),local.to_binary()),binaries);
        assert_eq!((base_rank.to_binary(),local_rank.to_binary()),binaries);
    }
}
