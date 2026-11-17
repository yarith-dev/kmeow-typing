//! Artificial keys test ranking mechanics only, never pronunciation or word admission.
use super::*;
use crate::{Engine, SuggestionFeatures, TypingContext, suggestion_output};

fn key(text: &str, priority: i64) -> Key { Key { text: text.into(), priority } }
fn word(id: u64, khmer: &str, rank: u32, readings: Vec<Vec<Vec<Key>>>) -> Word {
    Word::new(Candidate { mapping_id: Some(id), mapping_ids: vec![], khmer: khmer.into(), rank }, readings)
}
fn index(words: Vec<Word>) -> Index {
    let mut index = Index { words, ..Index::default() };
    for (id, word) in index.words.iter().enumerate() {
        let mut prefixes = BTreeSet::new();
        for reading in &word.readings {
            collect_prefixes(reading, 0, String::new(), &mut prefixes);
            index.has_rank_priorities |= reading.iter().flatten().any(|k| k.priority != 0);
        }
        for prefix in prefixes { index.prefixes.entry(prefix).or_default().push(id); }
    }
    for ids in index.prefixes.values_mut() {
        ids.sort_by_key(|&i| (index.words[i].candidate.rank, index.words[i].candidate.mapping_id));
    }
    index
}
fn output(hit: &RankedSuggestion) -> &str {
    match &hit.suggestion.candidate { SuggestionCandidate::Khmer(c) => &c.khmer, _ => panic!() }
}

#[test]
fn component_exact_and_prefix_paths_cannot_hide_existing_count_witnesses() {
    for priority in [-3_000_000,i64::from(i32::MAX)] {
        let fallback=component_words::FALLBACK_PRIORITY+priority;
        let sounds=index(vec![word(1,"ក",0,vec![vec![vec![key("kakkich",0)]],
            vec![vec![key("kakich",fallback)]]])]);
        let exact=sounds.suggest("kakich",true);
        assert_eq!(exact[0].suggestion.preferred_input.as_deref(),Some("kakkich"));
        assert_eq!(exact[0].priority,consonant_counts::FALLBACK_PRIORITY);
        let prefix=sounds.suggest("kaki",true);
        assert_eq!(prefix[0].suggestion.completion.as_deref(),Some("kakkich"));
        assert_eq!(prefix[0].priority,consonant_counts::FALLBACK_PRIORITY);
    }
}
fn collision() -> Index {
    index(vec![word(1, "ក", 0, vec![vec![vec![key("qa", -2)]]]),
        word(2, "ខ", 50, vec![vec![vec![key("qa", 0)]]])])
}

fn load_levels(engine: &mut Engine, levels: &[(&str, u8)]) {
    let file = |language: &str| serde_json::json!({"format":"kmeow-word-metadata-v2",
        "language":language,"snapshotId":"a".repeat(64),"words":if language=="khmer" {
            levels.iter().enumerate().map(|(id,(word,level))| serde_json::json!({
                "id":id.to_string(),"word":word,"context":null,"commonnessLevel":level,
                "partsOfSpeech":[],"meanings":[],"translations":[],"sources":[]
            })).collect::<Vec<_>>()
        } else { vec![] }});
    engine.load_word_metadata(&file("khmer").to_string(),&file("english").to_string()).unwrap();
}

#[test]
fn generated_context_survives_the_prefix_limit_but_respects_exact_and_match_priority() {
    let mut words: Vec<_> = rules().consonants.iter().take(25).enumerate().map(|(id, base)|
        word(id as u64+1,&base.khmer,0,vec![vec![vec![key("qaa",0)]]])).collect();
    words.push(word(90,"សុំ",999,vec![vec![vec![key("qaz",0)]]]));
    words.push(word(91,"សូម",999,vec![vec![vec![key("q",-2)]]]));
    let mut index = index(words);
    let scores = BTreeMap::from([("សុំ".to_owned(),(1,true))]);
    assert!(!index.suggest("q",true).iter().any(|hit| output(hit)=="សុំ"));
    let ranked = index.suggest_with_context_policy("q",true,None,Some(&scores));
    assert_eq!(ranked.len(),20);
    assert_eq!(output(&ranked[0]),"សូម");
    assert!(ranked[0].suggestion.completion.is_none());
    assert_eq!(output(&ranked[1]),"សុំ");
    assert_eq!(ranked[1].suggestion.completion.as_deref(),Some("qaz"));
    // Context cannot promote a weaker match over the neutral alternatives.
    index.words[25].readings[0][0][0].priority = -1;
    index.has_rank_priorities = true;
    let ranked = index.suggest_with_context_policy("q",true,None,Some(&scores));
    assert!(!ranked.iter().any(|hit| output(hit)=="សុំ"));
    assert_eq!(index.words[25].candidate.rank,999);
}

