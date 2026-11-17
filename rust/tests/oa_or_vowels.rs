use khmer_engine::{ApprovedDataset, Engine};

#[test]
fn vowel_alternatives_preserve_consonant_r_and_literal_saved_spellings() {
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/oa-or-vowels.json")).unwrap();
    let rows: Vec<_> = proof["pairs"].as_array().unwrap().iter()
        .chain(proof["consonantR"].as_array().unwrap()).collect();
    let data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion":2,"mappings":[{"id":99900,"romanized":"koa","khmer":"ខា","rank":0,"status":"approved"}],
        "words":rows.iter().enumerate().map(|(i,row)|serde_json::json!({
            "id":99000+i,"khmer":row[0],"rank":i,"status":"approved"
        })).collect::<Vec<_>>()
    })).unwrap();
    let core=Engine::new(data).unwrap();
    let bytes=core.to_binary();
    let restored=Engine::from_binary(bytes.clone()).unwrap();
    for engine in [&core,&restored] {
        for (field,accept) in [("pairs",true),("consonantR",false)] {
            for row in proof[field].as_array().unwrap() {
                let word=row[0].as_str().unwrap();
                for (input,expected) in [(row[1].as_str().unwrap(),true),(row[2].as_str().unwrap(),accept)] {
                    let result=engine.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
                    assert_eq!(result.exact_candidates.iter().any(|c|c.khmer==word),expected,"{input} → {word}");
                }
            }
        }
        assert!(engine.convert("koa").unwrap().candidates.iter().any(|c|c.khmer=="ខា"));
        assert!(!engine.convert("kor").unwrap().candidates.iter().any(|c|c.khmer=="ខា"));
        assert_eq!(engine.to_binary(),bytes);
    }
}
