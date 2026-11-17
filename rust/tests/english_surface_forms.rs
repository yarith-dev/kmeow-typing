use khmer_engine::{Engine, EnglishWordContext, RankingSuggestionResult, Suggestion,
    SuggestionCandidate, SuggestionFeatures, TypingContext, TypingMode};
use serde_json::json;

fn engine(words: &[(&str, u64, u32)]) -> Engine {
    Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],"englishWords":
        words.iter().map(|(text,id,rank)| json!({"id":id,"text":text,"rank":rank,
            "status":"approved"})).collect::<Vec<_>>()}).to_string()).unwrap()
}
fn features(completion: bool, correction: bool) -> SuggestionFeatures {
    SuggestionFeatures { mode: TypingMode::English, english_completion: completion,
        english_autocorrection: correction, ..Default::default() }
}
fn context(before: &str, after: &str) -> TypingContext {
    TypingContext { english_word_context: Some(EnglishWordContext {
        before: before.into(), after: after.into() }), ..Default::default() }
}
fn english(hit: &Suggestion) -> &khmer_engine::EnglishCandidate {
    match &hit.candidate { SuggestionCandidate::English(word) => word, _ => panic!("English fixture") }
}
fn preserved(hit: &Suggestion, input: &str) {
    let word = english(hit);
    assert_eq!(word.text, input);
    assert_eq!(word.mapping_id, 0);
    assert_eq!(word.rank, 0);
    assert!(!word.manual_only && !word.word_alternative);
    assert_eq!(hit.completion.as_deref(), Some(input));
    assert!(hit.correction.is_none());
}
fn ids(result: &RankingSuggestionResult) -> Vec<u32> {
    result.ranking.groups.iter().flat_map(|group| group.candidates.iter().map(|row| row.id)).collect()
}

#[test]
fn is_stays_literal_with_exact_case_while_issue_remains_a_completion() {
    // Actual shipped data has be + issue/island and no standalone is.
    let core = engine(&[("be", 1, 90), ("issue", 2637, 0), ("island", 2633, 0)]);
    let bytes = core.to_binary();
    for input in ["is", "Is", "IS", "iS"] {
        for correction in [false, true] {
            let settings = features(true, correction);
            let plain = core.suggest_with_features(input, settings).unwrap();
            preserved(&plain.candidates[0], input);
            let contextual = core.suggest_with_context(input, settings, &context("this ", "")).unwrap();
            preserved(&contextual.result.candidates[0], input);
            assert!(contextual.result.candidates.iter().any(|hit|
                english(hit).mapping_id == 2637 && english(hit).text == "issue"));
            assert!(contextual.result.candidates.len() <= 20);
        }
    }
    let prefix = core.suggest_with_features("issu", features(true, true)).unwrap();
    assert_eq!(english(&prefix.candidates[0]).text, "issue");
    assert_ne!(english(&prefix.candidates[0]).mapping_id, 0);
    assert_eq!(core.to_binary(), bytes);
}

#[test]
fn selected_families_preserve_complete_forms_without_productive_guessing() {
    let families: &[(&str, &[&str])] = &[
        ("be", &["am", "are", "is", "was", "been", "being"]),
        ("have", &["has", "had", "having"]), ("do", &["does", "did", "done", "doing"]),
        ("go", &["goes", "went", "gone", "going"]), ("get", &["gets", "got", "gotten", "getting"]),
        ("come", &["comes", "came", "coming"]), ("give", &["gives", "gave", "given", "giving"]),
        ("take", &["takes", "took", "taken", "taking"]), ("know", &["knows", "knew", "known", "knowing"]),
        ("tell", &["tells", "told", "telling"]), ("make", &["makes", "made", "making"]),
        ("say", &["says", "said", "saying"]), ("think", &["thinks", "thought", "thinking"]),
        ("book", &["books"]), ("cat", &["cats"]), ("dog", &["dogs"]),
        ("issue", &["issues", "issued", "issuing"]),
    ];
    for (lemma, forms) in families {
        let core = engine(&[(lemma, 1, 99)]);
        let bytes = core.to_binary();
        for input in *forms {
            for correction in [false, true] {
                let result = core.suggest_with_context(input, features(true, correction), &context("this ", "")).unwrap();
                preserved(&result.result.candidates[0], input);
                assert!(core.lookup_word_metadata_json("english", input).unwrap() == "[]");
                assert_eq!(core.ranking_word_prior("english", input), None);
            }
        }
        assert_eq!(core.ranking_vocabulary("english").unwrap(), [lemma.to_string()]);
        assert_eq!(core.to_binary(), bytes);
    }
    let core = engine(&[("make", 1, 0), ("book", 2, 0), ("be", 3, 0)]);
    for input in ["maked", "booker", "beb", "arex", "i", "is ", " is", "two words"] {
        assert!(!core.suggest_with_features(input, features(true, false)).unwrap().candidates.iter()
            .any(|hit| english(hit).mapping_id == 0), "unselected {input}");
    }
}