#[test]
fn starting_rule_context_does_not_admit_fragments_or_expand_ending_rules() {
    for words in [vec!["ពាក្យ","ពាក្យសុំ"],vec!["ការ","កិច្ច","ការកិច្ច"]] {
        let core = Engine::from_json(&serde_json::json!({"schemaVersion":2,"mappings":[],
            "words":words.iter().enumerate().map(|(i,word)|serde_json::json!({
                "id":i+1,"khmer":word,"rank":0,"status":"approved"})).collect::<Vec<_>>()}).to_string()).unwrap();
        let mut scores = BTreeMap::new();
        core.rule_context.extend_scores(&[words[0].to_owned()],&mut scores, |_|false);
        assert!(scores.is_empty(),"{words:?}");
    }
}

#[test]
fn commonness_respects_path_priority_and_committed_context() {
    let mut core = engine(false);
    load_levels(&mut core,&[("ក",1),("ខ",5)]);
    let context = TypingContext { previous_words: vec!["អ".into()], ..Default::default() };
    assert_eq!(core.suggest_with_context("qa",SuggestionFeatures::default(),&context).unwrap().exact_candidates[0].khmer,"ខ");
    // Neutral paths share match strength; context wins before commonness.
    core.sound_index = std::sync::OnceLock::new();
    assert!(core.sound_index.set(index(vec![word(1,"ក",0,vec![vec![vec![key("qa",0)]]]),
        word(2,"ខ",50,vec![vec![vec![key("qa",0)]]])])).is_ok());
    load_levels(&mut core,&[("ក",5),("ខ",1)]);
    assert_eq!(suggestion_output(&core.suggest("qa").unwrap().candidates[0]),"ខ");
    let result = core.suggest_with_context("qa",SuggestionFeatures::default(),&context).unwrap();
    assert_eq!(result.exact_candidates[0].khmer,"ក");
    assert_eq!(suggestion_output(&result.result.candidates[0]),"ក");
}

#[test]
fn generated_common_words_survive_the_prefix_limit_and_merge_with_saved_prefixes() {
    let mut core = engine(true);
    let words: Vec<_> = rules().consonants.iter().take(25).enumerate().map(|(id,base)|
        word(id as u64+1,&base.khmer,id as u32,vec![vec![vec![key("qaa",0)]]])).collect();
    let last = words.last().unwrap().candidate.khmer.clone();
    core.sound_index = std::sync::OnceLock::new();
    assert!(core.sound_index.set(index(words)).is_ok());
    load_levels(&mut core,&[("ក",5),(&last,1)]);
    let result = core.suggest("q").unwrap();
    assert_eq!(result.candidates.len(),20);
    assert_eq!(suggestion_output(&result.candidates[0]),last);
    assert_eq!(result.candidates.iter().map(suggestion_output).collect::<BTreeSet<_>>().len(),20);
    // Metadata replacement reorders the prepared cache without new readings.
    load_levels(&mut core,&[("ក",1),(&last,5)]);
    let result = core.suggest("q").unwrap();
    assert_eq!(suggestion_output(&result.candidates[0]),"ក");
    assert_eq!(result.candidates[0].candidate.mapping_id(),Some(100));
}

