use khmer_engine::{Engine, Pronunciation, SuggestionFeatures};
use serde_json::{json,Value};

#[test]
fn closed_ua_adds_u_without_changing_open_syllables_silent_r_other_vowels_or_aliases() {
    let proof:Value=serde_json::from_str(include_str!("../../test/fixtures/closed-ua-u.json")).unwrap();
    let rows:Vec<_>=proof["cases"].as_array().unwrap().iter().chain(proof["controls"].as_array().unwrap()).collect();
    let core=Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":rows.iter().enumerate().map(|(i,row)|json!({"id":100+i,"khmer":row["word"],"rank":i,"status":"approved"})).collect::<Vec<_>>(),
        "mappings":[{"id":900,"romanized":"juy","khmer":"កូន","rank":1000,"status":"approved"}]
    }).to_string()).unwrap();
    let bytes=core.to_binary();let alias=core.convert("juy").unwrap();
    for row in proof["cases"].as_array().unwrap() {
        let word=row["word"].as_str().unwrap();
        let mut expected:Vec<_>=row["previous"].as_array().unwrap().iter().chain(row["additional"].as_array().unwrap()).map(|v|v.as_str().unwrap().to_owned()).collect();expected.sort();
        assert_eq!(core.sound_map_inputs(word,"").unwrap().inputs,expected,"{word}");
        for input in expected {assert!(core.suggest_with_context(&input,Default::default(),&Default::default()).unwrap().exact_candidates.iter().any(|c|c.khmer==word),"{input}: {word}");}
    }
    for row in proof["controls"].as_array().unwrap() {assert_eq!(json!(core.sound_map_inputs(row["word"].as_str().unwrap(),"").unwrap().inputs),row["inputs"],"{}",row["word"]);}
    for (input,word) in [("mun","មុន"),("bun","បុន"),("puk","ពុក"),("rum","រុម"),("juy","កូន")] {
        assert_eq!(core.suggest_with_context(input,Default::default(),&Default::default()).unwrap().exact_candidates[0].khmer,word,"{input}");
    }
    assert!(!core.suggest_with_context("muy",SuggestionFeatures {sound_map:false,..Default::default()},&Default::default()).unwrap().exact_candidates.iter().any(|row|row.khmer=="មួយ"));
    assert_eq!(core.to_binary(),bytes);assert_eq!(core.convert("juy").unwrap(),alias);
}

#[test]
fn closed_ua_preserves_prediction_precedence_and_read_as_compounds() {
    let mut core=Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],"mappings":[],
        "words":(["ជួយ","កម្ម","ករ","កម្មករ"].iter().enumerate().map(|(i,word)|json!({"id":100+i,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>())
    }).to_string()).unwrap();
    assert_eq!(json!(core.suggest_with_context("ju",Default::default(),&Default::default()).unwrap().next_letter_weights),json!({"r":1000}));
    let bytes=core.to_binary();
    core.set_pronunciations(vec![Pronunciation {word:"កម្ម".into(),read_as:"ជួយ".into()}]).unwrap();
    for input in ["jury","juy","chury","chuy"] {
        for (input,word) in [(input.to_owned(),"កម្ម"),(format!("{input}kor"),"កម្មករ")] {
            assert!(core.suggest_with_context(&input,Default::default(),&Default::default()).unwrap().exact_candidates.iter().any(|row|row.khmer==word),"{input}: {word}");
        }
    }
    core.set_pronunciations(vec![Pronunciation {word:"កម្ម".into(),read_as:"មួយ".repeat(30).into()}]).unwrap();
    for input in ["mury".repeat(30),"muy".repeat(30),format!("{}{}","mury".repeat(15),"muy".repeat(15))] {
        assert!(core.suggest_with_context(&input,Default::default(),&Default::default()).unwrap().exact_candidates.iter().any(|row|row.khmer=="កម្ម"));
    }
    core.set_pronunciations(vec![]).unwrap();assert_eq!(core.to_binary(),bytes);
}
