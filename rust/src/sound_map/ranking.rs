//! Match-path ranking, independent of dictionary ranks and research confidence.
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::sync::Arc;

// Additive choices stay below existing strict, consonant-count and component
// paths, including their bounded signed key priorities, in every ranking view.
const PRESERVE_EXISTING_PRIORITY: i64 = -(1 << 55);
pub(super) const LATEST_PREDICTION_STAGE: u8 = 4;

/// A rule's local typing choice. Plain map strings remain neutral. An explicit
/// `{ "key": "o", "rankPriority": -2 }` lowers only paths using that choice.
/// Higher signed priorities win; path totals stay separate from saved ranks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Key {
    pub text: String,
    pub priority: i64,
    pub preserve_existing: bool,
    // Prediction precedence: 0 original, 1 added glottal vowels, 2 added
    // onsets, 3 closed-vowel alternatives, 4 ending alternatives. A later stage votes only when
    // earlier stages no longer match.
    pub prediction_stage: u8,
    // Large Read as choices retain their lattice instead of its Cartesian product.
    pub paths: Option<Arc<Vec<Vec<Vec<Key>>>>>,
}
impl From<String> for Key {
    fn from(text: String) -> Self { Self { text, priority: 0, preserve_existing: false, prediction_stage: 0, paths: None } }
}
impl From<&str> for Key {
    fn from(text: &str) -> Self { text.to_owned().into() }
}
impl std::ops::Deref for Key {
    type Target = str;
    fn deref(&self) -> &str { &self.text }
}
impl Key {
    pub(super) fn additive_path(priority: i32) -> Self {
        Self { text: String::new(), priority: PRESERVE_EXISTING_PRIORITY + i64::from(priority), preserve_existing: true, prediction_stage: 0, paths: None }
    }
    pub(super) fn factored(paths: Vec<Vec<Vec<Key>>>) -> Self {
        let text=paths.first().into_iter().flatten().filter_map(|keys|keys.first()).map(|key|key.text.as_str()).collect();
        Self {text,paths:Some(Arc::new(paths)),..Self::from(String::new())}
    }
    pub fn joined(&self, next: &Self) -> Self {
        if self.paths.is_some() || next.paths.is_some() {
            return Self::factored(vec![vec![vec![self.clone()],vec![next.clone()]]]);
        }
        Self { text: format!("{}{}", self.text, next.text),
            priority: self.priority.saturating_add(next.priority),
            preserve_existing: self.preserve_existing || next.preserve_existing,
            prediction_stage: self.prediction_stage.max(next.prediction_stage), paths: None }
    }
}
impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Ranked {
            key: String,
            rank_priority: i32,
            #[serde(default)]
            preserve_existing: bool,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Encoding { Plain(String), Ranked(Ranked) }
        let (text, priority, preserve_existing) = match Encoding::deserialize(deserializer)? {
            Encoding::Plain(text) => (text, 0, false),
            Encoding::Ranked(value) => (value.key, i64::from(value.rank_priority)
                + if value.preserve_existing { PRESERVE_EXISTING_PRIORITY } else { 0 }, value.preserve_existing),
        };
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_lowercase() || b == b'\'') {
            return Err(serde::de::Error::custom("Sound keys require nonempty lowercase ASCII or apostrophe."));
        }
        Ok(Self { text, priority, preserve_existing, prediction_stage: 0, paths: None })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Match {
    pub exact: bool,
    pub key: String,
    pub priority: i64,
}
impl Match {
    // Exactness always wins, then the highest actual matched-path priority.
    // Equal scores preserve the map's existing deterministic witness order.
    pub fn better_than(&self, other: &Self) -> bool {
        (!self.exact, std::cmp::Reverse(self.priority)) < (!other.exact, std::cmp::Reverse(other.priority))
    }
}
pub(super) type Memo = BTreeMap<(usize, usize), Option<Match>>;