#[test]
fn match_priority_precedes_saved_rank_and_depends_on_the_input_path() {
    let index = index(vec![word(1, "ក", 0, vec![vec![vec![key("qa", -2), key("qoa", 0)]]]),
        word(2, "ខ", 50, vec![vec![vec![key("qa", 0)]]])]);
    let hits = index.suggest("qa", true);
    assert_eq!(hits.iter().map(output).collect::<Vec<_>>(), ["ខ", "ក"]);
    assert_eq!(hits.iter().map(|h| h.priority).collect::<Vec<_>>(), [0, -2]);
    assert!(hits.iter().all(|h| h.suggestion.completion.is_none()));
    if let SuggestionCandidate::Khmer(c) = &hits[1].suggestion.candidate { assert_eq!(c.rank, 0); }
    let preferred = index.suggest("qoa", false);
    assert_eq!(output(&preferred[0]), "ក");
    assert_eq!(preferred[0].priority, 0);
    assert!(preferred[0].suggestion.completion.is_none());
    let prefix = index.suggest("q", true);
    assert_eq!(output(&prefix[0]), "ក");
    assert_eq!(prefix[0].priority, 0);
    assert_eq!(prefix[0].suggestion.completion.as_deref(), Some("qoa"));
}

#[test]
fn independent_readings_choose_best_exact_path_without_mixing_chunks() {
    let index = index(vec![word(1, "ក", 0, vec![
        vec![vec![key("a", -8)], vec![key("bc", 0)]],
        vec![vec![key("ab", 0)], vec![key("c", -1)]],
    ])]);
    assert_eq!(index.suggest("abc", true)[0].priority, -1);
    // a + c would be possible only by illegally mixing the two lattices.
    assert!(index.suggest("ac", true).is_empty());
}

#[test]
fn a_boosted_later_reading_beats_a_neutral_exact_path_and_word_rank() {
    let index = index(vec![
        word(1, "ក", 50, vec![
            vec![vec![key("qa", 0)]],
            vec![vec![key("q", 2)], vec![key("a", 1)]],
        ]),
        word(2, "ខ", 0, vec![vec![vec![key("qa", 0)]]]),
    ]);
    let hits = index.suggest("qa", false);
    assert_eq!(hits.iter().map(output).collect::<Vec<_>>(), ["ក", "ខ"]);
    assert_eq!(hits.iter().map(|h| h.priority).collect::<Vec<_>>(), [3, 0]);
    assert_eq!(index.suggest("q", true)[0].priority, 3);
}

#[test]
fn signed_paths_sum_and_equal_totals_retain_existing_rank_order() {
    let index = index(vec![
        word(1, "ក", 50, vec![vec![vec![key("q", 2)], vec![key("a", -2)]]]),
        word(2, "ខ", 0, vec![vec![vec![key("qa", 0)]]]),
        word(3, "គ", 0, vec![vec![vec![key("q", -2)], vec![key("a", -2)]]]),
    ]);
    let hits = index.suggest("qa", false);
    assert_eq!(hits.iter().map(output).collect::<Vec<_>>(), ["ខ", "ក", "គ"]);
    assert_eq!(hits.iter().map(|h| h.priority).collect::<Vec<_>>(), [0, 0, -4]);
}

#[test]
fn preferred_prefix_beyond_twenty_fallbacks_survives_the_limit() {
    let mut words = Vec::new();
    for (id, base) in rules().consonants.iter().take(21).enumerate() {
        words.push(word(id as u64 + 1, &base.khmer, id as u32,
            vec![vec![vec![key("qaa", if id == 20 { 0 } else { -2 })]]]));
    }
    let index = index(words);
    let hits = index.suggest("q", true);
    assert_eq!(hits.len(), crate::MAX_COMPLETIONS);
    assert_eq!(output(&hits[0]), index.words[20].candidate.khmer);
    assert_eq!(output(&hits[1]), index.words[0].candidate.khmer);
    // With no prefix completions requested, all twenty-one exact outputs stay
    // exact; ranking never turns a valid mapping into an uncommittable prefix.
    assert_eq!(index.suggest("qaa", false).len(), 21);
}

#[test]
fn fallback_exact_precedes_a_neutral_prefix_and_equal_tiers_keep_rank_and_id() {
    let index = index(vec![
        word(7, "ក", 30, vec![vec![vec![key("q", -8)]]]),
        word(3, "ខ", 0, vec![vec![vec![key("qa", 0)]]]),
        word(2, "គ", 0, vec![vec![vec![key("qb", 0)]]]),
    ]);
    let hits = index.suggest("q", true);
    assert_eq!(hits.iter().map(output).collect::<Vec<_>>(), ["ក", "គ", "ខ"]);
    assert!(hits[0].suggestion.completion.is_none());
    assert_eq!(hits[1].suggestion.completion.as_deref(), Some("qb"));
}

