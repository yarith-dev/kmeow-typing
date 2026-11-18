//! Explicit local settings over immutable bundled and personal dictionaries.
//! No persistence or platform dependencies; views are prepared before typing.
use crate::{invalid, Engine, EngineError, Candidate, Suggestion, SuggestionCandidate, SuggestionFeatures, TypingMode};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/// App-managed personal vocabulary, independent of Studio approvals and storage.
/// IDs and the internal dataset schema are prepared by the core, not the caller.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalWord {
    pub word: String,
    #[serde(default = "personal_language")]
    pub language: TypingMode,
    #[serde(default)]
    pub spellings: Vec<String>,
    #[serde(default)]
    pub rank: i32,
}
fn personal_language() -> TypingMode { TypingMode::Khmer }

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TypingOverride {
    pub language: String,
    pub word: String,
    pub sound_map_enabled: Option<bool>,
    pub rank: Option<i32>,
    /// Research-only runtime withholding. Never changes the compact source.
    pub excluded: Option<bool>,
}

impl Engine {
    pub fn from_personal_words_json(json: &str) -> Result<Self, EngineError> {
        if json.len() > 4 * 1024 * 1024 { return Err(invalid("Personal words are too large.")); }
        let words: Vec<crate::JsonObject<PersonalWord>> = serde_json::from_str(json).map_err(|e| invalid(&e.to_string()))?;
        Self::from_personal_words(words.into_iter().map(|word| word.0).collect())
    }

    pub fn from_personal_words(words: Vec<PersonalWord>) -> Result<Self, EngineError> {
        Self::from_personal_words_bounded(words, 10_000, 100)
    }
    pub(crate) fn from_personal_words_bounded(words: Vec<PersonalWord>, word_limit: usize, spelling_limit: usize) -> Result<Self, EngineError> {
        use crate::{ApprovedDataset, ApprovedEnglishWord, ApprovedMapping, ApprovedStatus, ApprovedWord};
        if words.len() > word_limit { return Err(invalid(&format!("Too many personal words (maximum {word_limit})."))); }
        let personal_source = std::sync::Arc::new(crate::shared_words::PersonalSource::new(words.clone()));
        let mut dataset = ApprovedDataset { schema_version: 3, mappings: Vec::new(),
            words: Some(Vec::new()), english_words: Some(Vec::new()),
            structures: None, sound_map_disabled_words: None };
        let mut seen = BTreeSet::new();
        for (index, word) in words.into_iter().enumerate() {
            let english = word.language == TypingMode::English;
            if !seen.insert((english, word.word.clone())) || word.spellings.len() > spelling_limit
                || (english && !word.spellings.is_empty()) {
                return Err(invalid("Personal words must be distinct; only Khmer words accept spellings (maximum 100)."));
            }
            let id = index as u64 + 1;
            if english {
                dataset.english_words.as_mut().unwrap().push(ApprovedEnglishWord {
                    id, text: word.word, rank: word.rank, status: ApprovedStatus::Approved,
                });
            } else {
                dataset.words.as_mut().unwrap().push(ApprovedWord {
                    id, khmer: word.word.clone(), rank: word.rank, status: ApprovedStatus::Approved,
                });
                for spelling in word.spellings {
                    dataset.mappings.push(ApprovedMapping { id: dataset.mappings.len() as u64 + 1,
                        romanized: spelling, khmer: word.word.clone(), rank: word.rank, status: ApprovedStatus::Approved });
                }
            }
        }
        let mut engine = Self::new(dataset)?;
        engine.personal_source = Some(personal_source);
        Ok(engine)
    }

