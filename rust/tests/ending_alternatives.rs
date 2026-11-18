use khmer_engine::{Engine, ComponentStatus, Pronunciation, SuggestionFeatures};
use serde_json::{json,Value};

fn engine(words:&[&str],structures:Value)->Engine {
    Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],
        "words":words.iter().enumerate().map(|(i,word)|json!({"id":100+i,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>(),
        "structures":structures,
        "mappings":[{"id":900,"romanized":"tex","khmer":"កូន","rank":1000,"status":"approved"},
            {"id":901,"romanized":"plex","khmer":"ភ្លេច","rank":0,"status":"approved"},
            {"id":902,"romanized":"fooich","khmer":"កូន","rank":0,"status":"approved"}]
    }).to_string()).unwrap()
}
fn has(core:&Engine,input:&str,word:&str)->bool {
    core.suggest_with_context(input,Default::default(),&Default::default()).unwrap()
        .exact_candidates.iter().any(|row|row.khmer==word)
}

#[test]
fn final_ech_ich_accept_ex_ix_without_rewriting_other_endings_onsets_or_saved_aliases() {
    let mut core=engine(&["តិច","ភ្លេច","កិច្ច","តូច","ដូច","ខូច","សាច់","ឆា","កូន"],json!([]));
    let bytes=core.to_binary();let aliases=["tex","plex","fooich"].map(|input|core.convert(input).unwrap());
    for (word,inputs) in [("តិច",vec!["tech","tich","tex","tix"]),
        ("ភ្លេច",vec!["phlech","plech","phlex","plex"]),("កិច្ច",vec!["kech","kich","kex","kix"])] {
        for input in inputs {assert!(has(&core,input,word),"{input}: {word}");}
    }
    for (word,input,invalid) in [("តូច","toch","tox"),("ដូច","doch","dox"),("ខូច","khoch","khox"),
        ("សាច់","sach","sax"),("ឆា","cha","xa")] {
        assert!(has(&core,input,word));assert!(!has(&core,invalid,word));
    }
    assert!(!has(&core,"fooix","កូន"));
    let result=core.suggest_with_context("tex",Default::default(),&Default::default()).unwrap();
    assert_eq!(result.exact_candidates[0].mapping_id,Some(900));
    assert_eq!(core.suggest_with_context("plex",Default::default(),&Default::default()).unwrap().exact_candidates[0].mapping_id,Some(901));
    let prediction=Engine::from_json(&json!({"schemaVersion":3,"englishWords":[],"mappings":[],"words":[{"id":1,"khmer":"តិច","rank":0,"status":"approved"}]}).to_string()).unwrap();
    assert_eq!(json!(prediction.suggest_with_context("te",Default::default(),&Default::default()).unwrap().next_letter_weights),json!({"c":1000}));
    for features in [SuggestionFeatures{sound_map:false,..Default::default()},SuggestionFeatures{romanized:false,..Default::default()}] {
        assert!(!core.suggest_with_context("tix",features,&Default::default()).unwrap().exact_candidates.iter().any(|row|row.khmer=="តិច"));
    }
    core.set_shorthands_json(r#"[{"word":"កូន","inputs":["barich"]}]"#).unwrap();
    assert!(has(&core,"barich","កូន"));assert!(!has(&core,"barix","កូន"));
    core.set_shorthands_json("[]").unwrap();
    assert_eq!(core.to_binary(),bytes);
    for (input,expected) in ["tex","plex","fooich"].iter().zip(aliases) {assert_eq!(core.convert(input).unwrap(),expected);}
}

#[test]
fn component_endings_require_complete_unambiguous_splits_and_respect_saved_definitions() {
    let words=["កា","ខា","តិច","តិចកា","កាខា","តិចកាខា","ភ្លេច","កូន"];
    let ambiguous=engine(&words,json!([]));
    assert_eq!(ambiguous.analyze_components("តិចកាខា").unwrap().status,ComponentStatus::Ambiguous);
    assert!(!has(&ambiguous,"tixkakha","តិចកាខា"));
    let saved=engine(&words,json!([{"wordId":105,"componentIds":[102,100,101]}]));
    for input in ["techkakha","tichkakha","texkakha","tixkakha"] {assert!(has(&saved,input,"តិចកាខា"),"{input}");}
    let partial=engine(&["តិច","តិចខា","ភ្លេច","កូន"],json!([]));
    assert_eq!(partial.analyze_components("តិចខា").unwrap().status,ComponentStatus::Partial);
    assert!(!has(&partial,"tixkha","តិចខា"));
    let repeated="តិច".repeat(8);
    let many=engine(&["តិច",&repeated,"កូន","ភ្លេច"],json!([{"wordId":101,"componentIds":[100,100,100,100,100,100,100,100]}]));
    for input in ["tich".repeat(8),"tix".repeat(8),format!("{}{}","tex".repeat(4),"tich".repeat(4))] {
        assert!(has(&many,&input,&repeated),"{input}");
    }
    let compounds=engine(&["កិច្ច","ការ","កិច្ចការ","កូន","ភ្លេច"],json!([]));
    for input in ["kechka","kichka","kexka","kixka"] {assert!(has(&compounds,input,"កិច្ចការ"),"{input}");}
}

#[test]
fn read_as_and_personal_components_keep_boundaries_and_large_lattices() {
    let mut core=engine(&["កម្ម","ករ","កម្មករ","កូន","ភ្លេច"],json!([]));
    let bytes=core.to_binary();
    core.set_pronunciations(vec![Pronunciation {word:"កម្ម".into(),read_as:"តិច".into()}]).unwrap();
    for input in ["tech","tich","tex","tix"] {
        assert!(has(&core,input,"កម្ម"));assert!(has(&core,&format!("{input}kor"),"កម្មករ"));
    }
    let local=Engine::from_personal_words_json(r#"[{"word":"កម្មកូន","spellings":["personalich"]}]"#).unwrap();
    assert!(core.suggest_with_extension(&local,"tixkon",Default::default(),&Default::default()).unwrap().result.candidates.iter().any(|row|
        matches!(&row.candidate,khmer_engine::SuggestionCandidate::Khmer(hit) if hit.khmer=="កម្មកូន")));
    assert!(core.suggest_with_extension(&local,"personalix",Default::default(),&Default::default()).unwrap().result.candidates.iter().all(|row|
        !matches!(&row.candidate,khmer_engine::SuggestionCandidate::Khmer(hit) if hit.khmer=="កម្មកូន")));
    core.set_pronunciations(vec![Pronunciation {word:"កម្ម".into(),read_as:format!("{}តិច","ថា".repeat(30)).into()}]).unwrap();
    for input in [format!("{}tix","tha".repeat(30)),format!("{}tex","ta".repeat(30))] {
        assert!(has(&core,&input,"កម្ម"));assert!(has(&core,&format!("{input}kor"),"កម្មករ"));
    }
    core.set_pronunciations(vec![]).unwrap();assert_eq!(core.to_binary(),bytes);
    core.set_input_exclusions_json(r#"[{"word":"កម្ម","inputs":["tex"]}]"#).unwrap();
    assert!(!has(&core,"tex","កម្ម"));
}
