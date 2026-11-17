//! Conservative English correction using screened vocabulary and finite
//! missing-apostrophe typing rules.
//! Exact words stay literal. A finite missing-apostrophe rule precedes prefix
//! protection; uncertain edits retain the original first and offer screened
//! alternatives for explicit selection.
use crate::{
    compact, contains_profanity, Engine, EnglishCandidate, ProfanityLanguage, Suggestion,
    SuggestionCandidate,
};
use std::collections::BTreeSet;

// Orthographic typing rules, not dictionary entries or spelling approvals.
// Never infer these from arbitrary suffixes: ill/well/were/its/lets, for example,
// are valid words whose intended meaning requires sentence context.
fn contraction(key: &str) -> Option<&'static str> {
    Some(match key {
        "im" => "I'm",
        "ive" => "I've",
        "youre" => "you're",
        "youve" => "you've",
        "youll" => "you'll",
        "youd" => "you'd",
        "theyre" => "they're",
        "theyve" => "they've",
        "theyll" => "they'll",
        "theyd" => "they'd",
        "weve" => "we've",
        "hes" => "he's",
        "shes" => "she's",
        "thats" => "that's",
        "theres" => "there's",
        "heres" => "here's",
        "whats" => "what's",
        "whos" => "who's",
        "wheres" => "where's",
        "whens" => "when's",
        "whys" => "why's",
        "hows" => "how's",
        "dont" => "don't",
        "doesnt" => "doesn't",
        "didnt" => "didn't",
        "isnt" => "isn't",
        "arent" => "aren't",
        "wasnt" => "wasn't",
        "werent" => "weren't",
        "havent" => "haven't",
        "hasnt" => "hasn't",
        "hadnt" => "hadn't",
        "cant" => "can't",
        "wont" => "won't",
        "couldnt" => "couldn't",
        "shouldnt" => "shouldn't",
        "wouldnt" => "wouldn't",
        "mustnt" => "mustn't",
        "neednt" => "needn't",
        "mightnt" => "mightn't",
        _ => return None,
    })
}

pub(crate) fn manual_alternative(key: &str) -> Option<&'static str> {
    Some(match key {
        "its" => "it's",
        "it's" => "its",
        "your" => "you're",
        "you're" => "your",
        "were" => "we're",
        "we're" => "were",
        "well" => "we'll",
        "ill" => "I'll",
        "id" => "I'd",
        "wed" => "we'd",
        "hell" => "he'll",
        "shell" => "she'll",
        "shed" => "she'd",
        "lets" => "let's",
        "let's" => "lets",
        "we'll" => "well",
        "i'll" => "ill",
        "i'd" => "id",
        "we'd" => "wed",
        "he'll" => "hell",
        "she'll" => "shell",
        "she'd" => "shed",
        _ => return None,
    })
}

