//! Bounded, query-local one-edit recovery over the existing approved indexes.
//! No expanded vocabulary, model, learned typing history or dictionary writes.
use crate::{context, contains_profanity, Candidate, Engine, ProfanityLanguage, Suggestion,
    SuggestionCandidate, SuggestionResult, MAX_COMPLETIONS};
use std::collections::{BTreeMap, BTreeSet};

const MAX_RECOVERIES: usize = 8;

/// One insertion, deletion, substitution or adjacent transposition. Apostrophes
/// stay structural: a typo cannot manufacture or remove a required coeng អ key.
fn edits(input: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    if !(4..=32).contains(&input.len()) || input.starts_with('\'') || input.contains("''")
        || !input.bytes().all(|b| b.is_ascii_lowercase() || b == b'\'') {
        return result;
    }
    let bytes = input.as_bytes();
    for at in 0..=bytes.len() {
        for letter in b'a'..=b'z' {
            // Reserve the final size once; inserting into to_vec() otherwise
            // reallocates and moves the suffix for every possible letter.
            let mut key = String::with_capacity(input.len() + 1);
            key.push_str(&input[..at]);
            key.push(char::from(letter));
            key.push_str(&input[at..]);
            result.insert(key);
            if at < bytes.len() && bytes[at] != b'\'' && bytes[at] != letter {
                let mut key = bytes.to_vec();
                key[at] = letter;
                result.insert(String::from_utf8(key).unwrap());
            }
        }
        if at < bytes.len() && bytes[at] != b'\'' {
            let mut key = bytes.to_vec();
            key.remove(at);
            result.insert(String::from_utf8(key).unwrap());
            if at + 1 < bytes.len() && bytes[at + 1] != b'\'' && bytes[at] != bytes[at + 1] {
                let mut key = bytes.to_vec();
                key.swap(at, at + 1);
                result.insert(String::from_utf8(key).unwrap());
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn batched_saved_recovery_matches_independent_exact_lookups_for_small_and_large_ranges() {
        for count in [4, 100, 2000] {
            let mappings: Vec<_> = (0..count).map(|i| json!({
                "id": i + 1, "romanized": format!("ab{}{}{}", char::from(b'a' + (i / 676) as u8),
                    char::from(b'a' + (i / 26 % 26) as u8), char::from(b'a' + (i % 26) as u8)),
                "khmer": "ក", "rank": i % 17, "status": "approved"
            })).collect();
            let engine = Engine::from_json(&json!({"schemaVersion":1,"mappings":mappings}).to_string()).unwrap();
            for keys in [edits("abaaa"), BTreeSet::from(["abaaa".into(), "abczz".into()]), BTreeSet::new()] {
                let expected = keys.iter().flat_map(|key| engine.exact_candidates(key, usize::MAX)
                    .into_iter().map(move |candidate| (key.clone(), candidate))).collect();
                let canonical = |rows: Vec<(String, Candidate)>| {
                    let mut rows: Vec<_> = rows.into_iter().map(|(key, c)| (key, c.mapping_id, c.rank, c.khmer)).collect();
                    rows.sort(); rows
                };
                let mut actual = Vec::new();
                engine.for_each_saved_recovery(&keys, |key, candidate| actual.push((key.to_owned(), candidate)));
                assert_eq!(canonical(actual), canonical(expected), "{count}");
            }
        }
    }
}

impl Engine {
    /// Choose between one bounded scan and independent seeks. Small saved
    /// dictionaries should not pay hundreds of binary searches per keystroke;
    /// large dictionaries must not scan a broad range for a handful of edits.
    /// Feed each hit straight into deduplication; buffering every alias first
    /// makes temporary memory grow with all matches instead of unique outputs.
    fn for_each_saved_recovery(&self, keys: &BTreeSet<String>, mut emit: impl FnMut(&str, Candidate)) {
        let (Some(first), Some(last)) = (keys.first(), keys.last()) else { return; };
        let index = &self.dictionary.romanized;
        let start = index.bound(first, false);
        let end = index.bound(last, true);
        let mut visit = |row: &crate::compact::Row<'_>| {
            if !self.blocked_mapping(row) {
                emit(row.key, Candidate { mapping_id: Some(row.id), mapping_ids: Vec::new(),
                    khmer: self.dictionary.pool.get(row.target).to_owned(), rank: row.rank });
            }
        };
        if end - start > keys.len().saturating_mul(crate::compact::BLOCK) {
            for key in keys {
                let mut cursor = index.cursor(index.bound(key, false));
                while let Some(row) = cursor.next().expect("validated compact dictionary") {
                    if row.key != key { break; }
                    visit(&row);
                }
            }
            return;
        }
        let mut cursor = index.cursor(start);
        while cursor.next < end {
            let row = cursor.next().expect("validated compact dictionary").unwrap();
            if keys.contains(row.key) { visit(&row); }
        }
    }

    pub(crate) fn add_recoveries(&self, result: &mut SuggestionResult, sound_map: bool,
        base_policy: Option<&crate::compact::Dictionary>, scores: Option<&context::Scores>) {
        if result.input.chars().last().is_some_and(char::is_whitespace)
            || result.candidates.iter().any(|hit| hit.completion.is_none())
            || contains_profanity(&result.normalized_input, ProfanityLanguage::Auto) { return; }
        let keys = edits(&result.normalized_input);
        if keys.is_empty() { return; }
        let existing: BTreeSet<_> = result.candidates.iter().map(|hit| crate::suggestion_output(hit).to_owned()).collect();
        // Keep one deterministic source per output, preferring an independently
        // saved alias over a generated path, then the existing rank and identity.
        let mut recovered: BTreeMap<String, (bool, String, Candidate)> = BTreeMap::new();
        let mut recover = |generated: bool, key: &str, candidate: Candidate| {
            if existing.contains(&candidate.khmer) || !self.dictionary.khmer.contains(&candidate.khmer)
                || contains_profanity(&candidate.khmer, ProfanityLanguage::Khmer) { return; }
            let replacement = (generated, key.to_owned(), candidate);
            match recovered.entry(replacement.2.khmer.clone()) {
                std::collections::btree_map::Entry::Vacant(slot) => { slot.insert(replacement); }
                std::collections::btree_map::Entry::Occupied(mut slot) => {
                    let old = slot.get();
                    if (replacement.0, replacement.2.rank, replacement.2.mapping_id, &replacement.1)
                        < (old.0, old.2.rank, old.2.mapping_id, &old.1) { slot.insert(replacement); }
                }
            }
        };
        self.for_each_saved_recovery(&keys, |key, candidate| recover(false, key, candidate));
        if let Some(index) = self.sound_index.get().filter(|_| sound_map) {
            let mut scratch = Vec::new();
            for key in &keys {
                for candidate in index.recovery_candidates(key, base_policy, &mut scratch) {
                    recover(true, key, candidate);
                }
            }
        }
        let mut recovered: Vec<_> = recovered.into_values().collect();
        recovered.sort_by_key(|(generated, key, c)| (std::cmp::Reverse(context::score(scores, &c.khmer)),
            self.commonness.level("khmer", &c.khmer), *generated, c.rank, c.mapping_id, key.clone(), c.khmer.clone()));
        result.candidates.extend(recovered.into_iter().take(MAX_RECOVERIES).map(|(_, key, candidate)| Suggestion { tap_only: false, preferred_input: None,
            candidate: SuggestionCandidate::Khmer(candidate), completion: Some(key.clone()), correction: Some(key),
        }));
        self.place_recoveries(&mut result.candidates, None);
    }

    /// User-selected order: Super common / Very common, recoveries, then Common
    /// / Uncommon / Academic / unrated. Stable within each ordinary match group.
    pub(crate) fn place_recoveries(&self, candidates: &mut Vec<Suggestion>, extension: Option<&Engine>) {
        if !candidates.iter().any(|hit| hit.correction.is_some()) { return; }
        candidates.sort_by_key(|hit| {
            if hit.correction.is_some() { return 1; }
            // Unique personal words retain their own reviewed level. Shared
            // spellings keep the base level, just as they keep base identities.
            let source = match (&hit.candidate, extension) {
                (SuggestionCandidate::Khmer(c), Some(extra)) if !self.dictionary.khmer.contains(&c.khmer) => extra,
                _ => self,
            };
            if source.suggestion_level(hit) <= 2 { 0 } else { 2 }
        });
        // Base and personal dictionaries recover independently; the merged
        // group shares the same limit, before ordinary display slots are cut.
        let mut recoveries = 0;
        candidates.retain(|hit| {
            if hit.correction.is_none() { return true; }
            recoveries += 1;
            recoveries <= MAX_RECOVERIES
        });
        // All exact alternatives remain available to the commit path, even if
        // a very ambiguous input exceeds the usual twenty display slots.
        while candidates.len() > MAX_COMPLETIONS {
            let Some(at) = candidates.iter().rposition(|hit| hit.completion.is_some()) else { break; };
            candidates.remove(at);
        }
    }

    pub(crate) fn next_words(&self, scores: Option<&context::Scores>) -> Vec<Suggestion> {
        let mut candidates = Vec::new();
        for word in scores.into_iter().flat_map(|scores| scores.keys()) {
            if contains_profanity(word, ProfanityLanguage::Khmer) { continue; }
            let index = &self.dictionary.khmer;
            let mut cursor = index.cursor(index.bound(word, false));
            let Some(row) = cursor.next().expect("validated dictionary").filter(|row| row.key == word) else { continue; };
            candidates.push(Suggestion { tap_only: false, preferred_input: None,
                candidate: SuggestionCandidate::Khmer(Candidate {
                    mapping_id: Some(row.id), mapping_ids: Vec::new(), khmer: word.clone(), rank: row.rank,
                }), completion: Some(word.clone()), correction: None,
            });
        }
        candidates.sort_by_key(|hit| (std::cmp::Reverse(context::score(scores, crate::suggestion_output(hit))),
            self.suggestion_level(hit), hit.candidate.rank(), hit.candidate.mapping_id(), crate::suggestion_output(hit).to_owned()));
        candidates.truncate(MAX_COMPLETIONS);
        candidates
    }
}