    /// Add personal exact readings without changing base identities or ordering.
    /// Complete saved readings precede compositions; base duplicates win.
    pub fn convert_with_extension(&self, extension: &Engine, input: &str) -> Result<crate::ConversionResult, EngineError> {
        self.shared_query(Some(extension), |extension, layer| self.convert_with_extension_base(extension, input, layer)).expect("extension supplied")
    }
    pub(crate) fn convert_with_extension_base(&self, extension: &Engine, input: &str, layer: bool) -> Result<crate::ConversionResult, EngineError> {
        let mut base = if layer { self.convert(input)? } else { self.convert_base(input)? };
        let extra = extension.convert(input)?;
        let mut seen = BTreeSet::new();
        let mut complete = Vec::new();
        let mut composed = Vec::new();
        for candidate in base.candidates.into_iter().chain(extra.candidates) {
            if self.dictionary.excluded_khmer.contains(&candidate.khmer)
                || extension.dictionary.excluded_khmer.contains(&candidate.khmer)
                || self.input_excluded(&base.normalized_input, &candidate.khmer, Some(&extension.dictionary)) { continue; }
            if candidate.mapping_id.is_some() { complete.push(candidate); }
            else { composed.push(candidate); }
        }
        complete.retain(|candidate| seen.insert(candidate.khmer.clone()));
        let limit = crate::MAX_COMPOSED_CANDIDATES.max(complete.len());
        complete.extend(composed.into_iter().filter(|candidate| seen.insert(candidate.khmer.clone())));
        complete.truncate(limit);
        base.candidates = complete;
        Ok(base)
    }

    pub fn convert_with_extension_json(&self, extension: &Engine, input: &str) -> Result<String, EngineError> {
        serde_json::to_string(&self.convert_with_extension(extension, input)?)
            .map_err(|e| invalid(&e.to_string()))
    }

    fn local_rank_active(&self, extension: &Engine, features: SuggestionFeatures) -> bool {
        if features.mode == TypingMode::English {
            self.dictionary.english.has_local_ranks() || extension.dictionary.english.has_local_ranks()
        } else { self.dictionary.khmer.has_local_ranks() || extension.dictionary.khmer.has_local_ranks() }
    }

    pub(crate) fn candidate_has_local_rank(&self, candidate: &SuggestionCandidate) -> bool {
        match candidate {
            SuggestionCandidate::Khmer(word) => self.dictionary.khmer.has_local_rank_for_word(&word.khmer)
                || self.dictionary.romanized.has_local_rank_for_word(&word.khmer),
            SuggestionCandidate::English(word) => self.dictionary.english.has_local_rank_for_word(&word.text.to_ascii_lowercase()),
        }
    }

    fn local_order(&self, extension: &Engine, input: &str, features: SuggestionFeatures,
        scores: &crate::context::Scores) -> (crate::context::Scores, BTreeMap<String, i64>, BTreeSet<String>) {
        let scores = scores.clone();
        let mut priorities = BTreeMap::new();
        let mut saved = BTreeSet::new();
        if features.mode == TypingMode::Khmer {
            for engine in [self, extension] {
                let aliases = engine.saved_match_priorities(input);
                for candidate in engine.exact_candidates(input, usize::MAX) {
                    saved.insert(candidate.khmer.clone());
                    let value = candidate.mapping_id.and_then(|id| aliases.get(&id)).copied().unwrap_or(0);
                    priorities.entry(candidate.khmer).and_modify(|old: &mut i64| *old = (*old).max(value)).or_insert(value);
                }
                if features.sound_map {
                    for hit in engine.sound_index.get().into_iter().flat_map(|index|
                        index.suggest_with_context_policy(input, true, Some(&self.dictionary), Some(&scores))) {
                        let word = crate::suggestion_output(&hit.suggestion).to_owned();
                        priorities.entry(word).and_modify(|old| *old = (*old).max(hit.priority)).or_insert(hit.priority);
                    }
                }
            }
        }
        (scores, priorities, saved)
    }

