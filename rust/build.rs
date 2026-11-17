// Build tooling only. The portable core embeds these tables and performs no I/O.
use std::{collections::BTreeSet, env, fs, path::PathBuf};

fn main() {
    let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../data/profanity.json");
    println!("cargo:rerun-if-changed={}", source.display());
    let text = fs::read_to_string(&source).expect("Cannot read engine/data/profanity.json");
    let value: serde_json::Value = serde_json::from_str(&text)
        .expect("Invalid JSON in engine/data/profanity.json");
    let policy = value.as_object().expect("Profanity policy must be a JSON object");
    assert!(
        policy.len() == 4 && ["policyVersion", "english", "khmer", "khmerWhole"]
            .iter().all(|key| policy.contains_key(*key)),
        "Profanity policy requires only policyVersion, english, khmer and khmerWhole"
    );
    let version = policy["policyVersion"].as_str().expect("policyVersion must be text");
    assert!(
        version.starts_with("profanity-v") && version.len() > "profanity-v".len()
            && version.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.')),
        "Use a profanity-v… policyVersion"
    );
    let mut output = format!("pub const POLICY_VERSION: &str = {version:?};\n");
    let mut khmer_seen = BTreeSet::new();
    for (field, constant) in [("english", "ENGLISH"), ("khmer", "KHMER_TERMS"), ("khmerWhole", "KHMER_WHOLE")] {
        let entries = policy[field].as_array().expect("Profanity groups must be arrays");
        assert!(!entries.is_empty(), "Profanity group {field} must not be empty");
        let mut seen = BTreeSet::new();
        let mut terms = Vec::new();
        for entry in entries {
            let term = entry.as_str().expect("Profanity terms must be text");
            assert!(!term.is_empty() && term.chars().count() <= 512, "Invalid term length in {field}");
            assert!(seen.insert(term), "Duplicate profanity term in {field}: {term}");
            if field == "english" {
                // The matcher checks whole English tokens, never phrase substrings.
                assert!(term.bytes().all(|b| b.is_ascii_lowercase()), "English terms must use lowercase a–z: {term}");
            } else {
                assert!(term.chars().all(|ch| ('\u{1780}'..='\u{17d3}').contains(&ch)), "Invalid Khmer term: {term}");
                assert!(khmer_seen.insert(term), "Khmer term occurs in both matching groups: {term}");
            }
            terms.push(term);
        }
        // Preserve allocation-free binary search without requiring editors to sort.
        if field == "english" { terms.sort_unstable(); }
        output.push_str(&format!("const {constant}: &[&str] = &{terms:?};\n"));
    }
    let destination = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("profanity_policy.rs");
    fs::write(destination, output).expect("Cannot generate embedded profanity policy");
}
