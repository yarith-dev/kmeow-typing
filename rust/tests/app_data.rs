use khmer_engine::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn fixture() -> (Engine, Value, Value) {
    let engine = Engine::from_json(&json!({
        "schemaVersion": 3, "mappings": [],
        "words": [{"id": 1, "khmer": "សៀវភៅ", "rank": 1, "status": "approved"}],
        "englishWords": [{"id": 2, "text": "Book", "rank": 1, "status": "approved"}]
    }).to_string()).unwrap();
    let file = |language: &str, word: &str| json!({
        "format": "kmeow-word-metadata-v2", "language": language, "snapshotId": "a".repeat(64),
        "words": [{"id": language, "word": word, "context": null, "partsOfSpeech": ["noun"],
            "commonnessLevel": 1, "meanings": [], "translations": [], "sources": []}], "translations": []
    });
    (engine, file("khmer", "សៀវភៅ"), file("english", "book"))
}
fn provenance(engine: &Engine, khmer: &str, english: &str) -> String {
    let mut compact: Value = serde_json::from_str(&engine.stats_json()).unwrap();
    compact["artifactHash"] = json!(hash(engine.compiled_bytes()));
    compact["datasetHash"] = json!("b".repeat(64));
    let file = |language: &str, content: &str| json!({
        "file": format!("{language}-words.json"), "bytes": content.len(), "sha256": hash(content.as_bytes()),
        "words": serde_json::from_str::<Value>(content).unwrap()["words"].as_array().unwrap().len()
    });
    json!({"compactManifest": compact, "metadataManifest": {
        "format": "kmeow-word-metadata-bundle-v1", "snapshotId": "a".repeat(64),
        "compactArtifactHash": hash(engine.compiled_bytes()),
        "files": {"khmer": file("khmer", khmer), "english": file("english", english)}
    }, "notices": [{"file": "test.txt", "text": "Isolated fixture"}]}).to_string()
}

#[test]
fn packages_typable_words_without_requiring_romanized_aliases() {
    let (engine, khmer, english) = fixture();
    let (khmer, english) = (khmer.to_string(), english.to_string());
    let (kb, eb) = engine.compile_app_data(&khmer, &english, &provenance(&engine, &khmer, &english)).unwrap();
    let loaded = Engine::from_app_data(&kb, &eb).unwrap();
    assert_eq!(loaded.compiled_bytes(), engine.compiled_bytes());
    let metadata: Value = serde_json::from_str(&loaded.lookup_word_metadata_json("english", "book").unwrap()).unwrap();
    assert_eq!(metadata.as_array().unwrap().len(), 1);
}

#[test]
fn app_bundles_preserve_free_form_sense_and_translation_contexts() {
    let (mut engine, original_khmer, original_english) = fixture();
    for context in ["first sense\nsecond\tline\r\n🙂".to_owned(), format!("\n{}", "x".repeat(3999))] {
        let mut khmer = original_khmer.clone();
        let mut english = original_english.clone();
        for (file, source, target) in [(&mut khmer, "សៀវភៅ", "book"), (&mut english, "book", "សៀវភៅ")] {
            file["words"][0]["context"] = json!(context);
            file["words"][0]["translations"] = json!([{"word":target,"context":context,"claimId":"pair"}]);
            file["translations"] = json!([{"source":source,"target":target,"context":context,"claimId":"pair"}]);
        }
        let (km, en) = (khmer.to_string(), english.to_string());
        engine.load_word_metadata(&km, &en).unwrap();
        let (kb, eb) = engine.compile_app_data(&km, &en, &provenance(&engine, &km, &en)).unwrap();
        let loaded = Engine::from_app_data(&kb, &eb).unwrap();
        for (language, word) in [("khmer", "សៀវភៅ"), ("english", "book")] {
            let senses: Value = serde_json::from_str(&loaded.lookup_word_metadata_json(language, word).unwrap()).unwrap();
            assert_eq!(senses[0]["context"], context);
            assert_eq!(senses[0]["translations"][0]["context"], context);
            let claims: Value = serde_json::from_str(&loaded.lookup_word_translations_json(language, word).unwrap()).unwrap();
            assert_eq!(claims[0]["context"], context);
        }
        for invalid in ["sense\0context".to_owned(), "x".repeat(4001), String::new()] {
            let mut bad = khmer.clone();
            bad["words"][0]["context"] = json!(invalid);
            bad["words"][0]["translations"][0]["context"] = json!(invalid);
            bad["translations"][0]["context"] = json!(invalid);
            assert!(engine.load_word_metadata(&bad.to_string(), &en).is_err());
        }
        let mut bad = khmer.clone(); bad["words"][0]["id"] = json!("bad\nid");
        assert!(engine.load_word_metadata(&bad.to_string(), &en).is_err());
        let senses: Value = serde_json::from_str(&engine.lookup_word_metadata_json("khmer", "សៀវភៅ").unwrap()).unwrap();
        assert_eq!(senses[0]["context"], context);
    }
}

