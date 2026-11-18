use khmer_engine::{ApprovedDataset, ApprovedMapping, ApprovedStatus, Candidate, Engine};
use std::collections::BTreeSet;

fn mapping(id: u64, input: &str, output: &str, rank: i32) -> ApprovedMapping {
    ApprovedMapping {
        id,
        romanized: input.into(),
        khmer: output.into(),
        rank,
        status: ApprovedStatus::Approved,
    }
}
fn engine(mappings: Vec<ApprovedMapping>) -> Engine {
    Engine::new(ApprovedDataset {
        structures: None, sound_map_disabled_words: None,
        schema_version: 1,
        mappings,
        words: None,
        english_words: None,
    })
    .unwrap()
}

#[test]
fn complete_paths_beat_dead_ends_and_exact_rows_keep_priority() {
    let mappings = vec![
        mapping(1, "a b", "ក", 0), // A greedy longest match leaves c uncovered.
        mapping(2, "a", "ខ", 2),
        mapping(3, "b c", "គ ឃ", 3),
    ];
    let composed = engine(mappings.clone()).convert("a b c").unwrap();
    assert_eq!(
        composed.candidates,
        vec![Candidate {
            mapping_id: None,
            mapping_ids: vec![2, 3],
            khmer: "ខគ ឃ".into(),
            rank: 5,
        }]
    );
    let mut with_exact = mappings;
    with_exact.push(mapping(4, "a b c", "ច", 99));
    let exact = engine(with_exact).convert("a b c").unwrap();
    assert_eq!(exact.candidates.len(), 1);
    assert_eq!(exact.candidates[0].mapping_id, Some(4));
    assert!(exact.candidates[0].mapping_ids.is_empty());
}

#[test]
fn joined_filtering_does_not_hide_later_component_or_suffix_alternatives() {
    // Each component is clean in isolation. Its join with the other component
    // is blocked for the first twenty choices, followed by clean alternatives.
    let mut rows = vec![mapping(1, "a", "ចុ", 0)];
    rows.extend((0..20).map(|i| mapping(i + 2, "b", &format!("យ{}", char::from_u32(0x1780 + i as u32).unwrap()), 0)));
    rows.extend((0..25).map(|i| mapping(i + 22, "b", &format!("ក{}", char::from_u32(0x1780 + i as u32).unwrap()), 1)));
    rows.push(mapping(100, "c", "ខ", 0));
    let core = engine(rows.clone());
    for input in ["a b", "a b c"] {
        let result = core.convert(input).unwrap();
        assert_eq!(result.candidates.len(), 20, "{input}");
        for (i, candidate) in result.candidates.iter().enumerate() {
            let mut ids = vec![1, i as u64 + 22];
            if input.ends_with(" c") { ids.push(100); }
            assert_eq!(candidate.mapping_ids, ids);
            assert!(!khmer_engine::profanity::check_profanity(&candidate.khmer,
                khmer_engine::profanity::ProfanityLanguage::Khmer).unwrap().contains_profanity);
        }
        assert_eq!(result, Engine::from_binary(core.to_binary()).unwrap().convert(input).unwrap());
        rows.reverse();
        assert_eq!(result, engine(rows.clone()).convert(input).unwrap());
    }

    let mut rows: Vec<_> = (0..20).map(|i| mapping(i + 1, "a",
        &format!("{}ចុ", char::from_u32(0x1780 + i as u32).unwrap()), 0)).collect();
    rows.extend([mapping(21, "a", "ខ", 1), mapping(22, "b", "យក", 0)]);
    let result = engine(rows).convert("a b").unwrap();
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].mapping_ids, [21, 22]);
}

#[test]
fn attached_suffix_guard_survives_the_composition_limit() {
    let mut rows = vec![mapping(1, "a", "ចុ", 0)];
    rows.extend((0..20).map(|i| mapping(i + 2, "b", &format!("យ{}", char::from_u32(0x1780 + i as u32).unwrap()), 0)));
    rows.push(mapping(22, "b", "យ៉ាក", 1));
    let result = engine(rows).convert("a b").unwrap();
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].khmer, "ចុយ៉ាក");
    assert_eq!(result.candidates[0].mapping_ids, [1, 22]);

    // Here the term is completed across two earlier components. The suffix's
    // first attached sign still changes eligibility at that later join.
    let mut rows = vec![mapping(1, "a", "ច", 0), mapping(2, "b", "ុយ", 0)];
    rows.extend((0..20).map(|i| mapping(i + 3, "c", &char::from_u32(0x1780 + i as u32).unwrap().to_string(), 0)));
    rows.push(mapping(23, "c", "៉ាក", 1));
    let result = engine(rows).convert("a b c").unwrap();
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].khmer, "ចុយ៉ាក");
    assert_eq!(result.candidates[0].mapping_ids, [1, 2, 23]);
}

