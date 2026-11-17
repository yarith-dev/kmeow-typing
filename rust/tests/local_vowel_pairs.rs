use khmer_engine::{Engine, SuggestionFeatures, TypingMode};

#[test]
fn local_pairs_choose_vowels_without_rewriting_explicit_spellings_or_unrelated_controls() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/local-vowel-pairs.json")).unwrap();
    let rows: Vec<_> = proof["cases"].as_array().unwrap().iter()
        .chain(proof["unchanged"].as_array().unwrap()).collect();
    let core = Engine::from_json(&serde_json::json!({"schemaVersion":2,"mappings":[],
        "words":rows.iter().enumerate().map(|(i,row)|serde_json::json!({
            "id":98500+i,"khmer":row["word"],"rank":i,"status":"approved"
        })).collect::<Vec<_>>()
    }).to_string()).unwrap();
    let binary = core.to_binary();
    for row in &rows {
        let word = row["word"].as_str().unwrap();
        let page = core.browse_words(word,None,100).unwrap();
        let generated = &page.words.iter().find(|entry|entry.khmer == word).unwrap().sound_map_inputs;
        if let Some(prefixes) = row["prefixes"].as_array() {
            assert!(!generated.is_empty(),"{word}");
            assert!(generated.iter().all(|input|prefixes.iter().any(|prefix|input.starts_with(prefix.as_str().unwrap()))),"{word}: {generated:?}");
        } else { assert_eq!(serde_json::json!(generated),row["inputs"],"{word}"); }
        for input in row["yes"].as_array().or_else(||row["inputs"].as_array()).unwrap() {
            let input = input.as_str().unwrap();
            assert!(core.suggest_with_context(input,Default::default(),&Default::default()).unwrap()
                .exact_candidates.iter().any(|candidate|candidate.khmer == word),"{input}: {word}");
            for features in [SuggestionFeatures {sound_map:false,..Default::default()},
                SuggestionFeatures {romanized:false,..Default::default()},
                SuggestionFeatures {mode:TypingMode::English,..Default::default()}] {
                assert!(!core.suggest_with_context(input,features,&Default::default()).unwrap()
                    .exact_candidates.iter().any(|candidate|candidate.khmer == word));
            }
        }
        if let Some(rejected) = row["no"].as_array() {
            for input in rejected {
                assert!(!core.suggest_with_context(input.as_str().unwrap(),Default::default(),&Default::default()).unwrap()
                    .exact_candidates.iter().any(|candidate|candidate.khmer == word),"{input}: {word}");
            }
        }
    }
    let personal = Engine::from_personal_words_json(r#"[
        {"word":"សប្បាយ","spellings":["sabay","sorbbay","sarbbay"]},
        {"word":"បណ្ណាល័យ","spellings":["bornnalai","my-library"]},
        {"word":"លទ្ធិ","spellings":["lerthi","lotthi"]},
        {"word":"វគ្គ","spellings":["veak","vokko"]}
    ]"#).unwrap();
    let personal_bytes = personal.to_binary();
    for sound_map in [true,false] {
        for (input,word) in [("sabay","សប្បាយ"),("sorbbay","សប្បាយ"),("sarbbay","សប្បាយ"),
            ("bornnalai","បណ្ណាល័យ"),("my-library","បណ្ណាល័យ"),("lerthi","លទ្ធិ"),
            ("lotthi","លទ្ធិ"),("veak","វគ្គ"),("vokko","វគ្គ")] {
            assert!(core.suggest_with_extension(&personal,input,
                SuggestionFeatures {sound_map,..Default::default()},&Default::default()).unwrap()
                .exact_candidates.iter().any(|candidate|candidate.khmer == word),"personal {input}: {word}");
        }
    }
    assert_eq!(core.to_binary(),binary);
    assert_eq!(personal.to_binary(),personal_bytes);
}
