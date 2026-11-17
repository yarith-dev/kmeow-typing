//! Optional model-agnostic ranking transport. Models cannot establish eligibility.
//! Ordinary suggestions/automatic identities remain independent of display order.
use super::*;

const MAX_RANKING_CANDIDATES: usize = 64;
const MAX_RANKING_SCAN: usize = 4096;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct RankingRequest {
    pub max_candidates: usize,
    /// Transient complete approved English words or core-recognized typed
    /// surfaces. Never Khmer context input or newly admitted output vocabulary.
    pub english_previous_words: Vec<String>,
}
impl Default for RankingRequest {
    fn default() -> Self { Self { max_candidates: MAX_RANKING_CANDIDATES, english_previous_words: Vec::new() } }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RankingCandidate {
    /// Unique only within this response. Never a Studio or persistent identity.
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_index: Option<usize>,
    pub suggestion: Suggestion,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RankingGroup {
    pub id: u32,
    pub reorderable: bool,
    /// Fixed baseline slots: expanded pools cannot displace another group.
    pub display_slots: usize,
    pub candidates: Vec<RankingCandidate>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RankingEnvelope {
    pub format: String,
    pub language: String,
    pub context_words: Vec<String>,
    pub max_candidates: usize,
    pub display_limit: usize,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_automatic_candidate: Option<Suggestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_exact_candidate: Option<Candidate>,
    pub groups: Vec<RankingGroup>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RankingSuggestionResult {
    #[serde(flatten)]
    pub baseline: ContextualSuggestionResult,
    pub ranking: RankingEnvelope,
}

impl RankingEnvelope {
    /// A complete request-local permutation. It cannot cross immutable groups,
    /// add candidates, move frozen alternatives or change automatic identities.
    /// Invalid/unavailable proposals return None for deterministic caller fallback.
    pub fn apply_order(&self, order: &[u32]) -> Option<Vec<Suggestion>> {
        let count: usize = self.groups.iter().map(|group| group.candidates.len()).sum();
        if count == 0 || count > MAX_RANKING_CANDIDATES || order.len() != count { return None; }
        let proposed: BTreeSet<_> = order.iter().copied().collect();
        let actual: BTreeSet<_> = self.groups.iter().flat_map(|group| group.candidates.iter().map(|candidate| candidate.id)).collect();
        if proposed.len() != count || actual.len() != count || proposed != actual { return None; }
        let mut result = Vec::new();
        for group in &self.groups {
            if group.display_slots > group.candidates.len() { return None; }
            let members: BTreeMap<_, _> = group.candidates.iter().map(|candidate| (candidate.id, candidate)).collect();
            let sorted: Vec<_> = order.iter().filter_map(|id| members.get(id).copied()).collect();
            if !group.reorderable && !sorted.iter().map(|candidate| candidate.id).eq(group.candidates.iter().map(|candidate| candidate.id)) { return None; }
            result.extend(sorted.into_iter().take(group.display_slots).map(|candidate| candidate.suggestion.clone()));
        }
        (result.len() <= self.display_limit).then_some(result)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PolicyKey {
    complete: bool,
    priority: i64,
    context: context::Score,
    generated_exact: bool,
    named_exact_priority: usize,
    recovery_band: u8,
}

fn baseline_automatic(result: &ContextualSuggestionResult, features: SuggestionFeatures) -> Option<Suggestion> {
    if result.result.kind == Some(SuggestionKind::NextWord)
        || features.mode == TypingMode::English && result.result.candidates.iter().any(|hit|
            matches!(&hit.candidate, SuggestionCandidate::English(word) if word.manual_only)) { return None; }
    if let Some(exact) = result.exact_candidates.first() {
        return result.result.candidates.iter().find(|hit|
            matches!(&hit.candidate, SuggestionCandidate::Khmer(word) if word.mapping_id == exact.mapping_id && word.khmer == exact.khmer)).filter(|hit| !hit.tap_only).cloned();
    }
    result.result.candidates.iter().find(|hit| hit.correction.is_none()
        && !matches!(&hit.candidate, SuggestionCandidate::English(word) if word.manual_only)).filter(|hit| !hit.tap_only).cloned()
}

impl Engine {
    /// Stable exact text order over currently eligible lexical outputs only.
    /// No aliases, metadata, filesystem, stored history or model dependencies.
    pub fn ranking_vocabulary(&self, language: &str) -> Result<Vec<String>, EngineError> {
        let (index, english) = match language {
            "english" => (&self.dictionary.english, true),
            "khmer" => (&self.dictionary.khmer, false),
            _ => return Err(invalid("Unsupported ranking vocabulary language.")),
        };
        let mut words = BTreeSet::new();
        let mut cursor = index.cursor(0);
        while let Some(row) = cursor.next()? {
            let text = if english { compact::english_text(&row).into_owned() } else { row.key.to_owned() };
            if !(if english { contains_profanity(&text, ProfanityLanguage::English) }
                else { self.dictionary.excluded_khmer.contains(&text) || contains_profanity(&text, ProfanityLanguage::Khmer) }) {
                words.insert(text);
            }
        }
        Ok(words.into_iter().collect())
    }
    pub fn ranking_vocabulary_json(&self, language: &str) -> Result<String, EngineError> {
        serde_json::to_string(&self.ranking_vocabulary(language)?).map_err(|error| invalid(&error.to_string()))
    }
    /// Current static fallback values, read directly from compact typing data.
    /// Values are not changed by a caller's learned display score.
    pub fn ranking_word_prior(&self, language: &str, word: &str) -> Option<(u8, u32)> {
        let (index, key, english) = match language {
            "english" if valid_english(word) && !contains_profanity(word, ProfanityLanguage::English) =>
                (&self.dictionary.english, word.to_ascii_lowercase(), true),
            "khmer" if valid_khmer(word) && !self.dictionary.excluded_khmer.contains(word)
                && !contains_profanity(word, ProfanityLanguage::Khmer) => (&self.dictionary.khmer, word.to_owned(), false),
            _ => return None,
        };
        let mut cursor = index.cursor(index.bound(&key, false));
        while let Some(row) = cursor.next().expect("validated dictionary") {
            if row.key != key { break; }
            let text = if english { compact::english_text(&row) } else { std::borrow::Cow::Borrowed(row.key) };
            if text == word { return Some((self.commonness.level(language, word), row.rank)); }
        }
        None
    }

    fn ranking_context(&self, features: SuggestionFeatures, context: &TypingContext, request: &RankingRequest) -> (Vec<String>, Option<String>) {
        if features.mode == TypingMode::English {
            if request.english_previous_words.len() > 3 { return (Vec::new(), Some("invalid-english-context".into())); }
            let mut result = Vec::new();
            for word in &request.english_previous_words {
                if !valid_english(word) { return (Vec::new(), Some("invalid-english-context".into())); }
                let key = word.to_ascii_lowercase();
                if self.dictionary.excluded_english.contains(&key) {
                    return (Vec::new(), Some("unknown-english-context".into()));
                }
                let index = &self.dictionary.english;
                let mut cursor = index.cursor(index.bound(&key, false));
                let mut choices = Vec::new();
                while let Some(row) = cursor.next().expect("validated dictionary") {
                    if row.key != key { break; }
                    let text = compact::english_text(&row).into_owned();
                    if !contains_profanity(&text, ProfanityLanguage::English) {
                        choices.push((self.commonness.level("english", &text), row.rank, text));
                    }
                }
                choices.sort();
                let text = if let Some((_, _, text)) = choices.into_iter().next() { text }
                    else if self.known_english_surface(word, &key, None) { key }
                    else { return (Vec::new(), Some("unknown-english-context".into())); };
                result.push(text);
            }
            (result, None)
        } else {
            let words: Vec<_> = context.previous_words.iter().rev().take(3).cloned().collect::<Vec<_>>().into_iter().rev().collect();
            if words.iter().any(|word| !self.dictionary.khmer.contains(word) || self.dictionary.excluded_khmer.contains(word)
                || contains_profanity(word, ProfanityLanguage::Khmer)) { (Vec::new(), Some("unknown-khmer-context".into())) }
            else { (words, None) }
        }
    }

    pub fn suggest_with_ranking(&self, input: &str, features: SuggestionFeatures, context: &TypingContext,
        request: &RankingRequest) -> Result<RankingSuggestionResult, EngineError> {
        let baseline = self.suggest_with_context(input, features, context)?;
        self.ranking_from_baseline(baseline, input, features, context, request, false)
    }
    pub fn suggest_with_extension_ranking(&self, extension: &Engine, input: &str, features: SuggestionFeatures,
        context: &TypingContext, request: &RankingRequest) -> Result<RankingSuggestionResult, EngineError> {
        let baseline = self.suggest_with_extension(extension, input, features, context)?;
        self.ranking_from_baseline(baseline, input, features, context, request, true)
    }
    pub fn suggest_with_ranking_json(&self, input: &str, features: SuggestionFeatures, context: &TypingContext,
        request: &RankingRequest) -> Result<String, EngineError> {
        serde_json::to_string(&self.suggest_with_ranking(input, features, context, request)?).map_err(|error| invalid(&error.to_string()))
    }
    pub fn suggest_with_extension_ranking_json(&self, extension: &Engine, input: &str, features: SuggestionFeatures,
        context: &TypingContext, request: &RankingRequest) -> Result<String, EngineError> {
        serde_json::to_string(&self.suggest_with_extension_ranking(extension, input, features, context, request)?).map_err(|error| invalid(&error.to_string()))
    }

    fn ranking_from_baseline(&self, baseline: ContextualSuggestionResult, input: &str, features: SuggestionFeatures,
        context: &TypingContext, request: &RankingRequest, personal: bool) -> Result<RankingSuggestionResult, EngineError> {
        let (context_words, context_fallback) = self.ranking_context(features, context, request);
        let mut envelope = RankingEnvelope {
            format: "kmeow-ranking-envelope-v1".into(), language: if features.mode == TypingMode::English { "english" } else { "khmer" }.into(),
            context_words, max_candidates: request.max_candidates.clamp(MAX_COMPLETIONS, MAX_RANKING_CANDIDATES),
            display_limit: MAX_COMPLETIONS, truncated: false, fallback_reason: context_fallback,
            baseline_automatic_candidate: baseline_automatic(&baseline, features), baseline_exact_candidate: baseline.exact_candidates.first().cloned(), groups: Vec::new(),
        };
        let frozen = personal || baseline.result.kind == Some(SuggestionKind::NextWord)
            || (features.mode == TypingMode::English && (!features.english_completion || self.dictionary.english.has_local_ranks()))
            || (features.mode == TypingMode::Khmer && (!features.khmer_completion || self.dictionary.khmer.has_local_ranks()));
        if baseline.result.candidates.len() > MAX_COMPLETIONS {
            envelope.fallback_reason = Some("large-exact-set".into());
            envelope.truncated = true;
            return Ok(RankingSuggestionResult { baseline, ranking: envelope });
        }
        let normalized = &baseline.result.normalized_input;
        let context_scores = if features.mode == TypingMode::Khmer { self.component_context_scores(&context.previous_words,None) }
            else { Default::default() };
        let scores = (!context_scores.is_empty()).then_some(&context_scores);
        let saved = if features.mode == TypingMode::Khmer && features.romanized {
            self.exact_candidates(normalized, usize::MAX)
        } else { Vec::new() };
        let saved_outputs: BTreeSet<_> = saved.iter().map(|candidate| candidate.khmer.as_str()).collect();
        let saved_priorities = self.saved_match_priorities(normalized);
        let mut saved_prefix_outputs = BTreeSet::new();
        let mut extras = Vec::new();
        let mut sound_priorities = BTreeMap::new();
        let mut policies_proven = true;
        let can_expand = !frozen && !input.is_empty() && !input.chars().last().is_some_and(char::is_whitespace)
            && baseline.result.disabled_feature.is_none() && !contains_profanity(normalized, ProfanityLanguage::Auto);
        if can_expand {
            let english = features.mode == TypingMode::English;
            let direct = normalized.chars().all(|value| ('\u{1780}'..='\u{17ff}').contains(&value));
            let index = if english { &self.dictionary.english } else if direct { &self.dictionary.khmer } else { &self.dictionary.romanized };
            let (start, end) = index.prefix(normalized);
            if end - start <= MAX_RANKING_SCAN {
                extras = if english { self.english_completions_with_limit(normalized, envelope.max_candidates) }
                    else if direct { self.ranked_completions_with_limit(index, normalized, scores, envelope.max_candidates) }
                    else if features.romanized { self.ranked_completions_with_limit(index, normalized, scores, envelope.max_candidates) }
                    else { Vec::new() };
                if !english && !direct {
                    saved_prefix_outputs.extend(extras.iter().map(|hit| suggestion_output(hit).to_owned()));
                }
            } else { envelope.truncated = true; policies_proven = false; envelope.fallback_reason = Some("ranking-scan-budget".into()); }
            if !english && !direct && features.romanized && features.sound_map {
                if let Some(index) = self.sound_index.get() {
                    let within = index.ranking_pool_within_budget(normalized, MAX_RANKING_SCAN);
                    let limit = if within { envelope.max_candidates } else { MAX_COMPLETIONS };
                    if !within { envelope.truncated = true; policies_proven = false; envelope.fallback_reason = Some("ranking-scan-budget".into()); }
                    for hit in index.suggest_with_context_policy_limit(normalized, true, None, scores, limit) {
                        let word = suggestion_output(&hit.suggestion).to_owned();
                        sound_priorities.entry(word).and_modify(|old: &mut i64| *old = (*old).max(hit.priority)).or_insert(hit.priority);
                        if within { extras.push(hit.suggestion); }
                    }
                }
            }
        }
        self.mark_tap_only(&mut extras, None, normalized);
        let has_recoveries = baseline.result.candidates.iter().any(|hit| hit.correction.is_some());
        let policy = |hit: &Suggestion| -> Option<PolicyKey> {
            if frozen || !policies_proven || hit.tap_only || hit.correction.is_some() || baseline.result.kind == Some(SuggestionKind::NextWord) { return None; }
            let word = suggestion_output(hit);
            let english = matches!(&hit.candidate, SuggestionCandidate::English(_));
            if matches!(&hit.candidate, SuggestionCandidate::English(candidate) if candidate.manual_only || candidate.word_alternative || candidate.mapping_id == 0
                || candidate.text.eq_ignore_ascii_case(normalized)) { return None; }
            let complete = hit.completion.is_none() || !english && word == normalized;
            if !english && hit.candidate.mapping_id().is_none() { return None; }
            if complete && !english && !baseline.exact_candidates.iter().any(|candidate| candidate.khmer == word) { return None; }
            let generated_exact = complete && features.romanized && !saved_outputs.contains(word);
            let sound = sound_priorities.get(word).copied().unwrap_or(0);
            let priority = if complete && saved_outputs.contains(word) {
                hit.candidate.mapping_id().and_then(|id| saved_priorities.get(&id).copied())
                    .map(|value| sound_priorities.get(word).map_or(value, |matched| value.max(*matched))).unwrap_or(0)
            } else if hit.completion.is_some() && saved_prefix_outputs.contains(word) { sound.max(0) } else { sound };
            Some(PolicyKey { complete, priority, context: if english { (0, false) } else { context::score(scores, word) },
                generated_exact, named_exact_priority: if generated_exact { sound_map::exact_word_priority(normalized, word) } else { 0 },
                recovery_band: if has_recoveries && self.suggestion_level(hit) > 2 { 2 } else { 0 } })
        };
        let mut keys: Vec<Option<PolicyKey>> = Vec::new();
        let mut next_id = 0u32;
        for (position, suggestion) in baseline.result.candidates.iter().enumerate() {
            let key = policy(suggestion);
            let join = key.is_some() && keys.last() == Some(&key);
            if !join {
                let id = envelope.groups.len() as u32;
                envelope.groups.push(RankingGroup { id, reorderable: key.is_some(), display_slots: 0, candidates: Vec::new() });
                keys.push(key);
            }
            let group = envelope.groups.last_mut().unwrap();
            group.display_slots += 1;
            group.candidates.push(RankingCandidate { id: next_id, baseline_index: Some(position), suggestion: suggestion.clone() });
            next_id += 1;
        }
        let mut outputs: BTreeSet<_> = baseline.result.candidates.iter().map(|hit| suggestion_output(hit).to_owned()).collect();
        for suggestion in extras {
            if outputs.contains(suggestion_output(&suggestion)) { continue; }
            let Some(key) = policy(&suggestion) else { continue; };
            let Some(position) = keys.iter().position(|old| old.as_ref() == Some(&key)) else { continue; };
            if next_id as usize >= envelope.max_candidates { envelope.truncated = true; break; }
            outputs.insert(suggestion_output(&suggestion).to_owned());
            envelope.groups[position].candidates.push(RankingCandidate { id: next_id, baseline_index: None, suggestion });
            next_id += 1;
        }
        Ok(RankingSuggestionResult { baseline, ranking: envelope })
    }
}