#[test]
fn source_backed_common_inflections_keep_complete_text_and_case_without_metadata() {
    // These independent grammatical examples cover plural spelling changes,
    // regular/irregular verb forms and the reported noun/verb coverage gaps.
    // Already stored words such as running/writing/mice are deliberately not
    // regenerated from their base; ordinary dictionary identities own them.
    let examples = [
        ("word", "words"), ("child", "children"), ("type", "types"),
        ("form", "forms"), ("case", "cases"), ("write", "wrote"),
        ("write", "writes"), ("run", "ran"), ("run", "runs"),
        ("want", "wanted"), ("want", "wants"), ("walk", "walked"),
        ("watch", "watches"), ("study", "studied"), ("study", "studying"),
        ("carry", "carried"), ("move", "moved"), ("house", "houses"),
        ("bus", "buses"), ("work", "worked"), ("work", "works"),
        ("try", "tried"), ("try", "tries"), ("travel", "traveled"),
        ("travel", "travelled"), ("baby", "babies"), ("city", "cities"),
    ];
    for (base, form) in examples {
        let core = engine(&[(base, 11, 97), ("wordsmith", 12, 0)]);
        let bytes = core.to_binary();
        for input in [form.to_owned(), form.to_ascii_uppercase(),
            format!("{}{}", form[..1].to_ascii_uppercase(), &form[1..])] {
            for correction in [false, true] {
                let result = core.suggest_with_context(&input, features(true, correction),
                    &context("these ", "")).unwrap();
                preserved(&result.result.candidates[0], &input);
            }
            assert!(core.suggest_with_features(&input, features(false, true)).unwrap()
                .candidates.is_empty(), "spelling-only {input}");
            assert!(core.suggest_with_features(&input, features(false, false)).unwrap()
                .candidates.is_empty());
            assert_eq!(core.lookup_word_metadata_json("english", &input).unwrap(), "[]");
            assert_eq!(core.ranking_word_prior("english", &input), None);
        }
        assert_eq!(core.to_binary(), bytes);
        assert_eq!(core.ranking_vocabulary("english").unwrap(),
            if base < "wordsmith" { vec![base.to_string(), "wordsmith".into()] }
            else { vec!["wordsmith".into(), base.to_string()] });
    }
}

#[test]
fn broader_guard_does_not_expand_prefixes_or_accept_productive_guesses() {
    let core = engine(&[("word", 1, 0), ("wordsmith", 2, 0), ("book", 3, 0),
        ("write", 4, 0), ("queen", 5, 0), ("have", 6, 0), ("study", 7, 0),
        ("be", 8, 0), ("know", 9, 0), ("read", 10, 0)]);
    for input in ["wor", "childr", "wrot", "writ", "wordss", "wordes", "bookes",
        "knowed", "readed", "havings", "studyingx", "queened", "isn", "ve", "hafta",
        "words ", " words", "words!", "word1", "two words", "wørds"] {
        let result = core.suggest_with_features(input, features(true, false)).unwrap();
        assert!(result.candidates.iter().all(|hit| english(hit).mapping_id != 0), "{input}");
    }
    let prefix = core.suggest_with_features("wordsm", features(true, true)).unwrap();
    assert_eq!((english(&prefix.candidates[0]).text.as_str(),
        english(&prefix.candidates[0]).mapping_id), ("wordsmith", 2));
    let result = core.suggest_with_features("words", features(true, true)).unwrap();
    preserved(&result.candidates[0], "words");
    assert!(result.candidates.iter().any(|hit| english(hit).mapping_id == 2));
    assert_eq!(core.lookup_word_metadata_json("english", "words").unwrap(), "[]");
    // Source identity ownership is intentional: no duplicate generated rows for
    // these already published spellings if a small test lexicon omits them.
    let core = engine(&[("run", 1, 0), ("write", 2, 0), ("mouse", 3, 0)]);
    for input in ["running", "writing", "mice"] {
        assert!(core.suggest_with_features(input, features(true, false)).unwrap().candidates
            .iter().all(|hit| english(hit).mapping_id != 0), "stored spelling {input}");
    }
}

