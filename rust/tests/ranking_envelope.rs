use khmer_engine::{Engine, RankingRequest, RankingSuggestionResult, Suggestion, SuggestionCandidate, SuggestionFeatures, TypingContext, TypingMode};
use serde_json::json;

fn english(words: &[String]) -> Engine {
    Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],
        "englishWords":words.iter().enumerate().map(|(id,text)|json!({"id":id+1,"text":text,"rank":id,"status":"approved"})).collect::<Vec<_>>()}).to_string()).unwrap()
}
fn en() -> SuggestionFeatures { SuggestionFeatures { mode: TypingMode::English, ..Default::default() } }
fn output(hit: &Suggestion) -> &str {
    match &hit.candidate { SuggestionCandidate::Khmer(word) => &word.khmer, SuggestionCandidate::English(word) => &word.text }
}
fn order(result: &RankingSuggestionResult) -> Vec<u32> {
    result.ranking.groups.iter().flat_map(|group| group.candidates.iter().map(|candidate| candidate.id)).collect()
}
fn generated_words(count: usize) -> Vec<String> {
    (0..count).map(|value| format!("b{}{}{}", (b'a'+(value/676) as u8) as char,
        (b'a'+((value/26)%26) as u8) as char, (b'a'+(value%26) as u8) as char)).collect()
}

#[test]
fn expanded_shortlist_keeps_baseline_and_automatic_identity_separate() {
    let mut words = generated_words(60); words.push("we".into());
    let core = english(&words); let bytes = core.to_binary();
    let request = RankingRequest { english_previous_words: vec!["WE".into()], ..Default::default() };
    let ranked = core.suggest_with_ranking("b", en(), &TypingContext::default(), &request).unwrap();
    assert_eq!(ranked.baseline, core.suggest_with_context("b", en(), &TypingContext::default()).unwrap());
    assert_eq!(ranked.ranking.context_words, ["we"]);
    assert_eq!(order(&ranked).len(), 60);
    assert_eq!(ranked.ranking.apply_order(&order(&ranked)).unwrap(), ranked.baseline.result.candidates);
    let original_auto = ranked.ranking.baseline_automatic_candidate.clone().unwrap();
    let mut reversed = order(&ranked); reversed.reverse();
    let display = ranked.ranking.apply_order(&reversed).unwrap();
    assert_eq!(display.len(), 20);
    assert_ne!(display[0], original_auto);
    assert_eq!(ranked.ranking.baseline_automatic_candidate.as_ref(), Some(&original_auto));
    assert_eq!(core.to_binary(), bytes);
}

#[test]
fn bad_permutations_and_forged_candidate_ids_fall_back() {
    let core = english(&generated_words(30));
    let result = core.suggest_with_ranking("b", en(), &Default::default(), &Default::default()).unwrap();
    let ids = order(&result);
    assert!(result.ranking.apply_order(&ids[..ids.len()-1]).is_none());
    let mut invalid = ids.clone(); invalid[0] = u32::MAX;
    assert!(result.ranking.apply_order(&invalid).is_none());
    invalid[0] = invalid[1]; assert!(result.ranking.apply_order(&invalid).is_none());
}

#[test]
fn exact_english_and_contextual_word_choices_keep_their_slots() {
    let core = english(&["its","itself","book","books","bookcase"].map(str::to_owned));
    let result = core.suggest_with_ranking("book", en(), &Default::default(), &Default::default()).unwrap();
    let mut reversed = order(&result); reversed.reverse();
    assert_eq!(output(&result.ranking.apply_order(&reversed).unwrap()[0]), "book");
    let result = core.suggest_with_ranking("its", en(), &Default::default(), &Default::default()).unwrap();
    assert_eq!(output(result.ranking.baseline_automatic_candidate.as_ref().unwrap()), "its");
    let manual = result.ranking.groups.iter().find(|group| group.candidates.iter().any(|row|
        matches!(&row.suggestion.candidate, SuggestionCandidate::English(word) if word.word_alternative))).unwrap();
    assert!(!manual.reorderable);
    assert_eq!(manual.display_slots, 1);
    let mut reversed = order(&result); reversed.reverse();
    let display = result.ranking.apply_order(&reversed).unwrap();
    assert_eq!(output(&display[0]), "its");
    assert!(matches!(&display[1].candidate, SuggestionCandidate::English(word) if word.word_alternative));
}

