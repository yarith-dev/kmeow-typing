//! Read as scopes and shorthands, compiled into the parser's ordered
//! phase indexes. Cases retain their complete scope and independent readings.
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const PHASES: [(&str, &str, &str); 4] = [
    ("syllable", "fixedSyllables", "khmer"),
    ("ending", "terminalLexicalUnits", "khmer"),
    ("terminalRoot", "checkedTerminalRoots", "root"),
    ("scoped", "wordReadings", "khmer"),
];

pub(super) fn parse_rules(source: &str) -> Result<super::Rules, String> {
    let mut value: Value = serde_json::from_str(source).map_err(|error| error.to_string())?;
    compile_reading_scopes(value["runtime"].as_object_mut().ok_or("Missing runtime object")?)?;
    let mut rules: super::Rules = serde_json::from_value(value).map_err(|error| error.to_string())?;
    // Added onset spellings keep the established prefix priors until their
    // new spelling is typed. Coda fallbacks remain independent.
    for consonant in &mut rules.consonants {
        for key in &mut consonant.onset_keys {
            if key.preserve_existing { key.prediction_stage = 2; }
        }
    }
    for (vowel,keys) in &mut rules.runtime.closed_vowel_alternatives {
        if !rules.dependent_vowels.iter().chain(&rules.additional_vowel_forms).any(|row|row.khmer==*vowel)
            || keys.is_empty() || keys.iter().any(|key|!key.preserve_existing || key.priority>=0) {
            return Err("Closed-vowel alternatives require a mapped vowel and additive keys with negative priority.".into());
        }
        for key in keys {key.prediction_stage=3;}
    }
    super::ending_alternatives::validate(&rules.runtime.ending_alternatives)?;
    super::local_vowels::validate(&rules)?;
    rules.runtime.consonant_count_tolerance.validate()?;
    let sign = &rules.runtime.short_inherent_sign;
    if sign.mark != "ៈ" || sign.a_series_keys.is_empty() || sign.o_series_keys.is_empty()
        || sign.additional_path_priority >= 0 {
        return Err("The shared ៈ rule requires both vowel series and a negative additive priority".into());
    }
    if rules.runtime.word_rule_reuse.enabled && rules.runtime.word_rule_reuse.rank_priority>=0 {
        return Err("Word rule reuse requires a negative fallback priority".into());
    }
    let boundaries = &rules.runtime.nasal_boundaries;
    for (family, cases, allowed) in [
        ("written", &boundaries.written, &["ntr", "ndr", "nhR"][..]),
        ("unwritten", &boundaries.unwritten, &["stop", "aspirated"][..]),
    ] {
        let mut seen = BTreeSet::new();
        for case in cases {
            if !allowed.contains(&case.id.as_str()) {
                return Err(format!("Unknown {family} nasal boundary ID: {}. Expected {}.", case.id, allowed.join(", ")));
            }
            if !seen.insert(&case.id) {
                return Err(format!("Duplicate {family} nasal boundary ID: {}", case.id));
            }
        }
    }
    Ok(rules)
}

