use khmer_engine::Engine;
use serde_json::json;

#[test]
fn target_policy_keeps_alias_identity_and_khmer_completion_through_binary() {
    let data=json!({"schemaVersion":2,"words":[
        {"id":1,"khmer":"កា","rank":4,"status":"approved"},
        {"id":2,"khmer":"កា","rank":0,"status":"approved"},
        {"id":3,"khmer":"កាៗ","rank":5,"status":"approved"}],
        "mappings":[{"id":2,"romanized":"special","khmer":"កា","rank":0,"status":"approved"}],
        "soundMapDisabledWords":["កា"]});
    let engine=Engine::from_json(&data.to_string()).unwrap();
    assert_eq!(&engine.compiled_bytes()[..8],b"KMEOWC04");
    let restored=Engine::from_binary(engine.compiled_bytes().to_vec()).unwrap();
    for key in ["ka","kaka","special","កា"] {
        assert_eq!(engine.convert_json(key).unwrap(),restored.convert_json(key).unwrap());
    }
    let ka:serde_json::Value=serde_json::from_str(&engine.convert_json("ka").unwrap()).unwrap();
    assert!(ka["candidates"].as_array().unwrap().iter().all(|c|c["khmer"]!="កា"));
    let special:serde_json::Value=serde_json::from_str(&engine.convert_json("special").unwrap()).unwrap();
    assert_eq!(special["candidates"][0]["mappingId"],2);
}