    /// Only explicitly additive keys yield to existing exact matches in either
    /// dictionary. Older signed priorities retain their existing merge order.
    pub(crate) fn rank_added_exact(&self, candidates: &mut [Candidate], extension: &Engine, input: &str) {
        let mut existing = BTreeSet::new();
        for engine in [self, extension] {
            if let Some(index) = engine.sound_index.get() {
                existing.extend(index.existing_exact_words(input, Some(&self.dictionary)));
            }
            existing.extend(engine.exact_candidates(input, usize::MAX).into_iter().map(|word| word.khmer));
        }
        candidates.sort_by_key(|candidate| !existing.contains(&candidate.khmer));
    }

    pub(crate) fn rank_local_exact(&self, candidates: &mut [Candidate], extension: &Engine,
        input: &str, features: SuggestionFeatures, scores: &crate::context::Scores) {
        let ranks_apply = self.local_rank_active(extension, features) && candidates.iter().any(|word| {
            let candidate = SuggestionCandidate::Khmer(word.clone());
            self.candidate_has_local_rank(&candidate) || extension.candidate_has_local_rank(&candidate)
        });
        // An unrelated rank must not suppress the ordinary context ordering.
        if !ranks_apply {
            if scores.is_empty() { return; }
            let (_,priorities,_) = self.local_order(extension,input,features,scores);
            candidates.sort_by_key(|candidate|(
                std::cmp::Reverse(priorities.get(&candidate.khmer).copied().unwrap_or(0)),
                std::cmp::Reverse(crate::context::score(Some(scores),&candidate.khmer))));
            return;
        }
        let (scores, priorities, saved) = self.local_order(extension, input, features, scores);
        candidates.sort_by_key(|candidate| (
            std::cmp::Reverse(priorities.get(&candidate.khmer).copied().unwrap_or(0)),
            std::cmp::Reverse(crate::context::score(Some(&scores), &candidate.khmer)),
            !saved.contains(&candidate.khmer),
            self.local_level(extension, "khmer", &candidate.khmer),
            if saved.contains(&candidate.khmer) { 0 } else { crate::sound_map::exact_word_priority(input, &candidate.khmer) },
            candidate.rank,
        ));
    }

    fn local_level(&self, extension: &Engine, language: &str, word: &str) -> u8 {
        let bundled = if language == "english" { self.dictionary.english.contains(&word.to_ascii_lowercase()) }
            else { self.dictionary.khmer.contains(word) };
        if bundled { self.commonness.level(language, word) } else { extension.commonness.level(language, word) }
    }

    pub(crate) fn rank_local_merged(&self, candidates: &mut [Suggestion], extension: &Engine,
        input: &str, features: SuggestionFeatures, scores: &crate::context::Scores) {
        let ranks_apply = self.local_rank_active(extension, features) && candidates.iter().any(|hit|
            self.candidate_has_local_rank(&hit.candidate) || extension.candidate_has_local_rank(&hit.candidate));
        if !ranks_apply {
            if scores.is_empty() { return; }
            let (_,priorities,_) = self.local_order(extension,input,features,scores);
            candidates.sort_by_key(|hit| {
                let word = crate::suggestion_output(hit);
                (!hit.completion.is_none(),hit.correction.is_some(),
                    std::cmp::Reverse(priorities.get(word).copied().unwrap_or(0)),
                    std::cmp::Reverse(crate::context::score(Some(scores),word)))
            });
            return;
        }
        let (scores, priorities, saved) = self.local_order(extension, input, features, scores);
        candidates.sort_by_key(|hit| {
            let word = crate::suggestion_output(hit);
            let english = matches!(hit.candidate, SuggestionCandidate::English(_));
            let complete = hit.completion.is_none() || (english && word.eq_ignore_ascii_case(input));
            (
                !complete,
                hit.correction.is_some(),
                std::cmp::Reverse(priorities.get(word).copied().unwrap_or(0)),
                std::cmp::Reverse(crate::context::score(Some(&scores), word)),
                complete && !english && !saved.contains(word),
                self.local_level(extension, if english { "english" } else { "khmer" }, word),
                if complete && !english && !saved.contains(word) { crate::sound_map::exact_word_priority(input, word) } else { 0 },
                hit.candidate.rank(),
            )
        });
    }

