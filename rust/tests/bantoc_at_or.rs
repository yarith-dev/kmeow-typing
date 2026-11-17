use khmer_engine::{ApprovedDataset, Engine};

#[test]
fn o_series_bantoc_vowels_accept_or_and_keep_other_vowels_separate() {
    let positive = [("គាត់", "koat", "kort"), ("មាត់", "moat", "mort"),
        ("ទាត់", "toat", "tort"), ("ឃាត់", "khoat", "khort"),
        ("មាន់", "moan", "morn"), ("គាប់", "koap", "korp"), ("រាល់", "roal", "rorl"),
        ("គ្រាត់", "kroat", "krort"), ("ពួកគាត់", "purkkoat", "purkkort")];
    let controls = [("កាត់", "kat", "kort"), ("ខាត់", "khat", "khort"),
        ("លាក់", "leak", "lork"),
        ("មាត", "meat", "mort")];
    let data: ApprovedDataset = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2, "mappings": [],
        "words": positive.iter().chain(controls.iter()).enumerate().map(|(i, row)|
            serde_json::json!({"id": 99000+i, "khmer": row.0, "rank": i, "status": "approved"})
        ).collect::<Vec<_>>()
    })).unwrap();
    let core = Engine::new(data).unwrap();
    let restored = Engine::from_binary(core.to_binary()).unwrap();
    for engine in [&core, &restored] {
        for (rows, accepts_alternative) in [(&positive[..], true), (&controls[..], false)] {
            for (word, existing, alternative) in rows {
                for (input, expected) in [(*existing, true), (*alternative, accepts_alternative)] {
                    let result = engine.suggest_with_context(input, Default::default(), &Default::default()).unwrap();
                    assert_eq!(result.exact_candidates.iter().any(|c| c.khmer == *word), expected, "{input} → {word}");
                }
            }
        }
    }
}
