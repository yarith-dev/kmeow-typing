//! Read-only browser views of the same lattices used for typing. Search never
//! expands the lattice; only the selected page gets a bounded set of spellings.
use super::{Index, Key};
use std::collections::BTreeSet;

const DISPLAY_INPUT_LIMIT: usize = 64;

impl Index {
    pub(crate) fn browse_matches(&self, query: &str) -> Vec<&str> {
        let joined = query.replace(' ', "");
        let search = (!joined.is_empty() && joined.bytes().all(|b| b.is_ascii_lowercase() || b == b'\''))
            .then(|| Substring::new(joined.as_bytes()));
        self.words.iter().filter(|word| word.candidate.khmer.contains(query)
            || search.as_ref().is_some_and(|search| word.readings.iter().any(|reading| search.matches(reading))))
            .map(|word| word.candidate.khmer.as_str()).collect()
    }

    pub(crate) fn browse_inputs(&self, khmer: &str) -> (Vec<String>, bool) {
        let Ok(index) = self.words.binary_search_by(|word| word.candidate.khmer.as_str().cmp(khmer)) else {
            return (Vec::new(), false);
        };
        display_inputs(&self.words[index].readings, DISPLAY_INPUT_LIMIT)
    }
}

pub(super) fn display_inputs(readings: &[Vec<Vec<Key>>], limit: usize) -> (Vec<String>, bool) {
    let mut inputs = BTreeSet::new();
    let mut truncated = false;
    for reading in readings {
        let mut prefixes = vec![String::new()];
        for chunk in reading {
            let mut next = Vec::new();
            let mut seen = BTreeSet::new();
            'choices: for prefix in &prefixes {
                for key in chunk {
                    let input = format!("{prefix}{}", key.text);
                    if input.len() > 512 || !seen.insert(input.clone()) { continue; }
                    if next.len() == limit { truncated = true; break 'choices; }
                    next.push(input);
                }
            }
            prefixes = next;
        }
        for input in prefixes {
            if input.is_empty() || input.starts_with('\'') || input.contains("''") { continue; }
            inputs.insert(input);
            if inputs.len() > limit { inputs.pop_last(); truncated = true; }
        }
    }
    (inputs.into_iter().collect(), truncated)
}

/// KMP states across chunk boundaries keep substring search linear in the
/// lattice size and query length, even for billions of possible full inputs.
struct Substring {
    query: Vec<u8>,
    failure: Vec<usize>,
}
impl Substring {
    fn new(query: &[u8]) -> Self {
        let mut failure = vec![0; query.len()];
        for i in 1..query.len() {
            let mut state = failure[i - 1];
            while state > 0 && query[i] != query[state] { state = failure[state - 1]; }
            if query[i] == query[state] { state += 1; }
            failure[i] = state;
        }
        Self { query: query.to_vec(), failure }
    }
    fn matches(&self, reading: &[Vec<Key>]) -> bool {
        let mut states = BTreeSet::from([0]);
        for chunk in reading {
            let mut next = BTreeSet::new();
            for &start in &states {
                for key in chunk {
                    let mut state = start;
                    for byte in key.bytes() {
                        while state > 0 && byte != self.query[state] { state = self.failure[state - 1]; }
                        if byte == self.query[state] { state += 1; }
                        if state == self.query.len() { return true; }
                    }
                    next.insert(state);
                }
            }
            states = next;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn substring_search_crosses_chunks_without_mixing_independent_readings() {
        let reading = vec![vec![Key::from("siev"), Key::from("seav")], vec![Key::from("phov")]];
        for query in ["ievph", "avph", "phov", "siev", "vph"] {
            assert!(Substring::new(query.as_bytes()).matches(&reading), "{query}");
        }
        for query in ["seivs", "sievseav", "phovphov"] {
            assert!(!Substring::new(query.as_bytes()).matches(&reading), "{query}");
        }
        assert_eq!(display_inputs(&[reading], 64), (vec!["seavphov".into(), "sievphov".into()], false));
    }
    #[test]
    fn display_expansion_is_bounded_but_search_still_checks_every_alternative() {
        let reading = vec![vec![Key::from("a"), Key::from("b")]; 32];
        let (inputs, truncated) = display_inputs(&[reading.clone()], 64);
        assert!(truncated);
        assert_eq!(inputs.len(), 64);
        assert!(Substring::new("bbbbbbbb".as_bytes()).matches(&reading));
        assert!(!Substring::new("abc".as_bytes()).matches(&reading));
    }
}
