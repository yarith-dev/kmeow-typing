//! Explicit refactor audit: hashes complete reading lattices, not their bounded
//! browse previews, so alternative order and signed key priorities are covered.
use super::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn lattice(readings: Option<Vec<Vec<Vec<Key>>>>) -> Value {
    json!(readings.map(|readings| readings.into_iter().map(|chunks| chunks.into_iter()
        .map(|keys| keys.into_iter().map(|key| (key.text,key.priority)).collect::<Vec<_>>())
        .collect::<Vec<_>>()).collect::<Vec<_>>()))
}
fn words(value: &Value, output: &mut BTreeSet<String>) {
    match value {
        Value::String(word) if crate::valid_khmer(word) && !word.chars().any(char::is_whitespace) => { output.insert(word.clone()); },
        Value::Array(items) => for item in items { words(item,output); },
        Value::Object(fields) => for item in fields.values() { words(item,output); },
        _ => {},
    }
}

pub(super) fn fixture_words() -> BTreeSet<String> {
    let mut controls = BTreeSet::new();
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"),"/../test/fixtures")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "json") && path.file_name().unwrap() != "rule-consolidation.json" {
            let value: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            words(&value,&mut controls);
        }
    }
    controls
}

#[test]
#[ignore = "explicit before/after audit for rule refactors"]
#[cfg(feature = "bundled-dictionaries")]
fn complete_reading_fingerprint() {
    let engine = crate::Engine::bundled().unwrap();
    let index = Index::new(&engine.dictionary);
    let mut hash = Sha256::new();
    for word in &index.words {
        hash.update(serde_json::to_vec(&json!({"candidate":word.candidate,
            "readings":word.readings.iter().map(|reading| reading.iter().map(|keys|
                keys.iter().map(|key| (&key.text,key.priority)).collect::<Vec<_>>()).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "recoveryLengths":word.recovery_lengths})).unwrap());
    }
    hash.update(serde_json::to_vec(&index.prefixes).unwrap());
    hash.update([u8::from(index.has_rank_priorities)]);
    let controls = fixture_words();
    for word in &controls {
        hash.update(serde_json::to_vec(&json!([word,lattice(word_readings_mode(word,false)),lattice(word_readings_mode(word,true))])).unwrap());
    }
    println!("RULE_PARITY {} {} {:x}",index.words.len(),controls.len(),hash.finalize());
}
