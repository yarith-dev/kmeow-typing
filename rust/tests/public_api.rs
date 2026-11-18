use khmer_engine::{Engine, SuggestionFeatures, TypingContext};

const BASE: &str = r#"{"schemaVersion":3,"mappings":[
    {"id":1,"romanized":"a","khmer":"ក","rank":0,"status":"approved"},
    {"id":2,"romanized":"b","khmer":"ខ","rank":0,"status":"approved"}],
    "words":[{"id":1,"khmer":"ក","rank":0,"status":"approved"},
    {"id":2,"khmer":"ខ","rank":0,"status":"approved"}],"englishWords":[]}"#;

#[test]
fn personal_words_use_the_same_validated_dataset_and_shared_merge() {
    let base = Engine::from_json(BASE).unwrap();
    let binary = base.to_binary();
    let personal = Engine::from_personal_words_json(r#"[
        {"word":"សៀវភៅ","spellings":["mybook"]},
        {"word":"kmeow","language":"english"}]"#).unwrap();
    assert_eq!(personal.convert("mybook").unwrap().candidates[0].khmer, "សៀវភៅ");
    assert_eq!(base.convert_with_extension(&personal, "mybook").unwrap(), personal.convert("mybook").unwrap());
    let result = base.suggest_with_extension(&personal, "mybook", SuggestionFeatures::default(), &TypingContext::default()).unwrap();
    assert_eq!(result.result.candidates[0].candidate, personal.suggest("mybook").unwrap().candidates[0].candidate);
    assert_eq!(base.to_binary(), binary);
}

#[test]
fn personal_exact_readings_precede_compositions_and_keep_base_duplicates() {
    let base = Engine::from_json(BASE).unwrap();
    // The same output is a base composition but a personal exact reading.
    let personal = Engine::from_personal_words_json(r#"[
        {"word":"កខ","spellings":["a b"]},
        {"word":"ក","spellings":["a"]}]"#).unwrap();
    let merged = base.convert_with_extension(&personal, "a b").unwrap();
    assert_eq!(merged.candidates.len(), 1);
    assert!(merged.candidates[0].mapping_id.is_some());
    assert_eq!(base.convert_with_extension(&personal, "a").unwrap(), base.convert("a").unwrap());
    let withheld = base.with_typing_overrides_json(r#"[{"language":"khmer","word":"ក","excluded":true}]"#).unwrap();
    assert!(withheld.convert_with_extension(&personal, "a").unwrap().candidates.is_empty());
}

#[test]
fn public_personal_input_rejects_invalid_fields_instead_of_ignoring_them() {
    for input in [r#"[{"word":"x","language":"english","spellings":["x"]}]"#,
        r#"[{"word":"x","language":"english","rank":-1000001}]"#,
        r#"[{"word":"x","language":"english","rank":1000001}]"#,
        r#"[{"word":"x","language":"english","rnak":1}]"#,
        r#"[{"word":"ក","spellings":["UPPER"]}]"#,
        r#"[{"word":"ក"},{"word":"ក"}]"#, r#"{}"#,
        r#"[["ក","khmer",[],0]]"#, r#"[{"word":"ក","word":"ខ"}]"#] {
        assert!(Engine::from_personal_words_json(input).is_err(), "{input}");
    }
}

#[cfg(feature = "bundled-dictionaries")]
#[test]
fn bundled_native_factory_retains_exact_current_dictionary_bytes_and_results() {
    let simple = Engine::bundled().unwrap();
    let baseline = Engine::from_app_data(include_bytes!("../../compiled-data/khmer.kmd"),
        include_bytes!("../../compiled-data/english.kmd")).unwrap();
    assert_eq!(simple.to_binary(), baseline.to_binary());
    for input in ["", "s", "sievphov", "angkor", "niyom", "ក្ត", "books", "im"] {
        assert_eq!(simple.convert(input).unwrap(), baseline.convert(input).unwrap());
        assert_eq!(simple.suggest(input).unwrap(), baseline.suggest(input).unwrap());
    }
}
