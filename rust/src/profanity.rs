//! Small, versioned lexical policy shared by typing and Studio admission.
//! This is a detection list, never a source of approved typing vocabulary.
use crate::{invalid, EngineError};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeSet;

// Single data source: engine/data/profanity.json. Validated and embedded by
// build.rs, with no runtime parsing, filesystem access or per-entry allocation.
include!(concat!(env!("OUT_DIR"), "/profanity_policy.rs"));
pub const MAX_CHECK_CHARACTERS: usize = 16_384;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProfanityLanguage {
    #[default]
    Auto,
    English,
    Khmer,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfanityMatch {
    pub language: ProfanityLanguage,
    pub term: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfanityResult {
    pub policy_version: &'static str,
    pub contains_profanity: bool,
    pub matches: Vec<ProfanityMatch>,
}

// Original conservative policy selection, not an imported dictionary. Match
// complete English tokens: e.g. "Scunthorpe", "class" and "cockpit" stay clean.
// Deliberately omit ambiguous short words such as ass, cock, dick, hell and sex.
// The build sorts English terms for allocation-free binary search.

// Individual lexical checks: https://en.wiktionary.org/wiki/ចុយ and
// https://km.wiktionary.org/wiki/កណ្ដួយ . Only the vulgar terms/orthographic
// variants are selected; definitions and dictionary data are not copied.
// Longer terms can occur in unspaced Khmer text. Short terms are restricted to
// whole Khmer runs so ក្តៅ / ក្ដៅ and ក្តី / ក្ដី are not accidental matches.

fn ignored(ch: char) -> bool {
    matches!(ch, '\u{00ad}' | '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}')
}
fn cleaned(text: &str) -> Cow<'_, str> {
    if text.chars().any(ignored) {
        Cow::Owned(text.chars().filter(|&ch| !ignored(ch)).collect())
    } else {
        Cow::Borrowed(text)
    }
}
fn english_term(token: &str) -> Option<&'static str> {
    ENGLISH.binary_search_by(|term| {
        term.bytes().cmp(token.bytes().map(|b| b.to_ascii_lowercase()))
    }).ok().map(|i| ENGLISH[i])
}
fn english_tokens(text: &str) -> impl Iterator<Item = &str> {
    // Unicode letters stay in their word rather than creating ASCII substrings.
    text.split(|ch: char| ch != '_' && !ch.is_alphanumeric())
}
fn khmer_runs(text: &str) -> impl Iterator<Item = &str> {
    text.split(|ch| !('\u{1780}'..='\u{17ff}').contains(&ch))
}
fn khmer_contains(text: &str, term: &str) -> bool {
    text.match_indices(term).any(|(start, matched)| {
        // A match may not end in the middle of a larger Khmer syllable.
        let next = text[start + matched.len()..].chars().next();
        let previous = text[..start].chars().next_back();
        previous != Some('\u{17d2}')
            && !next.is_some_and(|ch| ('\u{17b4}'..='\u{17d3}').contains(&ch))
    })
}

/// Equivalence at the left edge of a clean composition suffix. Any identical
/// prefix makes suffixes in this class either all blocked or all allowed.
/// Keys come only from policy-term tails, so the number of classes is bounded
/// by the policy rather than the number of possible composed output strings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CompositionBoundary {
    first: u8,
    tail: &'static str,
    attached: bool,
    whole: &'static str,
}
pub(crate) fn composition_boundary(text: &str) -> CompositionBoundary {
    // Composition outputs contain only Khmer code points and spaces. English
    // tokens and ignored invisible separators cannot cross these joins.
    // A term completed entirely in a future multi-component prefix can be
    // neutralized by an attached sign or by extending a whole Khmer run.
    let first = match text.chars().next() {
        Some('\u{17b4}'..='\u{17d3}') => 2,
        Some('\u{1780}'..='\u{17ff}') => 1,
        _ => 0,
    };
    let mut tail = "";
    for &term in KHMER_TERMS {
        for (start, _) in term.char_indices().skip(1) {
            let candidate = &term[start..];
            if candidate.len() > tail.len() && text.starts_with(candidate) {
                tail = candidate;
            }
        }
    }
    // Every shorter matching tail is a prefix of this longest one; its next
    // character is already encoded in tail. Only the longest needs a guard.
    let attached = !tail.is_empty() && text[tail.len()..].chars().next()
        .is_some_and(|ch| ('\u{17b4}'..='\u{17d3}').contains(&ch));
    let run = khmer_runs(text).next().unwrap_or("");
    let whole = KHMER_WHOLE.iter().find_map(|&term| {
        term.char_indices().skip(1).map(|(start, _)| &term[start..])
            .find(|&candidate| candidate == run)
    }).unwrap_or("");
    CompositionBoundary { first, tail, attached, whole }
}

