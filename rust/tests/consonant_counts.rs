use khmer_engine::{Engine, SuggestionCandidate, SuggestionFeatures};

fn engine() -> Engine {
    Engine::from_json(r#"{"schemaVersion":2,"mappings":[],"words":[
        {"id":99100,"khmer":"លទ្ធិ","rank":0,"status":"approved"},
        {"id":99101,"khmer":"វគ្គីករណ៍","rank":0,"status":"approved"},
        {"id":99102,"khmer":"លទ្ធភាព","rank":0,"status":"approved"},
        {"id":99103,"khmer":"លទ្ធផល","rank":0,"status":"approved"}
    ]}"#).unwrap()
}
#[test]
fn generated_counts_are_exact_with_preferred_spelling_and_feature_gates() {
    let core = engine();
    let bytes = core.to_binary();
    for (word,canonical,alternate) in [("លទ្ធិ","lerthi","lertthi"),
        ("វគ្គីករណ៍","vaekkikor","vaekikor"),("លទ្ធភាព","lerthopheap","lertthopheap")] {
        let page = core.browse_words(word,None,100).unwrap();
        let row = page.words.iter().find(|row|row.khmer == word).unwrap();
        assert!(row.sound_map_inputs.iter().any(|key|key == canonical),"{word}: {:?}",row.sound_map_inputs);
        assert!(!row.sound_map_inputs.iter().any(|key|key == alternate),"{word}");
        for (input,preferred) in [(canonical,None),(alternate,Some(canonical))] {
            let result = core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
            assert!(result.exact_candidates.iter().any(|hit|hit.khmer == word),"{input}");
            let hit = result.result.candidates.iter().find(|hit|matches!(&hit.candidate,
                SuggestionCandidate::Khmer(candidate) if candidate.khmer == word)).unwrap();
            assert_eq!(hit.preferred_input.as_deref(),preferred,"{input}");
            assert!(hit.completion.is_none() && hit.correction.is_none(),"{input}");
            for features in [SuggestionFeatures {sound_map:false,..Default::default()},
                SuggestionFeatures {romanized:false,..Default::default()}] {
                assert!(core.suggest_with_context(input,features,&Default::default()).unwrap().exact_candidates.is_empty());
            }
        }
    }
    for (input,word) in [("lerttthe","លទ្ធិ"),("vaekkkikor","វគ្គីករណ៍"),
        ("leirthi","លទ្ធិ"),("vaakikor","វគ្គីករណ៍"),("vokkikor","វគ្គីករណ៍")] {
        assert!(!core.suggest_with_context(input,Default::default(),&Default::default()).unwrap()
            .exact_candidates.iter().any(|hit|hit.khmer == word),"{input}");
    }
    assert!(core.browse_words("លទ្ធផល",None,100).unwrap().words.iter()
        .find(|row|row.khmer == "លទ្ធផល").unwrap().sound_map_inputs.contains(&"lertpol".into()));
    for (prefix,word) in [("llertt","លទ្ធិ"),("vaeki","វគ្គីករណ៍")] {
        let result = core.suggest_with_context(prefix,Default::default(),&Default::default()).unwrap();
        assert!(result.result.candidates.iter().any(|hit|matches!(&hit.candidate,
            SuggestionCandidate::Khmer(candidate) if candidate.khmer == word)),"prefix {prefix}");
        assert!(!result.next_letter_weights.is_empty(),"priors {prefix}");
    }
    assert_eq!(core.to_binary(),bytes);
}

#[test]
fn explicit_spellings_stay_literal_and_win_alternate_count_collisions() {
    let core = engine();
    let personal = Engine::from_personal_words_json(r#"[
        {"word":"លទ្ធិ","spellings":["lertthi","llerrttthhi"]},
        {"word":"វគ្គីករណ៍","spellings":["my-double"]},
        {"word":"កូន","spellings":["vaekikor"]}
    ]"#).unwrap();
    let bytes = personal.to_binary();
    for (input,word) in [("lertthi","លទ្ធិ"),("llerrttthhi","លទ្ធិ"),("my-double","វគ្គីករណ៍")] {
        for sound_map in [true,false] {
            let result = core.suggest_with_extension(&personal,input,
                SuggestionFeatures {sound_map,..Default::default()},&Default::default()).unwrap();
            let hit = result.result.candidates.iter().find(|hit|matches!(&hit.candidate,
                SuggestionCandidate::Khmer(candidate) if candidate.khmer == word)).unwrap();
            assert!(hit.preferred_input.is_none(),"explicit {input}");
            assert!(result.exact_candidates.iter().any(|hit|hit.khmer == word));
        }
    }
    // Single/double tolerance must not synthesize aliases from personal keys.
    assert!(!core.suggest_with_extension(&personal,"myy-double",Default::default(),&Default::default()).unwrap()
        .exact_candidates.iter().any(|hit|hit.khmer == "វគ្គីករណ៍"));
    let result = core.suggest_with_extension(&personal,"vaekikor",Default::default(),&Default::default()).unwrap();
    let index = |word|result.exact_candidates.iter().position(|hit|hit.khmer == word).unwrap();
    assert!(index("កូន") < index("វគ្គីករណ៍"));
    let hit = result.result.candidates.iter().find(|hit|matches!(&hit.candidate,
        SuggestionCandidate::Khmer(candidate) if candidate.khmer == "វគ្គីករណ៍")).unwrap();
    assert_eq!(hit.preferred_input.as_deref(),Some("vaekkikor"));
    assert_eq!(personal.to_binary(),bytes);
}