pub(super) fn compile_reading_scopes(runtime: &mut Map<String, Value>) -> Result<(), String> {
    // Legacy Latin-key definitions exist only in migration regression tests.
    // Production rules come exclusively from Read as and Shorthands.
    #[cfg(test)]
    let mut parts = runtime.remove("wordParts").unwrap_or_else(|| Value::Array(Vec::new()))
        .as_array().ok_or("wordParts must be an array")?.clone();
    #[cfg(not(test))]
    let mut parts = {
        if runtime.contains_key("wordParts") { return Err("wordParts is retired; use pronunciationParts".into()); }
        Vec::new()
    };
    let pronunciations = runtime.remove("pronunciationParts")
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let mut pronunciation_defaults = Vec::new();
    for pronunciation in pronunciations.as_array().ok_or("pronunciationParts must be an array")? {
        let mut part = pronunciation.as_object().ok_or("Pronunciation must be an object")?.clone();
        let word = part.remove("word").ok_or("Missing pronunciation word")?;
        let read_as = part.remove("readAs").ok_or("Missing Read as spelling")?;
        let spelling = read_as.as_str().ok_or("Read as must be a Khmer spelling")?;
        if !crate::valid_khmer(spelling) || spelling.chars().count() > 120 || spelling.contains(' ') || word == read_as {
            return Err("Invalid built-in Read as spelling".into());
        }
        pronunciation_defaults.push(serde_json::json!({"word": word, "readAs": read_as}));
        part.insert("khmer".into(), word);
        part.insert("keys".into(), serde_json::json!({"readAs": read_as}));
        parts.push(Value::Object(part));
    }
    let shorthands=runtime.remove("shorthands").unwrap_or_else(||Value::Array(Vec::new()));
    for shorthand in shorthands.as_array().ok_or("shorthands must be an array")? {
        let mut entry=shorthand.as_object().ok_or("Shorthand must be an object")?.clone();
        if entry.get("readingType").is_some_and(|kind|kind!="shorthand") {
            return Err("Entries in shorthands must have shorthand reading type".into());
        }
        entry.insert("readingType".into(),Value::String("shorthand".into()));
        parts.push(Value::Object(entry));
    }
    let mut units = BTreeSet::new();
    let mut phases: BTreeMap<&str, BTreeMap<usize, Value>> = PHASES.iter()
        .map(|(_, field, _)| (*field, BTreeMap::new())).collect();
    let mut component_readings=Vec::new();
    let mut non_reusable_word_parts = Vec::new();
    let mut shorthand_readings=Vec::new();
    for (_, field, _) in PHASES {
        if runtime.contains_key(field) { return Err(format!("Duplicate word-part table: {field}")); }
    }
    for part in &parts {
        let unit = part["khmer"].as_str().filter(|text| !text.is_empty()).ok_or("Missing word-part spelling")?;
        if !units.insert(unit) { return Err(format!("Duplicate word part: {unit}")); }
        let keys = part.get("keys").filter(|keys| {
            keys.as_array().is_some_and(|keys| !keys.is_empty()) || keys.get("readAs").is_some()
        }).ok_or("Missing word-part keys")?;
        let cases = part["cases"].as_array().filter(|cases| !cases.is_empty()).ok_or("Missing word-part cases")?;
        let standalone_only=cases.iter().all(|case|case["positions"].as_array()
            .is_some_and(|positions|!positions.is_empty()&&positions.iter().all(|p|p=="standalone")));
        let reading_type=match part.get("readingType") {
            None=>"alternative",
            Some(value)=>value.as_str().ok_or("readingType must be alternative or shorthand")?,
        };
        if !["alternative","shorthand"].contains(&reading_type) { return Err("readingType must be alternative or shorthand".into()); }
        let mut shorthand_keys=Vec::new();
        let reuse=reading_type=="alternative" && match part.get("reusableAsComponent") {
            None=>!standalone_only,
            Some(value)=>value.as_bool().ok_or("reusableAsComponent must be a boolean")? && !standalone_only,
        };
        if reuse {
            component_readings.push(serde_json::json!({"khmer":unit,"keys":keys}));
        } else {
            non_reusable_word_parts.push(Value::String(unit.into()));
        }
        for case in cases {
            let mut row = case.as_object().ok_or("Word-part case must be an object")?.clone();
            let kind = row.remove("kind").ok_or("Missing case kind")?;
            let (_, field, spelling) = PHASES.iter().find(|(name, _, _)| Some(*name) == kind.as_str())
                .ok_or("Unknown word-part case kind")?;
            let order = row.remove("order").and_then(|value| value.as_u64()).and_then(|n| usize::try_from(n).ok())
                .ok_or("Missing case order")?;
            if row.contains_key("khmer") || row.contains_key("root") { return Err("Case must use its parent spelling".into()); }
            row.insert((*spelling).into(), Value::String(unit.into()));
            row.entry("keys").or_insert_with(|| keys.clone());
            if reading_type=="shorthand" {
                // A case can replace its parent's keys. Selection policy must
                // follow the exact same effective readings as the parser.
                if let Some(keys)=row["keys"].as_array() {
                    for key in keys {if !shorthand_keys.contains(key) {shorthand_keys.push(key.clone());}}
                }
                row.insert("words".into(),serde_json::json!([unit]));
                row.insert("positions".into(),serde_json::json!(["standalone"]));
                if kind=="syllable" {row.insert("originalSpellingOnly".into(),Value::Bool(true));}
            }
            if phases.get_mut(field).unwrap().insert(order, Value::Object(row)).is_some() {
                return Err(format!("Duplicate case order in {field}"));
            }
        }
        if reading_type=="shorthand" {
            shorthand_readings.push(serde_json::json!({"khmer":unit,"keys":shorthand_keys}));
        }
    }
    for (_, field, _) in PHASES {
        let rows = phases.remove(field).unwrap();
        if rows.keys().copied().ne(0..rows.len()) { return Err(format!("Non-contiguous case order in {field}")); }
        runtime.insert(field.into(), Value::Array(rows.into_values().collect()));
    }
    runtime.insert("pronunciationDefaults".into(), Value::Array(pronunciation_defaults));
    runtime.insert("componentReadings".into(),Value::Array(component_readings));
    non_reusable_word_parts.sort_by(|a,b|a.as_str().cmp(&b.as_str()));
    runtime.insert("nonReusableWordParts".into(), Value::Array(non_reusable_word_parts));
    runtime.insert("shorthandReadings".into(),Value::Array(shorthand_readings));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn rule_loader_rejects_unknown_and_duplicate_nasal_boundary_ids() {
        let source = include_str!("../../../data/khmer-sound-map.json");
        assert!(parse_rules(source).is_ok());
        for family in ["written", "unwritten"] {
            for invalid in ["unknown", "duplicate", "wrong family"] {
                let mut value: Value = serde_json::from_str(source).unwrap();
                let rows = value["runtime"]["nasalBoundaries"][family].as_array_mut().unwrap();
                if invalid == "duplicate" {
                    rows.push(rows[0].clone());
                } else {
                    rows[0]["id"] = Value::String(if invalid == "unknown" { "typo" }
                        else if family == "written" { "stop" } else { "ntr" }.into());
                }
                let error = parse_rules(&value.to_string()).err().expect("invalid ID must fail during loading");
                assert!(error.contains(family) && error.contains("nasal boundary ID"), "{error}");
            }
            // Removing a known case intentionally disables that shape.
            let mut value: Value = serde_json::from_str(source).unwrap();
            value["runtime"]["nasalBoundaries"][family].as_array_mut().unwrap().remove(0);
            assert!(parse_rules(&value.to_string()).is_ok());
        }
    }
    #[test]
    fn registry_matches_the_pinned_case_snapshot_in_exact_order() {
        let mut rules: Value = serde_json::from_str(include_str!("../../../data/khmer-sound-map.json")).unwrap();
        let proof: Value = serde_json::from_str(include_str!("../../../test/fixtures/rule-consolidation.json")).unwrap();
        let runtime = rules["runtime"].as_object_mut().unwrap();
        let parts = runtime.get("wordParts").and_then(Value::as_array).cloned().unwrap_or_default();
        assert_eq!(parts.len(), proof["wordParts"].as_u64().unwrap() as usize);
        let shorthands=runtime["shorthands"].as_array().unwrap();
        assert_eq!(shorthands.len(),proof["shorthands"].as_u64().unwrap() as usize);
        let pronunciations=runtime["pronunciationParts"].as_array().unwrap();
        assert_eq!(pronunciations.len(), proof["pronunciationParts"].as_u64().unwrap() as usize);
        assert_eq!(parts.iter().chain(shorthands).chain(pronunciations).map(|part| part["cases"].as_array().unwrap().len()).sum::<usize>(),
            proof["cases"].as_u64().unwrap() as usize);
        compile_reading_scopes(runtime).unwrap();
        let mut tables = Map::new();
        for (_, field, _) in PHASES {
            let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&runtime[field]).unwrap()));
            tables.insert(field.into(), Value::String(hash));
        }
        assert_eq!(Value::Object(tables), proof["originalTables"],
            "Ordered case snapshot changed; review rule changes before refreshing it");
    }
    #[test]
    fn separate_shorthands_compile_to_the_identical_runtime_configuration() {
        let mut separated:Value=serde_json::from_str(include_str!("../../../data/khmer-sound-map.json")).unwrap();
        let mut legacy=separated.clone();
        let old=legacy["runtime"].as_object_mut().unwrap().remove("shorthands").unwrap();
        for entry in old.as_array().unwrap() {
            let mut entry=entry.clone();entry["readingType"]="shorthand".into();
            legacy["runtime"].as_object_mut().unwrap().entry("wordParts").or_insert_with(||serde_json::json!([])).as_array_mut().unwrap().push(entry);
        }
        compile_reading_scopes(separated["runtime"].as_object_mut().unwrap()).unwrap();
        compile_reading_scopes(legacy["runtime"].as_object_mut().unwrap()).unwrap();
        assert_eq!(separated,legacy,"Moving the source entry must not change any generated rule, key, order or priority");
    }
    #[test]
    fn ambiguous_registry_definitions_are_rejected() {
        let part = serde_json::json!({"khmer":"កម្ម","keys":["kam"],"cases":[{"kind":"syllable","order":0}]});
        for parts in [serde_json::json!([part.clone(),part.clone()]),
            serde_json::json!([{"khmer":"កម្ម","keys":["kam"],"cases":[{"kind":"unknown","order":0}]}]),
            serde_json::json!([{"khmer":"កម្ម","keys":["kam"],"cases":[{"kind":"syllable","order":1}]}])] {
            let mut runtime = Map::new();runtime.insert("wordParts".into(),parts);
            assert!(compile_reading_scopes(&mut runtime).is_err());
        }
    }
    #[test]
    fn shared_boundaries_keep_the_original_combinations() {
        let rules: Value = serde_json::from_str(include_str!("../../../data/khmer-sound-map.json")).unwrap();
        let proof: Value = serde_json::from_str(include_str!("../../../test/fixtures/rule-consolidation.json")).unwrap();
        let boundaries = &rules["runtime"]["nasalBoundaries"];
        for (family, id, original) in [("written","ntr","writtenNtrBoundary"),
            ("written","ndr","writtenNdrBoundary"),("written","nhR","writtenNhRBoundary"),
            ("unwritten","stop","unwrittenNasalStopBoundary"),
            ("unwritten","aspirated","unwrittenNasalAspiratedBoundary")] {
            let cases = boundaries[family].as_array().unwrap();
            assert_eq!(cases.iter().filter(|case| case["id"] == id).count(),1);
            let mut row = cases.iter().find(|case| case["id"] == id).unwrap().as_object().unwrap().clone();
            row.remove("id");
            if id == "ntr" {
                let clusters = row.remove("clusters").unwrap();
                assert_eq!(clusters.as_array().unwrap().len(),1);
                row.insert("cluster".into(),clusters[0].clone());
            }
            let hash = format!("{:x}",Sha256::digest(serde_json::to_vec(&row).unwrap()));
            assert_eq!(hash,proof["originalBoundaryTables"][original].as_str().unwrap());
        }
    }

    #[test]
    fn local_vowel_table_rejects_ambiguous_or_invalid_cases() {
        for invalid in ["duplicate","letter","keys","shape","empty"] {
            let mut source: Value = serde_json::from_str(include_str!("../../../data/khmer-sound-map.json")).unwrap();
            let rows = source["runtime"]["localVowelPairs"].as_array_mut().unwrap();
            match invalid {
                "duplicate" => rows.push(rows[0].clone()),
                "letter" => rows[0]["first"] = "កខ".into(),
                "keys" => rows[0]["keys"] = serde_json::json!(["or"]),
                "shape" => rows[0]["structures"] = serde_json::json!(["unknown"]),
                _ => rows[0]["structures"] = serde_json::json!([]),
            }
            assert!(parse_rules(&source.to_string()).is_err(),"{invalid}");
        }
    }
}