/// One lazy matcher for both typing's early exit and the detailed public report.
/// Callers pass cleaned text; no term scanning or language rules live elsewhere.
fn matching_terms(text: &str, language: ProfanityLanguage) -> impl Iterator<Item = ProfanityMatch> + '_ {
    let english = english_tokens(text)
        .filter_map(english_term)
        .map(|term| ProfanityMatch { language: ProfanityLanguage::English, term })
        .take(if language == ProfanityLanguage::Khmer { 0 } else { usize::MAX });
    let khmer = KHMER_TERMS.iter().copied()
        .filter(move |term| khmer_contains(text, term))
        .chain(khmer_runs(text).filter_map(|token| KHMER_WHOLE.iter().copied().find(|term| token == *term)))
        .map(|term| ProfanityMatch { language: ProfanityLanguage::Khmer, term })
        .take(if language == ProfanityLanguage::English { 0 } else { usize::MAX });
    english.chain(khmer)
}

/// Fast path for already-bounded engine strings; no allocation for ordinary text.
pub(crate) fn contains(text: &str, language: ProfanityLanguage) -> bool {
    let text = cleaned(text);
    let found = matching_terms(&text, language).next().is_some();
    found
}

/// Does not rewrite text, make network calls, load a dictionary, or infer intent.
pub fn check_profanity(text: &str, language: ProfanityLanguage) -> Result<ProfanityResult, EngineError> {
    if text.chars().count() > MAX_CHECK_CHARACTERS {
        return Err(invalid("Profanity check exceeds 16384 characters."));
    }
    let text = cleaned(text);
    let mut english = BTreeSet::new();
    let mut khmer = BTreeSet::new();
    for found in matching_terms(&text, language) {
        if found.language == ProfanityLanguage::English {
            english.insert(found.term);
        } else {
            khmer.insert(found.term);
        }
    }
    // Keep the existing report order and uniqueness without allocating repeated
    // English occurrences. This formatting is separate from matching policy.
    let matches: Vec<_> = english.into_iter()
        .map(|term| ProfanityMatch { language: ProfanityLanguage::English, term })
        .chain(KHMER_TERMS.iter().chain(KHMER_WHOLE).copied()
            .filter(|term| khmer.contains(term))
            .map(|term| ProfanityMatch { language: ProfanityLanguage::Khmer, term }))
        .collect();
    Ok(ProfanityResult { policy_version: POLICY_VERSION, contains_profanity: !matches.is_empty(), matches })
}

pub fn check_profanity_json(text: &str, language: &str) -> Result<String, EngineError> {
    let language = match language {
        "auto" => ProfanityLanguage::Auto,
        "english" => ProfanityLanguage::English,
        "khmer" => ProfanityLanguage::Khmer,
        _ => return Err(invalid("Expected profanity language auto, english or khmer.")),
    };
    serde_json::to_string(&check_profanity(text, language)?).map_err(|e| invalid(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_is_sorted_and_preserves_neutral_words_and_syllables() {
        assert!(ENGLISH.windows(2).all(|pair| pair[0] < pair[1]));
        for text in ["Scunthorpe class assistant cockpit", "sex anatomy hell", "ខឹង", "ក្តៅ", "ក្ដៅ", "ក្តី", "ក្ដី", "ចុយ៉ា"] {
            assert!(!contains(text, ProfanityLanguage::Auto), "{text}");
        }
    }

    #[test]
    fn composition_boundary_classes_preserve_policy_edge_joins() {
        let mut prefixes = vec![String::new(), "ក".into(), "ក្".into(), "ខ ".into()];
        let mut suffixes = vec![String::new(), "ក".into(), "ខ".into(), " ក".into()];
        for &term in KHMER_TERMS.iter().chain(KHMER_WHOLE) {
            prefixes.push(term.to_owned());
            for (split, _) in term.char_indices().skip(1) {
                for before in ["", "ក", "ក្", "ខ "] {
                    prefixes.push(format!("{before}{}", &term[..split]));
                }
                for after in ["", "ក", "ខ", "៉ាក", " ក"] {
                    suffixes.push(format!("{}{after}", &term[split..]));
                }
            }
        }
        suffixes.retain(|text| !contains(text, ProfanityLanguage::Khmer));
        let mut groups: std::collections::BTreeMap<_, Vec<_>> = Default::default();
        for suffix in suffixes { groups.entry(composition_boundary(&suffix)).or_default().push(suffix); }
        for suffixes in groups.values() {
            for prefix in &prefixes {
                let blocked = contains(&format!("{prefix}{}", suffixes[0]), ProfanityLanguage::Khmer);
                for suffix in &suffixes[1..] {
                    assert_eq!(contains(&format!("{prefix}{suffix}"), ProfanityLanguage::Khmer), blocked,
                        "prefix {prefix:?}, suffixes {:?} and {suffix:?}", suffixes[0]);
                }
            }
        }
    }
}
