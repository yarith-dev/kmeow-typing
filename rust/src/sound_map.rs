//! Sound lookups are derived from the caller's approved Khmer words, never saved
//! aliases. The pinned ASCII choices live in data/khmer-sound-map.json. Mapped
//! letters remain usable without a reviewed pronunciation rule. Rules override
//! their local reading; genuinely unmapped signs still have no generated key.
use crate::{compact, Candidate, Suggestion, SuggestionCandidate};
use serde::Deserialize;
mod ranking;
mod browse;
mod input_pages;
mod next_letters;
mod config;
mod factored;
mod overrides;
mod local_vowels;
mod consonant_counts;
mod component_words;
mod read_as;
mod two_dot;
mod ending_alternatives;
use ranking::{Key, Match, match_chunks};
#[cfg(test)]
mod ranking_tests;
#[cfg(test)]
mod parity;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard, OnceLock};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Consonant {
    khmer: String,
    series: String,
    onset_keys: Vec<Key>,
    #[serde(default)]
    coda_fallback_keys: Option<Vec<Key>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Vowel {
    khmer: String,
    a_series_keys: Vec<Key>,
    o_series_keys: Vec<Key>,
    #[serde(default)]
    cluster_governance: Option<bool>,
}
#[derive(Deserialize)]
struct Independent {
    khmer: String,
    keys: Vec<Key>,
}
#[derive(Deserialize)]
struct IndependentCoengY {
    bases: Vec<String>,
    subscript: String,
    keys: Vec<Key>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinalCluster {
    khmer: String,
    series: String,
    inherent_keys: Vec<Key>,
    coda_keys: Vec<Key>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VowelBeforeCoda {
    vowel: String,
    coda: String,
    a_series_keys: Vec<Key>,
    o_series_keys: Vec<Key>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExplicitBantoc {
    vowel: String,
    a_series_keys: Vec<Key>,
    o_series_velar_keys: Vec<Key>,
    o_series_other_keys: Vec<Key>,
    #[serde(default)]
    o_series_keys_by_final: BTreeMap<String, Vec<Key>>,
    velar_finals: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrossSyllableGovernance {
    passive_consonants: Vec<String>,
    excluded_initials: Vec<String>,
    retained_series_words: Vec<String>,
    nasal_first_syllable_vowels: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SilentCodaCluster {
    khmer: String,
    coda: String,
    #[serde(default)]
    following_vowel_governance: Option<FollowingVowelGovernance>,
    #[serde(default)]
    initial_inherent_keys: BTreeMap<String, Vec<Key>>,
    #[serde(default)]
    vowel_substitutions: Vec<CodaVowelSubstitution>,
}
#[derive(Deserialize)]
struct CodaVowelSubstitution {
    onset: String,
    from: String,
    to: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FollowingVowelGovernance {
    onsets: Vec<String>,
    vowels: Vec<String>,
    target_series: String,
    #[serde(default)]
    terminal_mark: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalSilentCodaCluster {
    khmer: String,
    coda: String,
    preceding_vowels: Vec<String>,
    #[serde(default)]
    preceding_onsets: Vec<String>,
    #[serde(default)]
    original_terminal_required: bool,
}
#[derive(Deserialize)]
struct WrittenVowelContext {
    consonant: String,
    vowel: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalShortCodaCluster {
    khmer: String,
    coda: String,
    contexts: Vec<WrittenVowelContext>,
    retained_keys: Vec<Key>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalSilentGroup {
    khmer: String,
    vowel: String,
    preceding_consonants: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairVowelReading {
    coda: String,
    onset: String,
    vowel: String,
    final_consonant: String,
    first_vowel_keys: Vec<Key>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NasalPairGovernance {
    initials: Vec<String>,
    codas: Vec<String>,
    onsets: Vec<String>,
    vowels: Vec<String>,
    additional_shapes: Vec<NasalPairShape>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NasalPairShape {
    coda: String,
    onset: String,
    vowel: String,
    silent_finals: Vec<String>,
    #[serde(default)]
    initials: Option<Vec<String>>,
    #[serde(default)]
    allowed_finals: Option<Vec<String>>,
    #[serde(default)]
    allow_open: Option<bool>,
    #[serde(default)]
    allow_bantoc: Option<bool>,
}
#[derive(Deserialize)]
struct OptionalInitialOnset {
    base: String,
    subscripts: Vec<String>,
    keys: Vec<Key>,
}
#[derive(Deserialize)]
struct TerminalReadingUnit {
    khmer: String,
    retained: String,
    words: Vec<String>,
}
#[derive(Deserialize)]
struct FixedSyllable {
    khmer: String,
    keys: read_as::Keys,
    #[serde(default, rename = "originalSpellingOnly")]
    original_spelling_only: bool,
    #[serde(default)]
    words: Vec<String>,
    #[serde(default)]
    positions: Vec<WordReadingPosition>,
}
impl FixedSyllable {
    #[cfg(test)]
    fn checked_end(&self, word: &str, chars: &[char], start: usize, original: bool) -> Option<usize> {
        self.checked_end_in_context(overrides::Context::new(word).with_original(original),chars,start)
    }

    fn checked_end_in_context(&self, context: overrides::Context<'_>, chars: &[char], start: usize) -> Option<usize> {
        if self.original_spelling_only && !context.original
            || (!self.words.is_empty() && !self.words.iter().any(|entry|entry == context.word)) {
            return None;
        }
        let end = segment_end(chars,start,&self.khmer)?;
        if !clear_segment_end(chars,end) { return None; }
        if self.words.is_empty() && self.positions.is_empty() { return Some(end); }
        let original: Vec<_> = context.word.chars().collect();
        let original_start = context.offset + start;
        let original_end = context.offset + end;
        if segment_end(&original,original_start,&self.khmer) != Some(original_end)
            || !clear_segment_end(&original,original_end)
            // A genuine fragment boundary is allowed; cancellation must not
            // expose a new terminal match at an earlier character offset.
            || (end == chars.len() && original_end != context.end) {
            return None;
        }
        (self.positions.is_empty()
            || word_position_matches(&self.positions,original_start,original_end,original.len())).then_some(end)
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WordReading {
    khmer: String,
    keys: read_as::Keys,
    words: Vec<String>,
    positions: Vec<WordReadingPosition>,
    #[serde(default)]
    preserve_mapped_prefix_words: Vec<String>,
    #[serde(default)]
    independently_mapped_suffix_words: Vec<String>,
    #[serde(default)]
    checked_suffix_readings: Vec<CheckedComponentReading>,
    #[serde(default)]
    checked_prefix_readings: Vec<CheckedComponentReading>,
    #[serde(default)]
    checked_context_readings: Vec<CheckedComponentContext>,
    #[serde(default)]
    preserve_mapped: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckedComponentReading {
    word: String,
    khmer: String,
    keys: Vec<Key>,
    #[serde(default)]
    original_word_only: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckedComponentContext {
    word: String,
    prefix: String,
    prefix_keys: Vec<Key>,
    suffix: String,
    suffix_keys: Vec<Key>,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum WordReadingPosition {
    Start,
    Middle,
    End,
    Anywhere,
    Standalone,
}
fn word_position_matches(positions: &[WordReadingPosition], start: usize, end: usize, original_len: usize) -> bool {
    if start >= end || end > original_len { return false; }
    let position = match (start == 0, end == original_len) {
        (true, true) => WordReadingPosition::Standalone,
        (true, false) => WordReadingPosition::Start,
        (false, true) => WordReadingPosition::End,
        (false, false) => WordReadingPosition::Middle,
    };
    positions.contains(&position)
        || (position != WordReadingPosition::Standalone && positions.contains(&WordReadingPosition::Anywhere))
}
fn original_part_end(word: &str, chars: &[char], start: usize, spelling: &str) -> Option<(usize, usize)> {
    let end = segment_end(chars,start,spelling)?;
    let original: Vec<_> = word.chars().collect();
    if segment_end(&original,start,spelling) != Some(end)
        || !clear_segment_end(&original,end) || !clear_segment_end(chars,end)
        || (end == chars.len() && end != original.len()) {
        return None;
    }
    Some((end,original.len()))
}
impl WordReading {
    fn allows_position(&self, start: usize, end: usize, original_len: usize) -> bool {
        word_position_matches(&self.positions,start,end,original_len)
    }
    fn checked_end(&self, word: &str, chars: &[char], start: usize) -> Option<usize> {
        if !self.words.iter().any(|checked| checked == word) || self.keys.is_empty() {
            return None;
        }
        let (end, original_len) = original_part_end(word,chars,start,&self.khmer)?;
        self.allows_position(start,end,original_len).then_some(end)
    }
}
#[derive(Deserialize)]
struct SilentWordEnding {
    ending: String,
    words: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReviewedWordFinalCoda {
    consonant: String,
    words: Vec<String>,
    keys: Vec<Key>,
    retain_mapped_keys: bool,
}
#[derive(Deserialize)]
struct RepeatedInitialPresyllable {
    words: Vec<String>,
}
#[derive(Deserialize)]
struct CheckedAamNgBoundary {
    words: Vec<String>,
}
#[derive(Deserialize)]
struct ShortInherentBeforeCoda {
    coda: String,
    series: String,
    keys: Vec<Key>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GlottalOnset {
    subscript: String,
    keys: Vec<Key>,
    series: String,
    inherent_keys: Vec<Key>,
    optional_onset_keys: BTreeMap<String, Vec<Key>>,
    #[serde(default)]
    additional_vowel_keys: BTreeMap<String, Vec<Key>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScopedOnsetCluster {
    khmer: String,
    series: String,
    onset_keys: Vec<Key>,
    words: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OnsetClusterGovernance {
    dominant_consonants: Vec<String>,
    passive_consonants: Vec<String>,
    excluded_clusters: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalWrittenClusterGovernance {
    pairs: Vec<String>,
    target_series: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WeakRSecondVowelGovernance {
    first_clusters: Vec<String>,
    target: String,
    target_series: String,
    #[serde(default)]
    allowed_finals: Vec<String>,
    #[serde(default)]
    allow_open: bool,
    #[serde(default)]
    allow_bantoc: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalCancellation {
    mark: String,
    attached_vowels: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarkedSonorantGovernance {
    onsets: Vec<String>,
    blocked_prefix_marks: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RiyaGovernance {
    unit: String,
    controller_series: String,
    target_series: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenLaaGovernance {
    unit: String,
    controller_series: String,
    target_series: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitialSamaaGovernance {
    prefix: String,
    target_series: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckedRootContinuation {
    root: String,
    keys: read_as::Keys,
    words: Vec<String>,
    #[serde(default)]
    preserve_mapped: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalLexicalUnit {
    khmer: String,
    keys: read_as::Keys,
    #[serde(default)]
    retained_readings: Vec<Vec<Key>>,
    #[serde(default)]
    retained_chunk_groups: Vec<Vec<Vec<Key>>>,
    #[serde(default)]
    words: Vec<String>,
}
#[derive(Deserialize)]
struct NasalBoundaries {
    written: Vec<NasalBoundary>,
    unwritten: Vec<NasalBoundary>,
}
#[derive(Deserialize)]
struct NasalBoundary {
    id: String,
    initials: Vec<String>,
    clusters: Vec<String>,
    #[serde(default)]
    finals: Option<Vec<String>>,
}
#[derive(Deserialize)]
struct WrittenNhRBoundary {
    initials: Vec<String>,
    clusters: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WrittenNhBoundary {
    first_vowels: Vec<String>,
    cluster: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarkedRhyme {
    ending: String,
    series: String,
    keys: Vec<Key>,
    #[serde(default)]
    composable: bool,
    #[serde(default)]
    composable_onsets: Vec<String>,
    #[serde(default)]
    original_spelling_only: bool,
    #[serde(default)]
    whole_word: Option<bool>,
    #[serde(default)]
    sonorant_governance: Option<MarkedSonorantGovernance>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalPlainASign {
    mark: String,
    series: String,
    keys: Vec<Key>,
    excluded_onsets: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminalWrittenSameOSign {
    mark: String,
    series: String,
    keys: Vec<Key>,
}
#[derive(Deserialize)]
struct ExplicitGlottalBantoc {
    vowel: String,
    mark: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompactOrdinaryABoundaryPriority {
    rank_priority: i32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedInputMatchPriority {
    input: String,
    khmer: String,
    rank_priority: i32,
}
#[derive(Deserialize)]
struct InitialCarrierNasalFrame {
    nasal: String,
    subscripts: Vec<String>,
}
#[derive(Deserialize)]
struct InitialCarrierNasal {
    frames: Vec<InitialCarrierNasalFrame>,
    keys: Vec<Key>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Runtime {
    inherent: BTreeMap<String, Vec<Key>>,
    short_inherent: BTreeMap<String, Vec<Key>>,
    closed_inherent: BTreeMap<String, Vec<Key>>,
    #[serde(default)]
    local_vowel_pairs: Vec<local_vowels::Pair>,
    #[serde(default)]
    consonant_count_tolerance: consonant_counts::Settings,
    #[serde(default)]
    initial_carrier_nasal: Option<InitialCarrierNasal>,
    #[serde(default)]
    initial_carrier_direct_vowel: Vec<Key>,
    codas: BTreeMap<String, Vec<Key>>,
    word_final_silent_consonants: Vec<String>,
    final_clusters: Vec<FinalCluster>,
    vowels_before_coda: Vec<VowelBeforeCoda>,
    explicit_bantoc: ExplicitBantoc,
    cross_syllable_governance: CrossSyllableGovernance,
    silent_coda_clusters: Vec<SilentCodaCluster>,
    terminal_silent_coda_clusters: Vec<TerminalSilentCodaCluster>,
    terminal_short_coda_clusters: Vec<TerminalShortCodaCluster>,
    terminal_silent_groups: Vec<TerminalSilentGroup>,
    pair_vowel_readings: Vec<PairVowelReading>,
    nasal_pair_governance: NasalPairGovernance,
    optional_initial_onsets: Vec<OptionalInitialOnset>,
    terminal_inherent_boundary_codas: Vec<String>,
    #[serde(default)]
    checked_aam_ng_boundary: Option<CheckedAamNgBoundary>,
    fixed_syllables: Vec<FixedSyllable>,
    #[serde(default)]
    word_rule_reuse: component_words::Settings,
    #[serde(default)]
    component_readings: Vec<component_words::Reading>,
    #[serde(default)]
    non_reusable_word_parts: BTreeSet<String>,
    #[serde(default)]
    pronunciation_defaults: Vec<crate::Pronunciation>,
    #[serde(default)]
    default_component_parts: Vec<component_words::DefaultPart>,
    #[serde(default)]
    shorthand_readings: Vec<component_words::Reading>,
    #[serde(default)]
    terminal_reading_units: Vec<TerminalReadingUnit>,
    #[serde(default)]
    word_readings: Vec<WordReading>,
    silent_word_endings: Vec<SilentWordEnding>,
    reviewed_word_final_codas: Vec<ReviewedWordFinalCoda>,
    #[serde(default)]
    closed_vowel_alternatives: BTreeMap<String, Vec<Key>>,
    #[serde(default)]
    ending_alternatives: ending_alternatives::Settings,
    repeated_initial_presyllable: RepeatedInitialPresyllable,
    short_inherent_before_coda: Vec<ShortInherentBeforeCoda>,
    glottal_onset: GlottalOnset,
    scoped_onset_clusters: Vec<ScopedOnsetCluster>,
    onset_cluster_governance: OnsetClusterGovernance,
    local_written_cluster_governance: LocalWrittenClusterGovernance,
    pair_before_subscript: bool,
    nasal_boundaries: NasalBoundaries,
    written_nasal_glottal_boundary: WrittenNhRBoundary,
    explicit_glottal_bantoc: ExplicitGlottalBantoc,
    terminal_plain_a_sign: TerminalPlainASign,
    terminal_written_same_o_sign: TerminalWrittenSameOSign,
    short_inherent_sign: two_dot::Rule,
    written_velar_r_boundary: WrittenNhRBoundary,
    unwritten_nasal_r_boundary: WrittenNhRBoundary,
    terminal_nasal_compound_boundary: WrittenNhRBoundary,
    written_nh_boundary: WrittenNhBoundary,
    terminal_combined_vowels: Vec<Vowel>,
    independent_coeng_y: IndependentCoengY,
    terminal_cancellation: TerminalCancellation,
    simple_marked_rhymes: Vec<MarkedRhyme>,
    riya_governance: RiyaGovernance,
    open_laa_governance: OpenLaaGovernance,
    initial_samaa_governance: InitialSamaaGovernance,
    weak_r_second_vowel_governance: Vec<WeakRSecondVowelGovernance>,
    checked_terminal_roots: Vec<CheckedRootContinuation>,
    terminal_lexical_units: Vec<TerminalLexicalUnit>,
    exact_input_priorities: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    compact_ordinary_a_boundary_priority: Option<CompactOrdinaryABoundaryPriority>,
    #[serde(default)]
    saved_input_match_priorities: Vec<SavedInputMatchPriority>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rules {
    consonants: Vec<Consonant>,
    dependent_vowels: Vec<Vowel>,
    additional_vowel_forms: Vec<Vowel>,
    independent_vowels: Vec<Independent>,
    runtime: Runtime,
    #[serde(default)]
    pronunciation_syllables: Vec<read_as::Definition>,
    #[serde(skip)]
    prepared_pronunciations: OnceLock<read_as::Prepared>,
}
fn rules() -> &'static Rules {
    #[cfg(test)]
    if let Some(rules) = overrides::configured_for_test() { return rules; }
    static RULES: OnceLock<Rules> = OnceLock::new();
    RULES.get_or_init(|| config::parse_rules(include_str!("../../data/khmer-sound-map.json"))
        .expect("validated pinned sound map"))
}
pub(crate) fn starting_rule_context(dictionary: &compact::Dictionary) -> crate::context::RuleIndex {
    let mut index = crate::context::RuleIndex::default();
    for unit in &rules().runtime.word_readings {
        if !dictionary.khmer.contains(&unit.khmer) {
            continue;
        }
        for word in &unit.words {
            if !dictionary.khmer.contains(word) {
                continue;
            }
            let chars: Vec<_> = word.chars().collect();
            let Some(end) = unit.checked_end(word, &chars, 0) else { continue; };
            if end == chars.len() {
                continue;
            }
            let next: String = chars[end..].iter().collect();
            // The entire remainder must already be a lexical word. Neither a
            // substring nor a bound fragment becomes a new context component.
            if crate::valid_khmer(&next) && !next.contains(' ')
                && dictionary.khmer.contains(&next) {
                index.insert(word, &unit.khmer, next);
            }
        }
    }
    for part in &rules().runtime.default_component_parts {
        if !dictionary.khmer.contains(&part.khmer) {continue;}
        for word in &part.starting_words {
            if !dictionary.khmer.contains(word) {continue;}
            let Some(next)=word.strip_prefix(&part.khmer) else {continue;};
            let chars:Vec<_>=word.chars().collect();
            if !next.is_empty() && clear_segment_end(&chars,part.khmer.chars().count())
                && crate::valid_khmer(next) && !next.contains(' ') && dictionary.khmer.contains(next) {
                index.insert(word,&part.khmer,next.to_owned());
            }
        }
    }
    index
}
fn consonant(ch: char) -> Option<&'static Consonant> {
    rules().consonants.iter().find(|c| c.khmer.starts_with(ch))
}
fn is_vowel(ch: char) -> bool {
    ('\u{17b6}'..='\u{17c7}').contains(&ch)
}

/// A named full-input policy scores only an independently saved canonical
/// alias. It creates no generated reading and never changes a stored rank.
pub(crate) fn saved_input_match_priority(input: &str, text: &str) -> Option<i64> {
    rules().runtime.saved_input_match_priorities.iter()
        .find(|rule| rule.input == input && rule.khmer == text)
        .map(|rule| i64::from(rule.rank_priority))
        .or_else(|| compact_ordinary_a_priority(input, text))
}

#[cfg(test)]
mod saved_input_priority_tests {
    use super::*;

    #[test]
    fn named_saved_input_priority_is_exact_signed_and_separate_from_compact_policy() {
        let proof: serde_json::Value = serde_json::from_str(include_str!(
            "../../test/fixtures/group03-selected-typing.json"
        )).unwrap();
        let policy: SavedInputMatchPriority = serde_json::from_value(proof["savedInputPolicy"].clone()).unwrap();
        assert_eq!(policy.rank_priority, -2);
        assert_eq!(saved_input_match_priority(&policy.input, &policy.khmer), Some(-2));
        for input in ["po", "poha", "poh poh", "p oh", "POH", "poh "] {
            assert_eq!(saved_input_match_priority(input, &policy.khmer), None, "{input}");
        }
        for word in ["ពោះ", "ប៉ុស្តិ៍រ", "អប៉ុស្តិ៍", "ប៉ុស្តិ៍ៗ"] {
            assert_eq!(saved_input_match_priority("poh", word), None, "{word}");
        }
        assert_eq!(saved_input_match_priority("terkop", "ទឹកអប់"), Some(-1));
        assert_eq!(saved_input_match_priority("terkorop", "ទឹកអប់"), None);
        for priority in [serde_json::json!(-2.5), serde_json::json!(2147483648u64), serde_json::json!(-2147483649i64)] {
            let mut invalid = proof["savedInputPolicy"].clone();
            invalid["rankPriority"] = priority;
            assert!(serde_json::from_value::<SavedInputMatchPriority>(invalid).is_err());
        }
    }
}

/// Recognize an existing compact alias, without adding a pronunciation path.
/// Only an original simple written-vowel syllable with a bare coda followed
/// by ordinary អ + a bare final (optionally bantoc) qualifies. Coeng អ,
/// open prefixes, shifts, nasal vowels, other signs and longer frames do not.
pub(crate) fn compact_ordinary_a_priority(input: &str, text: &str) -> Option<i64> {
    let rule = rules().runtime.compact_ordinary_a_boundary_priority.as_ref()?;
    let chars: Vec<char> = text.chars().collect();
    if chars.first() == Some(&'អ') || consonant(*chars.first()?)?.onset_keys.is_empty() {
        return None;
    }
    let mut seam = 1;
    while chars.get(seam).is_some_and(|&ch| is_vowel(ch)) { seam += 1; }
    if seam == 1 || chars[1..seam].contains(&'ំ') || chars[1..seam].contains(&'ះ') {
        return None;
    }
    let vowel: String = chars[1..seam].iter().collect();
    if !rules().dependent_vowels.iter().chain(&rules().additional_vowel_forms).any(|v| v.khmer == vowel) {
        return None;
    }
    rules().runtime.codas.get(&chars.get(seam)?.to_string())?;
    if chars.get(seam + 1) != Some(&'អ') { return None; }
    rules().runtime.codas.get(&chars.get(seam + 2)?.to_string())?;
    let end = seam + 3;
    if chars.len() != end && !(chars.len() == end + 1 && chars[end] == '់') {
        return None;
    }
    // Parsing each complete part independently also verifies the written
    // vowel and compatibility of the suffix shortening mark.
    let prefix: String = chars[..seam + 1].iter().collect();
    let suffix: String = chars[seam + 1..].iter().collect();
    let mut compact = mapped_chunks(&prefix)?;
    compact.extend(mapped_chunks(&suffix)?);
    if !match_chunks(&compact, 0, input, &mut BTreeMap::new()).is_some_and(|hit| hit.exact) {
        return None;
    }
    // A complete original mapped reading is an independent path. Do not
    // lower it merely because the spelling contains an unmarked អ seam.
    if chunks(text).is_some_and(|reading| {
        match_chunks(&reading, 0, input, &mut BTreeMap::new()).is_some_and(|hit| hit.exact)
    }) { return None; }
    Some(i64::from(rule.rank_priority))
}

// Reviewed simple word-initial C + coeng C + written-vowel scope. Do not
// infer boundaries in a longer word or expose an onset by stripping silence.
// A stop/fricative controls a sonorant regardless of which is the subscript;
// for ordinary mixed stop/fricative pairs, the A-base/O-subscript uses O.
fn onset_cluster_series(chars: &[char], original_terminal: bool) -> Option<&'static str> {
    if !original_terminal || chars.get(1) != Some(&'្') {
        return None;
    }
    let base = consonant(*chars.first()?)?;
    let subscript = consonant(*chars.get(2)?)?;
    let rule = &rules().runtime.onset_cluster_governance;
    let cluster: String = chars[..3].iter().collect();
    if rule.excluded_clusters.contains(&cluster) {
        return None;
    }
    let end = 3 + chars[3..].iter().take_while(|&&ch| is_vowel(ch)).count();
    if end == 3 {
        return None;
    }
    let vowel: String = chars[3..end].iter().collect();
    if !mapped_written_vowel(&vowel, end == chars.len()) {
        return None;
    }
    match &chars[end..] {
        [] => {},
        [last] | [last, '់'] if consonant(*last).is_some() => {},
        _ => return None,
    }
    let dominant = |c: &Consonant| rule.dominant_consonants.contains(&c.khmer);
    let passive = |c: &Consonant| rule.passive_consonants.contains(&c.khmer);
    if base.series == subscript.series || (dominant(subscript) && passive(base)) {
        Some(subscript.series.as_str())
    } else if dominant(base) && passive(subscript) {
        Some(base.series.as_str())
    } else if dominant(base) && dominant(subscript)
        && base.series == "a" && subscript.series == "o" {
        Some(subscript.series.as_str())
    } else {
        None
    }
}

// Continue only the source-checked A-stop/O-sonorant pairs at a parsed
// syllable onset. A complete mapped written vowel is required; do not infer
// an onset from a coda, add coengs, or expose a cluster by stripping silence.
fn local_written_cluster_series(chars: &[char], onset: usize, original: bool) -> Option<&'static str> {
    if !original || chars.get(onset + 1) != Some(&'្') {
        return None;
    }
    let base = consonant(*chars.get(onset)?)?;
    let subscript = consonant(*chars.get(onset + 2)?)?;
    let rule = &rules().runtime.local_written_cluster_governance;
    let cluster: String = chars[onset..onset + 3].iter().collect();
    if !rule.pairs.contains(&cluster) || base.series != "a" || subscript.series != "o" {
        return None;
    }
    let start = onset + 3;
    let end = start + chars[start..].iter().take_while(|&&ch| is_vowel(ch)).count();
    let vowel: String = chars[start..end].iter().collect();
    if end == start || !mapped_written_vowel(&vowel, end == chars.len())
        || chars.get(end).is_some_and(|&ch| consonant(ch).is_none()) {
        return None;
    }
    Some(rule.target_series.as_str())
}

// Called only after the parser identifies a syllable ending. The same final
// keys apply inside compounds and at word end, including ស -> h/s.
// Where there is no ending rule, use the letter's actual mapped keys.
fn coda_keys(ch: char) -> Option<Vec<Key>> {
    if rules().runtime.codas.contains_key(&ch.to_string()) {
        return rules().runtime.codas.get(&ch.to_string()).cloned();
    }
    let base = consonant(ch)?;
    // An onset-only typing alternative must not silently become a new coda
    // through the ordinary letter fallback. Explicit reviewed codas still win.
    let keys = base.coda_fallback_keys.as_ref().unwrap_or(&base.onset_keys);
    (!keys.is_empty()).then(|| keys.clone())
}

fn vowel_keys(vowel: &str, series: &str) -> Option<Vec<Vec<Key>>> {
    let readings = || rules().additional_vowel_forms.iter().chain(&rules().dependent_vowels);
    let keys = |v: &Vowel| if series == "a" { v.a_series_keys.clone() } else { v.o_series_keys.clone() };
    // Combined readings such as េះ and ិះ win over individual signs. If no
    // combined rule exists, retain each mapped sign instead of dropping a word.
    if let Some(v) = readings().find(|v| v.khmer == vowel) {
        return Some(vec![keys(v)]);
    }
    vowel.chars().map(|ch| readings().find(|v| v.khmer == ch.to_string()).map(keys)).collect()
}

// Apply only after a parsed subscript អ, before the ordinary coda rules.
// Matching the complete vowel form leaves composite vowels and plain អ alone.
fn add_glottal_vowel_keys(vowel: &str, mapped: &mut [Vec<Key>]) {
    let [keys] = mapped else { return; };
    if let Some(additional) = rules().runtime.glottal_onset.additional_vowel_keys.get(vowel) {
        for key in additional {
            let mut key = key.clone();
            key.prediction_stage = 1;
            if !keys.contains(&key) { keys.push(key); }
        }
    }
}

// Reviewed combinations require the original written word end.
// Keep the per-sign mapped fallback at medial or cancellation-exposed ends.
fn terminal_combined_vowel_keys(vowel: &str, series: &str) -> Option<Vec<Vec<Key>>> {
    let form = rules().runtime.terminal_combined_vowels.iter().find(|v| v.khmer == vowel)?;
    let keys = if series == "a" { &form.a_series_keys } else { &form.o_series_keys };
    // An unreviewed series stays on its existing per-sign mapped path.
    (!keys.is_empty()).then(|| vec![keys.clone()])
}

// Callers already guard original spelling. A complete terminal combination
// can participate in the existing cluster controller without changing it.
fn mapped_written_vowel(vowel: &str, word_final: bool) -> bool {
    rules().additional_vowel_forms.iter().chain(&rules().dependent_vowels)
        .any(|v| v.khmer == vowel)
        // A series-specific key selection must not also admit a previously
        // unreviewed composite into the cluster-governance selector.
        || (word_final && rules().runtime.terminal_combined_vowels.iter()
            .any(|v| v.khmer == vowel && v.cluster_governance != Some(false)))
}

fn segment_end(chars: &[char], start: usize, spelling: &str) -> Option<usize> {
    if spelling.is_empty() {
        return None;
    }
    let end = start.checked_add(spelling.chars().count())?;
    chars.get(start..end)?.iter().copied().eq(spelling.chars()).then_some(end)
}
fn clear_segment_end(chars: &[char], end: usize) -> bool {
    end == chars.len() || chars.get(end).is_some_and(|&ch| consonant(ch).is_some())
}
// Apply only after the parser has opened simple មា and decided it has no
// coda. The exact initial frame supplies sa + ma; it does not govern later
// sonorants or invent a boundary for a closed មា (e.g. សមាជ / សមាស).
fn initial_samaa_series(
    chars: &[char], onset: usize, vowel_start: usize, parsed_end: usize,
    original_spelling: bool,
) -> Option<&'static str> {
    let rule = &rules().runtime.initial_samaa_governance;
    if !original_spelling || onset != 1 || vowel_start != 2
        || chars.contains(&rules().runtime.terminal_cancellation.mark.chars().next()?) {
        return None;
    }
    let end = segment_end(chars, 0, &rule.prefix)?;
    if parsed_end != end || consonant(*chars.get(end)?).is_none() {
        return None;
    }
    Some(rule.target_series.as_str())
}
// Extend the reviewed open first-syllable shape only to plain លា followed
// by a simple written-vowel syllable. The parser must actually open ល as
// an onset; no earlier coda or stripped ending supplies a controller.
fn open_laa_target(chars: &[char], onset: usize, original_spelling: bool) -> Option<usize> {
    if !original_spelling || onset != 0
        || chars.contains(&rules().runtime.terminal_cancellation.mark.chars().next()?) {
        return None;
    }
    let rule = &rules().runtime.open_laa_governance;
    let first = consonant(*chars.first()?)?;
    if first.series != rule.controller_series
        || !rules().runtime.onset_cluster_governance.dominant_consonants.contains(&first.khmer)
        || rules().runtime.cross_syllable_governance.excluded_initials.contains(&first.khmer) {
        return None;
    }
    let first_vowel = *chars.get(1)?;
    if !('\u{17b6}'..='\u{17c5}').contains(&first_vowel) {
        return None;
    }
    vowel_keys(&first_vowel.to_string(), &first.series)?;
    let target = 2;
    let next = segment_end(chars, target, &rule.unit)?;
    let following = consonant(*chars.get(next)?)?;
    let start = next + 1;
    let end = start + chars[start..].iter().take_while(|&&ch| is_vowel(ch)).count();
    if start == end || !clear_segment_end(chars, end) {
        return None;
    }
    let vowel: String = chars[start..end].iter().collect();
    // A full mapped combination is required; separate-sign fallback must
    // not make a shifter, coeng or extra attached vowel satisfy this guard.
    if !rules().additional_vowel_forms.iter().chain(&rules().dependent_vowels)
        .any(|v| v.khmer == vowel && if following.series == "a" {
            !v.a_series_keys.is_empty()
        } else { !v.o_series_keys.is_empty() }) {
        return None;
    }
    Some(target)
}
// Called only for a simple, unshifted consonant the parser has opened as an
// onset. A preceding coda is never promoted into a vowel controller. This
// local C + optional single vowel + រិយា scope does not govern a whole word.
fn riya_target(chars: &[char], onset: usize, original_spelling: bool) -> Option<usize> {
    if !original_spelling { return None; }
    let rule = &rules().runtime.riya_governance;
    let controller = consonant(*chars.get(onset)?)?;
    if controller.series != rule.controller_series
        || !rules().runtime.onset_cluster_governance.dominant_consonants.contains(&controller.khmer) {
        return None;
    }
    let mut start = onset + 1;
    if chars.get(start).is_some_and(|&ch| is_vowel(ch)) {
        let vowel = chars[start].to_string();
        vowel_keys(&vowel, &controller.series)?;
        start += 1;
    }
    let end = segment_end(chars, start, &rule.unit)?;
    if !clear_segment_end(chars, end) || chars.get(end + 1) == Some(&'់') {
        return None;
    }
    // The pinned unit is រ + ិ + យ + ា. Only យ's vowel changes; រ's
    // existing i/e choices and subsequent syllables remain untouched.
    Some(end - 2)
}
// This changes only the marked rhyme, not vowel governance in earlier
// syllables. Use the last preceding stop/fricative, including an intervening
// O-series controller. A shifted or cancelled prefix is outside this scope.
fn a_governed_marked_sonorant(
    rule: &MarkedRhyme, chars: &[char], start: usize, original_spelling: bool,
) -> bool {
    let Some(governance) = &rule.sonorant_governance else { return false; };
    if !original_spelling || start == 0 || rule.series != "a" || rule.ending != "័យ"
        || chars.get(start - 1) == Some(&'្')
        || !chars.get(start).is_some_and(|ch| governance.onsets.iter().any(|base| base.starts_with(*ch))) {
        return false;
    }
    let Some(end) = segment_end(chars, start + 1, &rule.ending) else { return false; };
    if !clear_segment_end(chars, end) {
        return false;
    }
    let prefix = &chars[..start];
    if prefix.iter().any(|ch| governance.blocked_prefix_marks.iter().any(|mark| mark.starts_with(*ch))) {
        return false;
    }
    let clusters = &rules().runtime.onset_cluster_governance;
    if clusters.excluded_clusters.iter().any(|cluster| {
        let spelling: Vec<_> = cluster.chars().collect();
        !spelling.is_empty() && prefix.windows(spelling.len()).any(|window| window == spelling)
    }) {
        return false;
    }
    prefix.iter().rev().find_map(|&ch| {
        clusters.dominant_consonants.iter().any(|base| base.starts_with(ch))
            .then(|| consonant(ch)).flatten()
    }).is_some_and(|controller| controller.series == "a")
}
// Reuse a reviewed marked rhyme at a parsed, unshifted simple onset. The
// exact ending must close here; attached vowels, marks and coeng do not fit.
// Composition can restrict reviewed onset classes and require the original
// spelling. Legacy whole-word readings retain their separate scope.
fn syllable_marked_rhyme_at(
    chars: &[char], start: usize, series: &str, original_spelling: bool,
) -> Option<(&'static MarkedRhyme, usize)> {
    let base = consonant(*chars.get(start)?)?;
    if base.onset_keys.is_empty() {
        return None;
    }
    rules().runtime.simple_marked_rhymes.iter().find_map(|rule| {
        let governed = a_governed_marked_sonorant(rule, chars, start, original_spelling);
        let effective_series = if governed { "a" } else { series };
        if !rule.composable || rule.series != effective_series || rule.keys.is_empty()
            || (base.series != effective_series && !governed)
            || (rule.original_spelling_only && !original_spelling)
            || (!rule.composable_onsets.is_empty() && !rule.composable_onsets.contains(&base.khmer) && !governed) {
            return None;
        }
        let end = segment_end(chars, start + 1, &rule.ending)?;
        clear_segment_end(chars, end).then_some((rule, end))
    })
}
#[cfg(test)]
fn fixed_syllable_at(word: &str, chars: &[char], start: usize, original_spelling: bool) -> Option<(&'static FixedSyllable, usize)> {
    overrides::at(&rules().runtime.fixed_syllables, word, chars, start, original_spelling)
}
// Read one complete word inside its checked combined entries. Keep the
// original full word for all later selectors; do not parse the tail anew.
#[cfg(test)]
fn word_reading_at(
    word: &str, chars: &[char], start: usize,
) -> Option<(&'static WordReading, usize)> {
    selected_word_reading_at(word, chars, start, true)
}
#[cfg(test)]
fn selected_word_reading_at(
    word: &str, chars: &[char], start: usize, include_additive: bool,
) -> Option<(&'static WordReading, usize)> {
    selected_word_reading_with_overrides(word, chars, start, include_additive,
        overrides::Context::new(word).with_original(chars.iter().copied().eq(word.chars())))
}
fn selected_word_reading_with_overrides(
    word: &str, chars: &[char], start: usize, include_additive: bool, override_context: overrides::Context<'_>,
) -> Option<(&'static WordReading, usize)> {
    if !override_context.uses_part_boundaries() || !override_context.allows_original_word_rules(word) {
        return None;
    }
    rules().runtime.word_readings.iter().find_map(|rule| {
        if (override_context.skip_shorthands || override_context.word != word) && has_shorthand_rule(&rule.khmer) { return None; }
        // A checked component pair adds its own complete lattice below. Mixing
        // its root with ordinary suffix alternatives would enable unselected inputs.
        if rule.checked_suffix_readings.iter().chain(&rule.checked_prefix_readings)
            .any(|neighbor| neighbor.word == word)
            || rule.checked_context_readings.iter().any(|context| context.word == word) {
            return None;
        }
        if rule.preserve_mapped && !include_additive { return None; }
        let end = rule.checked_end(word, chars, start)?;
        if override_context.in_range(chars,start,end) { return None; }
        Some((rule, end))
    })
}
// A reviewed lexical unit at the original word end. The mapped parser must
// already support the full word and its exact local chunks. Add the short
// reading alongside that existing path; do not repair or reinterpret a prefix.
fn add_terminal_lexical_readings(text: &str, chunks: &mut Vec<Vec<Key>>, override_context: overrides::Context<'_>) {
    if text.contains('៍') {
        return;
    }
    if override_context.reads_pronunciation() { return; }
    for unit in &rules().runtime.terminal_lexical_units {
        if (override_context.skip_shorthands || override_context.word != text) && has_shorthand_rule(&unit.khmer) {
            continue;
        }
        // A standalone ending can also be a reusable part, but only its
        // explicitly reusable keys may cross a pronunciation component cut.
        let reusable = if !unit.words.is_empty() && !override_context.allows_original_word_rules(text) {
            let Some(part) = rules().runtime.component_readings.iter()
                .find(|part| part.khmer == text && part.khmer == unit.khmer) else { continue; };
            Some(override_context.word_part_keys(&part.khmer).unwrap_or(&part.keys))
        } else {
            None
        };
        if !unit.words.is_empty() && !unit.words.iter().any(|word| word == text) {
            continue;
        }
        let Some(prefix) = text.strip_suffix(&unit.khmer) else { continue; };
        if prefix.ends_with('្') || unit.keys.is_empty() {
            continue;
        }
        let chars: Vec<_> = text.chars().collect();
        if override_context.in_range(&chars,prefix.chars().count(),chars.len()) { continue; }
        let singleton_groups = unit.retained_readings.iter().map(|reading| {
            reading.iter().map(|key| vec![key.clone()]).collect::<Vec<_>>()
        });
        for retained in singleton_groups.chain(unit.retained_chunk_groups.iter().cloned()) {
            let Some(start) = chunks.len().checked_sub(retained.len()) else { continue; };
            // Exact local chunk groups retain written-vowel alternatives without
            // accepting other parsed shapes or constructing cross-syllable paths.
            if retained.is_empty() || !chunks[start..].iter().zip(&retained).all(|(actual, shape)| {
                let matches = |expected: &[Key]| actual.len() == expected.len()
                    && actual.iter().zip(expected).all(|(a,b)|a.text == b.text);
                matches(shape) || (shape.len() == 1 && rules().runtime.inherent.values().any(|keys|
                    // A retained signature can name one original inherent key.
                    // Its live group may gain equivalent typing alternatives;
                    // require that complete group, keeping all other chunks exact.
                    keys.iter().any(|key|key.text == shape[0].text) && matches(keys)))
            }) {
                continue;
            }
            let mut keys = vec![Key::from(String::new())];
            // Structural guards compare spelling, not ranking metadata. Carry
            // the actual mapped priorities when retaining a full old reading.
            for alternatives in &chunks[start..] {
                keys = keys.iter().flat_map(|prefix| {
                    alternatives.iter().map(move |key| prefix.joined(key))
                }).collect();
            }
            for key in override_context.word_part_keys(&unit.khmer).unwrap_or(&unit.keys) {
                if !key.is_empty() && !keys.contains(key)
                    && reusable.is_none_or(|allowed| allowed.contains(key)) {
                    keys.push(key.clone());
                }
            }
            chunks.truncate(start);
            chunks.push(keys);
            break;
        }
    }
}
fn silent_coda_at(chars: &[char], start: usize) -> Option<(&'static SilentCodaCluster, usize)> {
    rules().runtime.silent_coda_clusters.iter().find_map(|rule| {
        let end = segment_end(chars, start, &rule.khmer)?;
        clear_segment_end(chars, end).then_some((rule, end))
    })
}
// A consumed silent coda can govern a specifically reviewed following vowel.
// Keep this local to the next plain onset and its listed vowel or original
// terminal mark. No inferred syllables, shifters, other attached marks,
// other vowels or cancellation-exposed spellings qualify.
fn silent_coda_following_series(
    cluster: &'static SilentCodaCluster,
    chars: &[char],
    onset: usize,
    original: bool,
) -> Option<&'static str> {
    let rule = cluster.following_vowel_governance.as_ref()?;
    let next = consonant(*chars.get(onset)?)?;
    if !original || next.series != "o" || !rule.onsets.contains(&next.khmer) {
        return None;
    }
    let start = onset + 1;
    if rule.terminal_mark.as_ref().is_some_and(|mark| {
        segment_end(chars, start, mark) == Some(chars.len())
    }) {
        return Some(rule.target_series.as_str());
    }
    let end = start + chars[start..].iter().take_while(|&&ch| is_vowel(ch)).count();
    let vowel: String = chars[start..end].iter().collect();
    (rule.vowels.contains(&vowel) && clear_segment_end(chars, end))
        .then_some(rule.target_series.as_str())
}
fn terminal_silent_coda_at(
    chars: &[char],
    start: usize,
    vowel: &str,
    onset: &str,
    original_terminal: bool,
) -> Option<&'static TerminalSilentCodaCluster> {
    rules().runtime.terminal_silent_coda_clusters.iter().find(|rule| {
        // An explicit reviewed vowel precedes a bare closing group. Requiring
        // the actual end excludes attached vowels/marks and extra coengs;
        // the vowel allowlist also excludes already-closed nasal/h forms.
        rule.preceding_vowels.iter().any(|v| v == vowel)
            && (rule.preceding_onsets.is_empty() || rule.preceding_onsets.iter().any(|o| o == onset))
            && (!rule.original_terminal_required || original_terminal)
            && segment_end(chars, start, &rule.khmer) == Some(chars.len())
    })
}
fn terminal_short_coda_at(
    chars: &[char], start: usize, initial: char, vowel: &str, simple_onset: bool,
) -> Option<&'static TerminalShortCodaCluster> {
    simple_onset.then_some(())?;
    rules().runtime.terminal_short_coda_clusters.iter().find(|rule| {
        rule.contexts.iter().any(|c| c.consonant.starts_with(initial) && c.vowel == vowel)
            && segment_end(chars, start, &rule.khmer) == Some(chars.len())
    })
}
fn terminal_silent_group_at(chars: &[char], start: usize, vowel_start: usize, vowel: &str) -> bool {
    rules().runtime.terminal_silent_groups.iter().any(|rule| {
        rule.vowel == vowel
            && vowel_start.checked_sub(1).and_then(|i| chars.get(i)).is_some_and(|&ch| {
                rule.preceding_consonants.iter().any(|c| c.starts_with(ch))
            })
            && segment_end(chars, start, &rule.khmer) == Some(chars.len())
    })
}

#[cfg(test)]
mod closing_evidence_tests {
    use super::*;

    #[test]
    fn word_reading_positions_use_original_offsets_and_explicit_combinations() {
        let make = |positions: serde_json::Value| -> WordReading {
            serde_json::from_value(serde_json::json!({
                "khmer": "ការ", "keys": ["ka"], "positions": positions,
                "words": ["ការ", "ការងារ", "មេការ", "អ្នកការទូត", "ការា", "ការគ៍"]
            })).unwrap()
        };
        let cases = [("ការ", 0, WordReadingPosition::Standalone),
            ("ការងារ", 0, WordReadingPosition::Start),
            ("មេការ", 2, WordReadingPosition::End),
            ("អ្នកការទូត", 4, WordReadingPosition::Middle)];
        for (positions, allowed) in [
            (serde_json::json!(["start", "end"]), vec![WordReadingPosition::Start, WordReadingPosition::End]),
            (serde_json::json!(["middle"]), vec![WordReadingPosition::Middle]),
            (serde_json::json!(["anywhere"]), vec![WordReadingPosition::Start, WordReadingPosition::Middle, WordReadingPosition::End]),
            (serde_json::json!(["standalone"]), vec![WordReadingPosition::Standalone]),
            (serde_json::json!(["start", "middle", "standalone"]), vec![WordReadingPosition::Start, WordReadingPosition::Middle, WordReadingPosition::Standalone]),
            (serde_json::json!([]), vec![]),
        ] {
            let rule = make(positions);
            for (word, start, position) in cases {
                assert_eq!(rule.checked_end(word, &word.chars().collect::<Vec<_>>(), start).is_some(),
                    allowed.contains(&position), "{word}: {position:?}");
            }
            assert!(!rule.allows_position(0, 0, 3));
            assert!(!rule.allows_position(2, 1, 3));
            assert!(!rule.allows_position(0, 4, 3));
        }
        let rule = make(serde_json::json!(["anywhere", "standalone"]));
        assert!(rule.checked_end("ការា", &"ការា".chars().collect::<Vec<_>>(), 0).is_none());
        assert!(rule.checked_end("អ្នកការទូត", &"ការទូត".chars().collect::<Vec<_>>(), 0).is_none());
        assert!(rule.checked_end("ការគ៍", &"ការ".chars().collect::<Vec<_>>(), 0).is_none());
        assert!(rule.checked_end("កការ", &"កការ".chars().collect::<Vec<_>>(), 1).is_none());
    }

    #[test]
    fn checked_aam_ng_boundary_requires_both_original_word_and_local_position() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/checked-aam-ng-boundary.json")).unwrap();
        for word in proof["proofWords"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()) {
            let chars: Vec<_> = word.chars().collect();
            assert!(checked_aam_ng_boundary(word, &chars, 0, 3, 4, "ាំ", true));
            assert!(!checked_aam_ng_boundary(word, &chars, 0, 3, 4, "ាំ", false));
            for (onset, coda, next, vowel) in [(1, 3, 4, "ាំ"), (0, 2, 4, "ាំ"), (0, 3, 5, "ាំ"), (0, 3, 4, "ំ")] {
                assert!(!checked_aam_ng_boundary(word, &chars, onset, coda, next, vowel, true));
            }
            let altered: Vec<_> = format!("{word}ា").chars().collect();
            assert!(!checked_aam_ng_boundary(word, &altered, 0, 3, 4, "ាំ", true));
            let reading = chunks(word).unwrap();
            assert!(reading.iter().flatten().all(|key| key.priority == 0));
            assert_eq!(reading.iter().map(Vec::len).product::<usize>(), if word == "វាំងនន" { 2 } else { 6 });
        }
        for word in proof["outsideScope"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()) {
            assert!(!checked_aam_ng_boundary(word, &word.chars().collect::<Vec<_>>(), 0, 3, 4, "ាំ", true), "{word}");
        }
    }

    #[test]
    fn additive_word_units_keep_separate_full_paths_and_original_guards() {
        for (head, count) in [("បន្ទប់", 4)] {
            let unit = rules().runtime.word_readings.iter().find(|u| u.khmer == head).unwrap();
            assert!(unit.preserve_mapped);
            assert_eq!(unit.words.len(), count);
            assert!(unit.keys.iter().all(|k| k.priority == 0));
            for word in &unit.words {
                let chars: Vec<_> = word.chars().collect();
                assert!(selected_word_reading_at(word, &chars, 0, true).is_some());
                assert!(selected_word_reading_at(word, &chars, 0, false).is_none());
                // An altered parser view cannot expose a reviewed complete unit.
                assert!(selected_word_reading_at(word, &head.chars().collect::<Vec<_>>(), 0, true).is_some() == (word == head));
                for sign in ["ា", "់", "្គ"] {
                    let altered: Vec<_> = format!("{head}{sign}").chars().collect();
                    assert!(selected_word_reading_at(word, &altered, 0, true).is_none());
                }
            }
        }
        for word in ["បន្ទប់រៀន", "បន្ទប់ដេក", "បន្ទប់ទឹក"] {
            let readings = word_readings(word).unwrap();
            assert_eq!(readings.len(), 2, "{word}");
            assert!(readings[0][0].iter().any(|key|key.text == "bontup"));
            assert!(readings[0][0].iter().any(|key|key.text == "bontop"));
            assert_ne!(readings[1][0][0].text, "bontup");
        }
        for word in ["ម្ចាស់ដី", "បន្ទប់គេង", "ម្ចាស់ផ្ទះក៍", "បន្ទប់រៀនក៍"] {
            assert!(word_reading_at(word, &word.chars().collect::<Vec<_>>(), 0).is_none());
        }
        let kda_proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/checked-kda-and-compound-exceptions.json")).unwrap();
        for word in kda_proof["outsideScope"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()) {
            assert!(selected_word_reading_at(word, &word.chars().collect::<Vec<_>>(), 0, true).is_none(), "{word}");
        }
        for word in ["ក្ដារខៀន", "ក្ដារចុច"] {
            let readings = word_readings(word).unwrap();
            assert_eq!(readings.len(), 1, "{word}");
            assert_ne!(readings[0][0][0].text, "kda");
        }
        for word in ["អ្នក", "ពេជ្រ"] {
            assert_eq!(word_readings(word).unwrap().len(), 1);
            assert!(rules().runtime.fixed_syllables.iter().find(|u| u.khmer == word).unwrap().words.iter().any(|entry|entry == word));
        }
        let glottal = mapped_chunks("ផ្អែម").unwrap();
        assert_eq!(glottal[0].iter().filter(|k| k.text == "p").count(), 1);
        for letter in ['ផ', 'ភ'] {
            let c = consonant(letter).unwrap();
            assert_eq!(c.onset_keys.iter().map(|k| k.text.as_str()).collect::<Vec<_>>(), vec!["ph", "p"]);
            assert!(c.onset_keys.iter().all(|k| k.priority == 0));
            assert_eq!(coda_keys(letter).unwrap().iter()
                .map(|k| k.text.as_str()).collect::<Vec<_>>(), vec!["ph", "p"]);
        }
    }

    #[test]
    fn checked_roots_do_not_match_other_words_marks_or_exposed_spellings() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/confirmed-narrow-readings.json")).unwrap();
        for word in ["សេរីភាព", "សាមីចិ"] {
            let chars: Vec<_> = word.chars().collect();
            assert!(fixed_syllable_at(word, &chars, 0, true).is_none());
            assert!(fixed_syllable_at(word, &chars, 0, false).is_none());
            assert!(fixed_syllable_at(word, &chars, 1, true).is_none());
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            assert!(fixed_syllable_at(word, &word.chars().collect::<Vec<_>>(), 0, true).is_none(), "{word}");
        }
        for (field, expected) in [("soundYes", true), ("compatibility", true), ("no", false), ("aliasYes", false)] {
            for pair in proof[field].as_array().unwrap() {
                let key = pair[0].as_str().unwrap();
                let word = pair[1].as_str().unwrap();
                let exact = chunks(word).is_some_and(|lattice| {
                    match_chunks(&lattice, 0, key, &mut BTreeMap::new()).is_some_and(|hit| hit.exact)
                });
                assert_eq!(exact, expected, "{field}: {key}: {word}");
            }
        }
    }

    #[test]
    fn checked_word_reading_keeps_original_boundaries_and_tail_context() {
        let unit = rules().runtime.fixed_syllables.iter().find(|unit| unit.khmer == "អ្នក").unwrap();
        assert_eq!(unit.khmer, "អ្នក");
        assert_eq!(unit.words.len(), 38);
        for word in &unit.words {
            let chars: Vec<char> = word.chars().collect();
            let start = if word == "លោកអ្នក" { 3 } else { 0 };
            let (reading, end) = fixed_syllable_at(word, &chars, start, true).unwrap();
            assert_eq!(end, start + 4, "{word}");
            assert_eq!(reading.keys[0].text, "neak");
            assert_eq!(reading.keys[0].priority, 0);
        }
        for word in ["អ្នកា", "អ្នក់", "អ្នក្គ", "អ្នាក់", "អ្នកអ្នក", "មនុស្សអ្នក"] {
            assert!(fixed_syllable_at(word, &word.chars().collect::<Vec<_>>(), 0, true).is_none());
        }
        // These altered parser views use checked original word identities, so
        // the guards are exercised independently of the full-word scope mask.
        assert!(fixed_syllable_at("លោកអ្នក", &"អ្នក".chars().collect::<Vec<_>>(), 0, false).is_none());
        assert!(fixed_syllable_at("អ្នកលក់", &"អ្នក".chars().collect::<Vec<_>>(), 0, false).is_none());
        assert!(fixed_syllable_at("អ្នកលក់", &"អ្នកា".chars().collect::<Vec<_>>(), 0, false).is_none());
        assert!(fixed_syllable_at("អ្នកលក់", &"អ្នក្គ".chars().collect::<Vec<_>>(), 0, false).is_none());
        let tail_cancelled: Vec<char> = "អ្នករាយការ".chars().collect();
        assert!(fixed_syllable_at("អ្នករាយការណ៍", &tail_cancelled, 0, false).is_some());
        assert_eq!(mapped_chunks("អ្នករាយការណ៍").unwrap()[0][0].text, "neak");
    }

    #[test]
    fn initial_samaa_requires_original_open_target_and_leaves_later_syllables() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/initial-open-samaa.json")).unwrap();
        for word in proof["selectorYes"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            assert_eq!(initial_samaa_series(&chars, 1, 2, 3, true), Some("a"));
            assert!(initial_samaa_series(&chars, 1, 2, 3, false).is_none());
            assert!(initial_samaa_series(&chars, 0, 2, 3, true).is_none());
            assert!(initial_samaa_series(&chars, 1, 3, 3, true).is_none());
        }
        // The parser has consumed these final consonants as codas, so the
        // target is closed. Merely matching the initial three characters is
        // insufficient. No word-specific exclusion is used.
        for word in ["សមាជ", "សមាស"] {
            assert!(initial_samaa_series(&word.chars().collect::<Vec<_>>(), 1, 2, 4, true).is_none());
        }
        for word in ["សមា", "សាមាគម", "ស្មាគម", "សម៉ាគម", "សមាីគម", "សមា់គម", "សមា្គម", "សមាគមធិ៍", "កសមាគម"] {
            assert!(initial_samaa_series(&word.chars().collect::<Vec<_>>(), 1, 2, 3, true).is_none(), "guarded frame: {word}");
        }
        for (field, expected) in [("yes", true), ("partialMappedPaths", true), ("compatibility", true), ("no", false)] {
            for pair in proof[field].as_array().unwrap() {
                let key = pair[0].as_str().unwrap();
                let word = pair[1].as_str().unwrap();
                let exact = chunks(word).is_some_and(|lattice| {
                    match_chunks(&lattice, 0, key, &mut BTreeMap::new()).is_some_and(|hit| hit.exact)
                });
                assert_eq!(exact, expected, "{field}: {key}: {word}");
            }
        }
    }

    #[test]
    fn open_laa_extends_only_the_guarded_prefix_and_keeps_later_vowels() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/open-second-laa.json")).unwrap();
        assert_eq!(proof["proofWords"].as_array().unwrap().len(), 9);
        assert_eq!(proof["selectorYes"].as_array().unwrap().len(), 25);
        for word in proof["selectorYes"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            let chars: Vec<_> = word.chars().collect();
            assert_eq!(open_laa_target(&chars, 0, true), Some(2), "{word}");
            assert!(open_laa_target(&chars, 0, false).is_none(), "exposed spelling: {word}");
            assert!(open_laa_target(&chars, 2, true).is_none(), "medial controller: {word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            assert!(open_laa_target(&word.chars().collect::<Vec<_>>(), 0, true).is_none(), "outside scope: {word}");
        }
        for (field, expected) in [("yes", true), ("partialMappedPaths", true), ("compatibility", true), ("no", false)] {
            for pair in proof[field].as_array().unwrap() {
                let key = pair[0].as_str().unwrap();
                let word = pair[1].as_str().unwrap();
                // Additive word units keep the old mapped path as a separate reading.
                let exact = word_readings(word).is_some_and(|readings| readings.iter().any(|lattice| {
                    match_chunks(lattice, 0, key, &mut BTreeMap::new()).is_some_and(|hit| hit.exact)
                }));
                assert_eq!(exact, expected, "{field}: {key}: {word}");
            }
        }
    }

    #[test]
    fn local_riya_uses_a_parsed_onset_and_keeps_other_boundaries_and_vowels() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/simple-a-riya.json")).unwrap();
        assert_eq!(proof["proofWords"].as_array().unwrap().len(), 11);
        for pair in proof["selectorYes"].as_array().unwrap() {
            let word = pair[0].as_str().unwrap();
            let onset = pair[1].as_u64().unwrap() as usize;
            let chars: Vec<_> = word.chars().collect();
            assert!(riya_target(&chars, onset, true).is_some(), "{word}");
            assert!(riya_target(&chars, onset, false).is_none(), "cancelled {word}");
        }
        for pair in proof["selectorNo"].as_array().unwrap() {
            let word = pair[0].as_str().unwrap();
            let chars: Vec<_> = word.chars().collect();
            assert!(riya_target(&chars, pair[1].as_u64().unwrap() as usize, true).is_none(), "outside scope {word}");
        }
        for (field, expected) in [("yes", true), ("mappedCompatibility", true), ("compatibility", true), ("no", false)] {
            for pair in proof[field].as_array().unwrap() {
                let key = pair[0].as_str().unwrap();
                let word = pair[1].as_str().unwrap();
                // Additive word units keep the old mapped path as a separate reading.
                let exact = word_readings(word).is_some_and(|readings| readings.iter().any(|lattice| {
                    match_chunks(lattice, 0, key, &mut BTreeMap::new()).is_some_and(|hit| hit.exact)
                }));
                assert_eq!(exact, expected, "{field}: {key}: {word}");
            }
        }
    }

    #[test]
    fn marked_ay_sonorants_use_the_last_unshifted_controller_only_for_the_rhyme() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/marked-ay-sonorant.json")).unwrap();
        assert_eq!(proof["proofWords"].as_array().unwrap().len(), 13);
        let rule = rules().runtime.simple_marked_rhymes.iter()
            .find(|rule| rule.sonorant_governance.is_some()).unwrap();
        let marked_start = |chars: &[char]| chars.windows(3)
            .rposition(|unit| matches!(unit[0], 'ម' | 'ល') && unit[1..] == ['័', 'យ'])
            .or_else(|| chars.iter().rposition(|ch| matches!(ch, 'ម' | 'ល'))).unwrap();
        for word in proof["proofWords"].as_array().unwrap().iter()
            .chain(proof["selectorYesControls"].as_array().unwrap()) {
            let word = word.as_str().unwrap();
            let chars: Vec<_> = word.chars().collect();
            let start = marked_start(&chars);
            assert!(a_governed_marked_sonorant(rule, &chars, start, true), "{word}");
            assert!(!a_governed_marked_sonorant(rule, &chars, start, false), "exposed spelling: {word}");
            assert_eq!(syllable_marked_rhyme_at(&chars, start, "o", true).unwrap().0.keys, [Key::from("ai")], "{word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            let chars: Vec<_> = word.chars().collect();
            assert!(!a_governed_marked_sonorant(rule, &chars, marked_start(&chars), true), "outside scope: {word}");
        }
        for (field, expected) in [("yes", true), ("mappedCompatibility", true), ("compatibility", true), ("no", false)] {
            for pair in proof[field].as_array().unwrap() {
                let key = pair[0].as_str().unwrap();
                let word = pair[1].as_str().unwrap();
                let exact = chunks(word).is_some_and(|lattice| {
                    match_chunks(&lattice, 0, key, &mut BTreeMap::new()).is_some_and(|hit| hit.exact)
                });
                assert_eq!(exact, expected, "{field}: {key}: {word}");
            }
        }
    }

    #[test]
    fn marked_rhyme_composition_keeps_onset_series_and_attached_endings() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/marked-rhyme.json")).unwrap();
        assert_eq!(proof["proofWords"].as_array().unwrap().len(), 14);
        for word in ["ជ័រ", "ញ័រ", "ព័រ", "ជ័រលុប"] {
            let chars: Vec<_> = word.chars().collect();
            assert!(syllable_marked_rhyme_at(&chars, 0, "o", true).is_some(), "{word}");
            assert!(syllable_marked_rhyme_at(&chars, 0, "a", true).is_none(), "series override: {word}");
        }
        for word in ["ប័រ", "ប៊័រ", "ស៊័រ", "ជ៉័រ", "ខ្ទ័រ", "ភ្ជ័រ", "ជ័រា", "ជ័រិ", "ជ័រ៉", "ជ័រ្យ", "ជ័រះ", "ជ័រ៍"] {
            assert!(chunks(word).is_none(), "unreviewed onset or attached ending: {word}");
        }
        for pair in proof["yes"].as_array().unwrap() {
            let key = pair[0].as_str().unwrap();
            let word = pair[1].as_str().unwrap();
            let lattice = chunks(word).unwrap_or_else(|| panic!("expected reading: {word}"));
            assert!(match_chunks(&lattice, 0, key, &mut BTreeMap::new()).is_some_and(|hit| hit.exact), "{key}: {word}");
        }
    }

    #[test]
    fn local_written_cluster_selector_requires_checked_pairs_and_complete_vowels() {
        let rule = &rules().runtime.local_written_cluster_governance;
        assert_eq!(rule.pairs.len(), 19);
        for pair in &rule.pairs {
            let word = format!("{pair}ារៀន");
            let chars: Vec<_> = word.chars().collect();
            assert_eq!(local_written_cluster_series(&chars, 0, true), Some("a"), "{word}");
            assert!(local_written_cluster_series(&chars, 0, false).is_none(), "exposed {word}");
        }
        for word in ["ស្មារៀន", "ខ្ញីវខ្ញូវ", "ភ្លារៀន", "ហ្វារ", "ខ្ល៉ាឃ្មុំ", "ខ្លាីឃ្មុំ", "ខ្លា់ឃ្មុំ", "ខ្ល្យាឃ្មុំ", "ខ្ល្ពាន", "ក្រពើ"] {
            let chars: Vec<_> = word.chars().collect();
            assert!(local_written_cluster_series(&chars, 0, true).is_none(), "outside scope {word}");
        }
        let medial: Vec<_> = "កាខ្លា".chars().collect();
        assert_eq!(local_written_cluster_series(&medial, 2, true), Some("a"));
        // The first-pair parser, rather than this selector, decides which
        // positions are onsets. An adjacent nasal plus two coengs cannot fit.
        for word in ["កន្ត្រៃ", "មន្ត្រី"] {
            let chars: Vec<_> = word.chars().collect();
            assert!(local_written_cluster_series(&chars, 1, true).is_none(), "nasal {word}");
        }
    }

    #[test]
    fn marked_ay_composition_requires_reviewed_series_and_original_spelling() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/marked-ay.json")).unwrap();
        assert_eq!(proof["proofWords"].as_array().unwrap().len(), 18);
        for (word, series) in [("ក័យ", "a"), ("ច័យ", "a"), ("ឆ័យ", "a"), ("ស័យ", "a"), ("ហ័យ", "a"),
            ("ជ័យ", "o"), ("ភ័យ", "o"), ("ទ័យ", "o"), ("ន័យ", "o"), ("វ័យ", "o")] {
            let chars: Vec<_> = word.chars().collect();
            assert!(syllable_marked_rhyme_at(&chars, 0, series, true).is_some(), "{word}");
            assert!(syllable_marked_rhyme_at(&chars, 0, series, false).is_none(), "exposed spelling: {word}");
            let other = if series == "a" { "o" } else { "a" };
            assert!(syllable_marked_rhyme_at(&chars, 0, other, true).is_none(), "series override: {word}");
        }
        for word in ["ម័យ", "ល័យ", "គ័យ", "ប័យ", "ត័យ", "ខ័យ", "ឡ័យ", "ភ័យា", "ន័យ្យ", "ក័យះ", "ក័យ៍", "ប៊័យ", "ស៊័យ", "ម៉័យ", "ក្ព័យ", "ស្ព័យ"] {
            let chars: Vec<_> = word.chars().collect();
            assert!(syllable_marked_rhyme_at(&chars, 0, consonant(chars[0]).unwrap().series.as_str(), true).is_none(), "outside composition scope: {word}");
        }
        for field in ["yes", "no"] {
            for pair in proof[field].as_array().unwrap() {
                let key = pair[0].as_str().unwrap();
                let word = pair[1].as_str().unwrap();
                let exact = chunks(word).is_some_and(|lattice| match_chunks(&lattice, 0, key, &mut BTreeMap::new()).is_some_and(|hit| hit.exact));
                assert_eq!(exact, field == "yes", "{key}: {word}");
            }
        }
    }

    #[test]
    fn nasal_pair_governance_covers_sourced_words_with_structural_guards() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/nasal-pair.json")).unwrap();
        assert_eq!(proof["proofWords"].as_array().unwrap().len(), 30);
        for word in proof["proofWords"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            let chars: Vec<_> = word.chars().collect();
            assert!(nasal_pair_governance(&chars, true), "reviewed word: {word}");
            assert!(!nasal_pair_governance(&chars, false), "exposed ending: {word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            assert!(!nasal_pair_governance(&word.chars().collect::<Vec<_>>(), true), "outside scope: {word}");
        }
    }

    #[test]
    fn written_ndr_requires_original_complete_two_syllable_shape() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/written-ndr-boundary.json")).unwrap();
        for word in proof["proofWords"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            assert!(written_ndr_boundary(&chars, 1, true), "{word}");
            assert!(!written_ndr_boundary(&chars, 1, false), "exposed: {word}");
            assert!(!written_ndr_boundary(&chars, 2, true), "medial: {word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            assert!(!written_ndr_boundary(&word.as_str().unwrap().chars().collect::<Vec<_>>(), 1, true), "outside: {word}");
        }
    }

    #[test]
    fn terminal_nasal_compound_requires_original_simple_three_syllable_frame() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/terminal-nasal-compound-boundary.json")).unwrap();
        for word in proof["selectorYes"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            assert!(terminal_nasal_compound_chunks(&chars, true).is_some(), "{word}");
            assert!(terminal_nasal_compound_chunks(&chars, false).is_none(), "exposed: {word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            assert!(terminal_nasal_compound_chunks(&chars, true).is_none(), "outside: {word}");
            assert!(terminal_nasal_compound_chunks(&chars, false).is_none(), "exposed guard: {word}");
        }
        for word in ["", "ក", "មី", "កញ្ចប់"] {
            assert!(terminal_nasal_compound_chunks(&word.chars().collect::<Vec<_>>(), true).is_none());
        }
    }

    #[test]
    fn unwritten_nasal_aspirated_boundary_requires_original_two_syllable_frame() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/unwritten-nasal-aspirated-boundary.json")).unwrap();
        for word in proof["selectorYes"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            assert!(unwritten_nasal_aspirated_boundary(&chars, 1, true), "{word}");
            assert!(!unwritten_nasal_aspirated_boundary(&chars, 1, false), "exposed: {word}");
            assert!(!unwritten_nasal_aspirated_boundary(&chars, 2, true), "medial: {word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            assert!(!unwritten_nasal_aspirated_boundary(&word.as_str().unwrap().chars().collect::<Vec<_>>(), 1, true), "outside: {word}");
        }
    }

    #[test]
    fn terminal_plain_a_sign_requires_original_plain_written_a_series() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/plain-a-terminal-sign.json")).unwrap();
        for word in proof["selectorYes"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            let start = chars.len() - 2;
            assert!(terminal_plain_a_sign_keys(&chars, start, "a", true, false).is_some(), "{word}");
            assert!(terminal_plain_a_sign_keys(&chars, start, "a", false, false).is_none(), "exposed: {word}");
            assert!(terminal_plain_a_sign_keys(&chars, start, "o", true, false).is_none(), "shifted: {word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            for start in 0..chars.len() {
                assert!(terminal_plain_a_sign_keys(&chars, start, "a", true, false).is_none(), "outside: {word}");
            }
        }
    }

    #[test]
    fn terminal_written_same_o_sign_requires_original_two_plain_written_syllables() {
        let proof: serde_json::Value = serde_json::from_str(include_str!(
            "../../test/fixtures/written-same-o-terminal-sign.json"
        )).unwrap();
        for word in proof["selectorYes"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            assert!(terminal_written_same_o_sign_chunks(&chars, true).is_some(), "{word}");
            assert!(terminal_written_same_o_sign_chunks(&chars, false).is_none(), "exposed: {word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            assert!(terminal_written_same_o_sign_chunks(&chars, true).is_none(), "outside: {word}");
        }
    }

    #[test]
    fn explicit_glottal_bantoc_requires_original_written_aa_and_one_final() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/explicit-glottal-bantoc.json")).unwrap();
        for word in proof["selectorYes"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            assert!(explicit_glottal_bantoc_chunks(&chars, true, overrides::Context::bare("")).is_some(), "{word}");
            assert!(explicit_glottal_bantoc_chunks(&chars, false, overrides::Context::bare("")).is_none(), "exposed: {word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            assert!(explicit_glottal_bantoc_chunks(&word.as_str().unwrap().chars().collect::<Vec<_>>(), true, overrides::Context::bare("")).is_none(), "outside: {word}");
        }
    }

    #[test]
    fn nasal_glottal_boundary_requires_original_written_two_syllable_frame() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/written-nasal-glottal-boundary.json")).unwrap();
        for word in proof["proofWords"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            assert!(written_nasal_glottal_chunks(&chars, true, overrides::Context::bare("")).is_some(), "{word}");
            assert!(written_nasal_glottal_chunks(&chars, false, overrides::Context::bare("")).is_none(), "exposed: {word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            assert!(written_nasal_glottal_chunks(&word.as_str().unwrap().chars().collect::<Vec<_>>(), true, overrides::Context::bare("")).is_none(), "outside: {word}");
        }
    }

    #[test]
    fn nasal_ngvae_extension_requires_its_reviewed_coda_vowel_and_original_end() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/nasal-ngvae.json")).unwrap();
        assert_eq!(proof["proofWords"].as_array().unwrap().len(), 5);
        for word in proof["proofWords"].as_array().unwrap() {
            let chars: Vec<_> = word.as_str().unwrap().chars().collect();
            assert!(nasal_pair_governance(&chars, true), "reviewed word: {word}");
            assert!(!nasal_pair_governance(&chars, false), "exposed ending: {word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            assert!(!nasal_pair_governance(&word.as_str().unwrap().chars().collect::<Vec<_>>(), true), "outside scope: {word}");
        }
    }

    #[test]
    fn nasal_ngvaa_requires_original_scoped_initial_and_bare_target_ending() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/nasal-ngvaa.json")).unwrap();
        for word in proof["structuralFrames"].as_array().unwrap() {
            let chars: Vec<char> = word.as_str().unwrap().chars().collect();
            assert!(nasal_pair_governance(&chars, true), "reviewed structural frame: {word}");
            assert!(!nasal_pair_governance(&chars, false), "cancellation exposed: {word}");
        }
        for word in ["កង្វា", "បង្វា", "សង្វាន", "សង្វារ", "ចង្វាក់", "សង្វាត់",
            "ចង្វា៍", "ចង្វាយ៍", "ចង្វាត៍", "កាចង្វា", "ចង្វាកា"] {
            assert!(!nasal_pair_governance(&word.chars().collect::<Vec<_>>(), true), "outside scope: {word}");
        }
        assert_eq!(proof["proofWords"].as_array().unwrap().len(), 4);
        assert_eq!(proof["latentFramesNotProof"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn onset_governance_requires_the_reviewed_whole_word_shape() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/onset-series.json")).unwrap();
        for row in proof["proof"].as_array().unwrap() {
            let word = row["word"].as_str().unwrap();
            let chars: Vec<_> = word.chars().collect();
            assert_eq!(onset_cluster_series(&chars, true), row["series"].as_str(), "{word}");
            assert!(onset_cluster_series(&chars, false).is_none(), "stripped ending: {word}");
        }
        for word in proof["selectorNo"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            assert!(onset_cluster_series(&word.chars().collect::<Vec<_>>(), true).is_none(), "out of scope: {word}");
        }
    }

    #[test]
    fn every_mapped_bare_consonant_remains_reachable_without_a_new_short_rule() {
        for consonant in &rules().consonants {
            if consonant.onset_keys.is_empty() { continue; }
            let word = format!("កា{}", consonant.khmer);
            let chunks = chunks(&word).unwrap_or_else(|| panic!("mapped ending: {word}"));
            let ending = if consonant.khmer == "រ" { String::new() }
                else { coda_keys(consonant.khmer.chars().next().unwrap()).unwrap()[0].text.clone() };
            let key = format!("ka{ending}");
            assert!(match_chunks(&chunks, 0, &key, &mut BTreeMap::new()).is_some_and(|hit| hit.exact), "{word}: {key}");
        }
        // Empty keys and signs with no implemented reading are not epsilon
        // matches. Nor may a default undo a combined vowel or silent ending.
        for word in ["ឨ", "ក៌", "ក៏", "ក៝"] { assert!(chunks(word).is_none(), "{word}"); }
    }

    #[test]
    fn defaults_keep_a_bounded_chunk_lattice_instead_of_expanding_spellings() {
        // 32 two-choice vowels represent over four billion possible keys.
        let word = format!("{}ឃ", "ភា".repeat(32));
        let lattice = chunks(&word).unwrap();
        assert!(lattice.len() <= word.chars().count() * 2);
        let key = format!("{}kh", "phia".repeat(32));
        assert!(match_chunks(&lattice, 0, &key, &mut BTreeMap::new()).is_some_and(|hit| hit.exact));
        let mut prefixes = BTreeSet::new();
        collect_prefixes(&lattice, 0, String::new(), &mut prefixes);
        assert!(prefixes.len() < 10);
        assert!(chunks(&"ភា".repeat(65)).is_none());
    }

    #[test]
    fn velar_selector_covers_reviewed_forms_and_rejects_pronounced_contexts() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/original-gaps.json")).unwrap();
        let selected = |word: &str| {
            let chars: Vec<_> = word.chars().collect();
            let end = chars.len() - 3;
            let vowel_start = end - 1;
            let onset_start = if chars.get(vowel_start - 2) == Some(&'្') { vowel_start - 3 } else { vowel_start - 1 };
            terminal_silent_coda_at(&chars, end, &chars[vowel_start].to_string(),
                &chars[onset_start..vowel_start].iter().collect::<String>(), true).is_some()
        };
        for word in fixture["velarProofWords"].as_array().unwrap() {
            assert!(selected(word.as_str().unwrap()), "{word}");
        }
        // Extra pronounced syllable, conflicting vowel, and a different onset
        // requiring its own vowel governance: no per-word exclusions.
        for word in ["រាជ្យាង្គ", "ស្រឹង្គ", "តុរិយាង្គ"] {
            assert!(!selected(word), "{word}");
        }
    }

    #[test]
    fn selectors_cover_all_reviewed_forms_without_claiming_their_earlier_syllables() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../../test/fixtures/narrow-closing.json")).unwrap();
        let mut stems = 0;
        let mut ry = 0;
        for word in fixture["proofWords"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            let chars: Vec<_> = word.chars().collect();
            let end = chars.len() - 3;
            if word.ends_with("ទ្ធ") {
                let vowel = chars[end - 1].to_string();
                assert!(terminal_short_coda_at(&chars, end, chars[end - 2], &vowel, true).is_some(), "{word}");
                stems += 1;
            } else {
                assert!(terminal_silent_group_at(&chars, end, end - 1, &chars[end - 1].to_string()), "{word}");
                ry += 1;
            }
        }
        assert_eq!((stems, ry), (11, 16));
        // These are attested pronounced/conflicting contexts, not a lookup
        // exclusion list. The structural selectors themselves must fail.
        for word in ["សមិទ្ធ", "អយុទ្ធ"] {
            let chars: Vec<_> = word.chars().collect();
            let end = chars.len() - 3;
            assert!(terminal_short_coda_at(&chars, end, chars[end - 2], &chars[end - 1].to_string(), true).is_none());
        }
        for word in ["អារ្យ", "អនារ្យ", "សូរ្យ", "ពិទូរ្យ"] {
            let chars: Vec<_> = word.chars().collect();
            let end = chars.len() - 3;
            assert!(!terminal_silent_group_at(&chars, end, end - 1, &chars[end - 1].to_string()));
        }
    }
}

// Exactly the accepted first-pair + coeng ល + ាំង shape. Wider nasal
// governance needs its own evidence; this does not split every nasal group.
fn pair_vowel_reading(chars: &[char]) -> Option<&'static PairVowelReading> {
    let first = consonant(*chars.first()?)?;
    let governance = &rules().runtime.cross_syllable_governance;
    if first.series != "a" || governance.passive_consonants.contains(&first.khmer)
        || governance.excluded_initials.contains(&first.khmer) || chars.get(2) != Some(&'្') {
        return None;
    }
    rules().runtime.pair_vowel_readings.iter().find(|rule| {
        let suffix = format!("{}្{}{}{}", rule.coda, rule.onset, rule.vowel, rule.final_consonant);
        chars[1..].iter().copied().eq(suffix.chars())
    })
}

// C1 + nasal C2 + coeng C3: C2 closes the first syllable; C3 starts
// the second with A-series vowel keys. Only the reviewed original two-syllable
// shape qualifies, never a suffix exposed by cancellation or terminal silence.
fn nasal_pair_governance(chars: &[char], original_terminal: bool) -> bool {
    let rule = &rules().runtime.nasal_pair_governance;
    let Some(first) = chars.first().and_then(|&ch| consonant(ch)) else { return false; };
    let Some(coda) = chars.get(1).and_then(|&ch| consonant(ch)) else { return false; };
    let Some(onset) = chars.get(3).and_then(|&ch| consonant(ch)) else { return false; };
    if !original_terminal || first.series != "a" || chars.get(2) != Some(&'្')
        || coda.khmer == onset.khmer || onset.series != "o" || onset.onset_keys.is_empty()
        || !rule.initials.contains(&first.khmer) || !rule.codas.contains(&coda.khmer)
        || coda_keys(chars[1]).is_none()
    {
        return false;
    }
    let end = 4 + chars[4..].iter().take_while(|&&ch| is_vowel(ch)).count();
    let vowel: String = chars[4..end].iter().collect();
    if !rule.vowels.contains(&vowel) || vowel_keys(&vowel, "a").is_none() {
        return false;
    }
    // The reviewed extension is a coda/onset/vowel combination, not a
    // general addition of វ to every nasal pair and every written vowel.
    let additional_shape = rule.additional_shapes.iter().find(|shape| {
        shape.coda == coda.khmer && shape.onset == onset.khmer && shape.vowel == vowel
    });
    if !rule.onsets.contains(&onset.khmer) && additional_shape.is_none() {
        return false;
    }
    if additional_shape.is_some_and(|shape| {
        shape.initials.as_ref().is_some_and(|initials| !initials.contains(&first.khmer))
    }) {
        return false;
    }
    let tail = &chars[end..];
    // These composites already contain a closing sound. Only the reviewed
    // ាំ + ង combination can take another written final.
    if vowel == "ាំ" { return tail == ['ង']; }
    if vowel.ends_with('ះ') { return tail.is_empty(); }
    match tail {
        [] => additional_shape.and_then(|shape| shape.allow_open).unwrap_or(true),
        [final_consonant] => {
            let final_spelling = final_consonant.to_string();
            if additional_shape.is_some_and(|shape| {
                shape.allowed_finals.as_ref().is_some_and(|finals| !finals.contains(&final_spelling))
            }) {
                return false;
            }
            rules().runtime.codas.contains_key(&final_spelling)
                || additional_shape.is_some_and(|shape| {
                    shape.silent_finals.contains(&final_spelling)
                        && rules().runtime.word_final_silent_consonants.contains(&final_spelling)
                })
        }
        [final_consonant, '់'] => additional_shape.and_then(|shape| shape.allow_bantoc).unwrap_or(true)
            && vowel == rules().runtime.explicit_bantoc.vowel
            && additional_shape.is_none_or(|shape| {
                shape.allowed_finals.as_ref().is_none_or(|finals| finals.contains(&final_consonant.to_string()))
            })
            && rules().runtime.codas.contains_key(&final_consonant.to_string()),
        _ => false,
    }
}

fn supported_shifter(initial: char, mark: char) -> bool {
    (mark == '៉' && matches!(initial, 'ង' | 'ញ' | 'ម' | 'យ' | 'រ' | 'វ' | 'ប'))
        || (mark == '៊' && matches!(initial, 'ស' | 'ហ' | 'ប' | 'អ'))
}

// A shifter sits between the next simple onset and its written vowel. It
// does not turn that onset into the preceding syllable's final consonant.
fn shifted_written_onset(chars: &[char], start: usize) -> bool {
    let Some(&initial) = chars.get(start) else { return false };
    let Some(&mark) = chars.get(start + 1) else { return false };
    if consonant(initial).is_none() || !supported_shifter(initial, mark) {
        return false;
    }
    let vowel: String = chars[start + 2..].iter().take_while(|&&ch| is_vowel(ch)).collect();
    !vowel.is_empty() && rules().additional_vowel_forms.iter().chain(&rules().dependent_vowels)
        .any(|v| v.khmer == vowel)
}

// A reviewed coda can precede a final C+C syllable whose vowel is unwritten.
// Keep learned m/l/t reactivation out of this selector, and require an actual
// bare final (optionally bantoc); no new word/component boundaries are saved.
fn terminal_inherent_boundary(chars: &[char], coda: usize, next: usize, vowel: &str) -> bool {
    if vowel.is_empty() || vowel.contains(['ំ', 'ះ'])
        || !chars.get(coda).is_some_and(|ch| rules().runtime.terminal_inherent_boundary_codas.contains(&ch.to_string())) {
        return false;
    }
    let Some(&initial) = chars.get(next) else { return false };
    let Some(&last) = chars.get(next + 1) else { return false };
    initial != 'អ' && consonant(initial).is_some_and(|c| !c.onset_keys.is_empty())
        && rules().runtime.codas.contains_key(&last.to_string())
        && (next + 2 == chars.len() || (next + 3 == chars.len() && chars.last() == Some(&'់')))
}

// Two checked whole words, not a general nasal-vowel boundary. Close their
// first ាំ+ង syllable with the existing a/ea and ng keys, then resume ordinary
// suffix parsing. Original spelling prevents cancellation or a longer word
// from exposing the same local shape.
fn checked_aam_ng_boundary(
    text: &str, chars: &[char], onset: usize, coda: usize, next: usize,
    vowel: &str, original: bool,
) -> bool {
    let Some(rule) = &rules().runtime.checked_aam_ng_boundary else { return false };
    original && onset == 0 && coda == 3 && next == 4 && vowel == "ាំ"
        && chars.get(coda) == Some(&'ង')
        && chars.iter().copied().eq(text.chars())
        && rule.words.iter().any(|word| word == text)
}

// In the reviewed original ក/ត/ម + ន្ត្រ + written-vowel frame, ន
// closes the first syllable and ត្រ starts the second. Neither coeng is silent.
fn written_nasal_boundary(chars: &[char], i: usize, original: bool, id: &str) -> bool {
    let Some(rule) = rules().runtime.nasal_boundaries.written.iter().find(|rule| rule.id == id) else { return false };
    if !original || i != 1 || chars.len() < 7
        || !rule.initials.contains(&chars[0].to_string())
        || consonant(chars[0]).is_none_or(|c| c.onset_keys.is_empty())
        || !rule.clusters.iter().any(|cluster| segment_end(chars, i, cluster) == Some(6)) {
        return false;
    }
    let end = 6 + chars[6..].iter().take_while(|&&ch| is_vowel(ch)).count();
    let vowel: String = chars[6..end].iter().collect();
    if end == 6 || !rules().dependent_vowels.iter().chain(&rules().additional_vowel_forms)
        .any(|v| v.khmer == vowel) { return false; }
    match &chars[end..] {
        [] => true,
        [last] if rules().runtime.codas.contains_key(&last.to_string()) => true,
        [last, '់'] if vowel == rules().runtime.explicit_bantoc.vowel
            && rules().runtime.codas.contains_key(&last.to_string()) => true,
        _ => false,
    }
}
// Keep named entry points and parser precedence while sharing the matching mechanics.
fn written_ntr_boundary(chars: &[char], i: usize, original: bool) -> bool {
    written_nasal_boundary(chars,i,original,"ntr")
}
fn written_ndr_boundary(chars: &[char], i: usize, original: bool) -> bool {
    written_nasal_boundary(chars,i,original,"ndr")
}
fn written_nh_r_boundary(chars: &[char], i: usize, original: bool) -> bool {
    written_nasal_boundary(chars,i,original,"nhR")
}

// In the original ច/ស + ង/coeng ក or គ/coeng រ frame, ង closes
// the first syllable. Both subscripts start the complete closed second syllable.
fn written_velar_r_boundary(chars: &[char], i: usize, original: bool) -> bool {
    let rule = &rules().runtime.written_velar_r_boundary;
    if !original || i != 1 || chars.len() < 7
        || !rule.initials.contains(&chars[0].to_string())
        || consonant(chars[0]).is_none_or(|c| c.onset_keys.is_empty())
        || !rule.clusters.iter().any(|cluster| segment_end(chars, i, cluster) == Some(6))
    {
        return false;
    }
    let end = 6 + chars[6..].iter().take_while(|&&ch| is_vowel(ch)).count();
    let vowel: String = chars[6..end].iter().collect();
    if end == 6 || !rules().dependent_vowels.iter().chain(&rules().additional_vowel_forms)
        .any(|v| v.khmer == vowel)
    {
        return false;
    }
    // Nasal-vowel signs and open syllables stay held. Visarga already closes
    // its syllable and must not gain a following consonant through this rule.
    if vowel.contains('ំ') { return false; }
    if vowel.ends_with('ះ') { return end == chars.len(); }
    match &chars[end..] {
        [last] if rules().runtime.codas.contains_key(&last.to_string()) => true,
        [last, '់'] if vowel == rules().runtime.explicit_bantoc.vowel
            && rules().runtime.codas.contains_key(&last.to_string()) => true,
        _ => false,
    }
}

// A written first vowel does not turn the reviewed nasal coda into an onset.
// In the original two-syllable C + i/u + NH/coeng C frame, consume NH with
// the existing coda keys and let subscript C independently start the next.
fn written_nh_boundary(chars: &[char], onset: usize, i: usize, original: bool) -> bool {
    let rule = &rules().runtime.written_nh_boundary;
    if !original || onset != 0 || i != 2 || chars.len() < 6
        || chars[0] == 'អ' || consonant(chars[0]).is_none()
        || !rule.first_vowels.contains(&chars[1].to_string())
        || segment_end(chars, i, &rule.cluster) != Some(5)
    {
        return false;
    }
    let end = 5 + chars[5..].iter().take_while(|&&ch| is_vowel(ch)).count();
    let vowel: String = chars[5..end].iter().collect();
    if end == 5 || !rules().dependent_vowels.iter().chain(&rules().additional_vowel_forms)
        .any(|v| v.khmer == vowel)
    {
        return false;
    }
    match &chars[end..] {
        [] => true,
        [last] if rules().runtime.codas.contains_key(&last.to_string()) => true,
        [last, '់'] if vowel == rules().runtime.explicit_bantoc.vowel
            && rules().runtime.codas.contains_key(&last.to_string()) => true,
        _ => false,
    }
}

// A bare final completes the reviewed second syllable even without a written
// vowel. The nasal closes the first; the two coengs form a pronounced stop/r
// onset. Do not infer this split in longer, shifted or cancelled spellings.
fn unwritten_nasal_r_boundary(chars: &[char], i: usize, original: bool) -> bool {
    let rule = &rules().runtime.unwritten_nasal_r_boundary;
    if !original || i != 1 || !matches!(chars.len(), 7 | 8)
        || !rule.initials.contains(&chars[0].to_string())
        || !rule.clusters.iter().any(|cluster| segment_end(chars, i, cluster) == Some(6))
        || !rules().runtime.codas.contains_key(&chars[6].to_string())
    {
        return false;
    }
    chars.len() == 7 || chars[7] == '់'
}

// Compose one explicit prefix syllable with an already reviewed complete
// terminal nasal block. Keep three syllables and the original spelling guard;
// complex prefixes, other pairs and longer continuations retain their rules.
fn terminal_nasal_compound_chunks(chars: &[char], original: bool) -> Option<Vec<Vec<Key>>> {
    if !original { return None; }
    let rule = &rules().runtime.terminal_nasal_compound_boundary;
    let suffix_len = if chars.last() == Some(&'់') { 6 } else { 5 };
    let start = chars.len().checked_sub(suffix_len)?;
    if start < 2 { return None; }
    let suffix = &chars[start..];
    if !rule.initials.contains(&suffix[0].to_string())
        || !rule.clusters.iter().any(|cluster| segment_end(suffix, 1, cluster) == Some(4)) {
        return None;
    }
    if !unwritten_nasal_stop_boundary(suffix, 1, true)
        && !unwritten_nasal_aspirated_boundary(suffix, 1, true) { return None; }
    let prefix = &chars[..start];
    let base = consonant(prefix[0])?;
    if prefix[0] == 'អ' { return None; }
    let end = 1 + prefix[1..].iter().take_while(|&&ch| is_vowel(ch)).count();
    if end == 1 { return None; }
    let vowel: String = prefix[1..end].iter().collect();
    if !rules().dependent_vowels.iter().chain(&rules().additional_vowel_forms).any(|v| {
        v.khmer == vowel && if base.series == "a" { !v.a_series_keys.is_empty() } else { !v.o_series_keys.is_empty() }
    }) || vowel.contains('ំ') { return None; }
    if vowel.ends_with('ះ') {
        if end != prefix.len() { return None; }
    } else {
        match &prefix[end..] {
            [] => {},
            [last] if *last != 'រ' && rules().runtime.codas.contains_key(&last.to_string()) => {},
            [last, '់'] if *last != 'រ' && vowel == rules().runtime.explicit_bantoc.vowel
                && rules().runtime.codas.contains_key(&last.to_string()) => {},
            _ => return None,
        }
    }
    // Both pieces are strictly shorter than the original word. Reuse their
    // reviewed mappings without inferring another boundary or silent letter.
    let prefix_text: String = prefix.iter().collect();
    let suffix_text: String = suffix.iter().collect();
    let mut result = chunks(&prefix_text)?;
    result.extend(chunks(&suffix_text)?);
    Some(result)
}

// In the reviewed two-syllable frame, the nasal closes the first pair and
// its single stop subscript starts the second. Keep the later mapped vowel
// and final keys; a bare final is required, so longer/open forms stay separate.
fn unwritten_nasal_boundary(chars: &[char], i: usize, original: bool, id: &str) -> bool {
    let Some(rule) = rules().runtime.nasal_boundaries.unwritten.iter().find(|rule| rule.id == id) else { return false };
    if !original || i != 1 || !matches!(chars.len(), 5 | 6)
        || !rule.initials.contains(&chars[0].to_string())
        || !rule.clusters.iter().any(|cluster| segment_end(chars, i, cluster) == Some(4))
        || !rules().runtime.codas.contains_key(&chars[4].to_string())
        || rule.finals.as_ref().is_some_and(|finals| !finals.contains(&chars[4].to_string())) {
        return false;
    }
    chars.len() == 5 || chars[5] == '់'
}
fn unwritten_nasal_stop_boundary(chars: &[char], i: usize, original: bool) -> bool {
    unwritten_nasal_boundary(chars,i,original,"stop")
}
fn unwritten_nasal_aspirated_boundary(chars: &[char], i: usize, original: bool) -> bool {
    unwritten_nasal_boundary(chars,i,original,"aspirated")
}

// Only the independently parsed stop/r onset in this exact implicit-vowel
// frame gains its stop's series. Arbitrary unwritten clusters keep defaults.
fn unwritten_nasal_r_series(chars: &[char], onset: usize, original: bool) -> Option<&'static str> {
    if onset != 3 || !unwritten_nasal_r_boundary(chars, 1, original) {
        return None;
    }
    Some(consonant(chars[onset])?.series.as_str())
}

// In C1 C2 coeng C3 + vowel, the first consonant pair can form a
// syllable and C3 begins the next. This is not specific to medial ម.
// Doubled C2/C3 spellings and unresolved series governance retain their own
// handling; no dictionary components or syllables are saved here.
fn pair_before_subscript(series: &str, chars: &[char], i: usize, nasal_pair: bool) -> Option<bool> {
    if !rules().runtime.pair_before_subscript || chars.get(i + 1) != Some(&'្') {
        return Some(false);
    }
    let second = consonant(*chars.get(i)?)?;
    let third = consonant(*chars.get(i + 2)?)?;
    if second.khmer == third.khmer || third.onset_keys.is_empty() {
        return Some(false);
    }
    let start = i + 3;
    let end = chars[start..].iter().take_while(|&&ch| is_vowel(ch)).count() + start;
    if start == end {
        return Some(false);
    }
    rules().runtime.codas.get(&second.khmer)?;
    let vowel: String = chars[start..end].iter().collect();
    let reading = rules().additional_vowel_forms.iter().chain(&rules().dependent_vowels)
        .find(|v| v.khmer == vowel)?;
    if series == "a" && third.series == "o"
        && rules().runtime.cross_syllable_governance.passive_consonants.contains(&third.khmer)
        // Shared typing keys for ឺ/ើ do not resolve their held series governance.
        // Changing a typing key must not newly split this syllable boundary.
        && (matches!(reading.khmer.as_str(), "ឺ" | "ើ")
            // ោះ already permits this boundary. Its later A/O key split must
            // not change an earlier syllable, e.g. jon- in ចន្លោះ.
            || (reading.khmer != "ោះ" && reading.a_series_keys != reading.o_series_keys))
    {
        // This boundary is clear, but the later vowel still needs its own
        // governance rule. An invariant vowel such as ៀ is already safe.
        if i != 1 || (pair_vowel_reading(chars).is_none() && !nasal_pair) {
            return None;
        }
    }
    Some(true)
}

// A-series governance in two simple syllables: an open first syllable or a
// listed nasal vowel ending (ំ). A later stop/fricative keeps its own series;
// other closed first syllables, clusters and longer units remain unsupported.
fn governed_onset(text: &str, chars: &[char]) -> Option<usize> {
    let rule = &rules().runtime.cross_syllable_governance;
    if rule.retained_series_words.iter().any(|word| word == text) {
        return None;
    }
    let first = consonant(*chars.first()?)?;
    if first.series != "a"
        || rule.passive_consonants.contains(&first.khmer)
        || rule.excluded_initials.contains(&first.khmer)
    {
        return None;
    }
    let first_vowel = chars.get(1)?;
    if !('\u{17b6}'..='\u{17c5}').contains(first_vowel)
        && !rule.nasal_first_syllable_vowels.contains(&first_vowel.to_string()) {
        return None;
    }
    let onset = 2;
    let second = consonant(*chars.get(onset)?)?;
    if second.series != "o" || !rule.passive_consonants.contains(&second.khmer) {
        return None;
    }
    if !chars
        .get(3)
        .is_some_and(|ch| ('\u{17b6}'..='\u{17c5}').contains(ch))
    {
        return None;
    }
    let i = 4;
    let simple_end = i == chars.len()
        || (consonant(*chars.get(i)?).is_some()
            && (i + 1 == chars.len() || (i + 2 == chars.len() && chars.get(i + 1) == Some(&'់'))));
    simple_end.then_some(onset)
}

// Keep mapped alternatives in chunk lattices, without expanding full spellings.
// A checked additive root may retain a second, independent mapped lattice;
// their chunks must never mix across a lexical boundary.
struct Word {
    candidate: Candidate,
    readings: Vec<Vec<Vec<Key>>>,
    graph: Option<factored::Graph>,
    apostrophe_graph: Option<factored::Graph>,
    recovery_lengths: Vec<(usize, usize)>,
    // Only shorthand-bearing words need a separate inspection view. Matching
    // keeps the original lattices, including overlapping shorthand keys.
    ordinary_readings: Option<Vec<Vec<Vec<Key>>>>,
}

pub(crate) fn default_shorthands(word:&str)->Vec<String> {
    rules().runtime.shorthand_readings.iter().filter(|rule|rule.khmer==word)
        .flat_map(|rule|rule.keys.iter().map(|key|key.text.clone())).collect()
}
#[cfg(test)]
pub(crate) fn shorthand_match(input:&str,word:&str)->bool {
    let normalizer=rules().runtime.consonant_count_tolerance.normalizer();
    rules().runtime.shorthand_readings.iter().any(|rule|rule.khmer==word
        && rule.keys.iter().any(|key|key.text==input || normalizer.is_some_and(|normalizer|
            normalizer.input(input).is_some_and(|normalized|normalizer.input(&key.text).as_ref()==Some(&normalized)))))
}

fn has_shorthand_rule(word:&str)->bool {
    rules().runtime.shorthand_readings.iter().any(|rule|rule.khmer==word)
}

// Only complete independently available words establish this boundary. A
// substring match cannot turn a genuine final ស into the next part's onset.
// This guard never imports a part's readings, aliases, ranks or policies.
fn lexical_part_onsets(word: &str, dictionary: &compact::Dictionary, base: Option<&compact::Dictionary>) -> Vec<usize> {
    if !word.contains('ស') { return Vec::new(); }
    let contains = |part: &str|dictionary.khmer.contains(part) || base.is_some_and(|base|base.khmer.contains(part));
    let boundaries: Vec<_> = word.char_indices().map(|(byte,_)|byte).chain(std::iter::once(word.len())).collect();
    let mut previous = None;
    word.char_indices().enumerate().filter_map(|(offset,(byte,ch))| {
        let subscript = previous == Some('្');
        previous = Some(ch);
        (ch == 'ស' && offset > 0 && !subscript && byte + ch.len_utf8() < word.len()
            && contains(&word[..byte])
            // A middle part requires the complete remaining word too. Never
            // infer a boundary from a matching substring or the lone letter ស.
            && boundaries.iter().skip(offset + 2).any(|&end|
                contains(&word[byte..end]) && (end == word.len() || contains(&word[end..]))))
            .then_some(offset)
    }).collect()
}
impl Word {
    fn ordinary_readings(&self) -> &[Vec<Vec<Key>>] {
        self.ordinary_readings.as_deref().unwrap_or(&self.readings)
    }
    fn new(candidate: Candidate, readings: Vec<Vec<Vec<Key>>>) -> Self {
        let recovery_lengths = readings.iter().map(|reading| reading.iter().fold((0, 0), |(min, max), keys|
            (min + keys.iter().map(|key| key.len()).min().unwrap_or(0),
             max + keys.iter().map(|key| key.len()).max().unwrap_or(0)))).collect();
        let graph=factored::has_paths(&readings).then(||factored::Graph::new(&readings));
        Self { candidate, readings, graph, apostrophe_graph: None, recovery_lengths, ordinary_readings: None }
    }
}
pub(crate) struct RankedSuggestion {
    pub suggestion: Suggestion,
    pub priority: i64,
}

/// Completion witnesses retain their apostrophes. A literal or existing
/// consonant-count prefix is ordinary; only omitted-apostrophe paths are
/// deferred when merging independent dictionaries. Saved prefixes stay literal.
pub(crate) fn omitted_apostrophe_completion(hit:&Suggestion,input:&str)->bool {
    let Some(key)=&hit.completion else {return false;};
    if hit.correction.is_some() || !matches!(&hit.candidate,SuggestionCandidate::Khmer(_))
        || !key.contains('\'') || key.starts_with(input) {return false;}
    !rules().runtime.consonant_count_tolerance.normalizer().is_some_and(|n|
        n.input(key).zip(n.input(input)).is_some_and(|(key,input)|key.starts_with(&input)))
}
#[derive(Default)]
pub(crate) struct Index {
    words: Vec<Word>,
    prefixes: BTreeMap<String, Vec<usize>>,
    count_prefixes: BTreeMap<String, Vec<usize>>,
    apostrophe_prefixes: BTreeMap<String, Vec<usize>>,
    has_rank_priorities: bool,
    has_so: bool,
    dictionary: Option<compact::Dictionary>,
    part_policy: Mutex<Option<PartPolicy>>,
    commonness: Option<crate::word_metadata::Commonness>,
}
struct PartPolicy {
    source: compact::Dictionary,
    index: Box<Index>,
}
pub(crate) fn has_pronunciation_default(word: &str) -> bool {
    read_as::owns(word) || rules().runtime.pronunciation_defaults.iter().any(|rule| rule.word == word)
}

pub(crate) fn validate_pronunciation_boundaries(rules:&[crate::Pronunciation])->Result<(),String> {
    read_as::configured(rules).map(|_|())
}
pub(crate) fn pronunciation_inputs(text:&str)->Vec<String> {
    let mut readings=word_readings_in_context(text,false,overrides::Context::new(text).for_pronunciation()).unwrap_or_default();
    ending_alternatives::append(&mut readings);
    browse::display_inputs(&readings,64).0
}

struct PronunciationPaths {
    readings: Vec<Vec<Vec<Key>>>,
    preserves_checked_neighbors: bool,
}

// Read a configured alternate in isolation, then join reading lattices at
// complete component boundaries. Re-parsing joined Khmer would change its
// vowels again and could turn a valid parent's readings into an empty set.
fn pronunciation_readings(word:&str,pronunciations:&[crate::Pronunciation],
    parts:Option<&crate::components::ReadingSplits>,context:overrides::Context<'_>,depth:usize,
    checked_parent: bool,
) -> Option<PronunciationPaths> {
    if let Some(rule)=pronunciations.iter().find(|rule|rule.word==word) {
        if depth > 0 && rules().runtime.non_reusable_word_parts.contains(word) {
            return None;
        }
        let readings = read_as::pronunciation_paths(&rule.read_as)?;
        let preserves_checked_neighbors = !checked_parent || depth == 0
            || component_words::agrees_with_checked_neighbor(context.word, word, &readings);
        return Some(PronunciationPaths { readings, preserves_checked_neighbors });
    }
    if depth>=32 || context.whole(word).is_some()
        || !pronunciations.iter().any(|rule|word.contains(&rule.word)) {return None;}
    let parts = parts?;
    let components = parts.parts(word)?;
    let chars: Vec<_> = word.chars().collect();
    let cuts: Vec<_> = components.iter().skip(1).map(|part| part.start).collect();
    // A complete dictionary split can still cut through a reviewed parser
    // unit. Keep the original parser path when the two boundaries conflict.
    if context.with_original(overrides::original_view(word)).crosses_reviewed_part(&chars, &cuts) {
        return None;
    }
    let mut changed = false;
    let mut preserves_checked_neighbors = true;
    let mut paths = vec![Vec::new()];
    for part in components {
        let child = context.part(part.start,part.end).as_pronunciation_component();
        let mut choices = if let Some(result) = pronunciation_readings(
            &part.text, pronunciations, Some(parts), child, depth + 1, checked_parent,
        ) {
            changed = true;
            preserves_checked_neighbors &= result.preserves_checked_neighbors;
            result.readings
        } else {
            let child = if parts.is_standalone(&part.text) {
                child.with_stock_pronunciations()
            } else {
                child
            };
            let mut readings = word_readings_in_context(&part.text, false, child).unwrap_or_default();
            component_words::append_scoped_part(&part.text, child, &mut readings);
            if readings.is_empty() { return None; }
            readings
        };
        ending_alternatives::append(&mut choices);
        let mut next=Vec::new();
        'paths: for prefix in &paths {
            for choice in &choices {
                if prefix.len()+choice.len()>128 {continue;}
                let mut joined=prefix.clone();joined.extend(choice.clone());
                if !next.contains(&joined) {next.push(joined);}
                if next.len()==64 {break 'paths;}
            }
        }
        if next.is_empty() {return None;}
        paths=next;
    }
    if !changed {
        return None;
    }
    if depth == 0 && preserves_checked_neighbors {
        // Keep the approved full-word root reading beside a customized
        // component path. A direct edit of the full word returns above.
        if let Some(checked) = checked_terminal_root(word, context.with_stock_pronunciations()) {
            if !paths.contains(&checked) {
                paths.push(checked);
            }
        }
    }
    Some(PronunciationPaths { readings: paths, preserves_checked_neighbors })
}
impl Index {
    pub(crate) fn new(dictionary: &compact::Dictionary) -> Self {
        let mut index = Self::new_with_parts(dictionary,None);
        index.dictionary = Some(dictionary.clone());
        index
    }
    fn new_with_parts(dictionary: &compact::Dictionary, base: Option<&compact::Dictionary>) -> Self {
        let mut index = Self::default();
        let overrides = base.map_or(&dictionary.pronunciations, |base| &base.pronunciations);
        let configured_pronunciations: Vec<_> = rules().runtime.pronunciation_defaults.iter()
            .filter(|stock| !overrides.iter().any(|rule| rule.word == stock.word))
            .chain(overrides.iter()).filter(|rule| {
                let blocked = |source: &compact::Dictionary| {
                    source.sound_map_disabled(&rule.word) || source.excluded_khmer.contains(&rule.word)
                };
                !blocked(dictionary) && !base.is_some_and(blocked)
            }).cloned().collect();
        let prepared = read_as::configured(&configured_pronunciations).expect("validated pronunciation settings");
        // Stock migrations already retain their original parser cases and scopes.
        // Only explicit edits rebuild whole component paths; rebuilding them
        // for every stock part would discard checked neighboring readings.
        let pronunciations:Vec<_>=overrides.iter().filter(|rule|
            configured_pronunciations.iter().any(|active| active.word == rule.word)
                && !read_as::owns(&rule.word)
                // Saving a stock spelling must retain its scoped paths,
                // rather than switching to whole-component reconstruction.
                && !rules().runtime.pronunciation_defaults.iter().any(|stock|
                    stock.word == rule.word && stock.read_as == rule.read_as)).cloned().collect();
        let components=(rules().runtime.word_rule_reuse.enabled || rules().runtime.ending_alternatives.enabled || !pronunciations.is_empty())
            .then(||crate::components::ReadingSplits::new(dictionary,base));
        let mut cursor = dictionary.khmer.cursor(0);
        while let Some(row) = cursor.next().expect("validated dictionary") {
            if row.target != 0 || crate::contains_profanity(row.key, crate::ProfanityLanguage::Khmer) {
                continue;
            }
            let onsets = lexical_part_onsets(row.key,dictionary,base);
            let standalone = base.and_then(|source| source.component_definitions.get(row.key))
                .or_else(|| dictionary.component_definitions.get(row.key))
                .is_some_and(Vec::is_empty);
            let shorthand_override=base.and_then(|base|base.shorthands.get(row.key)).or_else(||dictionary.shorthands.get(row.key));
            let shorthand_inputs=dictionary.shorthand_inputs(row.key,base);
            let mut context = overrides::Context::new(row.key).with_word_part_onsets(&onsets);
            if shorthand_override.is_some() {context=context.without_shorthands();}
            if !standalone {
                context = context.with_read_as(&prepared.syllables)
                    .with_word_part_pronunciations(&prepared.word_parts);
            } else if read_as::owns(row.key) {
                // A direct edit of the standalone word still applies.
                context = context.with_read_as(&prepared.syllables);
            }
            let direct_default = rules().runtime.pronunciation_defaults.iter().find(|rule|rule.word == row.key
                && configured_pronunciations.iter().any(|active|active.word == rule.word)
                && !overrides.iter().any(|edit| edit.word == row.key && edit.read_as != rule.read_as));
            let direct = direct_default.and_then(|rule| read_as::pronunciation_paths(&rule.read_as));
            let custom = direct.or_else(|| (!pronunciations.is_empty()).then(|| {
                pronunciation_readings(row.key, &pronunciations, components.as_ref(), context, 0,
                    component_words::protected_parent(row.key))
            }).flatten().filter(|result| result.preserves_checked_neighbors)
                .map(|result| result.readings));
            let customized=custom.is_some();
            let mut readings=custom.unwrap_or_else(||word_readings_in_context(row.key,false,context).unwrap_or_default());
            if !customized && (rules().runtime.word_rule_reuse.enabled || rules().runtime.ending_alternatives.enabled) {
                if let Some(components)=&components {component_words::append(row.key,components,&prepared.components,context,&mut readings);}
            }
            let mut ordinary_readings = if shorthand_inputs.is_empty() { None }
                else if shorthand_override.is_some() || customized { Some(readings.clone()) }
                else {
                    // Built-in shorthands can enter through the parser itself.
                    // Inspect that parser without them rather than subtracting
                    // key text that may also have a genuine phonetic path.
                    let context=context.without_shorthands();
                    let mut ordinary=word_readings_in_context(row.key,false,context).unwrap_or_default();
                    if rules().runtime.word_rule_reuse.enabled || rules().runtime.ending_alternatives.enabled {
                        if let Some(components)=&components {component_words::append(row.key,components,&prepared.components,context,&mut ordinary);}
                    }
                    Some(ordinary)
                };
            // End alternatives derive only from phonetic paths, never shorthands.
            let additions=ending_alternatives::alternatives(ordinary_readings.as_deref().unwrap_or(&readings));
            readings.extend(additions.clone());
            if let Some(ordinary)=&mut ordinary_readings {ordinary.extend(additions);}
            // Read as replaces phonetic paths, not this word's independent
            // shorthand. Explicit empty overrides still remove stock shortcuts.
            if shorthand_override.is_some() || customized {
                let keys:Vec<Key>=shorthand_override.map(|inputs|inputs.iter().cloned().map(Key::from).collect())
                    .unwrap_or_else(||rules().runtime.shorthand_readings.iter().filter(|rule|rule.khmer==row.key)
                        .flat_map(|rule|rule.keys.iter().cloned()).collect());
                if !keys.is_empty() {readings.push(vec![keys]);}
            }
            if readings.is_empty() { continue; }
            index.has_so |= row.key.contains('ស') || configured_pronunciations.iter()
                .any(|rule|rule.read_as.spellings().iter().any(|spelling|spelling.contains('ស')));
            let mut prefixes = BTreeSet::new();
            let mut count_prefixes = BTreeSet::new();
            if factored::has_paths(&readings) {
                let graph=factored::Graph::new(&readings);
                graph.prefixes(None,&mut prefixes);
                if let Some(n)=rules().runtime.consonant_count_tolerance.normalizer() {graph.prefixes(Some(n),&mut count_prefixes);}
                index.has_rank_priorities=true;
            }
            for reading in &readings {
                collect_prefixes(reading, 0, String::new(), &mut prefixes);
                if let Some(normalizer) = rules().runtime.consonant_count_tolerance.normalizer() {
                    normalizer.collect_prefixes(reading,&mut count_prefixes);
                }
                index.has_rank_priorities |= reading.iter().flatten().any(|key| key.priority != 0);
            }
            let id = index.words.len();
            for prefix in prefixes {
                index.prefixes.entry(prefix).or_default().push(id);
            }
            for prefix in count_prefixes {
                index.count_prefixes.entry(prefix).or_default().push(id);
            }
            let mut prepared=Word::new(
                Candidate {
                    mapping_id: Some(row.id),
                    mapping_ids: Vec::new(),
                    khmer: row.key.to_owned(),
                    rank: row.rank,
                },
                readings,
            );
            prepared.ordinary_readings=ordinary_readings;
            prepared.apostrophe_graph=factored::Graph::apostrophe_completions(prepared.ordinary_readings());
            if let Some(graph)=&prepared.apostrophe_graph {
                let mut prefixes=BTreeSet::new();graph.optional_apostrophe_prefixes(None,&mut prefixes);
                if let Some(n)=rules().runtime.consonant_count_tolerance.normalizer() {graph.optional_apostrophe_prefixes(Some(n),&mut prefixes);}
                for prefix in prefixes {index.apostrophe_prefixes.entry(prefix).or_default().push(id);}
            }
            index.words.push(prepared);
        }
        for ids in index.prefixes.values_mut().chain(index.count_prefixes.values_mut()).chain(index.apostrophe_prefixes.values_mut()) {
            ids.sort_by_key(|&i| {
                (
                    index.words[i].candidate.rank,
                    index.words[i].candidate.mapping_id,
                )
            });
        }
        index
    }
    // Personal dictionaries borrow complete part identities from the base too.
    // Prepare one bounded view per base generation, never reparse on keystrokes.
    // Holding the source Arc makes pointer identity safe across replacements.
    fn part_policy(&self, base: Option<&compact::Dictionary>) -> Option<MutexGuard<'_,Option<PartPolicy>>> {
        let base = base?;
        let dictionary = self.dictionary.as_ref()?;
        if dictionary.same_reading_policy(base)
            || (!self.has_so && !rules().runtime.word_rule_reuse.enabled && !rules().runtime.ending_alternatives.enabled && base.pronunciations.is_empty() && base.shorthands.is_empty()) { return None; }
        let mut cached = self.part_policy.lock().unwrap_or_else(|poisoned|poisoned.into_inner());
        if cached.as_ref().is_none_or(|view|!view.source.same_reading_policy(base)) {
            let mut index = Self::new_with_parts(dictionary,Some(base));
            // Reused components may introduce new prefixes. Retain reviewed
            // ordering while building this view once per base policy.
            if let Some(commonness)=&self.commonness { index.set_commonness(commonness); }
            *cached = Some(PartPolicy {source:base.clone(),index:Box::new(index)});
        }
        Some(cached)
    }
    pub(crate) fn set_commonness(&mut self, commonness: &crate::word_metadata::Commonness) {
        self.commonness=Some(commonness.clone());
        *self.part_policy.get_mut().unwrap_or_else(|poisoned|poisoned.into_inner()) = None;
        for ids in self.prefixes.values_mut().chain(self.count_prefixes.values_mut()) {
            ids.sort_by_key(|&i| {
                let candidate = &self.words[i].candidate;
                (commonness.level("khmer", &candidate.khmer), candidate.rank, candidate.mapping_id)
            });
        }
    }
    pub(crate) fn suggest(&self, input: &str, completions: bool) -> Vec<RankedSuggestion> {
        self.suggest_with_policy(input, completions, None)
    }
    /// Recovery needs complete matches only. Avoid constructing or exploring a
    /// long completion for each edited key; the ordinary matcher also supports
    /// prefixes and signed path ranking, which are unnecessary here.
    pub(crate) fn recovery_candidates(&self, input: &str,
        base_policy: Option<&compact::Dictionary>, scratch: &mut Vec<u8>) -> Vec<Candidate> {
        if let Some(prepared) = self.part_policy(base_policy) {
            return prepared.as_ref().unwrap().index.recovery_candidates(input,base_policy,scratch);
        }
        if input.is_empty() || input.starts_with('\'') || input.contains("''") { return Vec::new(); }
        let Some(ids) = self.prefixes.get(&input[..input.len().min(3)]) else { return Vec::new(); };
        ids.iter().filter_map(|&id| {
            let word = &self.words[id];
            if !Self::allows(word, base_policy) { return None; }
            if let Some(graph)=&word.graph {
                return graph.matching(input,None,false).is_some_and(|hit|hit.exact).then(||word.candidate.clone());
            }
            word.readings.iter().zip(&word.recovery_lengths).any(|(reading, &(min, max))| {
                if input.len() < min || input.len() > max { return false; }
                scratch.resize((reading.len() + 1) * (input.len() + 1), 0);
                scratch.fill(0);
                exact_recovery(reading, input, 0, 0, scratch)
            }).then(|| word.candidate.clone())
        }).collect()
    }
    fn allows(word: &Word, base_policy: Option<&compact::Dictionary>) -> bool {
        !base_policy.is_some_and(|policy| policy.sound_map_disabled(&word.candidate.khmer))
    }
    pub(crate) fn suggest_with_policy(&self, input: &str, completions: bool,
        base_policy: Option<&compact::Dictionary>) -> Vec<RankedSuggestion> {
        self.suggest_with_context_policy(input, completions, base_policy, None)
    }
    pub(crate) fn suggest_with_context_policy(&self, input: &str, completions: bool,
        base_policy: Option<&compact::Dictionary>, context: Option<&crate::context::Scores>) -> Vec<RankedSuggestion> {
        self.suggest_with_context_policy_limit(input, completions, base_policy, context, crate::MAX_COMPLETIONS)
    }
    pub(crate) fn ranking_pool_within_budget(&self, input: &str, budget: usize) -> bool {
        if !input.is_ascii() { return false; }
        let normalized = rules().runtime.consonant_count_tolerance.normalizer().and_then(|n|n.input(input));
        self.matching_ids(input,normalized.as_deref()).len() <= budget
    }
    fn matching_ids(&self, input: &str, normalized: Option<&str>) -> Vec<usize> {
        let mut seen = BTreeSet::new();
        self.prefixes.get(&input[..input.len().min(3)]).into_iter().flatten()
            .chain(normalized.and_then(|text|self.count_prefixes.get(&text[..text.len().min(3)])).into_iter().flatten())
            .copied().filter(|id|seen.insert(*id)).collect()
    }
    pub(crate) fn suggest_with_context_policy_limit(&self, input: &str, completions: bool,
        base_policy: Option<&compact::Dictionary>, context: Option<&crate::context::Scores>, limit: usize) -> Vec<RankedSuggestion> {
        if let Some(prepared) = self.part_policy(base_policy) {
            return prepared.as_ref().unwrap().index.suggest_with_context_policy_limit(input,completions,base_policy,context,limit);
        }
        // An internal apostrophe is a sound chunk for subscript អ. A trailing
        // apostrophe can complete that onset, but leading/repeated apostrophes
        // and other separators cannot synthesize a word.
        if input.is_empty()
            || input.starts_with('\'')
            || input.contains("''")
            || !input.bytes().all(|b| b.is_ascii_lowercase() || b == b'\'')
        {
            return Vec::new();
        }
        let normalizer = rules().runtime.consonant_count_tolerance.normalizer();
        let normalized = normalizer.and_then(|n|n.input(input));
        let mut ids = self.matching_ids(input,normalized.as_deref());
        if completions {
            let mut seen:BTreeSet<_>=ids.iter().copied().collect();
            ids.extend(self.apostrophe_prefixes.get(&input[..input.len().min(3)]).into_iter().flatten()
                .copied().filter(|id|seen.insert(*id)));
        }
        let mut exact = Vec::new();
        let mut prefixes = Vec::new();
        let mut count_completions = Vec::new();
        for id in ids {
            let word = &self.words[id];
            // A metadata-only personal copy cannot revive a base-disabled
            // generated word. Filter before
            // ranking/limits; saved aliases use a separate index and survive.
            if !Self::allows(word, base_policy)
                || self.dictionary.as_ref().is_some_and(|d| d.input_excluded(input, &word.candidate.khmer))
                || base_policy.is_some_and(|d| d.input_excluded(input, &word.candidate.khmer)) {
                continue;
            }
            let mut matched: Option<Match> = None;
            let need_completions = completions && (self.has_rank_priorities || context.is_some()
                || prefixes.len() < limit);
            if let Some(graph)=&word.graph {matched=graph.matching(input,None,false);}
            else { for (reading, &(min_length, max_length)) in word.readings.iter().zip(&word.recovery_lengths) {
                // Once enough ordinary prefixes are kept, only exact matches
                // can affect the result. Prove impossible lengths before any
                // lattice search, while retaining every exact alternative.
                if input.len() > max_length || (!need_completions && input.len() < min_length) { continue; }
                let mut memo = BTreeMap::new();
                if let Some(hit) = match_chunks(reading, 0, input, &mut memo) {
                    if matched.as_ref().is_none_or(|old| hit.better_than(old)) {
                        matched = Some(hit);
                    }
                }
            }}
            let mut count_match = false;
            // A strict component fallback must still yield to an older
            // consonant-count path, retaining that path's preferred spelling.
            if !matched.as_ref().is_some_and(|hit|hit.exact && hit.priority>component_words::FALLBACK_CEILING) {
                if let (Some(normalizer),Some(normalized)) = (normalizer,normalized.as_deref()) {
                    let mut alternative: Option<Match> = None;
                    if let Some(graph)=&word.graph {alternative=graph.matching(normalized,Some(normalizer),false);}
                    else { for reading in &word.readings {
                        if let Some(hit) = normalizer.match_chunks(reading,normalized) {
                            if alternative.as_ref().is_none_or(|old|hit.better_than(old)) { alternative = Some(hit); }
                        }
                    }}
                    // Preserve an existing prefix witness unless the alternate
                    // count completes this word. New prefixes cannot crowd out
                    // original completions before their normal limit is met.
                    if alternative.as_ref().is_some_and(|hit|matched.as_ref().is_none_or(|old|
                        (hit.exact && !old.exact) || (hit.exact==old.exact
                            && hit.priority.saturating_add(consonant_counts::FALLBACK_PRIORITY)>old.priority))) {
                        matched = alternative;
                        count_match = true;
                    }
                }
            }
            let omitted_apostrophe=matched.is_none() && completions;
            if omitted_apostrophe {
                matched=word.apostrophe_graph.as_ref().and_then(|graph|
                    graph.matching_optional_apostrophes(input,None).or_else(||
                        normalizer.and_then(|n|graph.matching_optional_apostrophes(input,Some(n)))));
            }
            let Some(hit) = matched else {
                continue;
            };
            let suggestion = RankedSuggestion {
                suggestion: Suggestion { tap_only: false,
                    preferred_input: (count_match && hit.exact).then(||hit.key.clone()),
                    candidate: SuggestionCandidate::Khmer(word.candidate.clone()),
                    completion: (!hit.exact || omitted_apostrophe).then_some(hit.key),
                    correction: None,
                },
                priority: if omitted_apostrophe {i64::MIN} else {hit.priority.saturating_add(if count_match {consonant_counts::FALLBACK_PRIORITY} else {0})},
            };
            if hit.exact && !omitted_apostrophe {
                exact.push(suggestion);
            } else if count_match && completions && (self.has_rank_priorities || context.is_some()
                || count_completions.len() < limit) {
                count_completions.push(suggestion);
            } else if !count_match && completions && (self.has_rank_priorities || context.is_some()
                || prefixes.len() < limit) {
                prefixes.push(suggestion);
            }
        }
        // Sort every eligible match before applying the completion limit. A
        // preferred witness must not be lost behind twenty lexical-rank hits.
        // Stable ties retain saved rank/ID ordering and existing key order.
        let priorities = rules().runtime.exact_input_priorities.get(input);
        exact.sort_by_key(|hit| (std::cmp::Reverse(hit.priority), priorities.map_or(0, |words| match &hit.suggestion.candidate {
            SuggestionCandidate::Khmer(c) => words.iter().position(|word| word == &c.khmer).unwrap_or(words.len()),
            _ => words.len(),
        })));
        prefixes.extend(count_completions);
        prefixes.sort_by_key(|hit| (std::cmp::Reverse(hit.priority),
            std::cmp::Reverse(crate::context::score(context, crate::suggestion_output(&hit.suggestion)))));
        prefixes.truncate(limit.saturating_sub(exact.len()));
        exact.extend(prefixes);
        exact
    }
    /// Existing paths exclude only choices explicitly marked as additive.
    /// Single/double consonant matches remain existing inputs as well.
    pub(crate) fn existing_exact_words(&self, input: &str,
        base_policy: Option<&compact::Dictionary>) -> BTreeSet<String> {
        if input.is_empty() || !input.is_ascii() { return BTreeSet::new(); }
        if let Some(prepared) = self.part_policy(base_policy) {
            return prepared.as_ref().unwrap().index.existing_exact_words(input, base_policy);
        }
        let normalizer = rules().runtime.consonant_count_tolerance.normalizer();
        let normalized = normalizer.and_then(|normalizer| normalizer.input(input));
        self.matching_ids(input, normalized.as_deref()).into_iter().filter_map(|id| {
            let word = &self.words[id];
            if !Self::allows(word, base_policy) { return None; }
            if let Some(graph)=&word.graph {
                let existing=graph.matching(input,None,true).is_some_and(|hit|hit.exact)
                    || normalizer.zip(normalized.as_deref()).is_some_and(|(n,input)|graph.matching(input,Some(n),true).is_some_and(|hit|hit.exact));
                return existing.then(||word.candidate.khmer.clone());
            }
            let existing = word.readings.iter().any(|reading| {
                let reading: Vec<Vec<Key>> = reading.iter().map(|keys|
                    keys.iter().filter(|key| !key.preserve_existing).cloned().collect()).collect();
                let mut scratch = vec![0; (reading.len() + 1) * (input.len() + 1)];
                exact_recovery(&reading, input, 0, 0, &mut scratch)
                    || normalizer.zip(normalized.as_deref()).is_some_and(|(normalizer, normalized)|
                        normalizer.match_chunks(&reading, normalized).is_some_and(|hit| hit.exact))
            });
            existing.then(|| word.candidate.khmer.clone())
        }).collect()
    }
    /// Context must not promote a fallback over a neutral Sound map/alias
    /// match. Avoid a second lookup altogether for the ordinary neutral map.
    pub(crate) fn exact_priorities(&self, input: &str, include_neutral: bool,
        base_policy: Option<&compact::Dictionary>) -> BTreeMap<String, i64> {
        if !self.has_rank_priorities && !include_neutral && rules().runtime.consonant_count_tolerance.normalizer().is_none() {
            return BTreeMap::new();
        }
        let hits = match base_policy {
            Some(policy) => self.suggest_with_policy(input, false, Some(policy)),
            None => self.suggest(input, false),
        };
        hits.into_iter().filter_map(|hit| {
            match hit.suggestion.candidate {
                SuggestionCandidate::Khmer(candidate) => Some((candidate.khmer, hit.priority)),
                _ => None,
            }
        }).collect()
    }
}
pub(crate) fn exact_word_priority(input: &str, word: &str) -> usize {
    rules().runtime.exact_input_priorities.get(input)
        .map_or(0, |words| words.iter().position(|value| value == word).unwrap_or(words.len()))
}
fn exact_recovery(reading: &[Vec<Key>], input: &str, unit: usize, offset: usize, memo: &mut [u8]) -> bool {
    if unit==0 && offset==0 && reading.iter().flatten().any(|key|key.paths.is_some()) {
        return factored::Graph::new(&[reading.to_vec()]).matching(input,None,false).is_some_and(|hit|hit.exact);
    }
    if unit == reading.len() { return offset == input.len(); }
    let state = unit * (input.len() + 1) + offset;
    if memo[state] != 0 { return memo[state] == 2; }
    let found = reading[unit].iter().any(|key| input[offset..].starts_with(key.text.as_str())
        && exact_recovery(reading, input, unit + 1, offset + key.len(), memo));
    memo[state] = if found { 2 } else { 1 };
    found
}
#[cfg(test)]
fn word_readings(text: &str) -> Option<Vec<Vec<Vec<Key>>>> {
    word_readings_mode(text, false)
}
#[cfg(test)]
fn word_readings_mode(text: &str, for_repetition: bool) -> Option<Vec<Vec<Vec<Key>>>> {
    word_readings_in_context(text,for_repetition,overrides::Context::new(text))
}
fn word_readings_in_context(text: &str, for_repetition: bool, override_context: overrides::Context<'_>) -> Option<Vec<Vec<Vec<Key>>>> {
    if let Some(reading) = override_context.whole(text) { return Some(vec![reading]); }
    let mut readings: Vec<_> = chunks_in_context(text,override_context).into_iter().collect();
    if rules().runtime.checked_terminal_roots.iter().any(|unit| {
        unit.preserve_mapped && unit.words.iter().any(|word| word == text)
    }) {
        if let Some(mapped) = mapped_chunks_in_context(text,true,override_context) {
            if !readings.contains(&mapped) { readings.push(mapped); }
        }
    }
    if rules().runtime.word_readings.iter().any(|unit| {
        unit.preserve_mapped && unit.words.iter().any(|word| word == text)
    }) {
        // Preserve the complete ordinary path as a separate lattice. Bypass
        // only additive word units, retaining replacement units such as អ្នក
        // and ពេជ្រ and every original full-word/boundary selector.
        if let Some(mapped) = mapped_chunks_in_context(text,false,override_context) {
            if !readings.contains(&mapped) { readings.push(mapped); }
        }
    }
    // Keep paired lexical selections separate from every earlier full path.
    // The checked suffix cannot combine with another root or suffix variant.
    readings.extend(checked_component_pair_readings(text, for_repetition,override_context));
    // Add the shared mark path beside every existing lattice. The empty key
    // carries the same preservation policy as other additive vowel choices.
    if rules().runtime.short_inherent_sign.enabled && !override_context.uses_shared_sign() && text.contains('ៈ') {
        if let Some(mut shared) = mapped_chunks_in_context(text, true, override_context.with_shared_sign()) {
            if !readings.contains(&shared) {
                shared.insert(0, vec![Key::additive_path(rules().runtime.short_inherent_sign.additional_path_priority)]);
                readings.push(shared);
            }
        }
    }
    (!readings.is_empty()).then_some(readings)
}
fn checked_component_pair_readings(text: &str, for_repetition: bool, override_context: overrides::Context<'_>) -> Vec<Vec<Vec<Key>>> {
    // A sliced component keeps its outer identity. Being a dictionary word
    // does not make that fragment an original-word-only checked context.
    if override_context.word!=text || override_context.offset!=0
        || override_context.end!=text.chars().count() {return Vec::new();}
    // These paths store explicit lexical choices. A general Read as edit may
    // change the mapped path, but must not rewrite a checked neighbor or unit.
    // Keep stock boundary matching and original offsets/static scoped rules.
    let override_context = override_context.with_stock_pronunciations();
    let mut readings = Vec::new();
    for unit in &rules().runtime.word_readings {
        if !unit.preserve_mapped
            || (unit.checked_suffix_readings.is_empty() && unit.checked_prefix_readings.is_empty()
                && unit.checked_context_readings.is_empty())
            || !unit.words.iter().any(|word| word == text) {
            continue;
        }
        // Each checked neighbor supplies a complete sourced lexical boundary.
        // Work only on the original word, never a cancelled or sliced view.
        let chars: Vec<_> = text.chars().collect();
        if let Some(end) = unit.checked_end(text, &chars, 0) {
            for suffix in &unit.checked_suffix_readings {
                if override_context.with_original(overrides::original_view(text)).crosses(&chars,&[end]) { continue; }
                if suffix.word != text || suffix.keys.is_empty() || end == chars.len()
                    || (for_repetition && suffix.original_word_only)
                    || segment_end(&chars, end, &suffix.khmer) != Some(chars.len()) {
                    continue;
                }
                if let (Some(mut reading), Some(tail)) = (
                    override_context.part(0,end).component(&unit.khmer,&unit.keys),
                    override_context.part(end,chars.len()).component(&suffix.khmer,&suffix.keys),
                ) {
                    reading.extend(tail);
                    readings.push(reading);
                }
            }
        }
        for prefix in &unit.checked_prefix_readings {
            if prefix.word != text || prefix.keys.is_empty()
                || (for_repetition && prefix.original_word_only) {
                continue;
            }
            let Some(start) = segment_end(&chars, 0, &prefix.khmer) else { continue; };
            if override_context.with_original(overrides::original_view(text)).crosses(&chars,&[start]) { continue; }
            if start == 0 || !clear_segment_end(&chars, start)
                || unit.checked_end(text, &chars, start) != Some(chars.len()) {
                continue;
            }
            if let (Some(mut reading), Some(tail)) = (
                override_context.part(0,start).component(&prefix.khmer,&prefix.keys),
                override_context.part(start,chars.len()).component(&unit.khmer,&unit.keys),
            ) {
                reading.extend(tail);
                readings.push(reading);
            }
        }
        // Middle units require both complete checked neighbors. These lattices
        // are original-word-only and never feed another rule or a repeated child.
        if !for_repetition {
            for context in &unit.checked_context_readings {
                if context.word != text || context.prefix_keys.is_empty()
                    || context.suffix_keys.is_empty() { continue; }
                let Some(start) = segment_end(&chars, 0, &context.prefix) else { continue; };
                if start == 0 || !clear_segment_end(&chars, start) { continue; }
                let Some(end) = unit.checked_end(text, &chars, start) else { continue; };
                if override_context.with_original(overrides::original_view(text)).crosses(&chars,&[start,end]) { continue; }
                if end == chars.len()
                    || segment_end(&chars, end, &context.suffix) != Some(chars.len()) { continue; }
                if let (Some(mut reading), Some(middle), Some(tail)) = (
                    override_context.part(0,start).component(&context.prefix,&context.prefix_keys),
                    override_context.part(start,end).component(&unit.khmer,&unit.keys),
                    override_context.part(end,chars.len()).component(&context.suffix,&context.suffix_keys),
                ) {
                    reading.extend(middle);
                    reading.extend(tail);
                    readings.push(reading);
                }
            }
        }
    }
    readings
}
fn collect_prefixes(
    chunks: &[Vec<Key>],
    unit: usize,
    prefix: String,
    out: &mut BTreeSet<String>,
) {
    for length in 1..=prefix.len().min(3) {
        out.insert(prefix[..length].to_owned());
    }
    if prefix.len() >= 3 || unit == chunks.len() {
        return;
    }
    for key in &chunks[unit] {
        collect_prefixes(chunks, unit + 1, format!("{prefix}{}", key.text), out);
    }
}
// Read only original, source-checked closing lexical roots. Parse their checked
// prefix independently; never strip signs generally or apply the root inside
// a different/longer word. The index separately retains mapped paths when asked.
fn checked_terminal_root(text: &str, override_context: overrides::Context<'_>) -> Option<Vec<Vec<Key>>> {
    if !override_context.allows_original_word_rules(text)
        || !override_context.uses_part_boundaries() { return None; }
    let unit = rules().runtime.checked_terminal_roots.iter()
        .find(|unit| unit.words.iter().any(|word| word == text))?;
    if unit.root.is_empty() || unit.keys.is_empty() {
        return None;
    }
    let prefix = text.strip_suffix(&unit.root)?;
    let chars: Vec<_> = text.chars().collect();
    let split = prefix.chars().count();
    if override_context.with_original(overrides::original_view(text)).crosses(&chars,&[split]) { return None; }
    let mut result = if prefix.is_empty() { Vec::new() } else {
        chunks_in_context(prefix,override_context.part(0,split))?
    };
    // A standalone-only source keeps its own pronunciation; it cannot
    // replace a separately scoped terminal rule on the same spelling.
    let keys = if rules().runtime.non_reusable_word_parts.contains(&unit.root) {
        &unit.keys[..]
    } else { override_context.word_part_keys(&unit.root).unwrap_or(&unit.keys) };
    result.extend(override_context.part(split,chars.len()).component(&unit.root,keys)?);
    Some(result)
}

fn chunks(text: &str) -> Option<Vec<Vec<Key>>> {
    chunks_in_context(text, overrides::Context::new(text))
}
fn chunks_in_context(text: &str, override_context: overrides::Context<'_>) -> Option<Vec<Vec<Key>>> {
    if let Some(reading) = override_context.whole(text) { return Some(reading); }
    if let Some(reading) = checked_terminal_root(text, override_context) {
        return Some(reading);
    }
    mapped_chunks_in_context(text, true, override_context)
}

// A written-aa glottal syllable with one shortened final keeps its existing
// A-series a key. Bare vowels and longer/cancelled spellings stay outside scope.
fn explicit_glottal_bantoc_chunks(chars: &[char], original: bool, context: overrides::Context<'_>) -> Option<Vec<Vec<Key>>> {
    let rule = &rules().runtime.explicit_glottal_bantoc;
    if !original || !chars.last().is_some_and(|&ch| rule.mark.starts_with(ch)) {
        return None;
    }
    let vowel = if chars.len() == 6 && chars.get(1) == Some(&'្') && chars.get(2) == Some(&'អ') {
        let first = consonant(chars[0])?;
        if first.onset_keys.is_empty() || chars[0] == 'អ' { return None; }
        3
    } else if chars.len() == 7 && chars.get(2) == Some(&'្') && chars.get(3) == Some(&'អ') {
        let boundary = &rules().runtime.written_nasal_glottal_boundary;
        if !boundary.initials.contains(&chars[0].to_string())
            || !boundary.clusters.iter().any(|cluster| segment_end(chars, 1, cluster) == Some(4)) {
            return None;
        }
        4
    } else { return None; };
    if !rule.vowel.starts_with(chars[vowel]) || coda_keys(chars[vowel + 1]).is_none() {
        return None;
    }
    // Reuse the existing glottal/written-nasal parsing without inventing keys.
    // Removing the final mark strictly shortens this bounded call, so it cannot
    // select itself. The loaded word and its identity retain their original spelling.
    let unmarked: String = chars[..chars.len() - 1].iter().collect();
    mapped_chunks_in_context(&unmarked, true, context.independent(&unmarked))
}

// In this original two-syllable frame, the nasal closes the first pair and
// subscript អ starts a glottal syllable. Keep its apostrophe and later mapped
// vowel/final keys; shortening marks and longer spellings stay outside scope.
fn written_nasal_glottal_chunks(chars: &[char], original: bool, context: overrides::Context<'_>) -> Option<Vec<Vec<Key>>> {
    let rule = &rules().runtime.written_nasal_glottal_boundary;
    if !original || chars.len() < 5 || !rule.initials.contains(&chars[0].to_string())
        || !rule.clusters.iter().any(|cluster| segment_end(chars, 1, cluster) == Some(4)) {
        return None;
    }
    let end = 4 + chars[4..].iter().take_while(|&&ch| is_vowel(ch)).count();
    let vowel: String = chars[4..end].iter().collect();
    if end == 4 || vowel.contains('ំ') || vowel_keys(&vowel, "a").is_none() {
        return None;
    }
    let tail = &chars[end..];
    if vowel.ends_with('ះ') && !tail.is_empty() { return None; }
    match tail {
        [] => {},
        [last] if coda_keys(*last).is_some()
            || rules().runtime.word_final_silent_consonants.contains(&last.to_string()) => {},
        _ => return None,
    }
    let first = consonant(chars[0])?;
    if first.series != "a" || first.onset_keys.is_empty() { return None; }
    let mut reading = vec![first.onset_keys.clone(), rules().runtime.closed_inherent.get("a")?.clone(),
        coda_keys(chars[1])?, rules().runtime.glottal_onset.keys.to_vec()];
    // After its onset, this inspected syllable is exactly a complete written
    // vowel and at most one final. Reuse the simple អ-led vowel/final parser;
    // អ is outside this selector, so this bounded call cannot select itself.
    let second: String = std::iter::once('អ').chain(chars[4..].iter().copied()).collect();
    reading.extend(mapped_chunks_in_context(&second, true, context.independent(&second).with_glottal_carrier())?);
    Some(reading)
}

// Original two-syllable weak-R frames only. The first cluster keeps its
// existing unwritten o; only the listed later plain vowel is governed.
fn weak_r_second_vowel_series(chars: &[char], original: bool) -> Option<&'static str> {
    if !original { return None; }
    for rule in &rules().runtime.weak_r_second_vowel_governance {
        let first = rule.first_clusters.iter().find(|cluster| segment_end(chars, 0, cluster) == Some(3));
        if first.is_none() { continue; }
        let Some(end) = segment_end(chars, 3, &rule.target) else { continue; };
        // Moo/Mao require one listed bare final; Vaa separately admits an
        // open ending or a listed final with bantoc. Default-false flags keep
        // earlier Maa/Nhaa optional-ending behavior unchanged.
        if !rule.allowed_finals.is_empty() || rule.allow_open || rule.allow_bantoc {
            let admitted = match &chars[end..] {
                [] => rule.allow_open,
                [last] => rule.allowed_finals.contains(&last.to_string()),
                [last, '់'] => rule.allow_bantoc && rule.allowed_finals.contains(&last.to_string()),
                _ => false,
            };
            if !admitted { continue; }
        }
        let complete = match &chars[end..] {
            [] => true,
            [last] => rules().runtime.codas.contains_key(&last.to_string())
                || rules().runtime.word_final_silent_consonants.contains(&last.to_string()),
            [last, '់'] => rules().runtime.codas.contains_key(&last.to_string()),
            _ => false,
        };
        if complete { return Some(rule.target_series.as_str()); }
    }
    None
}

#[cfg(test)]
mod weak_r_second_vowel_tests {
    use super::*;

    #[test]
    fn vaa_requires_original_reviewed_clusters_and_complete_open_or_shortened_endings() {
        let proof: serde_json::Value = serde_json::from_str(include_str!(
            "../../test/fixtures/weak-r-vaa.json"
        )).unwrap();
        for word in proof["proofWords"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            let chars: Vec<_> = word.chars().collect();
            assert_eq!(weak_r_second_vowel_series(&chars, true), Some("a"), "{word}");
            assert_eq!(weak_r_second_vowel_series(&chars, false), None, "exposed: {word}");
        }
        for word in proof["guardWords"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            assert_eq!(weak_r_second_vowel_series(&word.chars().collect::<Vec<_>>(), true), None, "outside: {word}");
        }
    }

    #[test]
    fn moo_mao_require_their_reviewed_original_first_groups_and_bare_finals() {
        let proof: serde_json::Value = serde_json::from_str(include_str!(
            "../../test/fixtures/weak-r-moo-mao.json"
        )).unwrap();
        for word in proof["proofWords"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            let chars: Vec<_> = word.chars().collect();
            assert_eq!(weak_r_second_vowel_series(&chars, true), Some("a"), "{word}");
            assert_eq!(weak_r_second_vowel_series(&chars, false), None, "exposed: {word}");
        }
        for word in proof["guardWords"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            assert_eq!(weak_r_second_vowel_series(&word.chars().collect::<Vec<_>>(), true), None, "outside: {word}");
        }
        // A source hold is not a spelling exclusion: this structural match
        // remains reference-only and is not counted among corroborated proof.
        let chars: Vec<_> = "ច្រមូញ".chars().collect();
        assert_eq!(weak_r_second_vowel_series(&chars, true), Some("a"));
    }

    #[test]
    fn only_reviewed_original_two_syllable_frames_govern_the_later_vowel() {
        let proof: serde_json::Value = serde_json::from_str(include_str!(
            "../../test/fixtures/weak-r-second-vowels.json"
        )).unwrap();
        for word in proof["proofWords"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            let chars: Vec<_> = word.chars().collect();
            assert_eq!(weak_r_second_vowel_series(&chars, true), Some("a"), "{word}");
            assert_eq!(weak_r_second_vowel_series(&chars, false), None, "exposed: {word}");
        }
        for word in proof["guardWords"].as_array().unwrap() {
            let word = word.as_str().unwrap();
            assert_eq!(weak_r_second_vowel_series(&word.chars().collect::<Vec<_>>(), true), None, "outside: {word}");
        }
        // Tail guards also apply to an otherwise qualifying original frame.
        for word in ["ក្រមារ", "ក្រមាក", "ក្រមាក់", "ប្រញារ"] {
            assert_eq!(weak_r_second_vowel_series(&word.chars().collect::<Vec<_>>(), true), Some("a"));
        }
        for word in ["ក្រមាវិធី", "ក្រមារ់", "ក្រមាថ៍", "ក្រមាះ", "ក្រមាំ", "អក្រមា", "ក្រមា្ម", "ក្រមា៏"] {
            assert_eq!(weak_r_second_vowel_series(&word.chars().collect::<Vec<_>>(), true), None, "attached: {word}");
        }
    }
}

// The two-dot mark supplies its reviewed short a/closure at a plain original
// A-series word end, including a separately verified silent-coda governance.
// Other written O-series and carrier/cluster/shift cases stay held.
fn terminal_plain_a_sign_keys(
    chars: &[char], start: usize, series: &str, original: bool, silent_coda_governed: bool,
) -> Option<&'static Vec<Key>> {
    let rule = &rules().runtime.terminal_plain_a_sign;
    let base = consonant(*chars.get(start)?)?;
    if !original || start + 2 != chars.len() || series != rule.series
        || (base.series != rule.series && !silent_coda_governed) || rule.excluded_onsets.contains(&base.khmer)
        || start.checked_sub(1).and_then(|i| chars.get(i)) == Some(&'្')
        || chars.get(start + 1).map(|ch| ch.to_string()).as_deref() != Some(rule.mark.as_str())
        || rule.keys.is_empty() {
        return None;
    }
    Some(&rule.keys)
}

// Only the original two plain written O-series syllables qualify. This does
// not infer series across earlier A-series syllables or an unwritten vowel.
fn terminal_written_same_o_sign_chunks(
    chars: &[char], original: bool,
) -> Option<Vec<Vec<Key>>> {
    let rule = &rules().runtime.terminal_written_same_o_sign;
    if !original || chars.len() != 4 || chars[3].to_string() != rule.mark
        || rule.keys.is_empty() {
        return None;
    }
    let first = consonant(chars[0])?;
    let last = consonant(chars[2])?;
    if first.series != rule.series || last.series != rule.series
        || first.onset_keys.is_empty() || last.onset_keys.is_empty() {
        return None;
    }
    let vowel = chars[1].to_string();
    if !rules().dependent_vowels.iter().any(|v| v.khmer == vowel)
        || vowel.contains(['ំ', 'ះ', '់']) {
        return None;
    }
    let mut result = vec![first.onset_keys.clone()];
    result.extend(vowel_keys(&vowel, &rule.series)?);
    result.push(last.onset_keys.clone());
    result.push(rule.keys.to_vec());
    Some(result)
}

fn mapped_chunks(text: &str) -> Option<Vec<Vec<Key>>> {
    mapped_chunks_with_word_readings(text, true)
}
fn initial_carrier_direct_vowel_keys(
    chars: &[char], context: overrides::Context<'_>,
) -> Option<&'static Vec<Key>> {
    let ['អ', next, vowel, ..] = chars else { return None; };
    // Match the original opening. An attached written vowel must immediately
    // follow the next consonant, so a coeng, shifter or standalone mark fails.
    if !context.original || context.offset != 0 || consonant(*next).is_none()
        || !('\u{17b6}'..='\u{17c5}').contains(vowel)
        || !context.word.chars().take(3).eq(chars[..3].iter().copied()) {
        return None;
    }
    let keys = &rules().runtime.initial_carrier_direct_vowel;
    (!keys.is_empty()).then_some(keys)
}
fn initial_carrier_nasal_keys(chars: &[char], original: bool) -> Option<&'static Vec<Key>> {
    // Change only the unwritten first vowel. The nasal, subscript onset and
    // later vowels continue through the ordinary parser, with no word alias
    // inheritance or series-governance change.
    if !original || chars.len() < 5 || chars[0] != 'អ' || chars[2] != '្'
        || matches!(chars[4], '្' | '៉' | '៊') {
        return None;
    }
    let rule = rules().runtime.initial_carrier_nasal.as_ref()?;
    rule.frames.iter().any(|frame| frame.nasal == chars[1].to_string()
        && frame.subscripts.contains(&chars[3].to_string()))
        .then_some(&rule.keys)
}
fn mapped_chunks_with_word_readings(text: &str, include_additive: bool) -> Option<Vec<Vec<Key>>> {
    mapped_chunks_in_context(text, include_additive, overrides::Context::new(text))
}
fn mapped_chunks_in_context(text: &str, include_additive: bool, override_context: overrides::Context<'_>) -> Option<Vec<Vec<Key>>> {
    if let Some(reading) = override_context.whole(text) { return Some(reading); }
    // These terminal signs are silent only in the listed, verified words.
    // Keep the original word/identity; parse its retained letters with the same
    // vowel and final rules. Never strip these signs in arbitrary words or inside
    // compounds (e.g. the medial vowel in ភូមិសាស្ត្រ is pronounced).
    let spelling = rules()
        .runtime
        .silent_word_endings
        .iter()
        .find(|rule| rule.words.iter().any(|word| word == text))
        .and_then(|rule| text.strip_suffix(&rule.ending))
        .unwrap_or(text);
    let mut chars: Vec<_> = spelling.chars().collect();
    if chars.len() > 128 { return None; }
    // Match a reviewed complete original ending before cancellation. Its
    // independently parsed retained form supplies the same local vowel rule.
    let terminal_unit = rules().runtime.terminal_reading_units.iter().find_map(|unit| {
        if spelling != text { return None; }
        let prefix = text.strip_suffix(&unit.khmer)?;
        let in_scope = if override_context.allows_original_word_rules(text) {
            unit.words.iter().any(|word| word == text)
        } else {
            override_context.original
                && override_context.end == override_context.word.chars().count()
                && unit.words.iter().any(|word| word == override_context.word)
                && override_context.word.strip_suffix(&unit.khmer).is_some_and(|outer_prefix| {
                    !outer_prefix.ends_with('្')
                        && outer_prefix.chars().count() == override_context.offset + prefix.chars().count()
                })
        };
        if !in_scope { return None; }
        if prefix.ends_with('្') || unit.retained.chars().count() >= unit.khmer.chars().count() { return None; }
        Some((prefix.chars().count(), unit))
    });
    let cancellation = &rules().runtime.terminal_cancellation;
    if chars.last().is_some_and(|&ch| cancellation.mark.starts_with(ch)) {
        let mut base = chars.len().checked_sub(2)?;
        while is_vowel(chars[base]) {
            base = base.checked_sub(1)?;
        }
        let attached: String = chars[base + 1..chars.len() - 1].iter().collect();
        if base == 0 || chars.get(base - 1) == Some(&'្')
            || consonant(chars[base]).is_none()
            || !cancellation.attached_vowels.contains(&attached)
        {
            return None;
        }
        // Cancel the marked consonant together with its reviewed attached
        // vowel. Complex coeng endings and medial marks remain unsupported.
        chars.truncate(base);
    }
    let original_terminal = spelling == text && chars.len() == text.chars().count();
    let override_original = override_context.with_original(original_terminal);
    let terminal_unit = terminal_unit.filter(|(start, _)|
        !override_original.in_range(&chars,*start,chars.len()));
    let has_override = override_original.in_range(&chars,0,chars.len());
    if chars.len() == 3 && !has_override {
        // This shortcut applies only to consonant-led marked rhymes. An
        // independent vowel must still reach the normal mapped parser.
        if let Some(base) = consonant(chars[0]) {
            let ending: String = chars[1..].iter().collect();
            if let Some(rhyme) = rules().runtime.simple_marked_rhymes.iter()
                .find(|r| r.whole_word != Some(false) && r.series == base.series && r.ending == ending)
            {
                if !base.onset_keys.is_empty() && !rhyme.keys.is_empty() {
                    return Some(vec![base.onset_keys.clone(), rhyme.keys.to_vec()]);
                }
            }
        }
    }
    // These new closing scopes require the actual original word end. A
    // separately cancelled later consonant must not expose a new short stem.
    let independent_y = &rules().runtime.independent_coeng_y;
    if !has_override && original_terminal && chars.len() == 3 && chars[1] == '្'
        && independent_y.bases.contains(&chars[0].to_string())
        && chars[2].to_string() == independent_y.subscript
        && !independent_y.keys.is_empty()
        && rules().independent_vowels.iter()
            .any(|v| v.khmer == chars[0].to_string() && !v.keys.is_empty()) {
        // The reviewed vowel+coeng-y units accept oy and aoy. Standalone vowels and other
        // attached, medial, shifted or cancellation-exposed groups stay mapped
        // separately; this is never a saved word alias.
        return Some(vec![independent_y.keys.to_vec()]);
    }
    if let Some(reading) = terminal_written_same_o_sign_chunks(&chars, original_terminal && !has_override && !override_context.uses_shared_sign()) {
        return Some(reading);
    }
    if let Some(reading) = explicit_glottal_bantoc_chunks(&chars, original_terminal && !has_override, override_context) {
        return Some(reading);
    }
    if let Some(reading) = written_nasal_glottal_chunks(&chars, original_terminal && !has_override, override_context) {
        return Some(reading);
    }
    if let Some(reading) = terminal_nasal_compound_chunks(&chars, original_terminal && !has_override) {
        return Some(reading);
    }
    let mut result = Vec::new();
    let pair_reading = pair_vowel_reading(&chars);
    let nasal_pair = nasal_pair_governance(&chars, original_terminal);
    let governed = (pair_reading.is_some() || nasal_pair).then_some(3)
        .or_else(|| governed_onset(text, &chars))
        .or_else(|| (override_context.uses_shared_sign() && original_terminal)
            .then(||two_dot::governed_onset(text, &chars)).flatten());
    let weak_r_second_series = weak_r_second_vowel_series(&chars, original_terminal);
    // A reviewed weak first syllable, not a rule that splits every repeated
    // consonant. Use the original word guard even after terminal silence.
    let repeated_initial = rules()
        .runtime
        .repeated_initial_presyllable
        .words
        .iter()
        .any(|word| word == text)
        && chars
            .first()
            .is_some_and(|first| chars.get(1) == Some(first));
    let mut i = 0;
    let mut riya_governed = None;
    let mut laa_governed = None;
    let mut silent_coda_governed = None;
    while i < chars.len() {
        // Reusable default-reading overrides precede every built-in word or
        // boundary choice. Saved and personal spellings are merged elsewhere.
        if let Some((unit, end)) = override_original.at(&chars,i) {
            result.extend(override_original.part(i, end).fixed_reading(unit)?);
            i = end;
            continue;
        }
        if let Some((_, unit)) = terminal_unit.filter(|(onset, _)| *onset == i) {
            result.extend(mapped_chunks_in_context(&unit.retained,include_additive,
                override_context.part(i,i + unit.khmer.chars().count()).with_original(false))?);
            break;
        }
        if let Some((unit, end)) = selected_word_reading_with_overrides(text, &chars, i, include_additive, override_original) {
            let alternate = (override_context.word == text)
                .then(|| override_context.word_part_keys(&unit.khmer)).flatten();
            if let Some(keys) = alternate {
                result.push(keys.to_vec());
            } else {
                result.extend(override_context.part(i, end).component(&unit.khmer, &unit.keys)?);
            }
            if original_terminal && end < chars.len()
                && unit.independently_mapped_suffix_words.iter().any(|word| word == text) {
                // Only a reviewed complete lexical suffix in an exact original
                // word may use its independently mapped reading. Other units
                // keep the original full-word context and all its selectors.
                let suffix: String = chars[end..].iter().collect();
                result.extend(chunks_in_context(&suffix,override_context.part(end,chars.len()))?);
                i = chars.len();
            } else {
                i = end;
            }
            continue;
        }
        let onset_start = i;
        let initial = chars[i];
        let mut series;
        if let Some(c) = consonant(initial) {
            let glottal_rule = &rules().runtime.glottal_onset;
            let glottal = initial != 'អ'
                && chars.get(i + 1) == Some(&'្')
                && chars.get(i + 2).is_some_and(|ch| glottal_rule.subscript.starts_with(*ch));
            let special_cluster = rules().runtime.scoped_onset_clusters.iter().find(|rule| {
                rule.words.iter().any(|word| word == text)
                    && chars[i..].iter().copied().take(rule.khmer.chars().count()).eq(rule.khmer.chars())
            });
            series = if glottal {
                glottal_rule.series.as_str()
            } else if let Some(rule) = special_cluster {
                rule.series.as_str()
            } else if let Some((_, target)) = silent_coda_governed.filter(|(onset, _)| *onset == i) {
                target
            } else if governed == Some(i) {
                "a"
            } else if let Some(target_series) = weak_r_second_series.filter(|_| i == 3) {
                target_series
            } else if riya_governed == Some(i) {
                rules().runtime.riya_governance.target_series.as_str()
            } else if laa_governed == Some(i) {
                rules().runtime.open_laa_governance.target_series.as_str()
            } else {
                c.series.as_str()
            };
            let mut clustered = glottal || special_cluster.is_some();
            if let Some(rule) = special_cluster {
                result.push(rule.onset_keys.clone());
                i += rule.khmer.chars().count();
            } else {
                if initial != 'អ' {
                    let mut keys = c.onset_keys.clone();
                    if glottal {
                        if let Some(optional) = glottal_rule.optional_onset_keys.get(&c.khmer) {
                            for key in optional {
                                if !keys.contains(key) { keys.push(key.clone()); }
                            }
                        }
                    }
                    if onset_start == 0 && chars.get(i + 1) == Some(&'្')
                        && chars.get(i + 3) != Some(&'្') {
                        if let Some(rule) = rules().runtime.optional_initial_onsets.iter().find(|rule| {
                            rule.base == c.khmer && chars.get(i + 2).is_some_and(|ch| rule.subscripts.contains(&ch.to_string()))
                        }) {
                            keys.extend(rule.keys.iter().cloned());
                        }
                    }
                    result.push(keys);
                }
                i += 1;
                if glottal {
                    result.push(glottal_rule.keys.to_vec());
                    i += 2;
                }
            }
            // Existing glottal/foreign scopes and simple whole-word behavior
            // keep precedence. The reviewed local extension changes only
            // the written-vowel controller at an independently parsed onset.
            let cluster_series = if !clustered {
                (onset_start == 0).then(|| onset_cluster_series(&chars, original_terminal)).flatten()
                    .or_else(|| local_written_cluster_series(&chars, onset_start, original_terminal))
                    .or_else(|| unwritten_nasal_r_series(&chars, onset_start, original_terminal))
            } else { None };
            if clustered && chars.get(i) == Some(&'្') {
                return None;
            }
            while chars.get(i) == Some(&'្') {
                clustered = true;
                let c = consonant(*chars.get(i + 1)?)?;
                if c.onset_keys.is_empty() {
                    return None;
                }
                series = cluster_series.unwrap_or(c.series.as_str());
                if i == onset_start + 1 && local_vowels::joins_cluster(&chars,onset_start,override_context) {
                    *result.last_mut()? = c.onset_keys.clone();
                } else { result.push(c.onset_keys.clone()); }
                i += 2;
            }
            if matches!(chars.get(i), Some('៉' | '៊')) {
                let mark = chars[i];
                if clustered {
                    return None;
                }
                if mark == '៉' && (matches!(initial, 'ង' | 'ញ' | 'ម' | 'យ' | 'រ' | 'វ')
                    || (initial == 'ល' && override_context.uses_shared_sign()))
                {
                    series = "a";
                } else if mark == '៊' && matches!(initial, 'ស' | 'ហ' | 'ប' | 'អ') {
                    series = "o";
                } else if mark == '៉' && initial == 'ប' {
                    *result.last_mut()? = vec!["p".into()];
                } else {
                    return None;
                }
                i += 1;
            }
            if !clustered && i == onset_start + 1 {
                if let Some(target) = open_laa_target(&chars, onset_start, original_terminal) {
                    laa_governed = Some(target);
                }
                if let Some(target) = riya_target(&chars, onset_start, original_terminal) {
                    riya_governed = Some(target);
                }
                if let Some((rhyme, end)) = syllable_marked_rhyme_at(&chars, onset_start, series, original_terminal) {
                    result.push(rhyme.keys.to_vec());
                    i = end;
                    continue;
                }
            }
            if !glottal && (override_context.uses_shared_sign() || override_context.reads_pronunciation()) {
                if let Some(keys) = two_dot::keys(&chars, onset_start, i, series, original_terminal,
                    override_context.reads_pronunciation()) {
                    result.push(keys);
                    i += 1;
                    continue;
                }
            }
            if !clustered && !glottal && i == onset_start + 1 {
                if let Some(keys) = terminal_plain_a_sign_keys(&chars, onset_start, series, original_terminal,
                    silent_coda_governed.is_some_and(|(onset, _)| onset == onset_start)) {
                    result.push(keys.clone());
                    i = chars.len();
                    continue;
                }
            }
            let start = i;
            while chars.get(i).is_some_and(|&ch| is_vowel(ch)) {
                i += 1;
            }
            let vowel: String = chars[start..i].iter().collect();
            let explicit = !vowel.is_empty();
            let local_vowel = (!explicit && !clustered && !glottal)
                .then(||local_vowels::at(&chars,onset_start,override_context)).flatten();
            let mut mapped_vowels = if explicit {
                if original_terminal && i == chars.len() {
                    terminal_combined_vowel_keys(&vowel, series)
                        .or_else(|| vowel_keys(&vowel, series))?
                } else {
                    vowel_keys(&vowel, series)?
                }
            } else if glottal {
                vec![glottal_rule.inherent_keys.clone()]
            } else if !clustered && original_terminal && start == onset_start + 1
                && selected_word_reading_with_overrides(text, &chars, i, include_additive, override_original)
                    .is_some_and(|(unit, _)| unit.preserve_mapped_prefix_words.iter().any(|word| word == text)) {
                // Keep the existing closed inherent vowel before the checked
                // root, without consuming that root's initial as a coda.
                vec![rules().runtime.closed_inherent.get(series)?.clone()]
            } else if onset_start == 0 && start == 1 {
                vec![initial_carrier_direct_vowel_keys(&chars, override_context)
                    .or_else(|| initial_carrier_nasal_keys(&chars, original_terminal))
                    .unwrap_or(rules().runtime.inherent.get(series)?).clone()]
            } else {
                vec![rules().runtime.inherent.get(series)?.clone()]
            };
            if glottal || (onset_start == 0 && initial == 'អ' && override_context.has_glottal_carrier()) {
                add_glottal_vowel_keys(&vowel, &mut mapped_vowels);
            }
            let mut keys = mapped_vowels.remove(0);
            if repeated_initial && i == 1 && !explicit {
                // Read the first consonant with its inherent vowel, then let
                // the normal parser read the main syllable from the second.
                result.push(keys);
                continue;
            }
            // Apply supported syllable-boundary rules. Otherwise keep reading
            // the mapped onsets instead of failing the whole word.
            let mut coda = None;
            let tail: String = chars[i..].iter().collect();
            let final_cluster = rules()
                .runtime
                .final_clusters
                .iter()
                .find(|cluster| !explicit && !glottal && cluster.series == series && cluster.khmer == tail);
            if let Some(selection) = local_vowel.as_ref().filter(|selection|selection.final_cluster) {
                coda = Some(coda_keys(chars[onset_start + 1])?);
                i = selection.end;
            } else if let Some(cluster) = terminal_short_coda_at(
                &chars, i, initial, &vowel, original_terminal && start == onset_start + 1,
            ) {
                // Three reviewed terminal stems gain their short t reading.
                // Keep the previous full-cluster keys as alternatives; this
                // does not infer a source's formal ASCII pronunciation.
                let mut endings = rules().runtime.codas.get(&cluster.coda)?.clone();
                endings.extend(cluster.retained_keys.iter().cloned());
                coda = Some(endings);
                i = chars.len();
            } else if original_terminal && !glottal && terminal_silent_group_at(&chars, i, start, &vowel) {
                // In the reviewed ក/ច/ហ + ា scope, the entire bare final រ្យ
                // is silent. The parsed onset can include coeng ច; attached
                // vowels/marks, other contexts and earlier gaps do not qualify.
                i = chars.len();
            } else if let Some(cluster) = terminal_silent_coda_at(
                &chars, i, &vowel, &chars[onset_start..start].iter().collect::<String>(), original_terminal,
            ) {
                // Retain the final ម/ន/ណ and silence only its reviewed
                // subscript. Existing vowel/series keys remain unchanged.
                coda = Some(rules().runtime.codas.get(&cluster.coda)?.clone());
                i = chars.len();
            } else if let Some((cluster, end)) = silent_coda_at(&chars, i) {
                // ស្ស/ក្ខ close this syllable: retain the base coda and omit
                // only its matched subscript. A following attached vowel or
                // mark prevents this rule; initial/medial onsets stay intact.
                coda = Some(rules().runtime.codas.get(&cluster.coda)?.clone());
                if !explicit && !glottal {
                    keys = rules().runtime.closed_inherent.get(series)?.clone();
                }
                // Only the original simple initial ទស្ស frame takes the
                // selected short u. Written vowels and exposed endings stay mapped.
                if original_terminal && onset_start == 0 && start == 1 && !explicit && !clustered && !glottal {
                    if let Some(selected) = cluster.initial_inherent_keys.get(&initial.to_string()) {
                        keys = selected.clone();
                    }
                }
                // Resolve the selected written-vowel substitution through the
                // live vowel table; never copy its current Romanized keys.
                if original_terminal && !clustered && !glottal && start == onset_start + 1 {
                    if let Some(substitution) = cluster.vowel_substitutions.iter()
                        .find(|rule| rule.onset == initial.to_string() && rule.from == vowel) {
                        mapped_vowels = vowel_keys(&substitution.to, series)?;
                        keys = mapped_vowels.remove(0);
                    }
                }
                silent_coda_governed = silent_coda_following_series(cluster, &chars, end, original_terminal)
                    .map(|series| (end, series));
                i = end;
            } else if let Some(cluster) = final_cluster {
                // Only the listed word-final cluster and vowel context are
                // supported. Initial coengs and other final clusters stay intact.
                keys = cluster.inherent_keys.clone();
                coda = Some(cluster.coda_keys.clone());
                i = chars.len();
            } else if explicit
                && i + 1 == chars.len()
                && rules().runtime.word_final_silent_consonants.contains(&tail)
            {
                // Consume the written ending, retaining the explicit vowel.
                // No empty chunk is added, and initial/medial រ is unaffected.
                i += 1;
            } else if chars.get(i).is_some_and(|&ch| consonant(ch).is_some())
                && override_original.at(&chars,i).is_none()
                && local_vowels::at(&chars,i,override_context).is_none()
                && selected_word_reading_with_overrides(text, &chars, i, include_additive, override_original).is_none() {
                let paired = if explicit && !glottal && !clustered
                    && written_nh_boundary(&chars, onset_start, i, original_terminal) {
                    true
                } else if !explicit && !glottal && !clustered && initial != 'អ' && i == onset_start + 1 {
                    written_ntr_boundary(&chars, i, original_terminal)
                        || written_nh_r_boundary(&chars, i, original_terminal)
                        || written_ndr_boundary(&chars, i, original_terminal)
                        || written_velar_r_boundary(&chars, i, original_terminal)
                        || unwritten_nasal_r_boundary(&chars, i, original_terminal)
                        || unwritten_nasal_stop_boundary(&chars, i, original_terminal)
                        || unwritten_nasal_aspirated_boundary(&chars, i, original_terminal)
                        || pair_before_subscript(series, &chars, i, nasal_pair).unwrap_or(false)
                } else { false };
                let after = i + 1 + usize::from(chars.get(i + 1) == Some(&'់'));
                let next_marked_onset = (override_context.reads_pronunciation() && override_context.uses_shared_sign() && original_terminal
                    && two_dot::has_onset(&chars, after)) || chars.get(after).and_then(|&ch| consonant(ch))
                    .is_some_and(|base| {
                        let series = if governed == Some(after) { "a" } else { &base.series };
                        syllable_marked_rhyme_at(&chars, after, series, original_terminal).is_some()
                    });
                let final_here = paired || after == chars.len()
                    || local_vowels::at(&chars,after,override_context).is_some()
                    || override_original.at(&chars,after).is_some()
                    || selected_word_reading_with_overrides(text, &chars, after, include_additive, override_original).is_some()
                    || next_marked_onset
                    || shifted_written_onset(&chars, after)
                    || terminal_inherent_boundary(&chars, i, after, &vowel)
                    || (!glottal && !clustered && checked_aam_ng_boundary(
                        text, &chars, onset_start, i, after, &vowel, original_terminal,
                    ))
                    || chars.get(after).is_some_and(|&ch| {
                        consonant(ch).is_some()
                            && chars
                                .get(after + 1)
                                .is_some_and(|&v| is_vowel(v) || v == '្')
                    });
                // A checked component must not consume the prefix's last
                // consonant as the preceding syllable's coda in these exact words.
                // The selected unit still enforces spelling and position guards.
                let preserve_mapped_prefix = original_terminal && after == i + 1
                    && selected_word_reading_with_overrides(text, &chars, after, include_additive, override_original)
                        .is_some_and(|(unit, _)| unit.preserve_mapped_prefix_words.iter().any(|word| word == text));
                if final_here && !preserve_mapped_prefix
                    && !terminal_unit.is_some_and(|(onset, _)| i == onset || after == onset)
                    && coda_keys(chars[i]).is_some() {
                    let mut endings = coda_keys(chars[i])?;
                    if chars[i] == 'ស' && override_original.starts_word_part(i) {
                        // Retain the legacy mapped s path without granting h
                        // to the initial of the next complete lexical part.
                        endings = consonant('ស')?.onset_keys.clone();
                    }
                    if original_terminal && explicit && i + 1 == chars.len() {
                        if let Some(rule) = rules().runtime.reviewed_word_final_codas.iter().find(|rule| {
                            rule.consonant.starts_with(chars[i]) && rule.words.iter().any(|word| word == text)
                        }) {
                            let mapped = endings;
                            endings = rule.keys.to_vec();
                            if rule.retain_mapped_keys {
                                for key in mapped {
                                    if !endings.contains(&key) { endings.push(key); }
                                }
                            }
                        }
                    }
                    coda = Some(endings);
                    if let Some(v) = rules()
                        .runtime
                        .vowels_before_coda
                        .iter()
                        .find(|v| v.vowel == vowel && v.coda == chars[i].to_string())
                    {
                        // A composite vowel may include a nasal key in isolation
                        // but use a different vowel before this reviewed coda.
                        // The coda contributes its sound once, below.
                        keys = if series == "a" {
                            v.a_series_keys.clone()
                        } else {
                            v.o_series_keys.clone()
                        };
                    }
                    if !explicit && !glottal {
                        keys = rules().runtime.closed_inherent.get(series)?.clone();
                        if onset_start == 0 {
                            if let Some(rule) = pair_reading {
                                keys = rule.first_vowel_keys.clone();
                            }
                        }
                    }
                    i += 1;
                    if paired {
                        // C2 closes the first pair. Skip the coeng marker and
                        // let C3 start the next syllable with its own reading.
                        i += 1;
                    }
                    if chars.get(i) == Some(&'់') {
                        if glottal {
                            return None;
                        }
                        if explicit {
                            let rule = &rules().runtime.explicit_bantoc;
                            if vowel != rule.vowel {
                                return None;
                            }
                            keys = if series == "a" {
                                rule.a_series_keys.clone()
                            } else if let Some(final_keys) = rule.o_series_keys_by_final.get(&chars[i - 1].to_string()) {
                                final_keys.clone()
                            } else if rule.velar_finals.contains(&chars[i - 1].to_string()) {
                                rule.o_series_velar_keys.clone()
                            } else {
                                rule.o_series_other_keys.clone()
                            };
                        } else {
                            keys = if let Some(rule) = rules()
                                .runtime
                                .short_inherent_before_coda
                                .iter()
                                .find(|rule| {
                                    i + 1 == chars.len()
                                        && rule.series == series
                                        && rule.coda == chars[i - 1].to_string()
                                }) {
                                rule.keys.to_vec()
                            } else {
                                rules().runtime.short_inherent.get(series)?.clone()
                            };
                        }
                        i += 1;
                    }
                }
            }
            if coda.is_none() {
                if let Some(target_series) = initial_samaa_series(
                    &chars, onset_start, start, i, original_terminal,
                ) {
                    // Only this written vowel changes, after coda decisions.
                    // Subsequent onsets keep their independently mapped series.
                    mapped_vowels = vowel_keys(&vowel, target_series)?;
                    keys = mapped_vowels.remove(0);
                }
            }
            if let Some(selection) = local_vowel {
                keys = selection.keys();
                if selection.joins_consonants() && i + 1 == selection.end { coda = None; }
            }
            // Reuse the parser's closing decision. Open syllables and the
            // earlier silent-final branch have no coda, so retain their keys.
            if coda.as_ref().is_some_and(|endings|endings.iter().any(|key|!key.text.is_empty())) {
                if let Some(additional)=rules().runtime.closed_vowel_alternatives.get(&vowel) {
                    for key in additional {if !keys.contains(key) {keys.push(key.clone());}}
                }
            }
            result.push(keys);
            result.extend(mapped_vowels);
            if let Some(keys) = coda {
                result.push(keys);
            }
        } else {
            let v = rules()
                .independent_vowels
                .iter()
                .find(|v| v.khmer.starts_with(initial))?;
            if v.keys.is_empty() {
                return None;
            }
            result.push(v.keys.to_vec());
            i += 1;
            // One bare final consonant closes this vowel instead of starting
            // an extra inherent-vowel syllable. Attached vowels, marks and
            // subscripts remain with the normal parser; no series is inferred
            // for an independent vowel or for a medial syllable boundary.
            if i + 1 == chars.len() {
                if let Some(keys) = coda_keys(chars[i]) {
                    result.push(keys);
                    i += 1;
                }
            }
        }
    }
    if result.is_empty()
        || result
            .iter()
            .any(|c| c.is_empty() || c.iter().any(|s| s.is_empty()))
    {
        None
    } else {
        if original_terminal {
            add_terminal_lexical_readings(text, &mut result, override_original);
        }
        Some(result)
    }
}