#[test]
fn unsupported_english_context_never_breaks_baseline_typing() {
    let core = english(&["book","books","we"].map(str::to_owned));
    for previous in [vec!["missing".into()], vec!["ក".into()], vec!["we".into();4]] {
        let request = RankingRequest { english_previous_words: previous, ..Default::default() };
        let result = core.suggest_with_ranking("bo", en(), &Default::default(), &request).unwrap();
        assert_eq!(result.baseline, core.suggest_with_context("bo", en(), &Default::default()).unwrap());
        assert!(result.ranking.context_words.is_empty()); assert!(result.ranking.fallback_reason.is_some());
    }
    // Existing Khmer context validation remains separate and strict.
    assert!(core.suggest_with_ranking("bo", en(), &TypingContext { previous_words: vec!["we".into()], ..Default::default() }, &Default::default()).is_err());
}

#[test]
fn recognized_surfaces_are_context_only_with_literal_identity_and_lemma_guards() {
    let core = english(&["be", "have", "do", "book", "issue"].map(str::to_owned));
    let vocabulary = core.ranking_vocabulary("english").unwrap();
    assert!(!vocabulary.iter().any(|s| s == "is"));
    for surface in ["is", "IS", "am", "are", "was", "were", "has", "does", "books"] {
        let request = RankingRequest { english_previous_words: vec![surface.into()], ..Default::default() };
        let result = core.suggest_with_ranking("issu", en(), &Default::default(), &request).unwrap();
        assert_eq!(result.ranking.context_words, [surface.to_ascii_lowercase()]);
        assert_eq!(result.baseline, core.suggest_with_context("issu", en(), &Default::default()).unwrap());
        assert_eq!(core.ranking_vocabulary("english").unwrap(), vocabulary);
    }
    let incomplete = english(&["issue"].map(str::to_owned));
    let request = RankingRequest { english_previous_words: vec!["is".into()], ..Default::default() };
    let result = incomplete.suggest_with_ranking("issu", en(), &Default::default(), &request).unwrap();
    assert!(result.ranking.context_words.is_empty());
    assert_eq!(result.ranking.fallback_reason.as_deref(), Some("unknown-english-context"));
    let ranked = core.suggest_with_ranking("is", en(), &Default::default(), &request).unwrap();
    let mut reversed = order(&ranked); reversed.reverse();
    assert_eq!(output(&ranked.ranking.apply_order(&reversed).unwrap()[0]), "is");
    assert_eq!(output(ranked.ranking.baseline_automatic_candidate.as_ref().unwrap()), "is");
    let withheld = core.with_typing_overrides_json(r#"[{"language":"english","word":"is","excluded":true}]"#).unwrap();
    let result = withheld.suggest_with_ranking("issu", en(), &Default::default(), &request).unwrap();
    assert!(result.ranking.context_words.is_empty());
    assert_eq!(result.ranking.fallback_reason.as_deref(), Some("unknown-english-context"));
    assert_eq!(result.baseline, withheld.suggest_with_context("issu", en(), &Default::default()).unwrap());
}

#[test]
fn approved_khmer_context_and_saved_priority_cannot_be_crossed() {
    let core = Engine::from_json(&json!({"schemaVersion":2,"mappings":[
        {"id":1,"romanized":"bat","khmer":"បាទ","rank":0,"status":"approved"},
        {"id":2,"romanized":"bat","khmer":"បត្រ","rank":20,"status":"approved"}],
        "words":[{"id":2,"khmer":"បត្រ","rank":20,"status":"approved"},
        {"id":3,"khmer":"សញ្ញា","rank":0,"status":"approved"},
        {"id":4,"khmer":"សញ្ញាបត្រ","rank":0,"status":"approved"}],
        "structures":[{"wordId":4,"componentIds":[3,2]}]}).to_string()).unwrap();
    let context = TypingContext { previous_words: vec!["សញ្ញា".into()], ..Default::default() };
    let result = core.suggest_with_ranking("bat", Default::default(), &context, &Default::default()).unwrap();
    let mut reversed = order(&result); reversed.reverse();
    assert_eq!(output(&result.ranking.apply_order(&reversed).unwrap()[0]), "បត្រ");
    assert_eq!(result.ranking.baseline_exact_candidate.unwrap().khmer, "បត្រ");
    let source: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/group03-selected-typing.json")).unwrap();
    let core = Engine::from_json(&source["rankControlDataset"].to_string()).unwrap();
    let result = core.suggest_with_ranking("poh", SuggestionFeatures{sound_map:false,..Default::default()}, &Default::default(), &Default::default()).unwrap();
    let mut reversed = order(&result); reversed.reverse();
    let expected = result.baseline.exact_candidates[0].khmer.clone();
    assert_eq!(output(&result.ranking.apply_order(&reversed).unwrap()[0]), expected);
}

#[test]
fn saved_and_generated_exact_paths_remain_distinct_even_when_ranks_are_soft() {
    let core = Engine::from_json(&json!({"schemaVersion":2,
        "mappings":[{"id":1,"romanized":"nov","khmer":"ក","rank":50,"status":"approved"}],
        "words":[{"id":1,"khmer":"ក","rank":50,"status":"approved"},
                 {"id":2,"khmer":"នៅ","rank":0,"status":"approved"}]}).to_string()).unwrap();
    let result = core.suggest_with_ranking("nov",Default::default(),&Default::default(),&Default::default()).unwrap();
    let saved = result.ranking.groups.iter().find(|group| group.candidates.iter().any(|row|output(&row.suggestion)=="ក")).unwrap();
    let generated = result.ranking.groups.iter().find(|group| group.candidates.iter().any(|row|output(&row.suggestion)=="នៅ")).unwrap();
    assert_ne!(saved.id,generated.id);
    let mut reversed = order(&result); reversed.reverse();
    assert_eq!(result.ranking.apply_order(&reversed).unwrap()[0],result.baseline.result.candidates[0]);
}

#[test]
fn recovery_bands_keep_original_slots_when_prefix_pool_expands() {
    let mut core = Engine::from_json(&json!({"schemaVersion":2,
        "mappings":[{"id":1,"romanized":"helloa","khmer":"ក","rank":0,"status":"approved"},
                    {"id":2,"romanized":"hellob","khmer":"ខ","rank":0,"status":"approved"},
                    {"id":3,"romanized":"helz","khmer":"គ","rank":0,"status":"approved"}],
        "words":[{"id":1,"khmer":"ក","rank":0,"status":"approved"},
                 {"id":2,"khmer":"ខ","rank":0,"status":"approved"},
                 {"id":3,"khmer":"គ","rank":0,"status":"approved"}]}).to_string()).unwrap();
    core.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","khmer":[
        {"word":"ក","level":2},{"word":"ខ","level":3},{"word":"គ","level":3}],"english":[]}).to_string()).unwrap();
    let result = core.suggest_with_ranking("hell",SuggestionFeatures{sound_map:false,..Default::default()},&Default::default(),&Default::default()).unwrap();
    assert!(result.baseline.result.candidates.iter().any(|hit|hit.correction.is_some()));
    let before = result.baseline.result.candidates.iter().position(|hit|hit.correction.is_some()).unwrap();
    let mut reversed = order(&result); reversed.reverse();
    let display = result.ranking.apply_order(&reversed).unwrap();
    assert_eq!(display.iter().position(|hit|hit.correction.is_some()),Some(before));
    assert!(result.ranking.groups.iter().filter(|group|group.candidates.iter().any(|row|row.suggestion.correction.is_some())).all(|group|!group.reorderable));
}

