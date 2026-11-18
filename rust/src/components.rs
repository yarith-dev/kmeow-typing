//! Cached, read-only component analysis. Detection never creates a typing path,
//! alias or dictionary record. Complete, unique splits supply cached context
//! continuations; saved structures remain authoritative.
use crate::{compact, invalid, Engine, EngineError};
use serde::{Deserialize,Serialize};
use std::{collections::{BTreeMap,BTreeSet}, sync::{Arc, Mutex}};

const MAX_ALTERNATIVES: usize = 2;
const MAX_QUERY_CACHE: usize = 256;
#[cfg(test)]
thread_local! { static DETECTIONS: std::cell::Cell<usize> = const {std::cell::Cell::new(0)}; }

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WordComponent {
    pub text: String,
    /// Unicode scalar offsets, not UTF-8 bytes or JavaScript UTF-16 offsets.
    pub start: usize,
    pub end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_id: Option<u64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentAnalysis {
    pub word: String,
    pub source: ComponentSource,
    pub status: ComponentStatus,
    pub parts: Vec<WordComponent>,
    pub alternatives: Vec<Vec<WordComponent>>,
    pub more_alternatives: bool,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ComponentSource { Saved, Automatic }
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ComponentStatus { Complete, Partial, Ambiguous, Unknown }

#[derive(Clone, Debug, PartialEq, Eq)]
struct Span { start: usize, end: usize, id: Option<u64> }
#[derive(Clone, Default)]
struct Analysis {
    saved: bool,
    covered: usize,
    components: usize,
    paths: Vec<Vec<Span>>,
    more: bool,
}
impl Analysis {
    fn offer(&mut self, covered: usize, components: usize, path: Vec<Span>, more: bool) {
        // Compare complete analyses, not greedy longest-prefix matches.
        // Unresolved spans do not count as recognized word components.
        if covered > self.covered || (covered == self.covered
            && (self.paths.is_empty() || components < self.components)) {
            self.covered = covered; self.components = components;
            self.paths.clear(); self.more = false;
        }
        if covered != self.covered || components != self.components { return; }
        self.more |= more;
        self.keep(path);
    }
    fn keep(&mut self, path: Vec<Span>) {
        if !self.paths.contains(&path) {
            if self.paths.len() < MAX_ALTERNATIVES { self.paths.push(path); } else { self.more = true; }
        }
    }
    fn render(&self, word: &str) -> ComponentAnalysis {
        let bytes: Vec<_> = word.char_indices().map(|(byte,_)|byte).chain(std::iter::once(word.len())).collect();
        let render = |path: &[Span]|path.iter().map(|span|WordComponent {
            text:word[bytes[span.start]..bytes[span.end]].to_owned(),
            start:span.start,end:span.end,word_id:span.id,
        }).collect::<Vec<_>>();
        let ambiguous = self.paths.len() > 1 || self.more;
        ComponentAnalysis {
            word:word.to_owned(),source:if self.saved {ComponentSource::Saved} else {ComponentSource::Automatic},
            status:if ambiguous {ComponentStatus::Ambiguous} else if self.covered == bytes.len() - 1 {ComponentStatus::Complete}
                else if self.covered > 0 || (self.saved && !self.paths.is_empty()) {ComponentStatus::Partial} else {ComponentStatus::Unknown},
            parts:if ambiguous {Vec::new()} else {self.paths.first().map(|path|render(path)).unwrap_or_default()},
            alternatives:if ambiguous {self.paths.iter().map(|path|render(path)).collect()} else {Vec::new()},
            more_alternatives:self.more,
        }
    }
}
type Lexicon = BTreeMap<String,u64>;
pub(crate) type Definitions = BTreeMap<String,Vec<Vec<String>>>;

fn add_continuation(index: &mut crate::context::Continuations, word: &str, analysis: &Analysis) {
    if analysis.more || analysis.paths.len() != 1 || analysis.covered != word.chars().count() { return; }
    let path = &analysis.paths[0];
    if path.len() < 2 || path.iter().any(|part|part.id.is_none()) { return; }
    let chars: Vec<_> = word.char_indices().map(|(byte,_)|byte).chain(std::iter::once(word.len())).collect();
    let parts: Vec<_> = path.iter().map(|part|word[chars[part.start]..chars[part.end]].to_owned()).collect();
    index.insert(&parts,analysis.saved);
}

fn boundary(chars: &[char], i: usize) -> bool {
    i == 0 || i == chars.len()
        || (matches!(chars[i],'\u{1780}'..='\u{17b3}' | '\u{17e0}'..='\u{17e9}') && chars[i-1] != '្')
}

fn lexicon(dictionary: &compact::Dictionary) -> Lexicon {
    let mut words = Lexicon::new();
    let mut cursor = dictionary.khmer.cursor(0);
    while let Some(row) = cursor.next().expect("validated dictionary") {
        if !row.key.contains(' ') && !dictionary.excluded_khmer.contains(row.key)
            && !crate::contains_profanity(row.key,crate::profanity::ProfanityLanguage::Khmer) {
            words.insert(row.key.to_owned(),row.id);
        }
    }
    words
}
fn definitions(dictionary: &compact::Dictionary) -> Definitions {
    dictionary.context.as_ref().map(|index|index.definitions(&dictionary.pool)).unwrap_or_default()
}

// Reuse the same complete/unique boundary detector for generated readings.
// Saved standalone/partial/ambiguous decisions remain authoritative.
pub(crate) struct ReadingSplits {
    words: Lexicon,
    saved: Definitions,
    cached: std::cell::RefCell<BTreeMap<String,Option<Vec<WordComponent>>>>,
}
impl ReadingSplits {
    pub(crate) fn new(dictionary: &compact::Dictionary, base: Option<&compact::Dictionary>) -> Self {
        let mut words=lexicon(dictionary);
        let mut saved=definitions(dictionary);
        saved.extend(dictionary.component_definitions.as_ref().clone());
        if let Some(base)=base {
            words.extend(lexicon(base));
            saved.extend(definitions(base));
            saved.extend(base.component_definitions.as_ref().clone());
        }
        words.retain(|word,_|!dictionary.sound_map_disabled(word)
            && !dictionary.excluded_khmer.contains(word)
            && !base.is_some_and(|base|base.sound_map_disabled(word)||base.excluded_khmer.contains(word)));
        Self {words,saved,cached:Default::default()}
    }
    pub(crate) fn is_standalone(&self, word: &str) -> bool {
        self.saved.get(word).is_some_and(Vec::is_empty)
    }

    pub(crate) fn parts(&self,word:&str)->Option<Vec<WordComponent>> {
        if let Some(parts)=self.cached.borrow().get(word) {return parts.clone();}
        let result=detect(word,|part|self.words.get(part).copied(),self.saved.get(word)).render(word);
        let parts=(result.status==ComponentStatus::Complete && result.parts.len()>=2
            && result.parts.iter().all(|part|part.word_id.is_some())).then_some(result.parts);
        let mut cached=self.cached.borrow_mut();
        if cached.len()>=MAX_QUERY_CACHE {cached.clear();}
        cached.insert(word.to_owned(),parts.clone());
        parts
    }
}

fn detect(word: &str, lookup: impl Fn(&str)->Option<u64>, saved: Option<&Vec<Vec<String>>>) -> Analysis {
    #[cfg(test)]
    DETECTIONS.with(|count|count.set(count.get() + 1));
    if let Some(saved) = saved {
        let mut result = Analysis {saved:true,..Default::default()};
        // An explicit standalone decision suppresses automatic splitting only.
        if saved.is_empty() {
            result.covered=word.chars().count();result.paths.push(Vec::new());return result;
        }
        for parts in saved {
            let mut start = 0;
            let mut covered = 0;
            let mut path = Vec::new();
            for part in parts {
                let end = start + part.chars().count();
                let id = lookup(part);
                if id.is_some() {covered += end-start;}
                path.push(Span {start,end,id});start = end;
            }
            // Saved alternatives are owner choices, not competing automatic
            // scores. Keep all of them even when availability differs.
            result.covered = if result.paths.is_empty() {covered} else {result.covered.min(covered)};
            result.keep(path);
        }
        return result;
    }
    let chars: Vec<_> = word.chars().collect();
    let bytes: Vec<_> = word.char_indices().map(|(byte,_)|byte).chain(std::iter::once(word.len())).collect();
    // A mark, vowel sign or coeng belongs to its base; a subscript is not a
    // new word onset. Independent vowel letters remain legal onsets.
    let positions: Vec<_> = (0..=chars.len()).filter(|&i|boundary(&chars,i)).collect();
    let mut states = vec![Analysis::default();positions.len()];
    states.last_mut().unwrap().paths.push(Vec::new());
    for from in (0..positions.len()-1).rev() {
        let start = positions[from];
        let mut best = Analysis::default();
        for to in from+1..positions.len() {
            let end = positions[to];
            let id = if start == 0 && end == chars.len() {None} else {lookup(&word[bytes[start]..bytes[end]])};
            // One unknown atom is enough: adjacent unresolved atoms coalesce,
            // so different unknown chunk sizes do not create fake ambiguity.
            if id.is_none() && to != from + 1 { continue; }
            let tail = &states[to];
            for suffix in &tail.paths {
                let mut path = Vec::with_capacity(suffix.len()+1);
                let mut span = Span {start,end,id};
                let merge = id.is_none() && suffix.first().is_some_and(|next|next.id.is_none());
                if merge { span.end = suffix[0].end; }
                path.push(span);
                path.extend_from_slice(&suffix[usize::from(merge)..]);
                best.offer(tail.covered + if id.is_some() {end-start} else {0},
                    tail.components + usize::from(id.is_some()),path,tail.more);
            }
        }
        states[from] = best;
    }
    states.remove(0)
}

struct Extension {
    source: Arc<Vec<u8>>,
    excluded: Arc<BTreeSet<String>>,
    words: Lexicon,
    saved: Definitions,
    definitions: Arc<Definitions>,
    context: Option<crate::context::Continuations>,
    queries: BTreeMap<String,Analysis>,
}
pub(crate) struct Index {
    words: Lexicon,
    excluded: Arc<BTreeSet<String>>,
    saved: Definitions,
    cached: BTreeMap<String,Analysis>,
    context: crate::context::Continuations,
    queries: Mutex<BTreeMap<String,Analysis>>,
    extension: Mutex<Option<Extension>>,
}
impl Index {
    fn new(dictionary: &compact::Dictionary, overrides: &Definitions) -> Self {
        let words = lexicon(dictionary);
        let mut saved = definitions(dictionary);
        saved.extend(overrides.clone());
        let cached: BTreeMap<_,_> = words.keys().map(|word|(word.clone(),detect(word,|part|words.get(part).copied(),saved.get(word))))
            .collect();
        let mut context = crate::context::Continuations::default();
        for (word,analysis) in &cached { add_continuation(&mut context,word,analysis); }
        Self {words,excluded:dictionary.excluded_khmer.clone(),saved,cached,context,queries:Mutex::new(BTreeMap::new()),extension:Mutex::new(None)}
    }
    fn extension(&self, dictionary: &compact::Dictionary, overrides: &Arc<Definitions>) -> std::sync::MutexGuard<'_,Option<Extension>> {
        let mut guard = self.extension.lock().unwrap_or_else(|error|error.into_inner());
        if guard.as_ref().is_none_or(|extra|!Arc::ptr_eq(&extra.source,&dictionary.data)
            || !Arc::ptr_eq(&extra.excluded,&dictionary.excluded_khmer) || !Arc::ptr_eq(&extra.definitions,overrides)) {
            let mut saved = definitions(dictionary);saved.extend(overrides.as_ref().clone());
            *guard = Some(Extension {source:dictionary.data.clone(),excluded:dictionary.excluded_khmer.clone(),
                definitions:overrides.clone(),context:None,words:lexicon(dictionary),saved,queries:BTreeMap::new()});
        }
        guard
    }
    fn detect_with_extension(&self, word: &str, extra: &Extension) -> Analysis {
        detect(word,|part| {
            if self.excluded.contains(part) || extra.excluded.contains(part) {None}
            else {self.words.get(part).or_else(||extra.words.get(part)).copied()}
        },self.saved.get(word).or_else(||extra.saved.get(word)))
    }
    fn scores(&self, previous: &[String], extension: Option<&Engine>, rules: &crate::context::RuleIndex) -> crate::context::Scores {
        if let Some(engine) = extension {
            let mut guard = self.extension(&engine.dictionary,&engine.component_definitions);
            let extra = guard.as_mut().unwrap();
            if extra.context.is_none() {
                let words: BTreeSet<_> = self.words.keys().chain(extra.words.keys()).collect();
                let mut context = crate::context::Continuations::default();
                for word in words {
                    if self.excluded.contains(word) || extra.excluded.contains(word) { continue; }
                    add_continuation(&mut context,word,&self.detect_with_extension(word,extra));
                }
                extra.context = Some(context);
            }
            let mut scores = extra.context.as_ref().unwrap().scores(previous);
            let available = |word: &str|!self.excluded.contains(word) && !extra.excluded.contains(word)
                && (self.words.contains_key(word) || extra.words.contains_key(word));
            rules.extend_scores(previous,&mut scores,|word|self.saved.contains_key(word) || extra.saved.contains_key(word)
                || !available(word) || previous.last().is_some_and(|word|!available(word)));
            scores.retain(|word,_|available(word));
            return scores;
        }
        let mut scores = self.context.scores(previous);
        rules.extend_scores(previous,&mut scores,|word|self.saved.contains_key(word) || !self.words.contains_key(word)
            || previous.last().is_some_and(|word|!self.words.contains_key(word)));
        scores.retain(|word,_|self.words.contains_key(word));
        scores
    }
    fn analyze(&self, word: &str, extension: Option<&Engine>) -> ComponentAnalysis {
        if let Some(engine) = extension {
            let mut guard = self.extension(&engine.dictionary,&engine.component_definitions);
            let extra = guard.as_mut().unwrap();
            if let Some(result) = extra.queries.get(word) {return result.render(word);}
            let result = self.detect_with_extension(word,extra);
            if extra.queries.len() >= MAX_QUERY_CACHE {extra.queries.clear();}
            extra.queries.insert(word.to_owned(),result.clone());
            return result.render(word);
        }
        if let Some(result) = self.cached.get(word) {return result.render(word);}
        let mut queries = self.queries.lock().unwrap_or_else(|error|error.into_inner());
        if let Some(result) = queries.get(word) {return result.render(word);}
        let result = detect(word,|part|self.words.get(part).copied(),self.saved.get(word));
        if queries.len() >= MAX_QUERY_CACHE {queries.clear();}
        queries.insert(word.to_owned(),result.clone());result.render(word)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefinitionFile { format: String, words: Vec<DefinitionWord> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefinitionWord { word: String, components: Vec<Vec<String>> }

impl Engine {
    /// Owner-edited definitions invalidate the runtime analysis/context cache;
    /// the compiled typing/context bytes remain unchanged.
    pub(crate) fn load_word_components(&mut self, provenance: &serde_json::Value) -> Result<(),EngineError> {
        let Some(value)=provenance.get("wordComponents") else {return Ok(());};
        let file: DefinitionFile=serde_json::from_value(value.clone()).map_err(|error|invalid(&error.to_string()))?;
        if file.format != "kmeow-word-components-v1" || file.words.len()>10_000 {
            return Err(invalid("Invalid word component definitions."));
        }
        let words=lexicon(&self.dictionary);
        let mut overrides=Definitions::new();
        for entry in file.words {
            if !words.contains_key(&entry.word) || overrides.contains_key(&entry.word) || entry.components.len()>64 {
                return Err(invalid("Component definitions require one existing Khmer word."));
            }
            let chars: Vec<_>=entry.word.chars().collect();
            let mut seen=BTreeSet::new();
            for parts in &entry.components {
                if !(2..=64).contains(&parts.len()) || parts.concat()!=entry.word || !seen.insert(parts.clone()) {
                    return Err(invalid("Components must join exactly to the word, with 2–64 existing parts per alternative."));
                }
                let mut end=0;
                for part in parts {
                    end+=part.chars().count();
                    if !words.contains_key(part) || !boundary(&chars,end) {
                        return Err(invalid("Choose existing Khmer components without splitting attached marks."));
                    }
                }
            }
            overrides.insert(entry.word,entry.components);
        }
        self.component_definitions=Arc::new(overrides);
        self.dictionary.component_definitions=self.component_definitions.clone();
        self.component_index=Default::default();
        let mut sounds=crate::sound_map::Index::new(&self.dictionary);
        sounds.set_commonness(&self.commonness);
        self.sound_index=std::sync::OnceLock::from(sounds);
        Ok(())
    }

    /// Release the cached personal component view without touching typing data.
    pub fn clear_component_extension_cache(&self) {
        if let Some(index) = self.component_index.get() {
            *index.extension.lock().unwrap_or_else(|error|error.into_inner()) = None;
        }
    }
    /// Analyze partial Khmer word components without changing typing behavior.
    /// Existing dictionary words are analyzed together on first use and cached.
    pub fn analyze_components(&self, word: &str) -> Result<ComponentAnalysis,EngineError> {
        self.analyze_components_using(word,None)
    }
    pub fn analyze_components_with_extension(&self, word: &str, extension: &Engine) -> Result<ComponentAnalysis,EngineError> {
        self.analyze_components_using(word,Some(extension))
    }
    fn analyze_components_using(&self, word: &str, extension: Option<&Engine>) -> Result<ComponentAnalysis,EngineError> {
        if !crate::valid_khmer(word) || word.contains(' ') {return Err(invalid("Component analysis requires one Khmer word of at most 240 characters."));}
        Ok(self.component_index.get_or_init(||Index::new(&self.dictionary,&self.component_definitions)).analyze(word,extension))
    }
    pub(crate) fn component_context_scores(&self, previous: &[String], extension: Option<&Engine>) -> crate::context::Scores {
        if previous.is_empty() { return Default::default(); }
        self.component_index.get_or_init(||Index::new(&self.dictionary,&self.component_definitions))
            .scores(previous,extension,&self.rule_context)
    }
    pub fn analyze_components_json(&self, word: &str) -> Result<String,EngineError> {
        serde_json::to_string(&self.analyze_components(word)?).map_err(|error|invalid(&error.to_string()))
    }
    pub fn analyze_components_with_extension_json(&self, word: &str, extension: &Engine) -> Result<String,EngineError> {
        serde_json::to_string(&self.analyze_components_with_extension(word,extension)?).map_err(|error|invalid(&error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edited_component_choices_rebuild_the_reading_cache() {
        let words=["កា","ខា","កិច្ច","កាកិច្ច","កិច្ចខា","កាកិច្ចខា"];
        let mut core=Engine::from_json(&serde_json::json!({"schemaVersion":2,"mappings":[],
            "words":words.iter().enumerate().map(|(i,word)|
                serde_json::json!({"id":i+1,"khmer":word,"rank":i,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        let has=|core:&Engine|core.suggest_with_context("kakichkha",Default::default(),&Default::default()).unwrap()
            .exact_candidates.iter().any(|c|c.khmer=="កាកិច្ចខា");
        let bytes=core.to_binary();assert!(!has(&core));
        for (parts,expected) in [(serde_json::json!([["កា","កិច្ច","ខា"]]),true),
            (serde_json::json!([]),false)] {
            core.load_word_components(&serde_json::json!({"wordComponents":{"format":"kmeow-word-components-v1",
                "words":[{"word":"កាកិច្ចខា","components":parts}]}})).unwrap();
            assert_eq!(has(&core),expected);assert_eq!(core.to_binary(),bytes);
        }
    }
    #[test]
    fn edited_components_refresh_context_and_validate_before_replacing_choices() {
        let mut engine=Engine::from_json(&serde_json::json!({"schemaVersion":2,
            "words":[{"id":1,"khmer":"កូន","rank":0,"status":"approved"},
                {"id":2,"khmer":"សិស្ស","rank":1,"status":"approved"},
                {"id":3,"khmer":"កូនសិស្ស","rank":2,"status":"approved"}],
            "mappings":[{"id":10,"romanized":"konsers","khmer":"កូនសិស្ស","rank":0,"status":"approved"}],
            "structures":[{"wordId":3,"componentIds":[1,2]}]
        }).to_string()).unwrap();
        let bytes=engine.to_binary();
        let inputs=["kon","konsers","សិស្ស","កូន"];
        let before: Vec<_>=inputs.iter().map(|input|engine.suggest_with_context(input,Default::default(),
            &crate::TypingContext {previous_words:vec!["កូន".into()],..Default::default()}).unwrap()).collect();
        let config=|components:serde_json::Value|serde_json::json!({"wordComponents":{
            "format":"kmeow-word-components-v1","words":[{"word":"កូនសិស្ស","components":components}]}});
        engine.load_word_components(&config(serde_json::json!([]))).unwrap();
        let standalone=engine.analyze_components("កូនសិស្ស").unwrap();
        assert_eq!(standalone.source,ComponentSource::Saved);assert_eq!(standalone.status,ComponentStatus::Complete);
        assert!(standalone.parts.is_empty());
        assert!(engine.component_context_scores(&["កូន".into()],None).is_empty());
        engine.load_word_components(&config(serde_json::json!([["កូន","សិស្ស"]]))).unwrap();
        let saved=engine.analyze_components("កូនសិស្ស").unwrap();
        assert_eq!(engine.component_context_scores(&["កូន".into()],None).get("សិស្ស"),Some(&(1,true)));
        assert_eq!(saved.parts.iter().map(|part|part.text.as_str()).collect::<Vec<_>>(),vec!["កូន","សិស្ស"]);
        for bad in [serde_json::json!([["កូន"]]),serde_json::json!([["កូន","ខា"]]),serde_json::json!([["កូនសិ","ស្ស"]])] {
            assert!(engine.load_word_components(&config(bad)).is_err());
            assert_eq!(engine.analyze_components("កូនសិស្ស").unwrap(),saved);
        }
        assert_eq!(engine.to_binary(),bytes);
        for (input,expected) in inputs.iter().zip(before) {
            assert_eq!(engine.suggest_with_context(input,Default::default(),
                &crate::TypingContext {previous_words:vec!["កូន".into()],..Default::default()}).unwrap(),expected);
        }
    }

    #[test]
    fn context_detection_is_cached_and_standalone_suppresses_reviewed_continuations() {
        let mut engine=Engine::from_json(&serde_json::json!({"schemaVersion":2,"mappings":[],
            "words":[{"id":1,"khmer":"ពាក្យ","rank":0,"status":"approved"},
                {"id":2,"khmer":"សុំ","rank":0,"status":"approved"},
                {"id":3,"khmer":"ពាក្យសុំ","rank":0,"status":"approved"}]
        }).to_string()).unwrap();
        let previous=vec!["ពាក្យ".into()];let bytes=engine.to_binary();
        assert!(engine.component_context_scores(&previous,None).contains_key("សុំ"));
        let count=DETECTIONS.with(|count|count.get());
        for _ in 0..4 {engine.component_context_scores(&previous,None);}
        assert_eq!(DETECTIONS.with(|count|count.get()),count);
        let local=Engine::from_personal_words_json(r#"[{"word":"ខា","spellings":["mine"]}]"#).unwrap();
        engine.component_context_scores(&previous,Some(&local));
        let count=DETECTIONS.with(|count|count.get());
        for _ in 0..4 {engine.component_context_scores(&previous,Some(&local));}
        assert_eq!(DETECTIONS.with(|count|count.get()),count);
        engine.clear_component_extension_cache();
        engine.component_context_scores(&previous,Some(&local));
        assert!(DETECTIONS.with(|count|count.get()) > count);
        engine.load_word_components(&serde_json::json!({"wordComponents":{"format":"kmeow-word-components-v1",
            "words":[{"word":"ពាក្យសុំ","components":[]}]}})).unwrap();
        assert!(engine.component_context_scores(&previous,None).is_empty());
        assert!(engine.component_context_scores(&previous,Some(&local)).is_empty());
        assert_eq!(engine.to_binary(),bytes);
    }

    #[test]
    #[cfg(feature="bundled-dictionaries")]
    fn every_saved_definition_remains_authoritative_after_automatic_detection() {
        let engine = Engine::bundled().unwrap();
        let before = engine.to_binary();
        let saved = definitions(&engine.dictionary);
        assert!(!saved.is_empty());
        for (word,alternatives) in saved {
            let result = engine.analyze_components(&word).unwrap();
            assert_eq!(result.source,ComponentSource::Saved,"{word}");
            let paths = if result.parts.is_empty() {result.alternatives} else {vec![result.parts]};
            for path in paths {
                assert!(alternatives.contains(&path.iter().map(|part|part.text.clone()).collect()),"{word}");
                assert!(path.iter().all(|part|part.word_id.is_some()),"{word}");
            }
        }
        assert_eq!(engine.to_binary(),before);
    }

    #[test]
    fn bounded_ambiguity_matches_an_exhaustive_reference() {
        let atoms = ["កា","ខា","គា","កា"];
        let word = atoms.concat();
        let vocabulary: Vec<_> = (0..atoms.len()).flat_map(|from|(from+1..=atoms.len()).map(move |to|(from,to)))
            .map(|(from,to)|atoms[from..to].concat()).collect::<BTreeSet<_>>().into_iter().collect();
        fn enumerate(at: usize, atoms: &[&str], words: &Lexicon, path: &mut Vec<Span>, paths: &mut Vec<Vec<Span>>) {
            if at == atoms.len() {
                let mut merged: Vec<Span> = Vec::new();
                for span in path.iter() {
                    if span.id.is_none() && merged.last().is_some_and(|previous|previous.id.is_none()) {
                        merged.last_mut().unwrap().end = span.end;
                    } else {merged.push(span.clone());}
                }
                if !paths.contains(&merged) {paths.push(merged);}
                return;
            }
            for end in at+1..=atoms.len() {
                let id = if at == 0 && end == atoms.len() {None} else {words.get(&atoms[at..end].concat()).copied()};
                for choice in [None,id].into_iter().take(1 + usize::from(id.is_some())) {
                    path.push(Span {start:at*2,end:end*2,id:choice});
                    enumerate(end,atoms,words,path,paths);path.pop();
                }
            }
        }
        for mask in 0..1usize << vocabulary.len() {
            let words:Lexicon = vocabulary.iter().enumerate().filter(|(i,_)|mask & (1<<i) != 0)
                .map(|(i,word)|(word.clone(),i as u64+1)).collect();
            let mut paths = Vec::new();enumerate(0,&atoms,&words,&mut Vec::new(),&mut paths);
            let score = |path: &Vec<Span>|(path.iter().filter(|span|span.id.is_some()).map(|span|span.end-span.start).sum::<usize>(),
                std::cmp::Reverse(path.iter().filter(|span|span.id.is_some()).count()));
            let best = paths.iter().map(&score).max().unwrap();
            let expected: Vec<_> = paths.into_iter().filter(|path|score(path)==best).collect();
            let actual = detect(&word,|part|words.get(part).copied(),None);
            assert_eq!((actual.covered,std::cmp::Reverse(actual.components)),best,"mask {mask}");
            assert_eq!(actual.paths.len(),expected.len().min(MAX_ALTERNATIVES),"mask {mask}");
            assert_eq!(actual.more,expected.len()>MAX_ALTERNATIVES,"mask {mask}");
            assert!(actual.paths.iter().all(|path|expected.contains(path)),"mask {mask}");
        }
    }

    #[test]
    fn clearing_personal_analysis_releases_its_source_immediately() {
        let core = Engine::from_personal_words_json(r#"[{"word":"កា"}]"#).unwrap();
        let local = Engine::from_personal_words_json(r#"[{"word":"ខា","spellings":["privatepart"]}]"#).unwrap();
        let weak = Arc::downgrade(&local.dictionary.data);
        let before = core.to_binary();
        core.analyze_components_with_extension("កាខា",&local).unwrap();
        drop(local);assert!(weak.upgrade().is_some());
        core.clear_component_extension_cache();assert!(weak.upgrade().is_none());
        assert_eq!(core.to_binary(),before);
        assert_eq!(core.analyze_components("កាខា").unwrap().status,ComponentStatus::Partial);
    }
}
