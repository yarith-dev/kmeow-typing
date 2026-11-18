use khmer_engine::{ApprovedDataset, Candidate, Engine, SuggestionResult};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    input: String,
    normalized_input: String,
    ids: Vec<Option<u64>>,
    candidates: Option<Vec<Candidate>>,
}
#[derive(Deserialize)]
struct Fixtures {
    dataset: ApprovedDataset,
    cases: Vec<Case>,
    suggestions: Vec<SuggestionResult>,
}
#[test]
fn shared_conversion_fixtures() {
    let fixture: Fixtures =
        serde_json::from_str(include_str!("../../test/fixtures/conversion.json")).unwrap();
    let engine = Engine::new(fixture.dataset).unwrap();
    for case in fixture.cases {
        let result = engine.convert(&case.input).unwrap();
        assert_eq!(result.input, case.input);
        assert_eq!(result.normalized_input, case.normalized_input);
        assert_eq!(
            result
                .candidates
                .iter()
                .map(|row| row.mapping_id)
                .collect::<Vec<_>>(),
            case.ids
        );
        if let Some(expected) = case.candidates {
            assert_eq!(result.candidates, expected);
        }
    }
    for expected in fixture.suggestions {
        assert_eq!(engine.suggest(&expected.input).unwrap(), expected);
    }
}
#[test]
fn reject_unapproved_invalid_and_duplicate_data() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../test/fixtures/conversion.json")).unwrap();
    for field in ["status", "rank", "id", "romanized", "khmer"] {
        let mut data = fixture["dataset"].clone();
        data["mappings"][0][field] = match field {
            "status" => "pending".into(),
            "rank" => (-1_000_001).into(),
            "id" => 0.into(),
            "romanized" => "SUR".into(),
            _ => "hello".into(),
        };
        assert!(Engine::from_json(&data.to_string()).is_err());
    }
    let mut data = fixture["dataset"].clone();
    data["mappings"][1]["id"] = data["mappings"][0]["id"].clone();
    assert!(Engine::from_json(&data.to_string()).is_err());
    let engine = Engine::from_json(r#"{"schemaVersion":1,"mappings":[]}"#).unwrap();
    assert!(engine.convert(&"a".repeat(513)).is_err());
    assert!(engine.suggest(&"a".repeat(513)).is_err());
    assert!(engine.suggest("s").unwrap().candidates.is_empty());
    assert!(Engine::from_json(r#"{"schemaVersion":2,"mappings":[]}"#).is_err());
}
