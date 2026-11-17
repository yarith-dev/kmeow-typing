use khmer_engine::{Engine, EnglishWordContext, SuggestionCandidate, SuggestionFeatures, TypingContext, TypingMode};
use serde_json::json;

fn engine(words: &[&str]) -> Engine {
    Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],"englishWords":
        words.iter().enumerate().map(|(id,text)| json!({"id":id+1,"text":text,"rank":0,"status":"approved"})).collect::<Vec<_>>()}).to_string()).unwrap()
}
fn features() -> SuggestionFeatures {
    SuggestionFeatures { mode: TypingMode::English, ..Default::default() }
}
fn context(before: &str, after: &str) -> TypingContext {
    TypingContext { english_word_context: Some(EnglishWordContext { before: before.into(), after: after.into() }), ..Default::default() }
}
fn text(candidate: &khmer_engine::Suggestion) -> &str {
    match &candidate.candidate { SuggestionCandidate::English(word) => &word.text, _ => panic!() }
}

#[test]
fn sentence_starts_subjects_and_existing_following_text_rank_word_choices() {
    let core = engine(&["lets", "its", "your", "were", "well", "ill", "shell", "shed", "id", "wed"]);
    let original = core.to_binary();
    for (input, before, after, expected) in [
        ("lets", "", "", "let's"), ("Lets", "", "", "let's"),
        ("lets", "She ", "", "lets"), ("let's", "She ", "", "lets"),
        ("lets", "She. ", "", "let's"), ("lets", "Done! ", "", "let's"),
        ("lets", "Earlier\n", "", "let's"), ("lets", "please ", "", "let's"),
        ("lets", "My sister ", "", "lets"),
        ("lets", "She now ", "", "lets"), ("lets", "My sister then ", "", "lets"),
        ("lets", "Okay, now ", "", "let's"), ("lets", "Done. Now ", "", "let's"),
        ("its", "", " raining", "it's"), ("it's", "", " color", "its"),
        ("your", "", " welcome", "you're"), ("you're", "", " cat", "your"),
        ("were", "They ", "", "were"), ("were", "", " ready", "we're"),
        ("ill", "I feel ", "", "ill"), ("ill", "", " go", "I'll"),
        ("well", "very ", "", "well"), ("well", "", " go", "we'll"),
    ] {
        let result = core.suggest_with_context(input, features(), &context(before, after)).unwrap();
        assert_eq!(text(&result.result.candidates[0]), expected, "{before}{input}|{after}");
        assert!(result.result.candidates.iter().take(2).all(|hit|
            matches!(&hit.candidate, SuggestionCandidate::English(word) if word.word_alternative && !word.manual_only)));
        let ranked = core.suggest_with_ranking(input, features(), &context(before, after), &Default::default()).unwrap();
        assert_eq!(text(ranked.ranking.baseline_automatic_candidate.as_ref().unwrap()), expected);
    }
    assert_eq!(original, core.to_binary());
}

#[test]
fn unclear_context_keeps_typed_spelling_and_personal_exact_words() {
    let core = engine(&["lets", "its"]);
    assert_eq!(text(&core.suggest_with_features("lets", features()).unwrap().candidates[0]), "lets");
    for input in ["its", "it's"] {
        assert_eq!(text(&core.suggest_with_context(input, features(), &context("", "")).unwrap().result.candidates[0]), input);
    }
    let personal = engine(&["lets"]);
    let merged = core.suggest_with_extension(&personal, "lets", features(), &context("", "")).unwrap();
    assert_eq!(text(&merged.result.candidates[0]), "lets");
    let clean = engine(&["other"]);
    let result = clean.suggest_with_context("she'll", features(), &context("the ", "")).unwrap();
    assert!(!result.result.candidates.iter().any(|hit| text(hit) == "shell"));
    let result = clean.suggest_with_context("let's", features(), &context("She ", "")).unwrap();
    assert_eq!(text(&result.result.candidates[0]), "lets");
    let result = engine(&["image"]).suggest_with_context("im", SuggestionFeatures {
        english_autocorrection: true, ..features() }, &context("", "")).unwrap();
    assert_eq!(text(&result.result.candidates[0]), "I'm");
}

#[test]
fn invalid_windows_and_disabled_completion_cannot_apply_word_choices() {
    let core = engine(&["lets"]);
    assert!(core.suggest_with_context("lets", features(), &context(&"x".repeat(513), "")).is_err());
    assert!(core.suggest_with_context("lets", features(), &context("", &"🙂".repeat(129))).is_err());
    let result = core.suggest_with_context("lets", SuggestionFeatures { english_completion: false,
        english_autocorrection: true, ..features() }, &context("", "")).unwrap();
    assert!(result.result.candidates.is_empty());
}
