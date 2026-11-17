use khmer_engine::{ApprovedDataset, Engine};

#[test]
fn siss_uses_the_live_replacement_vowel_in_plain_syllables() {
    let words = ["សិស្ស", "សឹស្ស", "កូនសិស្ស", "កូនសឹស្ស", "សិស្សា", "សឹស្សា", "និស្សិត", "សិស្សម៍"];
    let data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion":2,"mappings":[],"words":words.iter().enumerate().map(|(i,w)|
            serde_json::json!({"id":95000+i,"khmer":w,"rank":0,"status":"approved"})).collect::<Vec<_>>()
    })).unwrap();
    let core=Engine::new(data).unwrap();
    let page:serde_json::Value=serde_json::from_str(&core.browse_words_json(r#"{"query":"","limit":100}"#).unwrap()).unwrap();
    let keys=|word:&str|page["words"].as_array().unwrap().iter().find(|r|r["khmer"]==word).unwrap()["soundMapInputs"].clone();
    assert_eq!(keys("សិស្ស"),keys("សឹស្ស"));
    assert_eq!(keys("កូនសិស្ស"),keys("កូនសឹស្ស"));
    assert_ne!(keys("សិស្សា"),keys("សឹស្សា"));
    assert_eq!(keys("និស្សិត"),serde_json::json!(["nesset","nessit","nisset","nissit"]));
    assert_eq!(keys("សិស្សម៍"),serde_json::json!(["seh","ses","sih","sis"]));
}
