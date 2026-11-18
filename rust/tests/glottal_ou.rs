use khmer_engine::{Engine, Pronunciation, SuggestionFeatures};
use serde_json::{json, Value};

fn exact(core: &Engine, input: &str, word: &str) -> bool {
    core.suggest_with_context(input, Default::default(), &Default::default()).unwrap()
        .exact_candidates.iter().any(|row| row.khmer == word)
}

#[test]
fn glottal_long_u_adds_ou_without_changing_plain_carriers_other_vowels_or_aliases() {
    let proof: Value = serde_json::from_str(include_str!("../../test/fixtures/glottal-ou.json")).unwrap();
    let rows: Vec<_> = proof["cases"].as_array().unwrap().iter().chain(proof["controls"].as_array().unwrap()).collect();
    let mut core = Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "mappings":[{"id":900,"romanized":"p'oun","khmer":"កូន","rank":100,"status":"approved"}],
        "words":rows.iter().enumerate().map(|(i,row)|json!({"id":100+i,"khmer":row["word"],"rank":i,"status":"approved"})).collect::<Vec<_>>()
    }).to_string()).unwrap();
    let bytes = core.to_binary();
    let alias = core.convert("p'oun").unwrap();
    for row in proof["cases"].as_array().unwrap() {
        let word = row["word"].as_str().unwrap();
        let mut expected: Vec<String> = row["previous"].as_array().unwrap().iter().chain(row["additional"].as_array().unwrap())
            .map(|key|key.as_str().unwrap().to_owned()).collect();
        expected.sort();
        assert_eq!(core.sound_map_inputs(word, "").unwrap().inputs, expected, "{word}");
        for input in &expected { assert!(exact(&core,input,word),"{input}: {word}"); }
    }
    for row in proof["controls"].as_array().unwrap() {
        assert_eq!(json!(core.sound_map_inputs(row["word"].as_str().unwrap(), "").unwrap().inputs),row["inputs"]);
    }
    assert!(!exact(&core,"p'un","ប្អូន"));
    assert_eq!(core.suggest_with_context("p'oun",Default::default(),&Default::default()).unwrap().exact_candidates[0].khmer,"កូន");
    let disabled = SuggestionFeatures {sound_map:false,..Default::default()};
    assert!(!core.suggest_with_context("b'oun",disabled,&Default::default()).unwrap().exact_candidates.iter().any(|row|row.khmer == "ប្អូន"));
    core.set_pronunciations(vec![Pronunciation {word:"ប្អូន".into(),read_as:"មិត".into()}]).unwrap();
    assert!(exact(&core,"met","ប្អូន"));assert!(!exact(&core,"b'oun","ប្អូន"));
    core.set_pronunciations(vec![]).unwrap();assert!(exact(&core,"b'oun","ប្អូន"));
    assert_eq!(core.convert("p'oun").unwrap(),alias);assert_eq!(core.to_binary(),bytes);
}

#[test]
fn read_as_and_compound_boundaries_share_the_glottal_vowel_alternative() {
    let mut core = Engine::from_json(&json!({"schemaVersion":3,"mappings":[],"englishWords":[],
        "words":(["មិត្ត","ភាព","មិត្តភាព","កូន"].iter().enumerate().map(|(i,word)|
            json!({"id":100+i,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>())
    }).to_string()).unwrap();
    let bytes = core.to_binary();
    for (read_as, old, additional) in [("ប្អូន","p'on","p'oun"),("ចង្អូរ","jong'o","jong'ou")] {
        core.set_pronunciations(vec![Pronunciation {word:"មិត្ត".into(),read_as:read_as.into()}]).unwrap();
        assert!(exact(&core,old,"មិត្ត"));assert!(exact(&core,additional,"មិត្ត"));
        assert!(exact(&core,&format!("{old}peap"),"មិត្តភាព"));
        assert!(exact(&core,&format!("{additional}peap"),"មិត្តភាព"));
    }
    core.set_pronunciations(vec![]).unwrap();assert_eq!(core.to_binary(),bytes);
}

#[test]
fn glottal_ou_preserves_existing_touch_priors_and_supports_the_new_prefix() {
    let core = Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],"mappings":[],
        "words":(["ចង្អូរ","ចង្អុល","ប្អូន","ប្អូនថ្លៃ"].iter().enumerate().map(|(i,word)|
            json!({"id":100+i,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>())
    }).to_string()).unwrap();
    let weights = |input| core.suggest_with_context(input,Default::default(),&Default::default()).unwrap().next_letter_weights;
    for input in ["jong'o","jjong'o"] {
        assert_eq!(weights(input),std::collections::BTreeMap::from([("l".into(),500)]),"{input}");
    }
    for input in ["p'o","p'ou","pp'ou"] {
        assert_eq!(weights(input),std::collections::BTreeMap::from([("n".into(),1000)]),"{input}");
    }
}