impl Engine {
    pub(crate) fn english_correction(
        &self,
        input: &str,
        key: &str,
        extension: Option<&Engine>,
    ) -> Option<Vec<Suggestion>> {
        if !(2..=24).contains(&key.len())
            || input.chars().any(char::is_whitespace)
            || !key.bytes().all(|b| b.is_ascii_lowercase() || b == b'\'')
            || key.starts_with('\'')
            || key.ends_with('\'')
            || key.contains("''")
            || contains_profanity(key, ProfanityLanguage::English)
        {
            return None;
        }
        // A saved literal spelling (including a personal entry) wins over the
        // formatting rule. A longer prefix alone must not block "im"/"youre".
        if std::iter::once(self)
            .chain(extension)
            .any(|engine| engine.dictionary.english.contains(key))
        {
            return None;
        }
        // The screened vocabulary stores lemmas rather than every inflection.
        // A fully typed known form must not become a nearby spelling repair,
        // including when completion is disabled.
        if self.known_english_surface(input, key, extension) { return None; }
        if let Some(text) = contraction(key) {
            if contains_profanity(text, ProfanityLanguage::English) {
                return None;
            }
            return Some(vec![Suggestion { tap_only: false, preferred_input: None,
                completion: Some(text.to_owned()),
                correction: Some(text.to_owned()),
                candidate: SuggestionCandidate::English(EnglishCandidate {
                    // Zero denotes a runtime formatting rule with no lexical
                    // approval identity. Nothing is saved to either dictionary.
                    mapping_id: 0,
                    text: text.to_owned(),
                    rank: 0,
                    manual_only: false,
                    word_alternative: true,
                }),
            }]);
        }
        // An explicitly typed, known contraction remains valid even when the
        // lexical dictionary contains only its unpunctuated homograph.
        if key.contains('\'') {
            let bare = key.replace('\'', "");
            if contraction(&bare)
                .or_else(|| manual_alternative(&bare))
                .is_some_and(|text| text.eq_ignore_ascii_case(key))
            {
                return None;
            }
        }
        if key.len() < 3 || manual_alternative(key).is_some() {
            return None;
        }
        if std::iter::once(self).chain(extension).any(|engine| {
            let (start, end) = engine.dictionary.english.prefix(key);
            start != end
        }) {
            return None;
        }
        let lookup_in = |engine: &Engine, candidate: &str| -> Option<EnglishCandidate> {
            let index = &engine.dictionary.english;
            let mut cursor = index.cursor(index.bound(candidate, false));
            let row = cursor.next().expect("validated English dictionary")?;
            if row.key != candidate {
                return None;
            }
            let text = compact::english_text(&row);
            // Never substitute a proper name, uncommon technical term or a
            // filtered spelling. Unrated synthetic/legacy dictionaries remain
            // usable; current app data has independently reviewed levels.
            if text.bytes().any(|b| b.is_ascii_uppercase())
                || contains_profanity(&text, ProfanityLanguage::English)
                || engine.commonness.has_levels("english")
                    && engine.commonness.level("english", &text) > 3
            {
                return None;
            }
            Some(EnglishCandidate {
                mapping_id: row.id,
                text: text.to_string(),
                rank: row.rank,
                manual_only: false,
                word_alternative: false,
            })
        };
        // Check uniqueness across both sources. Looking up a spelling once
        // retains the base identity when a personal entry duplicates it.
        let lookup = |candidate: &str| {
            if self.dictionary.english.contains(candidate) {
                lookup_in(self, candidate)
            } else {
                extension.and_then(|engine| lookup_in(engine, candidate))
            }
        };
        let bytes = key.as_bytes();
        let mut swaps = BTreeSet::new();
        let mut edits = BTreeSet::new();
        for at in 0..bytes.len() {
            if at + 1 < bytes.len()
                && bytes[at] != bytes[at + 1]
                && bytes[at] != b'\''
                && bytes[at + 1] != b'\''
            {
                let mut swapped = bytes.to_vec();
                swapped.swap(at, at + 1);
                swaps.insert(String::from_utf8(swapped).unwrap());
            }
            // A spelling edit must not erase a deliberately typed apostrophe.
            if bytes[at] != b'\'' {
                let mut deleted = bytes.to_vec();
                deleted.remove(at);
                edits.insert(String::from_utf8(deleted).unwrap());
            }
        }
        for at in 0..=bytes.len() {
            for letter in (b'a'..=b'z').chain(std::iter::once(b'\'')) {
                let mut inserted = bytes.to_vec();
                inserted.insert(at, letter);
                edits.insert(String::from_utf8(inserted).unwrap());
                if at < bytes.len() && letter != b'\'' && bytes[at] != b'\'' {
                    let mut substituted = bytes.to_vec();
                    substituted[at] = letter;
                    edits.insert(String::from_utf8(substituted).unwrap());
                }
            }
        }
        let swaps_found: Vec<_> = swaps.iter().filter_map(|key| lookup(key)).collect();
        let mut hits = if swaps_found.len() == 1 {
            swaps_found
        } else {
            edits.extend(swaps.iter().cloned());
            edits.iter().filter_map(|key| lookup(key)).collect()
        };
        if hits.is_empty() { return None; }
        hits.sort_by_key(|candidate| {
            let source = if self.dictionary.english.contains(&candidate.text.to_ascii_lowercase()) {
                self
            } else { extension.unwrap_or(self) };
            (!swaps.contains(&candidate.text.to_ascii_lowercase()),
                source.commonness.level("english", &candidate.text), candidate.rank, candidate.text.clone())
        });
        let ambiguous = hits.len() > 1;
        let mut suggestions = Vec::new();
        if ambiguous {
            // Uncertain edits retain the typed word first. Auto accepts that
            // first choice; every screened alternative remains tappable.
            suggestions.push(Suggestion { tap_only: false, preferred_input: None,
                completion: Some(key.to_owned()), correction: None,
                candidate: SuggestionCandidate::English(EnglishCandidate {
                    mapping_id: 0, text: key.to_owned(), rank: 0,
                    manual_only: false, word_alternative: true,
                }),
            });
        }
        suggestions.extend(hits.into_iter().take(4).map(|candidate| Suggestion { tap_only: false, preferred_input: None,
            completion: Some(candidate.text.clone()), correction: Some(candidate.text.clone()),
            candidate: SuggestionCandidate::English(candidate),
        }));
        Some(suggestions)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Engine, SuggestionFeatures, TypingMode};
    use serde_json::json;

