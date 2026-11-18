//! The short inherent vowel and closure of ៈ, after a parsed syllable onset.
//! Irregular syllable splits belong in Khmer Read as spellings, not this rule.
use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Rule {
    pub enabled: bool,
    pub mark: String,
    pub a_series_keys: Vec<Key>,
    pub o_series_keys: Vec<Key>,
    pub medial_compatibility_a: Vec<Key>,
    pub medial_compatibility_o: Vec<Key>,
    pub additional_path_priority: i32,
}

// A coeng does not always form a pronounced onset cluster. In particular,
// doubled stops and nasal+stop spellings can span two syllables. Do not guess
// that split here. A dominant consonant plus a passive sonorant is unambiguous.
fn cluster_series(chars: &[char], start: usize, end: usize) -> Option<&'static str> {
    let [first, '្', second] = chars.get(start..end)? else { return None; };
    let first = consonant(*first)?;
    let second = consonant(*second)?;
    let rule = &rules().runtime.onset_cluster_governance;
    let cluster: String = chars[start..end].iter().collect();
    if rule.excluded_clusters.contains(&cluster)
        || !rule.dominant_consonants.contains(&first.khmer)
        || !rule.passive_consonants.contains(&second.khmer) { return None; }
    Some(first.series.as_str())
}

fn syllable_onset(ch: char) -> bool {
    consonant(ch).is_some() || rules().independent_vowels.iter()
        .any(|vowel|vowel.khmer.starts_with(ch) && !vowel.keys.is_empty())
}

pub(super) fn keys(chars: &[char], start: usize, mark: usize, series: &str,
    original: bool, pronunciation: bool) -> Option<Vec<Key>> {
    let rule = &rules().runtime.short_inherent_sign;
    if !original || chars.get(mark)?.to_string() != rule.mark
        || start >= mark || (start > 0 && chars[start - 1] == '្')
        || (mark + 1 < chars.len() && !syllable_onset(chars[mark + 1])) { return None; }
    let base = consonant(chars[start])?;
    let effective = match chars.get(start..mark)? {
        [_] => series,
        [_, '៉' | '៊'] => series, // Already validated by the onset parser.
        _ => cluster_series(chars, start, mark)?,
    };
    if base.onset_keys.is_empty() && chars[start] != 'អ' { return None; }
    let (keys, compatibility) = match effective {
        "a" => (&rule.a_series_keys, &rule.medial_compatibility_a),
        "o" => (&rule.o_series_keys, &rule.medial_compatibility_o),
        _ => return None,
    };
    let mut keys = keys.clone();
    // The previous explicit Read as implementation omitted medial closures.
    // Keep those accepted paths while making the full closure the first key.
    if pronunciation && mark == start + 1 && base.series == effective
        && chars.get(mark + 1).is_some_and(|&ch|consonant(ch).is_some()) {
        for key in compatibility { if !keys.contains(key) { keys.push(key.clone()); } }
    }
    (!keys.is_empty()).then_some(keys)
}

pub(super) fn has_onset(chars: &[char], start: usize) -> bool {
    let Some(base) = chars.get(start).and_then(|&ch| consonant(ch)) else { return false; };
    let mut end = start + 1;
    if chars.get(end) == Some(&'្') { end += 2; }
    if matches!(chars.get(end), Some('៉' | '៊')) { end += 1; }
    keys(chars, start, end, &base.series, true, false).is_some()
}

// Extend the existing open A-series syllable -> passive sonorant governance
// to a following explicit ៈ syllable. Never infer it across a coda or cluster.
pub(super) fn governed_onset(text: &str, chars: &[char]) -> Option<usize> {
    let first = consonant(*chars.first()?)?;
    let second = consonant(*chars.get(2)?)?;
    let rule = &rules().runtime.cross_syllable_governance;
    if rule.retained_series_words.iter().any(|word|word == text)
        || first.series != "a" || rule.passive_consonants.contains(&first.khmer)
        || rule.excluded_initials.contains(&first.khmer)
        || second.series != "o" || !rule.passive_consonants.contains(&second.khmer)
        || !chars.get(1).is_some_and(|ch| ('\u{17b6}'..='\u{17c5}').contains(ch))
        || chars.get(3)?.to_string() != rules().runtime.short_inherent_sign.mark {
        return None;
    }
    (chars.len() == 4 || syllable_onset(chars[4])).then_some(2)
}

#[cfg(test)]
mod tests {
    #[test]
    fn independent_vowels_start_a_new_syllable_after_the_sign() {
        for (word, expected) in [("គៈឥ","keakei"), ("គៈឥ","keakey"), ("កៈឯ","kakae"),
            ("ស្រៈឥ","srakei"), ("ស្រៈឥ","srakey")] {
            let inputs = crate::Engine::pronunciation_inputs(word).unwrap();
            assert!(inputs.iter().any(|input|input == expected),"{word}: {inputs:?}");
        }
        for word in ["គៈឨ", "កៈៈ", "ស្រៈា", "គៈ្", "គៈ៍"] {
            assert!(crate::Engine::pronunciation_inputs(word).is_err(),"{word}");
        }
    }

    #[test]
    fn shared_sign_keeps_series_clusters_shifters_and_internal_closures() {
        for (word, expected) in [("គៈណៈ", "keaknak"), ("កៈណៈ", "kaknak"),
            ("គៈ", "keak"), ("ណៈ", "nak"), ("ស្រៈ", "srak"),
            ("រ៉ៈ", "rak"), ("ល៉ៈ", "lak"), ("ស៊ៈ", "seak"),
            ("ឋានៈ", "thanak"), ("កាលៈទេសៈ", "kalaktesak")] {
            let inputs = crate::Engine::pronunciation_inputs(word).unwrap();
            assert!(inputs.iter().any(|input|input == expected), "{word}: {inputs:?}");
        }
        for word in ["គ៉ៈណៈ", "គាៈណៈ", "គៈៈ", "គៈា", "គៈ្", "គៈណៈ៍",
            "ម្ភៈ", "ជ្ជៈ", "ស្រាៈ", "រ៉៊ៈ", "ស្រៈា"] {
            assert!(crate::Engine::pronunciation_inputs(word).is_err(), "{word}");
        }
        // ះ remains the independent visarga vowel rule.
        assert!(!crate::Engine::pronunciation_inputs("គះ").unwrap().contains(&"keak".into()));
    }
}