    /// Exact matches already have their final context, rank and preservation
    /// order. Keep their display slots in that same order, retaining each
    /// suggestion's flags and leaving other suggestion types in their slots.
    pub(crate) fn align_exact_suggestions(candidates: &mut [Suggestion], exact: &[Candidate]) {
        if exact.len() < 2 { return; }
        let order: BTreeMap<_,_> = exact.iter().enumerate()
            .map(|(position, word)| (word.khmer.as_str(), position)).collect();
        let is_exact = |hit: &Suggestion| hit.completion.is_none() && hit.correction.is_none()
            && order.contains_key(crate::suggestion_output(hit));
        let mut ordered: Vec<_> = candidates.iter().filter(|hit| is_exact(hit)).cloned().collect();
        ordered.sort_by_key(|hit| order[crate::suggestion_output(hit)]);
        for (slot, hit) in candidates.iter_mut().filter(|hit| is_exact(hit)).zip(ordered) {
            *slot = hit;
        }
    }

    /// Returns an independent runtime view sharing the compact bytes. Missing
    /// settings inherit defaults; an explicit enabled setting can reopen a
    /// base-disabled generated word without affecting any saved alias.
    pub fn with_typing_overrides_json(&self, json: &str) -> Result<Self, EngineError> {
        if json.len() > 4 * 1024 * 1024 { return Err(invalid("Local typing settings are too large.")); }
        let settings: Vec<TypingOverride> = serde_json::from_str(json).map_err(|e| invalid(&e.to_string()))?;
        if settings.len() > 10_000 { return Err(invalid("Too many local typing settings.")); }
        let mut seen = BTreeSet::new();
        for setting in &settings {
            let valid_word = match setting.language.as_str() {
                "khmer" => crate::valid_khmer(&setting.word) && !setting.word.chars().any(char::is_whitespace),
                "english" => crate::valid_english(&setting.word) && setting.word == setting.word.to_ascii_lowercase(),
                _ => false,
            };
            if !valid_word || setting.rank.is_some_and(|rank| !crate::valid_rank(rank))
                || (setting.language == "english" && setting.sound_map_enabled.is_some())
                || (setting.rank.is_none() && setting.sound_map_enabled.is_none() && setting.excluded.is_none())
                || !seen.insert((&setting.language, &setting.word)) {
                return Err(invalid("Invalid or duplicate local typing setting."));
            }
        }
        let mut dictionary = self.dictionary.with_local_settings(&settings)?;
        dictionary.set_completion_commonness(&self.commonness);
        let mut sounds = crate::sound_map::Index::new(&dictionary);
        sounds.set_commonness(&self.commonness);
        Ok(Self {
            max_key_bytes: dictionary.max_key_bytes,
            dictionary,
            lexicon: Default::default(),
            commonness: self.commonness.clone(),
            shared_words: self.shared_words.clone(),
            personal_source: None,
            rule_context: self.rule_context.clone(),
            app_data: None,
            sound_index: OnceLock::from(sounds),
            component_index: Default::default(),
            component_definitions: self.component_definitions.clone(),
        })
    }

    /// Read the inherited setting for one spelling without exposing vocabulary.
    pub fn word_typing_settings_json(&self, language: &str, word: &str) -> Result<String, EngineError> {
        let index = match language {
            "khmer" => &self.dictionary.khmer,
            "english" => &self.dictionary.english,
            _ => return Err(invalid("Expected Khmer or English language.")),
        };
        let key = if language == "english" { word.to_ascii_lowercase() } else { word.to_owned() };
        let mut cursor = index.cursor(index.bound(&key, false));
        let row = cursor.next()?.filter(|row| row.key == key);
        Ok(serde_json::json!({
            "rank": row.as_ref().map(|row| row.rank),
            "soundMapEnabled": if language == "khmer" { row.as_ref().map(|row| row.target == 0) } else { None },
        }).to_string())
    }
}
