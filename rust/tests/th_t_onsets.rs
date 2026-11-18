use khmer_engine::{Engine, Pronunciation};
use serde_json::{json,Value};

#[test]
fn tha_and_tho_accept_t_at_onsets_keep_series_codas_existing_choices_and_aliases() {
    let proof: Value = serde_json::from_str(include_str!("../../test/fixtures/th-t-onsets.json")).unwrap();
    let rows: Vec<_> = proof["cases"].as_array().unwrap().iter().chain(proof["controls"].as_array().unwrap()).collect();
    let mut core = Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "mappings":[{"id":900,"romanized":"tver","khmer":"កូន","rank":100,"status":"approved"}],
        "words":rows.iter().enumerate().map(|(i,row)|json!({"id":100+i,"khmer":row["word"],"rank":i,"status":"approved"})).collect::<Vec<_>>()
    }).to_string()).unwrap();
    let bytes = core.to_binary();let alias = core.convert("tver").unwrap();
    for row in proof["cases"].as_array().unwrap() {
        let mut expected: Vec<String> = row["previous"].as_array().unwrap().iter().chain(row["additional"].as_array().unwrap())
            .map(|input|input.as_str().unwrap().to_owned()).collect();expected.sort();
        assert_eq!(core.sound_map_inputs(row["word"].as_str().unwrap(),"").unwrap().inputs,expected,"{}",row["word"]);
    }
    for row in proof["controls"].as_array().unwrap() {
        assert_eq!(json!(core.sound_map_inputs(row["word"].as_str().unwrap(),"").unwrap().inputs),row["inputs"],"{}",row["word"]);
    }
    for pair in proof["collisions"].as_array().unwrap() {
        let input = pair[0].as_str().unwrap();
        assert_eq!(core.suggest_with_context(input,Default::default(),&Default::default()).unwrap().exact_candidates[0].khmer,pair[1].as_str().unwrap(),"{input}");
    }
    core.set_pronunciations(vec![Pronunciation {word:"ថា".into(),read_as:"ធា".into()}]).unwrap();
    assert!(core.suggest_with_context("tea",Default::default(),&Default::default()).unwrap().exact_candidates.iter().any(|row|row.khmer=="ថា"));
    core.set_pronunciations(vec![]).unwrap();assert_eq!(core.to_binary(),bytes);assert_eq!(core.convert("tver").unwrap(),alias);
}

#[test]
fn older_ou_predictions_precede_the_new_t_onset_stage() {
    let core=Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"englishWords":[],
        "words":[{"id":100,"khmer":"ប្អូនថ្លៃ","rank":0,"status":"approved"}]
    }).to_string()).unwrap();
    for input in ["b'ount","p'ount"] {
        let result=core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
        assert_eq!(json!(result.next_letter_weights),json!({"h":1000}),"{input}");
    }
    for input in ["b'ountl","p'ountl"] {
        let result=core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
        assert_eq!(json!(result.next_letter_weights),json!({"a":1000}),"{input}");
    }
}

#[test]
fn large_read_as_choices_keep_all_inputs_and_compound_scopes_without_expansion() {
    let mut core=Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "mappings":[{"id":900,"romanized":"savedreading","khmer":"កម្ម","rank":0,"status":"approved"}],
        "words":(["កម្ម","ករ","កម្មករ"].iter().enumerate().map(|(i,word)|json!({"id":100+i,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>())
    }).to_string()).unwrap();
    let compact=core.to_binary();let alias=core.convert("savedreading").unwrap();
    core.set_pronunciations(serde_json::from_value(json!([{ "word":"កម្ម","readAs":["ប្រធានាធិបតី","ធូ"] }])).unwrap()).unwrap();
    let mut after=String::new();let mut inputs=Vec::new();
    loop {let page=core.sound_map_inputs("កម្ម",&after).unwrap();inputs.extend(page.inputs);if let Some(next)=page.next_after {after=next;}else{break;}}
    assert_eq!(inputs.len(),260);
    for input in &inputs {
        assert!(core.suggest_with_context(input,Default::default(),&Default::default()).unwrap().exact_candidates.iter().any(|row|row.khmer=="កម្ម"),"{input}");
        assert!(core.suggest_with_context(&format!("{input}kor"),Default::default(),&Default::default()).unwrap().exact_candidates.iter().any(|row|row.khmer=="កម្មករ"),"{input}kor");
    }
    assert!(core.browse_words("t",None,100).unwrap().words.iter().any(|row|row.khmer=="កម្ម"));
    core.set_pronunciations(vec![Pronunciation {word:"កម្ម".into(),read_as:"ថា".repeat(40).into()}]).unwrap();
    for input in ["tha".repeat(40),"ta".repeat(40),format!("{}{}","tha".repeat(20),"ta".repeat(20))] {
        assert!(core.suggest_with_context(&input,Default::default(),&Default::default()).unwrap().exact_candidates.iter().any(|row|row.khmer=="កម្ម"));
    }
    let before=core.suggest("tha").unwrap();
    assert!(core.set_pronunciations(vec![Pronunciation {word:"កម្ម".into(),read_as:"មិត".repeat(12).into()}]).is_err());
    assert_eq!(core.suggest("tha").unwrap(),before);
    core.set_pronunciations(vec![]).unwrap();assert_eq!(core.to_binary(),compact);assert_eq!(core.convert("savedreading").unwrap(),alias);
}
