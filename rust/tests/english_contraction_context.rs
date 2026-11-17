//! Literal-context restraint for finite English typing rules, without lexical writes.
use khmer_engine::{
    Engine, EnglishWordContext, Suggestion, SuggestionCandidate, SuggestionFeatures, TypingContext,
    TypingMode,
};
use serde_json::json;

fn engine(words: &[&str]) -> Engine {
    Engine::from_json(
        &json!({"schemaVersion":3,"mappings":[],"words":[],"englishWords":
            words.iter().enumerate().map(|(id, text)|
                json!({"id":id+1,"text":text,"rank":id,"status":"approved"}))
                .collect::<Vec<_>>()})
        .to_string(),
    )
    .unwrap()
}

fn features(completion: bool, correction: bool) -> SuggestionFeatures {
    SuggestionFeatures {
        mode: TypingMode::English,
        english_completion: completion,
        english_autocorrection: correction,
        ..Default::default()
    }
}

fn context(before: &str, after: &str) -> TypingContext {
    TypingContext {
        english_word_context: Some(EnglishWordContext {
            before: before.into(),
            after: after.into(),
        }),
        ..Default::default()
    }
}

fn text(hit: &Suggestion) -> &str {
    match &hit.candidate {
        SuggestionCandidate::English(word) => &word.text,
        _ => panic!("English fixture returned another language"),
    }
}

fn literal_pair(hits: &[Suggestion], input: &str, contraction: &str) {
    assert!(hits.len() >= 2, "{input}: {hits:?}");
    assert_eq!(text(&hits[0]), input, "literal spelling/case must survive");
    assert!(
        hits[0].correction.is_none(),
        "the literal is not a correction"
    );
    assert_eq!(text(&hits[1]), contraction);
    assert_eq!(hits[1].correction.as_deref(), Some(contraction));
    assert!(hits[..2].iter().all(|hit| matches!(&hit.candidate,
        SuggestionCandidate::English(word) if word.word_alternative && !word.manual_only)));
}

#[test]
fn explicit_literal_name_code_and_acronym_cues_keep_original_first() {
    let core = engine(&["image", "other"]);
    let bytes = core.to_binary();
    for completion in [false, true] {
        for (input, before, after, contraction) in [
            ("im", "Use the abbreviation ", "", "I'm"),
            ("im", "The token \"", "\"", "I'm"),
            ("Im", "Name: ", "", "I'm"),
            ("Im", "Mr. ", "", "I'm"),
            ("Ive", "Name: ", "", "I've"),
            ("dont", "variable = ", "", "don't"),
            ("dont", "```\nvalue = ", "", "don't"),
            ("ive", "Use `", "`", "I've"),
            ("IM", "", "", "I'm"),
            ("DONT", "The token \"", "\"", "don't"),
        ] {
            let result = core
                .suggest_with_context(input, features(completion, true), &context(before, after))
                .unwrap();
            literal_pair(&result.result.candidates, input, contraction);
        }
    }
    assert_eq!(core.to_binary(), bytes);
}

#[test]
fn ordinary_prose_dialogue_and_finished_sentences_keep_common_corrections() {
    let core = engine(&["image", "other"]);
    let bytes = core.to_binary();
    for completion in [false, true] {
        for (input, before, after, expected) in [
            ("im", "", "", "I'm"),
            ("Im", "", " tired", "I'm"),
            ("dont", "I ", " know", "don't"),
            ("ive", "I think ", " been", "I've"),
            ("im", "She said, \"", " tired\"", "I'm"),
            ("im", "That is a word. ", "", "I'm"),
            ("im", "maybe ", "", "I'm"),
        ] {
            let result = core
                .suggest_with_context(input, features(completion, true), &context(before, after))
                .unwrap();
            assert_eq!(
                text(&result.result.candidates[0]),
                expected,
                "{before}{input}|{after}"
            );
            assert_eq!(
                result.result.candidates[0].correction.as_deref(),
                Some(expected)
            );
        }
    }
    assert_eq!(core.to_binary(), bytes);
}

#[test]
fn literal_labels_also_prevent_contextual_homograph_reinterpretation() {
    let core = engine(&["lets", "its", "your", "were"]);
    let bytes = core.to_binary();
    for (input, before, after, alternative) in [
        ("lets", "The token \"", "\"", "let's"),
        ("Lets", "Name: ", "", "let's"),
        ("its", "The token \"", " raining", "it's"),
        ("your", "The token \"", " welcome", "you're"),
        ("it's", "The token \"", " color", "its"),
        ("Let's", "The token \"", "\"", "lets"),
    ] {
        let result = core
            .suggest_with_context(input, features(true, true), &context(before, after))
            .unwrap();
        assert_eq!(
            text(&result.result.candidates[0]),
            input,
            "{before}{input}|{after}"
        );
        assert!(result.result.candidates[0].correction.is_none());
        assert!(
            result
                .result
                .candidates
                .iter()
                .any(|hit| text(hit).eq_ignore_ascii_case(alternative)),
            "tap alternative remains available"
        );
    }
    assert_eq!(core.to_binary(), bytes);
}

#[test]
fn explicitly_typed_apostrophes_are_never_removed_in_literal_contexts() {
    let core = engine(&["image", "other"]);
    let bytes = core.to_binary();
    for completion in [false, true] {
        for input in ["I'm", "I've", "don't", "DON'T"] {
            let result = core
                .suggest_with_context(
                    input,
                    features(completion, true),
                    &context("The token \"", "\""),
                )
                .unwrap();
            assert!(
                result
                    .result
                    .candidates
                    .iter()
                    .all(|hit| hit.correction.is_none()),
                "{input}"
            );
            assert!(
                !result
                    .result
                    .candidates
                    .iter()
                    .any(|hit| text(hit).eq_ignore_ascii_case(&input.replace('\'', ""))),
                "{input}"
            );
        }
    }
    assert_eq!(core.to_binary(), bytes);
}

