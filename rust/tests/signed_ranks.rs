use khmer_engine::{Engine, ApprovedEnglishWord, ApprovedStatus, SuggestionFeatures, TypingMode};
use serde_json::json;

fn fixture(rank:i32)->Engine {
    Engine::from_json(&json!({"schemaVersion":3,
        "words":[{"id":1,"khmer":"កា","rank":rank,"status":"approved"},
            {"id":2,"khmer":"ខា","rank":0,"status":"approved"},
            {"id":3,"khmer":"កាខា","rank":0,"status":"approved"}],
        "mappings":[{"id":10,"romanized":"fixture","khmer":"កា","rank":rank,"status":"approved"},
            {"id":11,"romanized":"fixture","khmer":"ខា","rank":0,"status":"approved"},
            {"id":3,"romanized":"two words","khmer":"កាខា","rank":rank,"status":"approved"}],
        "englishWords":[{"id":20,"text":"alpha","rank":0,"status":"approved"},
            {"id":21,"text":"alpine","rank":rank,"status":"approved"}],
        "structures":[{"wordId":3,"componentIds":[1,2]}],"soundMapDisabledWords":["ខា"]
    }).to_string()).unwrap()
}

#[test]
fn signed_ranks_round_trip_in_exact_joined_generated_and_english_queries() {
    for rank in [-1_000_000,-1,0,1_000_000] {
        let core=fixture(rank);let binary=core.to_binary();
        assert_eq!(&binary[..8],if rank<0 {b"KMEOWC05"}else{b"KMEOWC04"});
        let loaded=Engine::from_binary(binary.clone()).unwrap();assert_eq!(loaded.to_binary(),binary);
        for input in ["fixture","fix","two words","twowords","ka","ក"] {
            assert_eq!(core.convert(input).unwrap(),loaded.convert(input).unwrap());
            assert_eq!(core.suggest(input).unwrap(),loaded.suggest(input).unwrap());
        }
        let exact=core.convert("fixture").unwrap();
        assert_eq!(exact.candidates.iter().find(|hit|hit.khmer=="កា").unwrap().rank,rank);
        assert_eq!(core.convert("twowords").unwrap().candidates[0].rank,rank);
        let features=SuggestionFeatures{mode:TypingMode::English,..Default::default()};
        assert_eq!(core.suggest_with_features("al",features).unwrap(),loaded.suggest_with_features("al",features).unwrap());
        if rank<0 {
            assert_eq!(exact.candidates[0].khmer,"កា");
            let first=core.suggest_with_features("al",features).unwrap().candidates.remove(0);
            assert!(matches!(first.candidate,khmer_engine::SuggestionCandidate::English(hit) if hit.text=="alpine"&&hit.rank==rank));
        }
    }
}

#[test]
fn signed_local_and_personal_ranks_preserve_original_bytes_and_lexical_identities() {
    let base=fixture(7);let original=base.to_binary();
    let local=base.with_typing_overrides_json(r#"[{"language":"khmer","word":"កា","rank":-10}]"#).unwrap();
    assert_eq!(local.to_binary(),original);assert_eq!(local.convert("fixture").unwrap().candidates[0].khmer,"កា");
    assert_eq!(local.convert("fixture").unwrap().candidates[0].mapping_id,Some(10));
    assert_eq!(local.convert("fixture").unwrap().candidates[0].rank,-10);
    let personal=Engine::from_personal_words_json(r#"[{"word":"កា","spellings":["personal"],"rank":-1000000}]"#).unwrap();
    assert_eq!(personal.convert("personal").unwrap().candidates[0].rank,-1_000_000);
    let merged=base.suggest_with_extension(&personal,"personal",Default::default(),&Default::default()).unwrap();
    assert!(merged.exact_candidates.iter().any(|hit|hit.khmer=="កា"&&hit.rank==-1_000_000));
    for rank in [-1_000_001,1_000_001] {
        assert!(base.with_typing_overrides_json(&json!([{"language":"khmer","word":"កា","rank":rank}]).to_string()).is_err());
        assert!(Engine::from_personal_words_json(&json!([{"word":"កា","rank":rank}]).to_string()).is_err());
    }
}

#[test]
fn streaming_english_and_block_pruning_support_negative_minimum_ranks() {
    let words=(0..40).map(|index|ApprovedEnglishWord{id:index+1,text:format!("sample{}{}",(b'a'+(index/26)as u8)as char,(b'a'+(index%26)as u8)as char),
        rank:if index==39 {-99}else{index as i32},status:ApprovedStatus::Approved});
    let core=Engine::compile_sorted_english(words).unwrap();
    let loaded=Engine::from_binary(core.to_binary()).unwrap();
    let features=SuggestionFeatures{mode:TypingMode::English,..Default::default()};
    let result=loaded.suggest_with_features("sample",features).unwrap();
    assert!(matches!(&result.candidates[0].candidate,khmer_engine::SuggestionCandidate::English(hit) if hit.text=="samplebn"&&hit.rank==-99));
    assert_eq!(result,core.suggest_with_features("sample",features).unwrap());
}

#[test]
fn legacy_readers_reject_signed_rank_payloads_and_corrupt_rank_summaries() {
    let mut binary=fixture(-1).to_binary();
    // Relabeling signed rows as C04 must fail even with a recomputed checksum.
    binary[..8].copy_from_slice(b"KMEOWC04");
    let end=binary.len()-8;
    let checksum=binary[..end].iter().fold(0xcbf29ce484222325u64,|hash,byte|(hash^u64::from(*byte)).wrapping_mul(0x100000001b3));
    binary[end..].copy_from_slice(&checksum.to_le_bytes());
    assert!(Engine::from_binary(binary).is_err());
    let mut corrupt=fixture(-1).to_binary();
    corrupt[52..56].copy_from_slice(&0i32.to_le_bytes());
    let end=corrupt.len()-8;
    let checksum=corrupt[..end].iter().fold(0xcbf29ce484222325u64,|hash,byte|(hash^u64::from(*byte)).wrapping_mul(0x100000001b3));
    corrupt[end..].copy_from_slice(&checksum.to_le_bytes());
    assert!(Engine::from_binary(corrupt).is_err());
}

#[test]
fn schema_one_aliases_and_signed_bounds_are_validated() {
    for rank in [-1_000_001,-1_000_000,-1,0,1_000_000,1_000_001] {
        let source=json!({"schemaVersion":1,"mappings":[{"id":1,"romanized":"fixture","khmer":"កា","rank":rank,"status":"approved"}]}).to_string();
        let core=Engine::from_json(&source);
        if !(-1_000_000..=1_000_000).contains(&rank) {assert!(core.is_err());continue;}
        let core=core.unwrap();
        assert_eq!(Engine::from_binary(core.to_binary()).unwrap().convert("fixture").unwrap().candidates[0].rank,rank);
    }
}