/// Search a chunk lattice without expanding all full spellings. Memoized
/// suffixes retain their best path; separate readings are never mixed.
pub(super) fn match_chunks(chunks: &[Vec<Key>], unit: usize, input: &str, memo: &mut Memo) -> Option<Match> {
    memo.clear();
    // Each suffix has an upper bound from the best choice in each chunk.
    // Reaching it with an exact match permits a safe early exit, including
    // signed boosts. A zero exact match alone is no longer unbeatable.
    let mut ceilings = vec![0i64; chunks.len() + 1];
    for i in (unit..chunks.len()).rev() {
        ceilings[i] = ceilings[i + 1].saturating_add(chunks[i].iter().map(|key| key.priority).max().unwrap_or(0));
    }
    match_suffix(chunks, &ceilings, unit, input, 0, memo)
}
fn match_suffix(chunks: &[Vec<Key>], ceilings: &[i64], unit: usize, input: &str, offset: usize, memo: &mut Memo) -> Option<Match> {
    if unit == chunks.len() {
        return input.is_empty().then(|| Match { exact: true, key: String::new(), priority: 0 });
    }
    // Every recursive input is a suffix of this query. Its byte offset is a
    // complete memo key and needs no allocation/copy of the remaining text.
    let state = (unit, offset);
    if let Some(result) = memo.get(&state) { return result.clone(); }
    let mut best: Option<Match> = None;
    for key in &chunks[unit] {
        let suffix = if let Some(rest) = input.strip_prefix(key.text.as_str()) {
            match_suffix(chunks, ceilings, unit + 1, rest, offset + key.len(), memo)
        } else if key.starts_with(input) {
            match_suffix(chunks, ceilings, unit + 1, "", offset + input.len(), memo).map(|mut hit| { hit.exact = false; hit })
        } else { None };
        if let Some(hit) = suffix {
            let result = Match { exact: hit.exact, key: format!("{}{}", key.text, hit.key),
                priority: key.priority.saturating_add(hit.priority) };
            if best.as_ref().is_none_or(|old| result.better_than(old)) { best = Some(result); }
            if best.as_ref().is_some_and(|hit| hit.exact && hit.priority == ceilings[unit]) { break; }
        }
    }
    memo.insert(state, best.clone());
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(text: &str, priority: i64) -> Key { Key { text: text.into(), priority, preserve_existing: false, prediction_stage: 0, paths: None } }
    fn hit(chunks: &[Vec<Key>], input: &str) -> Match {
        match_chunks(chunks, 0, input, &mut Memo::new()).unwrap()
    }

    #[test]
    fn ranked_map_choices_are_explicit_and_reject_malformed_config() {
        let plain: Key = serde_json::from_str("\"o\"").unwrap();
        let ranked: Key = serde_json::from_str(r#"{"key":"o","rankPriority":-2}"#).unwrap();
        assert_eq!(plain, key("o", 0));
        assert_eq!(ranked, key("o", -2));
        assert_eq!(serde_json::from_str::<Key>(r#"{"key":"oa","rankPriority":0}"#).unwrap(), key("oa", 0));
        assert_eq!(serde_json::from_str::<Key>(r#"{"key":"o","rankPriority":2}"#).unwrap(), key("o", 2));
        let additive: Key = serde_json::from_str(r#"{"key":"as","rankPriority":-1000000,"preserveExisting":true}"#).unwrap();
        assert!(additive.preserve_existing);
        assert_eq!(additive.priority, PRESERVE_EXISTING_PRIORITY - 1000000);
        assert!(key("k",0).joined(&additive).preserve_existing);
        for invalid in ["\"\"", r#""O""#, r#"" o""#, r#""អ""#,
            r#"{"key":"o"}"#, r#"{"key":"o","rankPenalty":1}"#,
            r#"{"key":"o","rankPriority":0.5}"#, r#"{"key":"o","rankPriority":2147483648}"#,
            r#"{"key":"o","rankPriority":-2147483649}"#,
            r#"{"key":"o","rankPriority":-2,"confidence":90}"#] {
            assert!(serde_json::from_str::<Key>(invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn ambiguous_segmentation_keeps_the_best_actual_path_not_first_match() {
        let lattice = vec![vec![key("a", -9), key("ab", 0)], vec![key("bc", 0), key("c", -2)]];
        assert_eq!(hit(&lattice, "abc"), Match { exact: true, key: "abc".into(), priority: -2 });
        assert_eq!(hit(&lattice, "a"), Match { exact: false, key: "abbc".into(), priority: 0 });
        // Scores include the untyped suffix when choosing a completion witness.
        let lattice = vec![vec![key("q", 0)], vec![key("o", -2), key("oa", 0)]];
        assert_eq!(hit(&lattice, "qo"), Match { exact: true, key: "qo".into(), priority: -2 });
        assert_eq!(hit(&lattice, "q"), Match { exact: false, key: "qoa".into(), priority: 0 });
        // The second exact segmentation beats the first neutral exact match.
        let lattice = vec![vec![key("a", 0), key("ab", 2)], vec![key("bc", 0), key("c", 1)]];
        assert_eq!(hit(&lattice, "abc").priority, 3);
    }
    #[test]
    fn bounded_signed_priorities_add_without_overflow_and_ties_keep_the_existing_witness() {
        let min = i64::from(i32::MIN);
        let max = i64::from(i32::MAX);
        let lattice = vec![vec![key("a", min)], vec![key("b", min), key("c", -10)]];
        assert_eq!(hit(&lattice, "ab").priority, 2 * min);
        assert_eq!(key("a", max).joined(&key("b", max)).priority, 2 * max);
        assert_eq!(key("a", max).joined(&key("b", min)).priority, -1);
        assert_eq!(hit(&lattice, "a").key, "ac");
        assert_eq!(hit(&vec![vec![key("a", -2)], vec![key("b", -2)]], "ab").priority, -4);
        let lattice = vec![vec![key("s", 0)], vec![key("oa", -2), key("o", -2)]];
        assert_eq!(hit(&lattice, "s").key, "soa");
    }
}