#[test]
fn source_form_and_base_exclusions_apply_across_both_runtime_views() {
    for (base_word, form) in [("be", "is"), ("word", "words"), ("child", "children"), ("write", "wrote")] {
        let base = engine(&[(base_word, 1, 0), ("wordsmith", 2, 0), ("issue", 3, 0)]);
        let other = engine(&[("other", 4, 0)]);
        let source_bytes = (base.to_binary(), other.to_binary());
        for excluded in [base_word, form] {
            let overrides = json!([{"language":"english","word":excluded,"excluded":true}]).to_string();
            let blocked = other.with_typing_overrides_json(&overrides).unwrap();
            for result in [base.suggest_with_extension(&blocked, form, features(true, true), &Default::default()).unwrap(),
                blocked.suggest_with_extension(&base, form, features(true, true), &Default::default()).unwrap()] {
                assert!(result.result.candidates.iter().all(|hit|
                    !english(hit).text.eq_ignore_ascii_case(form)), "{excluded} must withhold {form}");
            }
            let reset = blocked.with_typing_overrides_json("[]").unwrap();
            preserved(&base.suggest_with_extension(&reset, form, features(true, true),
                &Default::default()).unwrap().result.candidates[0], form);
        }
        assert_eq!((base.to_binary(),other.to_binary()),source_bytes);
    }
}

#[test]
fn multiple_documented_bases_require_at_least_one_available_unexcluded_support() {
    // bases can be the plural of base or basis; blocking one never authorizes
    // the other, and an absent second base cannot bypass the first exclusion.
    let base = engine(&[("base", 1, 0)]);
    let basis = engine(&[("basis", 2, 0)]);
    let other = engine(&[("other", 3, 0)]);
    let block_base = r#"[{"language":"english","word":"base","excluded":true}]"#;
    let block_basis = r#"[{"language":"english","word":"basis","excluded":true}]"#;
    let blocked = other.with_typing_overrides_json(block_base).unwrap();
    let no_support = base.suggest_with_extension(&blocked, "bases", features(true,true),
        &Default::default()).unwrap();
    assert!(no_support.result.candidates.iter().all(|hit| english(hit).text != "bases"));
    preserved(&basis.suggest_with_extension(&blocked, "bases", features(true,true),
        &Default::default()).unwrap().result.candidates[0], "bases");
    let blocked_basis = basis.with_typing_overrides_json(block_basis).unwrap();
    let no_support = blocked_basis.suggest_with_extension(&blocked, "bases", features(true,true),
        &Default::default()).unwrap();
    assert!(no_support.result.candidates.iter().all(|hit| english(hit).text != "bases"));
}

#[test]
fn broadened_guard_preserves_personal_exact_identities_and_contextual_lets_choice() {
    let base = engine(&[("word", 1, 0), ("wordsmith", 2, 0), ("let", 3, 0)]);
    let personal = engine(&[("words", 9001, 72)]);
    for input in ["words", "Words", "WORDS"] {
        let result = base.suggest_with_extension(&personal, input, features(true,true),
            &Default::default()).unwrap();
        let first = english(&result.result.candidates[0]);
        assert_eq!((first.text.as_str(),first.mapping_id,first.rank), ("words",9001,72));
        assert_eq!(result.result.candidates.iter().filter(|hit|
            english(hit).text.eq_ignore_ascii_case("words")).count(), 1);
    }
    for (before, after, expected) in [("", " go", "let's"), ("he ", " go", "lets"),
        ("this ", "", "lets")] {
        let result = base.suggest_with_context("lets",features(true,true),&context(before,after)).unwrap();
        assert_eq!(english(&result.result.candidates[0]).text,expected);
        assert!(english(&result.result.candidates[0]).word_alternative);
    }
}