    fn suggest(input: &str, enabled: bool, completion: bool) -> serde_json::Value {
        let words = [
            "the", "ten", "tea", "receive", "world", "keyboard", "hello", "help", "don't", "form",
            "from",
        ];
        let engine = Engine::from_json(
            &json!({"schemaVersion": 3, "mappings": [], "words": [],
            "englishWords": words.iter().enumerate().map(|(id, text)| json!({
                "id": id + 1, "text": text, "rank": 0, "status": "approved"
            })).collect::<Vec<_>>() })
            .to_string(),
        )
        .unwrap();
        serde_json::to_value(
            engine
                .suggest_with_features(
                    input,
                    SuggestionFeatures {
                        mode: TypingMode::English,
                        english_autocorrection: enabled,
                        english_completion: completion,
                        ..Default::default()
                    },
                )
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn explicit_contractions_never_lose_their_apostrophe_to_a_dictionary_homograph() {
        let words = [
            "well", "ill", "shell", "shed", "im", "dont", "youve", "wed", "lets",
        ];
        let engine = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],
            "englishWords": words.iter().enumerate().map(|(i,text)| json!({"id":i+1,"text":text,"rank":0,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        let before = engine.compiled_bytes().to_vec();
        for input in [
            "we'll", "I'll", "she'll", "she'd", "I'm", "don't", "you've", "we'd", "let's",
        ] {
            let result = engine
                .suggest_with_features(
                    input,
                    SuggestionFeatures {
                        mode: TypingMode::English,
                        english_completion: false,
                        english_autocorrection: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            assert!(
                result.candidates.is_empty(),
                "{input}: {:?}",
                result.candidates
            );
        }
        assert_eq!(before, engine.compiled_bytes());
    }

    #[test]
    fn word_alternatives_require_completion_and_are_not_spelling_errors() {
        let build = |word: &str| {
            Engine::from_json(
                &json!({"schemaVersion":3,"mappings":[],"words":[],
            "englishWords":[{"id":1,"text":word,"rank":0,"status":"approved"}]})
                .to_string(),
            )
            .unwrap()
        };
        let base = build("its");
        let personal = build("werewolf");
        let before = base.compiled_bytes().to_vec();
        for (input, expected) in [
            ("its", "it's"),
            ("your", "you're"),
            ("were", "we're"),
            ("well", "we'll"),
            ("ill", "I'll"),
            ("it's", "its"),
        ] {
            for correction in [false, true] {
                let features = SuggestionFeatures {
                    mode: TypingMode::English,
                    english_completion: true,
                    english_autocorrection: correction,
                    ..Default::default()
                };
                for candidates in [
                    base.suggest_with_features(input, features)
                        .unwrap()
                        .candidates,
                    base.suggest_with_extension(&personal, input, features, &Default::default())
                        .unwrap()
                        .result
                        .candidates,
                ] {
                    assert_eq!(
                        candidates
                            .iter()
                            .filter_map(|c| match &c.candidate {
                                crate::SuggestionCandidate::English(c) if c.word_alternative && !c.text.eq_ignore_ascii_case(input) =>
                                    Some(c.text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>(),
                        vec![expected]
                    );
                    assert!(candidates.iter().all(|c| c.correction.is_none()));
                }
                assert!(base
                    .suggest_with_features(
                        input,
                        SuggestionFeatures {
                            english_completion: false,
                            ..features
                        }
                    )
                    .unwrap()
                    .candidates
                    .is_empty());
            }
        }
        assert_eq!(before, base.compiled_bytes());
    }

    #[test]
    fn missing_apostrophes_are_finite_runtime_rules_independent_of_vocabulary() {
        // The shipped vocabulary has no contraction rows. Prefixes must not
        // suppress the rule, nor should a nearby word win the generic edit.
        let engine = Engine::from_json(
            &json!({"schemaVersion":3,"mappings":[],"words":[],
            "englishWords":[{"id":1,"text":"image","rank":0,"status":"approved"},
                {"id":2,"text":"yourextension","rank":0,"status":"approved"},
                {"id":3,"text":"does","rank":0,"status":"approved"}]})
            .to_string(),
        )
        .unwrap();
        let before = engine.compiled_bytes().to_vec();
        let inputs = [
            ("im", "I'm"),
            ("Im", "I'm"),
            ("youre", "you're"),
            ("Youre", "you're"),
            ("theyre", "they're"),
            ("ive", "I've"),
            ("dont", "don't"),
            ("doesnt", "doesn't"),
            ("couldnt", "couldn't"),
        ];
        for completion in [false, true] {
            let features = SuggestionFeatures {
                mode: TypingMode::English,
                english_autocorrection: true,
                english_completion: completion,
                ..Default::default()
            };
            for (input, expected) in inputs {
                let result = engine.suggest_with_features(input, features).unwrap();
                assert_eq!(
                    result.candidates[0].correction.as_deref(),
                    Some(expected),
                    "{input}"
                );
                assert_eq!(result.candidates[0].candidate.mapping_id(), Some(0));
                assert!(engine
                    .suggest_with_features(
                        input,
                        SuggestionFeatures {
                            english_autocorrection: false,
                            ..features
                        }
                    )
                    .unwrap()
                    .candidates
                    .iter()
                    .all(|c| c.correction.is_none()));
            }
        }
        for key in [
            "ill", "id", "well", "were", "wed", "hell", "shell", "shed", "its", "lets", "xy",
            "im ", "im@a", "im-word", "im2",
        ] {
            assert!(
                engine
                    .suggest_with_features(
                        key,
                        SuggestionFeatures {
                            mode: TypingMode::English,
                            english_autocorrection: true,
                            ..Default::default()
                        }
                    )
                    .unwrap()
                    .candidates
                    .iter()
                    .all(|c| c.correction.is_none()),
                "{key}"
            );
        }
        assert!(engine
            .suggest_with_features(
                "im",
                SuggestionFeatures {
                    mode: TypingMode::Khmer,
                    english_autocorrection: true,
                    ..Default::default()
                }
            )
            .unwrap()
            .candidates
            .iter()
            .all(|c| c.correction.is_none()));
        assert_eq!(before, engine.compiled_bytes());
    }

    #[test]
    fn personal_exact_words_override_contractions_but_longer_prefixes_do_not() {
        let build = |word: &str| {
            Engine::from_json(
                &json!({"schemaVersion":3,"mappings":[],"words":[],
            "englishWords":[{"id":1,"text":word,"rank":0,"status":"approved"}]})
                .to_string(),
            )
            .unwrap()
        };
        let base = build("image");
        for (word, expected) in [
            ("im", None),
            ("image", Some("I'm")),
            ("you're", Some("I'm")),
        ] {
            for completion in [false, true] {
                let result = base
                    .suggest_with_extension(
                        &build(word),
                        "im",
                        SuggestionFeatures {
                            mode: TypingMode::English,
                            english_autocorrection: true,
                            english_completion: completion,
                            ..Default::default()
                        },
                        &Default::default(),
                    )
                    .unwrap();
                assert_eq!(
                    result
                        .result
                        .candidates
                        .iter()
                        .find_map(|c| c.correction.as_deref()),
                    expected
                );
                if expected.is_some() {
                    assert_eq!(result.result.candidates[0].correction.as_deref(), expected);
                }
            }
        }
        assert!(build("im")
            .suggest_with_features(
                "im",
                SuggestionFeatures {
                    mode: TypingMode::English,
                    english_autocorrection: true,
                    ..Default::default()
                }
            )
            .unwrap()
            .candidates
            .iter()
            .all(|c| c.correction.is_none()));
    }

    #[test]
    fn corrects_bounded_unambiguous_typos_independently_of_completion() {
        for (input, expected) in [
            ("teh", "the"),
            ("recieve", "receive"),
            ("wrold", "world"),
            ("keyboarf", "keyboard"),
            ("dont", "don't"),
        ] {
            for completion in [false, true] {
                assert_eq!(
                    suggest(input, true, completion)["candidates"][0]["correction"],
                    expected,
                    "{input}"
                );
            }
            assert!(suggest(input, false, false)["candidates"]
                .as_array()
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn personal_words_and_prefixes_prevent_correction_even_with_completion_off() {
        let build = |word: &str| {
            Engine::from_json(
                &json!({"schemaVersion":3,"mappings":[],"words":[],
            "englishWords":[{"id":1,"text":word,"rank":0,"status":"approved"}]})
                .to_string(),
            )
            .unwrap()
        };
        let base = build("the");
        for word in ["teh", "tehran"] {
            let personal = build(word);
            let result = base
                .suggest_with_extension(
                    &personal,
                    "teh",
                    SuggestionFeatures {
                        mode: TypingMode::English,
                        english_autocorrection: true,
                        english_completion: false,
                        ..Default::default()
                    },
                    &Default::default(),
                )
                .unwrap();
            assert!(result
                .result
                .candidates
                .iter()
                .all(|candidate| candidate.correction.is_none()));
        }
    }

    #[test]
    fn merged_corrections_check_ambiguity_across_all_dictionary_entries() {
        let build = |words: &[&str]| {
            Engine::from_json(
                &json!({"schemaVersion":3,"mappings":[],"words":[],
                "englishWords":words.iter().enumerate().map(|(id, word)| json!({
                    "id":id + 1,"text":word,"rank":0,"status":"approved"
                })).collect::<Vec<_>>()})
                .to_string(),
            )
            .unwrap()
        };
        for (base_words, personal_words) in [
            (vec!["the"], vec!["eth"]),
            // Ambiguity within one source must not disappear before merging.
            (vec!["the"], vec!["the", "eth"]),
            (vec!["the", "eth"], vec!["the"]),
        ] {
            let base = build(&base_words);
            let personal = build(&personal_words);
            for english_completion in [false, true] {
                let result = base
                    .suggest_with_extension(
                        &personal,
                        "teh",
                        SuggestionFeatures {
                            mode: TypingMode::English,
                            english_autocorrection: true,
                            english_completion,
                            ..Default::default()
                        },
                        &Default::default(),
                    )
                    .unwrap();
                assert_eq!(result.result.candidates.len(), 3);
                assert!(result.result.candidates[0].correction.is_none());
                assert!(matches!(&result.result.candidates[0].candidate,
                    crate::SuggestionCandidate::English(word) if word.text == "teh"));
                let choices: std::collections::BTreeSet<_> = result.result.candidates.iter()
                    .filter_map(|candidate| candidate.correction.as_deref()).collect();
                assert_eq!(choices, std::collections::BTreeSet::from(["eth", "the"]));
            }
        }
        // A duplicate spelling is one choice, and an unrelated personal edit
        // must not outrank the globally unique adjacent swap.
        for personal_words in [vec!["the"], vec!["tech"]] {
            let base = build(&["the"]);
            let personal = build(&personal_words);
            let result = base
                .suggest_with_extension(
                    &personal,
                    "teh",
                    SuggestionFeatures {
                        mode: TypingMode::English,
                        english_autocorrection: true,
                        english_completion: false,
                        ..Default::default()
                    },
                    &Default::default(),
                )
                .unwrap();
            assert_eq!(result.result.candidates.len(), 1);
            assert_eq!(
                result.result.candidates[0].correction.as_deref(),
                Some("the")
            );
        }
    }

    #[test]
    fn rejects_proper_names_and_uncommon_targets_without_changing_the_dataset() {
        let mut engine = Engine::from_json(
            &json!({"schemaVersion": 3, "mappings": [], "words": [],
            "englishWords": [
                {"id":1,"text":"Zorbin","rank":0,"status":"approved"},
                {"id":2,"text":"keyboard","rank":0,"status":"approved"}
            ]})
            .to_string(),
        )
        .unwrap();
        let features = SuggestionFeatures {
            mode: TypingMode::English,
            english_autocorrection: true,
            english_completion: false,
            ..Default::default()
        };
        assert!(engine
            .suggest_with_features("zorbinx", features)
            .unwrap()
            .candidates
            .is_empty());
        let before = engine.compiled_bytes().to_vec();
        engine
            .load_word_ranking_json(
                &json!({"format":"kmeow-word-ranking-v1", "khmer":[],
            "english":[{"word":"keyboard","level":4}]})
                .to_string(),
            )
            .unwrap();
        assert!(engine
            .suggest_with_features("keyboarf", features)
            .unwrap()
            .candidates
            .is_empty());
        let personal = Engine::from_json(
            &json!({"schemaVersion":3,"mappings":[],"words":[],"englishWords":[
                {"id":101,"text":"keyboard","rank":0,"status":"approved"}
            ]})
            .to_string(),
        )
        .unwrap();
        assert!(
            engine
                .suggest_with_extension(&personal, "keyboarf", features, &Default::default())
                .unwrap()
                .result
                .candidates
                .is_empty(),
            "A personal duplicate must preserve the bundled commonness gate"
        );
        assert_eq!(before, engine.compiled_bytes());
    }

    #[test]
    fn preserves_real_words_prefixes_ambiguity_and_nonwords_outside_bounds() {
        for input in [
            "form",
            "from",
            "hell",
            "hel",
            "xy",
            "teh ",
            "a1bc",
            "កខគ",
            "teh@example",
            "teh-word",
        ] {
            assert!(
                suggest(input, true, true)["candidates"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|candidate| candidate.get("correction").is_none()),
                "{input}"
            );
        }
    }
}