// Exhaustively enumerate small inputs without pruning as an independent oracle
// for the bounded production search, including overlapping phrase boundaries.
fn enumerate(input: &str, mappings: &[ApprovedMapping]) -> Vec<Candidate> {
    if input.is_empty() {
        return vec![Candidate {
            mapping_id: None,
            mapping_ids: vec![],
            khmer: String::new(),
            rank: 0,
        }];
    }
    let mut results = vec![];
    for row in mappings {
        let tail = if input == row.romanized {
            ""
        } else {
            let Some(tail) = input.strip_prefix(&format!("{} ", row.romanized)) else {
                continue;
            };
            tail
        };
        for suffix in enumerate(tail, mappings) {
            let mut ids = vec![row.id];
            ids.extend(suffix.mapping_ids);
            results.push(Candidate {
                mapping_id: None,
                mapping_ids: ids,
                khmer: row.khmer.clone() + &suffix.khmer,
                rank: row.rank + suffix.rank,
            });
        }
    }
    results
}

#[test]
fn bounded_ranking_matches_exhaustive_unique_outputs_and_is_dataset_order_independent() {
    let mappings = vec![
        mapping(1, "a", "ក", 0),
        mapping(2, "a", "ខ", 1),
        mapping(3, "b", "គ", 2),
        mapping(4, "b", "ឃ", 0),
        mapping(5, "a b", "កគ", 9),
        mapping(6, "b a", "ង", 7),
        mapping(7, "a a", "កក", 4),
        mapping(8, "b b", "ច", 3),
    ];
    let core = engine(mappings.clone());
    let reversed = engine(mappings.iter().rev().cloned().collect());
    for count in 3..=7 {
        for bits in 0..(1 << count) {
            let input = (0..count)
                .map(|bit| if bits & (1 << bit) == 0 { "a" } else { "b" })
                .collect::<Vec<_>>()
                .join(" ");
            let mut expected = enumerate(&input, &mappings);
            expected.sort_by_key(|p| (p.mapping_ids.len(), p.rank, p.mapping_ids.clone()));
            let mut seen = BTreeSet::new();
            expected.retain(|p| seen.insert(p.khmer.clone()));
            expected.truncate(20);
            let actual = core.convert(&input).unwrap();
            assert_eq!(actual.candidates, expected, "input: {input}");
            assert_eq!(actual, reversed.convert(&input).unwrap());
        }
    }
}

#[test]
fn ambiguity_and_long_inputs_are_bounded_without_truncating_exact_candidates() {
    let mappings = (0..25)
        .map(|i| {
            mapping(
                i + 1,
                "a",
                &char::from_u32(0x1780 + i as u32).unwrap().to_string(),
                1_000_000,
            )
        })
        .collect();
    let core = engine(mappings);
    assert_eq!(core.convert("a").unwrap().candidates.len(), 25);
    let result = core.convert("a a").unwrap();
    assert_eq!(result.candidates.len(), 20);
    assert_eq!(result.candidates[0].mapping_ids, vec![1, 1]);
    assert_eq!(result.candidates[19].mapping_ids, vec![1, 20]);
    assert_eq!(result.candidates[0].rank, 2_000_000);
    let max = vec!["a"; 32].join(" ");
    let long = core.convert(&max).unwrap();
    assert_eq!(long.candidates.len(), 20);
    assert_eq!(long.candidates[0].mapping_ids.len(), 32);
    assert_eq!(long.candidates[0].rank, 32_000_000);
    assert!(core
        .convert(&vec!["a"; 33].join(" "))
        .unwrap()
        .candidates
        .is_empty());
    assert!(core
        .convert(&vec!["a"; 256].join(" "))
        .unwrap()
        .candidates
        .is_empty());
    assert!(core.convert("a unknown a").unwrap().candidates.is_empty());
    assert!(core.convert("a សួ a").unwrap().candidates.is_empty());
    assert!(core.convert("a,a").unwrap().candidates.is_empty());
}