#[test]
fn disabled_correction_does_not_create_finite_contraction_fixes() {
    let core = engine(&["image", "other"]);
    let bytes = core.to_binary();
    for completion in [false, true] {
        for (input, before) in [
            ("im", ""),
            ("im", "The token \""),
            ("IM", ""),
            ("dont", "I "),
            ("ive", "Name: "),
        ] {
            let result = core
                .suggest_with_context(input, features(completion, false), &context(before, ""))
                .unwrap();
            assert!(result
                .result
                .candidates
                .iter()
                .all(|hit| hit.correction.is_none()));
            assert!(!result.result.candidates.iter().any(
                |hit| matches!(&hit.candidate, SuggestionCandidate::English(word)
                    if word.mapping_id == 0 && word.text.contains('\''))
            ));
        }
    }
    assert_eq!(core.to_binary(), bytes);
}

#[test]
fn personal_exact_spellings_win_while_longer_prefixes_share_the_context_guard() {
    let core = engine(&["other"]);
    let exact = engine(&["im"]);
    let longer = engine(&["image"]);
    let bytes = (core.to_binary(), exact.to_binary(), longer.to_binary());
    for completion in [false, true] {
        for before in ["", "The token \""] {
            let result = core
                .suggest_with_extension(
                    &exact,
                    "im",
                    features(completion, true),
                    &context(before, ""),
                )
                .unwrap();
            assert!(result
                .result
                .candidates
                .iter()
                .all(|hit| hit.correction.is_none()));
            assert!(!result
                .result
                .candidates
                .iter()
                .any(|hit| text(hit) == "I'm"));
        }
        let literal = core
            .suggest_with_extension(
                &longer,
                "im",
                features(completion, true),
                &context("The token \"", "\""),
            )
            .unwrap();
        literal_pair(&literal.result.candidates, "im", "I'm");
        let prose = core
            .suggest_with_extension(&longer, "im", features(completion, true), &context("", ""))
            .unwrap();
        assert_eq!(
            prose.result.candidates[0].correction.as_deref(),
            Some("I'm")
        );
    }
    assert_eq!(
        (core.to_binary(), exact.to_binary(), longer.to_binary()),
        bytes
    );
}

#[test]
fn literal_identity_stays_the_automatic_baseline_with_and_without_personal_prefixes() {
    let core = engine(&["image", "other"]);
    let extra = engine(&["imagination"]);
    let bytes = (core.to_binary(), extra.to_binary());
    for completion in [false, true] {
        for (input, before, contraction) in [
            ("im", "The token \"", "I'm"),
            ("IM", "", "I'm"),
            ("dont", "variable = ", "don't"),
        ] {
            let ctx = context(before, "");
            let result = core
                .suggest_with_ranking(input, features(completion, true), &ctx, &Default::default())
                .unwrap();
            literal_pair(&result.baseline.result.candidates, input, contraction);
            assert_eq!(
                text(
                    result
                        .ranking
                        .baseline_automatic_candidate
                        .as_ref()
                        .unwrap()
                ),
                input
            );
            let merged = core
                .suggest_with_extension_ranking(
                    &extra,
                    input,
                    features(completion, true),
                    &ctx,
                    &Default::default(),
                )
                .unwrap();
            literal_pair(&merged.baseline.result.candidates, input, contraction);
            assert_eq!(
                text(
                    merged
                        .ranking
                        .baseline_automatic_candidate
                        .as_ref()
                        .unwrap()
                ),
                input
            );
        }
    }
    assert_eq!((core.to_binary(), extra.to_binary()), bytes);
}

#[test]
fn literal_homographs_retain_stored_and_personal_identity_and_rank() {
    // Exercise a bundled identity and a personal-only identity separately;
    // duplicate spellings intentionally keep their existing base identity.
    let core = engine(&["other", "lets"]);
    let personal_base = engine(&["other"]);
    let personal = engine(&["extra", "more", "lets"]);
    let bytes = (
        core.to_binary(),
        personal_base.to_binary(),
        personal.to_binary(),
    );
    let ordinary = core
        .suggest_with_context("lets", features(true, true), &Default::default())
        .unwrap();
    let merged = personal_base
        .suggest_with_extension(&personal, "lets", features(true, true), &Default::default())
        .unwrap();
    let identity = |hit: &Suggestion| match &hit.candidate {
        SuggestionCandidate::English(word) => (word.mapping_id, word.rank),
        _ => panic!("English fixture returned another language"),
    };
    assert_eq!(text(&ordinary.result.candidates[0]), "lets");
    assert_eq!(text(&merged.result.candidates[0]), "lets");
    assert_ne!(
        identity(&ordinary.result.candidates[0]),
        identity(&merged.result.candidates[0])
    );
    for input in ["lets", "Lets", "LETS"] {
        let literal = core
            .suggest_with_context(input, features(true, true), &context("The token \"", "\""))
            .unwrap();
        assert_eq!(text(&literal.result.candidates[0]), input);
        assert_eq!(
            identity(&literal.result.candidates[0]),
            identity(&ordinary.result.candidates[0])
        );
        let personal_literal = personal_base
            .suggest_with_extension(
                &personal,
                input,
                features(true, true),
                &context("The token \"", "\""),
            )
            .unwrap();
        assert_eq!(text(&personal_literal.result.candidates[0]), input);
        assert_eq!(
            identity(&personal_literal.result.candidates[0]),
            identity(&merged.result.candidates[0])
        );
    }
    assert_eq!(
        (
            core.to_binary(),
            personal_base.to_binary(),
            personal.to_binary()
        ),
        bytes
    );
}