fn engine(saved: bool) -> Engine {
    let source = serde_json::json!({"schemaVersion":2,
        "mappings": if saved { serde_json::json!([{"id":100,"romanized":"qa","khmer":"ក","rank":77,"status":"approved"}]) } else { serde_json::json!([]) },
        "words":[{"id":1,"khmer":"ក","rank":0,"status":"approved"},
            {"id":2,"khmer":"ខ","rank":50,"status":"approved"},
            {"id":3,"khmer":"អ","rank":0,"status":"approved"},
            {"id":4,"khmer":"អក","rank":0,"status":"approved"}],
        "structures":[{"wordId":4,"componentIds":[3,1]}]});
    let mut engine = Engine::from_json(&source.to_string()).unwrap();
    engine.sound_index = std::sync::OnceLock::new();
    assert!(engine.sound_index.set(collision()).is_ok());
    engine
}

#[test]
fn context_cannot_promote_fallback_but_a_saved_alias_is_an_independent_neutral_match() {
    let context = TypingContext { previous_words: vec!["អ".into()], ..Default::default() };
    let engine = engine(false);
    let bytes = engine.to_binary();
    let contextual = engine.suggest_with_context("qa", SuggestionFeatures::default(), &context).unwrap();
    assert_eq!(contextual.exact_candidates.iter().map(|c| c.khmer.as_str()).collect::<Vec<_>>(), ["ខ", "ក"]);
    // Verify the fixture's context really prefers ក when all scores are neutral.
    let states = engine.dictionary.context.as_ref().unwrap().states(&context.previous_words, &engine.dictionary.pool);
    assert!(engine.dictionary.context.as_ref().unwrap().score(&states, "ក", &engine.dictionary.pool)
        > engine.dictionary.context.as_ref().unwrap().score(&states, "ខ", &engine.dictionary.pool));
    assert_eq!(engine.to_binary(), bytes);
    let saved = self::engine(true);
    let first = &saved.suggest_with_context("qa", SuggestionFeatures::default(), &context).unwrap().exact_candidates[0];
    assert_eq!(first.khmer, "ក");
    assert_eq!(first.mapping_id, Some(100));
    assert_eq!(first.rank, 77);
    assert_eq!(saved.suggest_with_features("qa", SuggestionFeatures { sound_map: false, ..Default::default() }).unwrap().candidates.len(), 1);
}

#[test]
fn ranking_metadata_does_not_change_a_checked_lexical_shape_or_drop_retained_paths() {
    let mut chunks = vec![vec![key("j", 0)], vec![key("i", -4), key("e", 0)],
        vec![key("t", 0)], vec![key("t", 0)], vec![key("or", 0)]];
    add_terminal_lexical_readings("ចិត្ត", &mut chunks, overrides::Context::new("ចិត្ត"));
    assert_eq!(chunks.len(), 1);
    assert_eq!(match_chunks(&chunks, 0, "jittor", &mut BTreeMap::new()).unwrap().priority, -4);
    assert_eq!(match_chunks(&chunks, 0, "jettor", &mut BTreeMap::new()).unwrap().priority, 0);
    assert_eq!(match_chunks(&chunks, 0, "jit", &mut BTreeMap::new()).unwrap().priority, 0);
    // A source-shape guard still rejects a missing mapped unit; metadata does
    // not weaken the spelling/syllable guard or manufacture a new short input.
    let mut incomplete = vec![vec![key("j", 0)], vec![key("i", -4)], vec![key("t", 0)]];
    add_terminal_lexical_readings("ចិត្ត", &mut incomplete, overrides::Context::new("ចិត្ត"));
    assert_eq!(incomplete.len(), 3);
}

#[test]
fn compact_ordinary_a_detector_checks_the_actual_complete_input_and_original_frame() {
    assert_eq!(compact_ordinary_a_priority("terkop", "ទឹកអប់"), Some(-1));
    assert_eq!(compact_ordinary_a_priority("toeukop", "ទឹកអប់"), Some(-1));
    for input in ["terkaop", "toeukaop", "terk'op", "terk", "terko", "terkopx"] {
        assert_eq!(compact_ordinary_a_priority(input, "ទឹកអប់"), None, "{input}");
    }
    let proof: serde_json::Value = serde_json::from_str(include_str!("../../../test/fixtures/compact-ordinary-a-boundary-priority.json")).unwrap();
    for pair in proof["structuralControls"].as_array().unwrap() {
        assert_eq!(compact_ordinary_a_priority(pair[0].as_str().unwrap(), pair[1].as_str().unwrap()), None, "{pair}");
    }
}