#[test]
fn broader_complete_form_is_frozen_while_longer_completions_remain_tappable() {
    let core = engine(&[("word", 1, 99), ("wordsmith", 2, 0), ("wordsmiths", 3, 0)]);
    for input in ["words", "Words", "WORDS"] {
        let result = core.suggest_with_ranking(input, features(true,true),
            &context("these ",""), &Default::default()).unwrap();
        preserved(&result.baseline.result.candidates[0],input);
        preserved(result.ranking.baseline_automatic_candidate.as_ref().unwrap(),input);
        assert!(!result.ranking.groups[0].reorderable);
        assert_eq!(result.ranking.groups[0].display_slots,1);
        assert!(result.ranking.groups[1].reorderable);
        let mut order=ids(&result); order.reverse();
        let reordered=result.ranking.apply_order(&order).unwrap();
        preserved(&reordered[0],input);
        assert!(reordered.iter().any(|hit| english(hit).mapping_id==2));
        assert!(reordered.iter().any(|hit| english(hit).mapping_id==3));
    }
}

#[test]
fn known_forms_avoid_spelling_repairs_with_completion_disabled() {
    let core = engine(&[("be", 1, 0), ("do", 2, 0), ("cat", 3, 0), ("cast", 4, 0),
        ("dog", 5, 0), ("dose", 6, 0), ("book", 7, 0), ("bookshop", 8, 0),
        ("issue", 9, 0), ("have", 10, 0), ("saving", 11, 0), ("take", 12, 0), ("the", 13, 0)]);
    for input in ["is", "Is", "IS", "am", "are", "was", "has", "does", "cats", "dogs", "books", "issues", "having", "takes"] {
        assert!(core.suggest_with_features(input, features(false, true)).unwrap().candidates.is_empty(), "{input}");
        assert!(core.suggest_with_context(input, features(false, true), &context("this ", "")).unwrap().result.candidates.is_empty(), "{input}");
    }
    let typo = core.suggest_with_features("teh", features(false, true)).unwrap();
    assert_eq!(typo.candidates[0].correction.as_deref(), Some("the"));
    let contraction = core.suggest_with_features("im", features(false, true)).unwrap();
    assert_eq!(contraction.candidates[0].correction.as_deref(), Some("I'm"));
}

