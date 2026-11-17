use khmer_engine::{Engine, SuggestionFeatures, TypingContext};

fn source() -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 2,
        "mappings": [
            {"id":1,"romanized":"bat","khmer":"បាទ","rank":0,"status":"approved"},
            {"id":2,"romanized":"bat","khmer":"បត្រ","rank":20,"status":"approved"}
        ],
        "words": [
            {"id":2,"khmer":"បត្រ","rank":20,"status":"approved"},
            {"id":3,"khmer":"សញ្ញា","rank":0,"status":"approved"},
            {"id":4,"khmer":"សញ្ញាបត្រ","rank":0,"status":"approved"}
        ],
        "structures":[{"wordId":4,"componentIds":[3,2]}]
    })
}
fn rehash(bytes: &mut [u8]) {
    let end = bytes.len() - 8;
    let hash = bytes[..end].iter().fold(0xcbf29ce484222325u64, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    bytes[end..].copy_from_slice(&hash.to_le_bytes());
}
#[test]
fn word_only_components_need_no_romanized_alias_or_joined_index() {
    let core = Engine::from_json(&source().to_string()).unwrap();
    let context = TypingContext { previous_words: vec!["សញ្ញា".into()], ..Default::default() };
    assert_eq!(&core.to_binary()[..8], b"KMEOWC03");
    assert_eq!(
        core.suggest_with_context("bat", SuggestionFeatures::default(), &context)
            .unwrap()
            .exact_candidates[0]
            .khmer,
        "បត្រ"
    );
    assert_eq!(core.convert("bat").unwrap().candidates[0].khmer, "បាទ");
    let mut invalid = source();
    invalid["structures"][0]["componentIds"] = serde_json::json!([2, 3]);
    assert!(Engine::from_json(&invalid.to_string()).is_err());
    invalid["structures"][0]["componentIds"] = serde_json::json!([3, 999]);
    assert!(Engine::from_json(&invalid.to_string()).is_err());
}
#[test]
fn forged_context_offsets_and_edges_fail_without_panics_even_with_repaired_checksum() {
    let bytes = Engine::from_json(&source().to_string())
        .unwrap()
        .to_binary();
    let start = 32
        + (12..28)
            .step_by(4)
            .map(|at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize)
            .sum::<usize>();
    for end in 0..bytes.len() {
        assert!(Engine::from_binary(bytes[..end].to_vec()).is_err());
    }
    // Includes all directory offsets, output references, self/back edges, invalid
    // pool IDs, and malformed node counts. Valid altered graphs may still load.
    for at in start..bytes.len() - 8 {
        for byte in [0, 1, 127, 255] {
            let mut damaged = bytes.clone();
            damaged[at] = byte;
            rehash(&mut damaged);
            assert!(
                std::panic::catch_unwind(|| Engine::from_binary(damaged)).is_ok(),
                "byte {at}"
            );
        }
    }
    for at in [start, start + 4, start + 8, start + 12] {
        let mut damaged = bytes.clone();
        damaged[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        rehash(&mut damaged);
        assert!(Engine::from_binary(damaged).is_err());
    }
}