fn compact_alias_engine(collision: bool, joined: bool) -> Engine {
    let mut mappings = vec![serde_json::json!({"id":13281,"romanized":if joined {"terk op"} else {"terkop"},"khmer":"ទឹកអប់","rank":0,"status":"approved"}),
        serde_json::json!({"id":90,"romanized":"terkopa","khmer":"ខ","rank":0,"status":"approved"}),
        serde_json::json!({"id":91,"romanized":"a","khmer":"អ","rank":0,"status":"approved"})];
    if collision { mappings.push(serde_json::json!({"id":92,"romanized":"terkop","khmer":"ក","rank":999,"status":"approved"})); }
    Engine::from_json(&serde_json::json!({"schemaVersion":2,"mappings":mappings,
        "words":[{"id":12337,"khmer":"ទឹកអប់","rank":0,"status":"approved"},
            {"id":1,"khmer":"ក","rank":999,"status":"approved"},
            {"id":2,"khmer":"ខ","rank":0,"status":"approved"},
            {"id":3,"khmer":"អ","rank":0,"status":"approved"},
            {"id":4,"khmer":"អទឹកអប់","rank":0,"status":"approved"}],
        "structures":[{"wordId":4,"componentIds":[3,12337]}]}).to_string()).unwrap()
}

#[test]
fn compact_alias_priority_is_shared_by_conversion_suggestions_and_context_without_rank_writes() {
    let core = compact_alias_engine(true, false);
    let bytes = core.to_binary();
    assert_eq!(core.saved_match_priorities("terkop").get(&13281), Some(&-1));
    let context = TypingContext { previous_words: vec!["អ".into()], ..Default::default() };
    let index = core.dictionary.context.as_ref().unwrap();
    let states = index.states(&context.previous_words, &core.dictionary.pool);
    assert!(index.score(&states,"ទឹកអប់",&core.dictionary.pool) > index.score(&states,"ក",&core.dictionary.pool));
    for input in ["terkop", "TERKOP", "terkop "] {
        let converted = core.convert(input).unwrap();
        assert_eq!(converted.candidates.iter().map(|c| c.khmer.as_str()).collect::<Vec<_>>(), ["ក", "ទឹកអប់"]);
        for features in [SuggestionFeatures::default(), SuggestionFeatures {sound_map:false,..Default::default()}] {
            let plain = core.suggest_with_features(input, features).unwrap();
            assert_eq!(suggestion_output(&plain.candidates[0]), "ក");
            assert_eq!(suggestion_output(&plain.candidates[1]), "ទឹកអប់");
            assert!(plain.candidates[0..2].iter().all(|c| c.completion.is_none()));
            let contextual = core.suggest_with_context(input, features, &context).unwrap();
            assert_eq!(contextual.exact_candidates.iter().map(|c| c.khmer.as_str()).collect::<Vec<_>>(), ["ក", "ទឹកអប់"]);
            assert_eq!(contextual.exact_candidates[1].mapping_id, Some(13281));
            assert_eq!(contextual.exact_candidates[1].rank, 0);
        }
    }
    for input in ["terk", "terko", "terkopa", "aterkop"] { assert!(core.saved_match_priorities(input).is_empty()); }
    let joined = compact_alias_engine(true, true);
    assert!(joined.saved_match_priorities("terkop").is_empty());
    assert_eq!(joined.convert("terkop").unwrap().candidates, joined.exact_candidates("terkop",usize::MAX));
    assert_eq!(core.to_binary(), bytes);
}