#[test]
fn an_absent_or_withheld_lemma_does_not_add_a_surface_identity() {
    let core = engine(&[("issue", 2637, 0), ("island", 2633, 0)]);
    let ordinary = core.suggest_with_features("is", features(true, true)).unwrap();
    assert_eq!(english(&ordinary.candidates[0]).text, "issue");
    assert!(ordinary.candidates.iter().all(|hit| english(hit).mapping_id != 0));
    let with_be = engine(&[("be", 1, 0), ("issue", 2637, 0)]);
    let withheld = with_be.with_typing_overrides_json(r#"[{"language":"english","word":"be","excluded":true}]"#).unwrap();
    let result = withheld.suggest_with_features("is", features(true, true)).unwrap();
    assert_eq!(english(&result.candidates[0]).text, "issue");
    assert!(result.candidates.iter().all(|hit| english(hit).mapping_id != 0));
}

#[test]
fn explicit_surface_withholding_blocks_new_identities_in_either_view_and_reset_restores_them() {
    let base = engine(&[("be", 1, 0), ("issue", 2637, 0)]);
    let other = engine(&[("other", 2, 0)]);
    let snapshot = (base.to_binary(), other.to_binary());
    let exclude = r#"[{"language":"english","word":"is","excluded":true}]"#;
    let blocked_base = base.with_typing_overrides_json(exclude).unwrap();
    let blocked_other = other.with_typing_overrides_json(exclude).unwrap();
    for input in ["is", "Is", "IS"] {
        for correction in [false, true] {
            let settings = features(true, correction);
            let direct = blocked_base.suggest_with_context(input, settings, &Default::default()).unwrap();
            assert!(direct.result.candidates.iter().all(|hit| !english(hit).text.eq_ignore_ascii_case("is")));
            for result in [base.suggest_with_extension(&blocked_other, input, settings, &Default::default()).unwrap(),
                blocked_other.suggest_with_extension(&base, input, settings, &Default::default()).unwrap(),
                blocked_base.suggest_with_extension(&base, input, settings, &Default::default()).unwrap()] {
                assert!(result.result.candidates.iter().all(|hit| !english(hit).text.eq_ignore_ascii_case("is")));
            }
            for reset in [blocked_base.with_typing_overrides_json("[]").unwrap(),
                blocked_base.with_typing_overrides_json(r#"[{"language":"english","word":"is","excluded":false}]"#).unwrap()] {
                preserved(&reset.suggest_with_features(input, settings).unwrap().candidates[0], input);
            }
            let reset_other = blocked_other.with_typing_overrides_json("[]").unwrap();
            preserved(&base.suggest_with_extension(&reset_other, input, settings, &Default::default()).unwrap().result.candidates[0], input);
        }
    }
    assert_eq!((base.to_binary(), other.to_binary()), snapshot);
    assert_eq!(blocked_base.to_binary(), snapshot.0);
    assert_eq!(blocked_other.to_binary(), snapshot.1);
}

#[test]
fn real_exact_and_personal_entries_keep_their_identity_and_rank() {
    let base = engine(&[("be", 1, 90), ("issue", 2, 0)]);
    let exact = engine(&[("is", 9001, 77)]);
    let bundled = engine(&[("be", 1, 90), ("issue", 2, 0), ("is", 7, 88)]);
    let snapshots = (base.to_binary(), exact.to_binary(), bundled.to_binary());
    for input in ["is", "Is", "IS"] {
        for correction in [false, true] {
            let settings = features(true, correction);
            let result = base.suggest_with_extension(&exact, input, settings, &context("this ", "")).unwrap();
            let word = english(&result.result.candidates[0]);
            assert_eq!((word.text.as_str(), word.mapping_id, word.rank), ("is", 9001, 77));
            assert_eq!(result.result.candidates.iter().filter(|hit| english(hit).text.eq_ignore_ascii_case("is")).count(), 1);
            for result in [bundled.suggest_with_context(input, settings, &Default::default()).unwrap(),
                bundled.suggest_with_extension(&base, input, settings, &Default::default()).unwrap(),
                bundled.suggest_with_extension(&exact, input, settings, &Default::default()).unwrap()] {
                let word = english(&result.result.candidates[0]);
                assert_eq!((word.mapping_id, word.rank), (7, 88));
                assert!(result.result.candidates.iter().all(|hit| !english(hit).text.eq_ignore_ascii_case("is") || english(hit).mapping_id != 0));
            }
            assert!(base.suggest_with_extension(&exact, input, features(false, true), &Default::default()).unwrap().result.candidates.is_empty());
        }
    }
    assert_eq!((base.to_binary(), exact.to_binary(), bundled.to_binary()), snapshots);
}

#[test]
fn an_extension_lemma_supplies_only_a_typed_identity_and_ranking_cannot_move_it() {
    let base = engine(&[("issue", 2637, 0), ("island", 2633, 0)]);
    let lemma = engine(&[("be", 1, 99)]);
    let combined = engine(&[("be", 1, 99), ("issue", 2637, 0), ("island", 2633, 0)]);
    let context = context("this ", "");
    for input in ["is", "Is", "IS"] {
        let ranked = combined.suggest_with_ranking(input, features(true, true), &context, &Default::default()).unwrap();
        preserved(&ranked.baseline.result.candidates[0], input);
        preserved(ranked.ranking.baseline_automatic_candidate.as_ref().unwrap(), input);
        assert!(!ranked.ranking.groups[0].reorderable);
        assert_eq!(ranked.ranking.groups[0].display_slots, 1);
        assert!(ranked.ranking.groups[1].reorderable);
        let mut reversed = ids(&ranked); reversed.reverse();
        preserved(&ranked.ranking.apply_order(&reversed).unwrap()[0], input);
        let merged = base.suggest_with_extension_ranking(&lemma, input, features(true, true), &context, &Default::default()).unwrap();
        preserved(&merged.baseline.result.candidates[0], input);
        preserved(merged.ranking.baseline_automatic_candidate.as_ref().unwrap(), input);
        assert!(merged.ranking.groups.iter().all(|group| !group.reorderable));
        assert!(merged.baseline.result.candidates.iter().any(|hit| english(hit).text == "issue"));
    }
}

#[test]
fn existing_were_homograph_context_and_personal_choice_stay_authoritative() {
    let core = engine(&[("be", 1, 0)]);
    let result = core.suggest_with_context("were", features(true, true), &context("", " ready")).unwrap();
    assert_eq!(english(&result.result.candidates[0]).text, "we're");
    assert!(english(&result.result.candidates[0]).word_alternative);
    let personal = engine(&[("were", 9001, 9)]);
    let result = core.suggest_with_extension(&personal, "were", features(true, true), &context("", " ready")).unwrap();
    let word = english(&result.result.candidates[0]);
    assert_eq!((word.text.as_str(), word.mapping_id, word.rank), ("were", 9001, 9));
    assert!(word.word_alternative);
}