#[test]
fn personal_local_ranks_disabled_completion_and_predictions_are_frozen() {
    let core = english(&["book","books","bookcase"].map(str::to_owned));
    let extra = english(&["bookish"].map(str::to_owned));
    let result = core.suggest_with_extension_ranking(&extra,"bo",en(),&Default::default(),&Default::default()).unwrap();
    assert!(result.ranking.groups.iter().all(|group| !group.reorderable));
    let local = core.with_typing_overrides_json(r#"[{"language":"english","word":"books","rank":0}]"#).unwrap();
    let result = local.suggest_with_ranking("bo",en(),&Default::default(),&Default::default()).unwrap();
    assert!(result.ranking.groups.iter().all(|group| !group.reorderable));
    let result = core.suggest_with_ranking("bo",SuggestionFeatures{english_completion:false,..en()},&Default::default(),&Default::default()).unwrap();
    assert!(result.ranking.groups.is_empty());
}

#[test]
fn ranking_vocabulary_is_exact_sorted_and_prior_does_not_load_metadata() {
    let core = english(&["zebra","I","book"].map(str::to_owned));
    assert_eq!(core.ranking_vocabulary("english").unwrap(), ["I","book","zebra"]);
    assert_eq!(core.ranking_word_prior("english","book"), Some((6,2)));
    assert_eq!(core.ranking_word_prior("english","BOOK"), None);
    assert!(core.ranking_vocabulary("unsupported").is_err());
    assert_eq!(core.ranking_word_prior("khmer","book"), None);
}

#[test]
fn large_prefix_scan_falls_back_with_a_bounded_frozen_envelope() {
    let core = english(&generated_words(4100));
    let result = core.suggest_with_ranking("b",en(),&Default::default(),&Default::default()).unwrap();
    assert_eq!(result.ranking.fallback_reason.as_deref(), Some("ranking-scan-budget"));
    assert!(result.ranking.truncated);
    assert!(result.ranking.groups.iter().all(|group| !group.reorderable));
    assert!(order(&result).len() <= 64);
}