#[test]
fn local_ranks_do_not_promote_a_saved_fallback_over_a_neutral_match() {
    let base = compact_alias_engine(true, false);
    let policy = serde_json::json!([{"language":"khmer","word":"ក","rank":1000000},
        {"language":"khmer","word":"ទឹកអប់","rank":0}]).to_string();
    let local = base.with_typing_overrides_json(&policy).unwrap();
    let extra = Engine::from_json(r#"{"schemaVersion":3,"mappings":[],"words":[],"englishWords":[]}"#).unwrap()
        .with_typing_overrides_json(&policy).unwrap();
    let context = TypingContext { previous_words: vec!["អ".into()], ..Default::default() };
    for features in [SuggestionFeatures::default(),SuggestionFeatures {sound_map:false,..Default::default()}] {
        let query = local.suggest_with_extension(&extra,"terkop",features,&context).unwrap();
        assert_eq!(query.exact_candidates.iter().map(|candidate|candidate.khmer.as_str()).collect::<Vec<_>>(),["ក","ទឹកអប់"]);
    }
    assert_eq!(local.to_binary(),base.to_binary());
}

#[test]
fn compact_exact_still_precedes_prefixes_and_neutral_generated_exacts_can_precede_it() {
    let core = compact_alias_engine(false, false);
    let plain = core.suggest("terkop").unwrap();
    assert_eq!(suggestion_output(&plain.candidates[0]), "ទឹកអប់");
    assert!(plain.candidates[0].completion.is_none());
    assert!(plain.candidates.iter().any(|s| s.completion.as_deref()==Some("terkopa")));
    // An artificial collision exercises merge mechanics, not pronunciation.
    let mut core = compact_alias_engine(false, false);
    core.sound_index = std::sync::OnceLock::new();
    assert!(core.sound_index.set(index(vec![word(1,"ក",999,vec![vec![vec![key("terkop",0)]]])])).is_ok());
    let plain = core.suggest("terkop").unwrap();
    assert_eq!(plain.candidates[0..2].iter().map(suggestion_output).collect::<Vec<_>>(), ["ក", "ទឹកអប់"]);
    let contextual = core.suggest_with_context("terkop",SuggestionFeatures::default(),&TypingContext::default()).unwrap();
    assert_eq!(contextual.exact_candidates.iter().map(|c| c.khmer.as_str()).collect::<Vec<_>>(), ["ក", "ទឹកអប់"]);
    let off = core.suggest_with_context("terkop",SuggestionFeatures{sound_map:false,..Default::default()},&TypingContext::default()).unwrap();
    assert_eq!(off.exact_candidates[0].mapping_id,Some(13281));
}

#[test]
fn independent_neutral_same_output_path_is_visible_to_context_even_in_a_neutral_sound_index() {
    let mut core = compact_alias_engine(true, false);
    core.sound_index = std::sync::OnceLock::new();
    assert!(core.sound_index.set(index(vec![word(12337,"ទឹកអប់",0,vec![vec![vec![key("terkop",0)]]])])).is_ok());
    let context = TypingContext { previous_words: vec!["អ".into()], ..Default::default() };
    assert_eq!(core.suggest("terkop").unwrap().candidates[0].candidate.mapping_id(),Some(13281));
    let result = core.suggest_with_context("terkop",SuggestionFeatures::default(),&context).unwrap();
    // The independent generated neutral path restores the ordinary context
    // tie break, without replacing the saved alias identity with a word ID.
    assert_eq!(result.exact_candidates[0].mapping_id,Some(13281));
    assert_eq!(result.exact_candidates[0].rank,0);
    let off = core.suggest_with_context("terkop",SuggestionFeatures{sound_map:false,..Default::default()},&context).unwrap();
    assert_eq!(off.exact_candidates[0].mapping_id,Some(92));
}

#[test]
fn outside_boundary_frames_are_tested_with_actual_independently_mapped_keys() {
    let frames = [("ទឹ","អប់"),("ទឹក","អ៊ប់"),("ទឹក","អាប"),
        ("ទឹក","អប់រ"),("កៅ","អី"),("ក្អាត","អប់"),("ទុំក","អប់")];
    let mut checked=0;
    for (prefix,suffix) in frames {
        let (Some(mut reading),Some(tail))=(mapped_chunks(prefix),mapped_chunks(suffix)) else {continue};
        reading.extend(tail);
        // Every selected key is a real mapped witness for the separate parts;
        // these artificial compounds are structural controls, not new words.
        let input=reading.iter().map(|chunk| chunk[0].text.as_str()).collect::<String>();
        assert_eq!(compact_ordinary_a_priority(&input,&format!("{prefix}{suffix}")),None,"{input}: {prefix}|{suffix}");
        checked+=1;
    }
    assert!(checked>=5,"need at least five independently mapped structural controls, got {checked}");
}