#[test]
fn bundled_commonness_ranks_before_full_metadata_is_requested() {
    let (engine, mut khmer, mut english) = fixture();
    let mut dataset: Value = json!({"schemaVersion":3,"mappings":[],
        "words":[{"id":1,"khmer":"សៀវភៅ","rank":1,"status":"approved"}],
        "englishWords":[{"id":2,"text":"Book","rank":1,"status":"approved"}]});
    dataset["englishWords"].as_array_mut().unwrap().push(json!({"id":3,"text":"booklet","rank":0,"status":"approved"}));
    let core = Engine::from_json(&dataset.to_string()).unwrap();
    let mut other = english["words"][0].clone(); other["id"] = json!("booklet");
    other["word"] = json!("booklet"); other["commonnessLevel"] = json!(5);
    english["words"].as_array_mut().unwrap().push(other);
    for legacy in [false,true] {
        if legacy {
            khmer["format"] = json!("kmeow-word-metadata-v1");
            english["format"] = json!("kmeow-word-metadata-v1");
            english["words"][1]["commonnessLevel"] = json!(3);
        }
        let (km,en) = (khmer.to_string(),english.to_string());
        let (kb,eb) = core.compile_app_data(&km,&en,&provenance(&core,&km,&en)).unwrap();
        let loaded = Engine::from_app_data(&kb,&eb).unwrap();
        let features = khmer_engine::SuggestionFeatures {mode:khmer_engine::TypingMode::English,..Default::default()};
        let result = loaded.suggest_with_features("bo",features).unwrap();
        assert_eq!(result.candidates[0].candidate.mapping_id(),Some(2));
        assert_eq!(result.candidates[1].candidate.mapping_id(),Some(3));
        assert_eq!(loaded.compiled_bytes(),core.compiled_bytes());
    }
    assert_ne!(engine.compiled_bytes(),core.compiled_bytes());
}

#[test]
fn refuses_metadata_only_words_with_otherwise_valid_provenance() {
    for (language, word) in [("english", "table"), ("english", "café"), ("khmer", "តុ")] {
        let (engine, mut khmer, mut english) = fixture();
        let file = if language == "khmer" { &mut khmer } else { &mut english };
        let mut missing = file["words"][0].clone();
        missing["id"] = json!("not-typed"); missing["word"] = json!(word);
        file["words"].as_array_mut().unwrap().push(missing);
        let (khmer, english) = (khmer.to_string(), english.to_string());
        let original = engine.compiled_bytes().to_vec();
        let error = engine.compile_app_data(&khmer, &english, &provenance(&engine, &khmer, &english)).err().unwrap();
        assert_eq!(error.to_string(), format!("Metadata word is missing from the {language} typing dictionary: {word}"));
        assert_eq!(engine.compiled_bytes(), original);
    }
}
