use khmer_engine::{Engine, SuggestionCandidate, SuggestionFeatures, TypingMode};
use serde_json::json;

fn engine(words: &[&str]) -> Engine {
    Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"words":[],"englishWords":
        words.iter().enumerate().map(|(i,text)| json!({"id":i+1,"text":text,"rank":i,"status":"approved"})).collect::<Vec<_>>()}).to_string()).unwrap()
}
fn text(hit: &khmer_engine::Suggestion) -> &str {
    match &hit.candidate { SuggestionCandidate::English(word) => &word.text, _ => panic!() }
}

#[test]
fn merged_personal_prefixes_do_not_evict_english_contractions_before_the_cap() {
    for (input,contraction) in [("im","I'm"),("youre","you're")] {
        let prefix_words = |suffix| (b'a'..=b't').map(|letter|
            format!("{input}{suffix}{}",char::from(letter))).collect::<Vec<_>>();
        let base_words=prefix_words('a');let extra_words=prefix_words('b');
        let base=engine(&base_words.iter().map(String::as_str).collect::<Vec<_>>());
        let extra=Engine::from_personal_words_json(&json!(extra_words.iter().map(|word|
            json!({"word":word,"language":"english"})).collect::<Vec<_>>()).to_string()).unwrap();
        let before=(base.to_binary(),extra.to_binary());
        let restored=(Engine::from_binary(before.0.clone()).unwrap(),Engine::from_binary(before.1.clone()).unwrap());
        for (base,extra) in [(&base,&extra),(&restored.0,&restored.1)] {
            for completion in [true,false] {
                let features=SuggestionFeatures {mode:TypingMode::English,english_autocorrection:true,
                    english_completion:completion,..Default::default()};
                for (base,extra) in [(base,extra),(extra,base)] {
                    let result=base.suggest_with_extension_ranking(extra,input,features,&Default::default(),&Default::default()).unwrap();
                    assert_eq!(text(&result.baseline.result.candidates[0]),contraction);
                    assert_eq!(result.baseline.result.candidates[0].correction.as_deref(),Some(contraction));
                    let order=result.ranking.groups.iter().flat_map(|group|
                        group.candidates.iter().map(|candidate|candidate.id)).collect::<Vec<_>>();
                    let display=result.ranking.apply_order(&order).unwrap();
                    assert_eq!(text(&display[0]),contraction);
                    assert!(result.baseline.result.candidates.len()<=20);
                }
            }
            let features=SuggestionFeatures {mode:TypingMode::English,english_autocorrection:false,..Default::default()};
            let result=base.suggest_with_extension(extra,input,features,&Default::default()).unwrap();
            assert_eq!(result.result.candidates.len(),20);
            assert!(result.result.candidates.iter().all(|hit|hit.correction.is_none()));
            let literal=Engine::from_personal_words_json(&json!([{"word":input,"language":"english"}]).to_string()).unwrap();
            let result=base.suggest_with_extension(&literal,input,
                SuggestionFeatures {english_autocorrection:true,..features},&Default::default()).unwrap();
            assert_eq!(text(&result.result.candidates[0]),input);
            assert!(result.result.candidates.iter().all(|hit|hit.correction.is_none()));
        }
        assert_eq!((base.to_binary(),extra.to_binary()),before);
    }
}

#[test]
fn ordinary_spelling_edits_cover_swaps_missing_extra_and_wrong_letters() {
    let core = engine(&["the", "receive", "world", "keyboard", "because", "friend", "hello",
        "spelling", "language", "tomorrow", "separate", "definitely", "and", "that", "their", "with", "student", "until"]);
    let before = core.to_binary();
    for completion in [false, true] {
        for (input, expected) in [("teh", "the"), ("recieve", "receive"), ("wrold", "world"),
            ("keyboarf", "keyboard"), ("becuase", "because"), ("freind", "friend"), ("helllo", "hello"),
            ("speling", "spelling"), ("langauge", "language"), ("tommorrow", "tomorrow"),
            ("seperate", "separate"), ("definately", "definitely"), ("adn", "and"), ("taht", "that"),
            ("thier", "their"), ("wiht", "with"), ("worrld", "world"), ("studnet", "student"), ("untill", "until")] {
            let result = core.suggest_with_context(input, SuggestionFeatures { mode: TypingMode::English,
                english_autocorrection: true, english_completion: completion, ..Default::default() }, &Default::default()).unwrap();
            assert_eq!(result.result.candidates[0].correction.as_deref(), Some(expected), "{input}");
        }
    }
    assert_eq!(before, core.to_binary());
}

#[test]
fn uncertain_typos_offer_ranked_screened_choices_while_auto_keeps_the_original() {
    let core = engine(&["quick", "kick", "address", "dress", "hello", "help"]);
    let before = core.to_binary();
    for completion in [false, true] {
        let features = SuggestionFeatures { mode: TypingMode::English, english_autocorrection: true,
            english_completion: completion, ..Default::default() };
        for (input, words) in [("qick", ["quick", "kick"]), ("adress", ["address", "dress"]), ("hellp", ["hello", "help"])] {
            let result = core.suggest_with_ranking(input, features, &Default::default(), &Default::default()).unwrap();
            assert_eq!(text(&result.baseline.result.candidates[0]), input);
            assert!(result.baseline.result.candidates[0].correction.is_none());
            for word in words { assert!(result.baseline.result.candidates.iter().any(|hit| hit.correction.as_deref() == Some(word)), "{input}: {word}"); }
            assert!(result.baseline.result.candidates.iter().all(|hit|
                matches!(&hit.candidate, SuggestionCandidate::English(word) if !word.manual_only)));
            assert_eq!(text(result.ranking.baseline_automatic_candidate.as_ref().unwrap()), input);
        }
    }
    assert_eq!(before, core.to_binary());
}

#[test]
fn uncertain_choices_are_bounded_and_keep_screening_and_rank_order() {
    let mut core = engine(&["cat", "bat", "fat", "hat", "mat", "rat", "qat", "Nat"]);
    core.load_word_ranking_json(&json!({"format":"kmeow-word-ranking-v1","khmer":[],"english":[
        {"word":"cat","level":3}, {"word":"bat","level":2}, {"word":"fat","level":2},
        {"word":"hat","level":1}, {"word":"mat","level":1}, {"word":"rat","level":3},
        {"word":"qat","level":5}, {"word":"Nat","level":1}
    ]}).to_string()).unwrap();
    let before = core.to_binary();
    for completion in [false, true] {
        let result = core.suggest_with_context("zat", SuggestionFeatures { mode: TypingMode::English,
            english_autocorrection: true, english_completion: completion, ..Default::default() }, &Default::default()).unwrap();
        assert_eq!(result.result.candidates.iter().map(text).collect::<Vec<_>>(),
            ["zat", "hat", "mat", "bat", "fat"]);
    }
    assert_eq!(before, core.to_binary());
}

#[test]
fn spelling_recovery_keeps_deliberate_apostrophes_and_valid_words() {
    let core = engine(&["cats", "books", "can't", "form", "from"]);
    let features = SuggestionFeatures { mode: TypingMode::English, english_autocorrection: true,
        english_completion: false, ..Default::default() };
    for input in ["cat's", "book's", "form", "from", "ca'nt", "user@example", "item_1"] {
        let result = core.suggest_with_context(input, features, &Default::default()).unwrap();
        assert!(result.result.candidates.is_empty(), "{input}: {:?}", result.result.candidates);
    }
    assert_eq!(core.suggest_with_context("cna't", features, &Default::default()).unwrap().result.candidates[0].correction.as_deref(), Some("can't"));
}
