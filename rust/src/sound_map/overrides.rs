//! Shared default-reading overrides. Saved and personal spellings never pass
//! through this layer; it only chooses generated reading chunks.
use super::*;

/// Original word identity and the character span of a parser fragment. Default
/// phonology may parse fragments independently; override scopes must not.
#[derive(Clone, Copy)]
pub(super) struct Context<'a> {
    pub(super) word: &'a str,
    pub(super) offset: usize,
    pub(super) end: usize,
    pub(super) original: bool,
    word_part_onsets: &'a [usize],
    read_as: &'a [FixedSyllable],
    word_part_pronunciations: &'a [component_words::Reading],
    pronunciation: bool,
    shared_sign: bool,
    glottal_carrier: bool,
    part_boundaries: bool,
    pronunciation_component: bool,
    pub(super) skip_shorthands: bool,
}

impl<'a> Context<'a> {
    pub(super) fn new(word: &'a str) -> Self {
        Self::bare(word).with_read_as(&read_as::defaults().syllables)
    }
    pub(super) fn bare(word:&'a str)->Self {
        Self {word,offset:0,end:word.chars().count(),original:true,word_part_onsets:&[],read_as:&[],word_part_pronunciations:&[],pronunciation:false,shared_sign:false,glottal_carrier:false,part_boundaries:true,pronunciation_component:false,skip_shorthands:false}
    }
    pub(super) fn with_read_as(self, read_as:&'a [FixedSyllable])->Self {Self{read_as,..self}}

    pub(super) fn for_pronunciation(self) -> Self {
        Self { pronunciation: true, ..self }
    }

    pub(super) fn with_shared_sign(self) -> Self { Self { shared_sign: true, ..self } }
    // A nasal-boundary matcher delegates only its glottal vowel and coda to
    // a synthetic អ-led syllable. This identity never follows other fragments.
    pub(super) fn with_glottal_carrier(self) -> Self { Self { glottal_carrier: true, ..self } }
    pub(super) fn has_glottal_carrier(self) -> bool { self.glottal_carrier && self.offset == 0 }
    pub(super) fn uses_shared_sign(self) -> bool {
        // The switch controls inferred dictionary paths. Explicit phonetic
        // spellings must remain readable, including the built-in defaults.
        self.pronunciation || (rules().runtime.short_inherent_sign.enabled && self.shared_sign)
    }

    // The parser sometimes reads a shortened or reconstructed syllable.
    // Keep the current reading mode and prepared rules without re-entering
    // stock rule initialization or attributing it to the original compound.
    pub(super) fn independent<'b>(self, word: &'b str) -> Context<'b> where 'a: 'b {
        Context {
            pronunciation: self.pronunciation,
            shared_sign: self.shared_sign,
            part_boundaries: self.part_boundaries,
            skip_shorthands: self.skip_shorthands,
            ..Context::bare(word).with_read_as(self.read_as)
        }
    }

    pub(super) fn without_shorthands(self) -> Self { Self {skip_shorthands:true,..self} }

    pub(super) fn reads_pronunciation(self) -> bool { self.pronunciation }

    pub(super) fn uses_part_boundaries(self) -> bool { self.part_boundaries }

    pub(super) fn as_pronunciation_component(self) -> Self {
        Self { pronunciation_component: true, ..self }
    }

    pub(super) fn allows_original_word_rules(self, text: &str) -> bool {
        self.word == text || !self.pronunciation_component
    }

    pub(super) fn with_stock_pronunciations(self) -> Self {
        self.with_read_as(read_as::stock_syllables())
            .with_word_part_pronunciations(&[])
    }

    pub(super) fn with_word_part_pronunciations(self, readings: &'a [component_words::Reading]) -> Self {
        Self { word_part_pronunciations: readings, ..self }
    }

    // The parser selects the original rule and span before looking up its
    // respelling. A matching substring alone cannot supply a boundary.
    pub(super) fn word_part_keys(self, spelling: &str) -> Option<&'a [Key]> {
        self.word_part_pronunciations.iter().find(|reading| reading.khmer == spelling)
            .map(|reading| &reading.keys[..])
    }

    pub(super) fn with_word_part_onsets(self, onsets: &'a [usize]) -> Self {
        Self {word_part_onsets:onsets,..self}
    }

    /// This is an onset guard, not a new reading or a compound alias. Keep the
    /// original full-word offsets when a default parser reads a fragment.
    pub(super) fn starts_word_part(self, start: usize) -> bool {
        let start = self.offset + start;
        let chars: Vec<_> = self.word.chars().collect();
        if chars.get(start) != Some(&'ស') || start == 0 { return false; }
        if self.word_part_onsets.contains(&start) { return true; }
        if !self.original { return false; }
        // Additive parts still define a boundary when their extra readings are
        // disabled. Their word/position scopes remain authoritative.
        rules().runtime.word_readings.iter().any(|unit|unit.checked_end(self.word,&chars,start).is_some())
            || rules().runtime.checked_terminal_roots.iter().any(|unit|
                unit.words.iter().any(|word|word == self.word)
                    && segment_end(&chars,start,&unit.root) == Some(chars.len()))
            || rules().runtime.terminal_lexical_units.iter().any(|unit|
                !self.word.contains('៍') && (unit.words.is_empty() || unit.words.iter().any(|word|word == self.word))
                    && segment_end(&chars,start,&unit.khmer) == Some(chars.len()))
    }

    pub(super) fn part(self, start: usize, end: usize) -> Self {
        debug_assert!(start <= end && self.offset + end <= self.end);
        Self {offset:self.offset + start,end:self.offset + end,..self}
    }

    pub(super) fn with_original(self, original: bool) -> Self {
        Self {original:self.original && original,..self}
    }

    pub(super) fn at(self, chars: &[char], start: usize) -> Option<(&'a FixedSyllable, usize)> {
        let scoped = self.part_boundaries.then(|| at_in_context(
            &rules().runtime.fixed_syllables, self, chars, start)).flatten();
        let alternate=at_in_context(self.read_as,self,chars,start);
        match (scoped,alternate) {
            (Some(scoped),Some(alternate))=>Some(if alternate.1>scoped.1 {alternate} else {scoped}),
            (scoped,alternate)=>scoped.or(alternate),
        }
    }

    pub(super) fn whole(self, text: &str) -> Option<Vec<Vec<Key>>> {
        let chars: Vec<_> = text.chars().collect();
        if chars.len() > 128 { return None; }
        let (unit, end) = self.at(&chars,0)?;
        if end != chars.len() { return None; }
        self.fixed_reading(unit)
    }

    pub(super) fn fixed_reading(self, unit: &FixedSyllable) -> Option<Vec<Vec<Key>>> {
        // Stock Khmer respellings were already derived from the sound map.
        if self.pronunciation && !self.read_as.iter().any(|stock| std::ptr::eq(stock, unit)) {
            return self.mapped_part(&unit.khmer);
        }
        Some(vec![self.word_part_keys(&unit.khmer).unwrap_or(&unit.keys).to_vec()])
    }

    fn mapped_part(self, spelling: &str) -> Option<Vec<Vec<Key>>> {
        mapped_chunks_in_context(spelling, false, Self { part_boundaries: false, ..self })
    }

    fn selected_matches(self, chars: &'a [char]) -> impl Iterator<Item = (usize, usize)> + 'a {
        let mut offset = 0;
        std::iter::from_fn(move || {
            while offset < chars.len() {
                let start = offset;
                offset += 1;
                // A subscript inside a written cluster is not a new part onset.
                if start > 0 && chars[start - 1] == '្' { continue; }
                if let Some((_, end)) = self.at(chars,start) {
                    offset = end;
                    return Some((start,end));
                }
            }
            None
        })
    }

    pub(super) fn in_range(self, chars: &[char], start: usize, end: usize) -> bool {
        start < end && self.selected_matches(chars)
            .any(|(matched_start,matched_end)|matched_start < end && matched_end > start)
    }

    pub(super) fn crosses(self, chars: &[char], cuts: &[usize]) -> bool {
        self.selected_matches(chars).any(|(start,end)|cuts.iter().any(|&cut|start < cut && cut < end))
    }

    pub(super) fn crosses_reviewed_part(self, chars: &[char], cuts: &[usize]) -> bool {
        if self.crosses(chars, cuts) {
            return true;
        }
        let original: Vec<_> = self.word.chars().collect();
        let crosses = |start, end| {
            cuts.iter().any(|cut| start < self.offset + cut && self.offset + cut < end)
        };
        let crosses_ending = |ending: &str| {
            self.word.strip_suffix(ending)
                .is_some_and(|prefix| crosses(prefix.chars().count(), original.len()))
        };
        let runtime = &rules().runtime;
        // These reviewed endings are selected before cancellation. A split
        // in the original spelling must respect them even with a final mark.
        if runtime.terminal_reading_units.iter().any(|unit| {
            unit.words.iter().any(|word| word == self.word) && crosses_ending(&unit.khmer)
        }) || runtime.checked_terminal_roots.iter().any(|unit| {
            unit.words.iter().any(|word| word == self.word) && crosses_ending(&unit.root)
        }) {
            return true;
        }
        if !self.original { return false; }
        runtime.word_readings.iter()
            .filter(|unit| unit.words.iter().any(|word| word == self.word))
            .any(|unit| (self.offset..self.end).any(|start| {
                (start == 0 || original[start - 1] != '្')
                    && unit.checked_end(self.word, &original, start)
                        .is_some_and(|end| crosses(start, end))
            }))
            || runtime.terminal_lexical_units.iter().any(|unit| {
                unit.words.iter().any(|word| word == self.word) && crosses_ending(&unit.khmer)
            })
    }

    /// Resolve a stored default without treating its fragment as a standalone
    /// word. Retain equivalent grouping, priorities and key order.
    pub(super) fn component(self, spelling: &str, keys: &[Key]) -> Option<Vec<Vec<Key>>> {
        if self.pronunciation {
            return self.mapped_part(spelling);
        }
        let chars: Vec<_> = spelling.chars().collect();
        let original = original_view(spelling) || self.whole(spelling).is_some();
        if self.with_original(original).in_range(&chars,0,chars.len()) {
            let reading = mapped_chunks_in_context(spelling,true,self)?;
            if same_keys(&reading,keys) { Some(vec![keys.to_vec()]) } else { Some(reading) }
        } else {
            Some(vec![local_vowels::component(spelling,keys,self).unwrap_or_else(||keys.to_vec())])
        }
    }
}

fn at_in_context<'a>(
    units: &'a [FixedSyllable], context: Context<'_>, chars: &[char], start: usize,
) -> Option<(&'a FixedSyllable, usize)> {
    let mut longest = None;
    for unit in units {
        if context.skip_shorthands && has_shorthand_rule(&unit.khmer) {continue;}
        let Some(end) = unit.checked_end_in_context(context,chars,start) else { continue; };
        if longest.as_ref().is_none_or(|(_,previous)|end > *previous) {
            longest = Some((unit,end));
        }
    }
    longest
}

#[cfg(test)]
pub(super) fn whole(text: &str) -> Option<Vec<Vec<Key>>> {
    Context::new(text).whole(text)
}

// Match the parser's original-spelling protection before using an override to
// replace a stored default. An ineligible partial match must be a no-op.
pub(super) fn original_view(text: &str) -> bool {
    !text.chars().last().is_some_and(|ch|rules().runtime.terminal_cancellation.mark.starts_with(ch))
        && !rules().runtime.silent_word_endings.iter()
            .any(|rule|rule.words.iter().any(|word|word == text) && text.ends_with(&rule.ending))
}

#[cfg(test)]
pub(super) fn at<'a>(
    units: &'a [FixedSyllable], word: &str, chars: &[char], start: usize, original: bool,
) -> Option<(&'a FixedSyllable,usize)> {
    at_in_context(units,Context::new(word).with_original(original),chars,start)
}

#[cfg(test)]
fn in_range(word: &str, chars: &[char], start: usize, end: usize, original: bool) -> bool {
    Context::new(word).with_original(original).in_range(chars,start,end)
}

pub(super) fn same_keys(reading: &[Vec<Key>], keys: &[Key]) -> bool {
    if reading.iter().flatten().chain(keys).any(|key|key.paths.is_some()) {
        // A factored choice exceeded the flat budget. Do not mistake its
        // representative text for equivalence with a finite checked neighbor.
        return false;
    }
    let mut prefixes = vec![Key::from(String::new())];
    for group in reading {
        let mut next = Vec::new();
        for prefix in &prefixes {
            for key in group {
                let joined = prefix.joined(key);
                if !keys.iter().any(|expected| expected.text.starts_with(&joined.text)) { return false; }
                if !next.contains(&joined) { next.push(joined); }
                // This is only an equivalence shortcut, never an expansion of
                // a general lattice. Stop when it exceeds the stored key set.
                if next.len() > keys.len() { return false; }
            }
        }
        prefixes = next;
    }
    prefixes.iter().all(|key| keys.contains(key)) && keys.iter().all(|key| prefixes.contains(key))
}

#[cfg(test)]
thread_local! {
    static TEST_RULES: std::cell::Cell<Option<&'static Rules>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) fn configured_for_test() -> Option<&'static Rules> {
    TEST_RULES.with(|rules| rules.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, SuggestionFeatures};

    fn with_overrides(entries: &[(&str, &str)], run: impl FnOnce()) {
        with_configuration(|source| {
            let parts = source["runtime"]["wordParts"].as_array_mut().unwrap();
            let mut order = parts.iter().flat_map(|part| part["cases"].as_array().unwrap())
                .filter(|case| case["kind"] == "syllable")
                .map(|case| case["order"].as_u64().unwrap()).max().unwrap_or(0) + 1;
            for &(word, key) in entries {
                if !parts.iter().any(|part| part["khmer"] == word) {
                    parts.push(serde_json::json!({"khmer":word,"keys":[key],"cases":[]}));
                }
                let part = parts.iter_mut().find(|part| part["khmer"] == word).unwrap();
                let cases = part["cases"].as_array_mut().unwrap();
                if let Some(case) = cases.iter_mut().find(|case| case["kind"] == "syllable") {
                    case["keys"] = serde_json::json!([key]);
                    case.as_object_mut().unwrap().remove("words");
                    case.as_object_mut().unwrap().remove("positions");
                } else {
                    cases.push(serde_json::json!({"kind":"syllable","order":order,"keys":[key],"originalSpellingOnly":true}));
                    order += 1;
                }
            }
        },run);
    }

    fn with_configuration(edit: impl FnOnce(&mut serde_json::Value), run: impl FnOnce()) {
        let mut source: serde_json::Value = serde_json::from_str(include_str!("../../../data/khmer-sound-map.json")).unwrap();
        source["runtime"]["wordParts"] = serde_json::json!([]);
        // Legacy definitions are test-only controls for migration comparisons.
        let migrated = source["runtime"]["pronunciationParts"].as_array().unwrap().clone();
        source["runtime"]["pronunciationParts"] = serde_json::json!([]);
        for entry in &migrated {
            let mut part = entry.as_object().unwrap().clone();
            let word = part.remove("word").unwrap();
            let read_as = part.remove("readAs").unwrap();
            part.insert("khmer".into(), word);
            part.insert("keys".into(), serde_json::json!({"readAs":read_as}));
            source["runtime"]["wordParts"].as_array_mut().unwrap().push(part.into());
        }
        edit(&mut source);
        let parts = source["runtime"]["wordParts"].as_array_mut().unwrap();
        let mut restored = Vec::new();
        parts.retain(|part| {
            let Some(original) = migrated.iter().find(|entry|entry["word"] == part["khmer"]) else { return true; };
            // Tests that explicitly replace Latin keys or selection policy
            // are constructing an ordinary override, not editing Read as.
            if part["keys"].get("readAs").is_none()
                || part["readingType"] == "shorthand"
                || part["cases"].as_array().unwrap().iter().any(|case|case.get("keys").is_some()) {
                return true;
            }
            let mut entry = part.as_object().unwrap().clone();
            let word = entry.remove("khmer").unwrap();
            entry.remove("keys");
            entry.insert("word".into(),word);
            entry.insert("readAs".into(),original["readAs"].clone());
            restored.push(serde_json::Value::Object(entry));
            false
        });
        source["runtime"]["pronunciationParts"] = restored.into();
        // Reindex all tables after test-only insertion/removal of a case.
        for kind in ["syllable","ending","terminalRoot","scoped"] {
            let mut cases: Vec<_> = source["runtime"].as_object_mut().unwrap().iter_mut()
                .filter(|(name,_)|["wordParts","shorthands","pronunciationParts"].contains(&name.as_str()))
                .flat_map(|(_,parts)|parts.as_array_mut().unwrap())
                .flat_map(|part|part["cases"].as_array_mut().unwrap())
                .filter(|case|case["kind"] == kind).collect();
            cases.sort_by_key(|case|case["order"].as_u64().unwrap());
            for (order,case) in cases.into_iter().enumerate() {case["order"] = order.into();}
        }
        // Test-only parsed configurations stay on this test thread. Production
        // still uses the single embedded immutable configuration.
        let configured = Box::leak(Box::new(config::parse_rules(&source.to_string()).unwrap()));
        struct Restore(Option<&'static Rules>);
        impl Drop for Restore {
            fn drop(&mut self) { TEST_RULES.with(|rules| rules.set(self.0)); }
        }
        let _restore = Restore(TEST_RULES.with(|rules| rules.replace(Some(configured))));
        run();
    }

    fn add_case(source: &mut serde_json::Value, spelling: &str, mut case: serde_json::Value) {
        let parts = source["runtime"]["wordParts"].as_array_mut().unwrap();
        let order = parts.iter().flat_map(|part|part["cases"].as_array().unwrap())
            .filter(|other|other["kind"] == case["kind"])
            .map(|other|other["order"].as_u64().unwrap()).max().map_or(0,|last|last + 1);
        case["order"] = order.into();
        if !parts.iter().any(|part|part["khmer"] == spelling) {
            parts.push(serde_json::json!({"khmer":spelling,"keys":case["keys"],"cases":[]}));
        }
        parts.iter_mut().find(|part|part["khmer"] == spelling).unwrap()["cases"]
            .as_array_mut().unwrap().push(case);
    }

    #[test]
    #[cfg(feature = "bundled-dictionaries")]
    fn ar_alternative_preserves_complete_bundled_lattices_and_prefix_membership() {
        // Compare the vowel alternatives with the same standalone exceptions.
        // Compare complete
        // lattices, including priorities, rather than bounded browse previews.
        let engine = Engine::bundled().unwrap();
        let binary = engine.to_binary();
        let controls = super::super::parity::fixture_words();
        let mut previous = None;
        let mut previous_controls = Vec::new();
        with_configuration(|source| {
            source["runtime"]["inherent"]["a"] = serde_json::json!(["or"]);
        }, || {
            previous = Some(Index::new(&engine.dictionary));
            for word in &controls {
                previous_controls.push((word_readings_mode(word,false),word_readings_mode(word,true)));
            }
        });
        let previous = previous.unwrap();
        let current = Index::new(&engine.dictionary);
        assert_eq!(current.words.len(),previous.words.len());
        let mut exception = None;
        for (i,(before,after)) in previous.words.iter().zip(&current.words).enumerate() {
            assert_eq!(after.candidate,before.candidate);
            if before.candidate.khmer == "សកម្ម" {
                exception = Some(i);
                assert_eq!(reading_profile(&after.readings),reading_profile(&[vec![vec![Key::from("sakkam")]]]));
                continue;
            }
            assert_retained_lattices(&before.candidate.khmer,&before.readings,&after.readings);
        }
        assert!(exception.is_some());
        for (prefix,ids) in &previous.prefixes {
            let now = current.prefixes.get(prefix).map(Vec::as_slice).unwrap_or(&[]);
            for &id in ids {
                if Some(id) != exception { assert!(now.contains(&id),"{prefix}: lost {}",previous.words[id].candidate.khmer); }
            }
        }
        for (word,(before,repeated)) in controls.iter().zip(&previous_controls) {
            if word == "សកម្ម" { continue; }
            for (previous,now) in [(before,word_readings_mode(word,false)),
                (repeated,word_readings_mode(word,true))] {
                if let Some(previous) = previous {
                    assert_retained_lattices(word,previous,&now.unwrap_or_default());
                }
            }
        }
        assert_eq!(engine.to_binary(),binary);
    }

    #[derive(Debug,PartialEq,Eq)]
    struct ReadingProfile {
        exact:BTreeMap<String,i64>,
        prefixes:BTreeMap<String,(i64,String)>,
        counts:BTreeMap<String,(i64,String)>,
        count_prefixes:BTreeMap<String,(i64,String)>,
    }
    fn reading_profile(readings:&[Vec<Vec<Key>>])->Option<ReadingProfile> {
        let mut profile=ReadingProfile{exact:BTreeMap::new(),prefixes:BTreeMap::new(),counts:BTreeMap::new(),count_prefixes:BTreeMap::new()};
        let normalizer=rules().runtime.consonant_count_tolerance.normalizer();
        fn offer(map:&mut BTreeMap<String,(i64,String)>,input:&str,priority:i64,key:&str) {
            if map.get(input).is_none_or(|(old,_)|priority>*old) {map.insert(input.to_owned(),(priority,key.to_owned()));}
        }
        for reading in readings {
            let mut inputs=vec![(String::new(),0i64)];
            for choices in reading {
                if inputs.len().saturating_mul(choices.len())>32768 {return None;}
                inputs=inputs.into_iter().flat_map(|(prefix,score)|choices.iter().map(move |key|
                    (format!("{prefix}{}",key.text),score.saturating_add(key.priority)))).collect();
            }
            for (input,priority) in inputs {
                profile.exact.entry(input.clone()).and_modify(|old|*old=(*old).max(priority)).or_insert(priority);
                for size in 0..input.len() {offer(&mut profile.prefixes,&input[..size],priority,&input);}
                if let Some(count)=normalizer.and_then(|n|n.input(&input)) {
                    offer(&mut profile.counts,&count,priority,&input);
                    for size in 0..count.len() {offer(&mut profile.count_prefixes,&count[..size],priority,&input);}
                }
            }
        }
        Some(profile)
    }
    fn restore_redundant_parts(source:&mut serde_json::Value) {
        let fixture:serde_json::Value=serde_json::from_str(include_str!("../../../test/fixtures/redundant-word-parts.json")).unwrap();
        let former=fixture["formerParts"].as_array().unwrap();
        let parts=source["runtime"]["wordParts"].as_array_mut().unwrap();
        for entry in former {parts.push(entry["part"].clone());}
        // Case orders are shared with the separately listed shorthands.
        for kind in ["syllable","ending","terminalRoot","scoped"] {
            let mut remaining=Vec::new();
            for group in ["wordParts","shorthands"] {
                for (p,part) in source["runtime"][group].as_array().unwrap().iter().enumerate() {
                    for (c,case) in part["cases"].as_array().unwrap().iter().enumerate() {
                        if case["kind"]==kind {remaining.push((case["order"].as_u64().unwrap(),group,p,c));}
                    }
                }
            }
            remaining.sort();
            for (order,(_,group,p,c)) in remaining.into_iter().enumerate() {
                source["runtime"][group][p]["cases"][c]["order"]=order.into();
            }
        }
        source["runtime"]["defaultComponentParts"]=serde_json::json!([]);
    }
    #[test]
    #[cfg(feature="bundled-dictionaries")]
    fn redundant_reading_cleanup_preserves_bundled_and_fixture_matching_profiles() {
        let engine=Engine::bundled().unwrap();let current=Index::new(&engine.dictionary);
        let continuations=starting_rule_context(&engine.dictionary);
        let profiles:Vec<_>=current.words.iter().map(|word|reading_profile(&word.readings)).collect();
        let controls=super::super::parity::fixture_words();
        let control_profiles:Vec<_>=controls.iter().map(|word|[false,true].map(|mode|
            reading_profile(&word_readings_mode(word,mode).unwrap_or_default()))).collect();
        with_configuration(restore_redundant_parts,||{
            assert_eq!(starting_rule_context(&engine.dictionary),continuations);
            let former=Index::new(&engine.dictionary);assert_eq!(former.words.len(),current.words.len());
            for ((old,new),profile) in former.words.iter().zip(&current.words).zip(&profiles) {
                assert_eq!(old.candidate,new.candidate);assert!(profile.is_some(),"unbounded profile: {}",old.candidate.khmer);
                assert_eq!(reading_profile(&old.readings).as_ref(),profile.as_ref(),"{}",old.candidate.khmer);
            }
            for (word,profiles) in controls.iter().zip(&control_profiles) {
                for (mode,now) in [false,true].into_iter().zip(profiles) {
                    assert!(now.is_some(),"unbounded profile: {word}");
                    assert_eq!(reading_profile(&word_readings_mode(word,mode).unwrap_or_default()).as_ref(),now.as_ref(),"{word}");
                }
            }
        });
    }
    #[test]
    fn redundant_reading_cleanup_preserves_unlisted_personal_component_frames() {
        for part in ["ក្រដាស","ខុស","ច្បាស់","ម្ចាស់"] {for filler in ["កា","កង","ខគ"] {for at in 0..3 {
            let target=match at {0=>format!("{part}{filler}"),1=>format!("{filler}{part}"),_=>format!("{filler}{part}{filler}")};
            let source=serde_json::json!({"schemaVersion":2,"mappings":[],"words":[
                {"id":1,"khmer":part,"rank":0,"status":"approved"},
                {"id":2,"khmer":filler,"rank":1,"status":"approved"},
                {"id":3,"khmer":target,"rank":2,"status":"approved"}]});
            let engine=Engine::from_json(&source.to_string()).unwrap();let current=Index::new(&engine.dictionary);
            with_configuration(restore_redundant_parts,||{
                let former=Index::new(&engine.dictionary);
                assert_eq!(former.words.len(),current.words.len(),"{target}");
                for (before,after) in former.words.iter().zip(&current.words) {
                    assert_eq!(before.candidate,after.candidate);
                    assert_eq!(reading_profile(&before.readings),reading_profile(&after.readings),"{target}");
                }
            });
        }}}
    }

    fn retains_key(before:&Key, after:&Key)->bool {
        before.priority==after.priority && before.preserve_existing==after.preserve_existing
            && before.prediction_stage==after.prediction_stage && match (&before.paths,&after.paths) {
                (None,None)=>before.text==after.text,
                (Some(before),Some(after))=>before.iter().all(|reading|after.iter().any(|now|retains_lattice(reading,now))),
                _=>false,
            }
    }
    fn retains_lattice(before:&[Vec<Key>], after:&[Vec<Key>])->bool {
        before.len()==after.len() && before.iter().zip(after).all(|(old,new)| {
            let mut next=0;
            old.iter().all(|key| {
                let found=new[next..].iter().position(|candidate|retains_key(key,candidate));
                if let Some(offset)=found {next+=offset+1;true} else {false}
            })
        })
    }
    fn assert_retained_lattices(word: &str, before: &[Vec<Vec<Key>>], after: &[Vec<Vec<Key>>]) {
        for reading in before {
            assert!(after.iter().any(|now|retains_lattice(reading,now)),
                "{word} lost or reordered a complete reading lattice: {reading:?} -> {after:?}");
        }
    }

    #[test]
    #[cfg(feature="bundled-dictionaries")]
    fn component_word_reuse_retains_every_complete_bundled_reading_and_identity() {
        let mut engine=Engine::bundled().unwrap();
        // Test the ordinary additive rule layer independently of owner
        // pronunciation replacements, which intentionally take precedence.
        engine.set_pronunciations(vec![]).unwrap();
        let bytes=engine.to_binary();
        let mut previous=None;
        with_configuration(|source|source["runtime"]["wordRuleReuse"]["enabled"]=false.into(),||{
            previous=Some(Index::new(&engine.dictionary));
        });
        let previous=previous.unwrap();let current=Index::new(&engine.dictionary);
        for before in &previous.words {
            let after=current.words.iter().find(|word|word.candidate.khmer==before.candidate.khmer).unwrap();
            assert_eq!(before.candidate,after.candidate);
            assert_retained_lattices(&before.candidate.khmer,&before.readings,&after.readings);
        }
        assert!(current.words.iter().zip(&previous.words).any(|(after,before)|after.readings.len()>before.readings.len()));
        assert_eq!(engine.to_binary(),bytes);
    }

    #[test]
    fn explicit_read_as_and_personal_words_work_with_general_reuse_off() {
        with_configuration(|source| {
            source["runtime"]["wordRuleReuse"]["enabled"] = false.into();
        }, || {
            let mut engine = Engine::from_json(&serde_json::json!({
                "schemaVersion": 3, "mappings": [], "englishWords": [],
                "words": (["កិច្ច", "កូន", "កិច្ចកូន", "មិត្ត"].into_iter().enumerate()
                    .map(|(i, khmer)| serde_json::json!({
                        "id": i + 1, "khmer": khmer, "rank": i, "status": "approved"
                    })).collect::<Vec<_>>())
            }).to_string()).unwrap();
            let personal = Engine::from_personal_words_json(
                r#"[{"word":"មិត្តកូន","spellings":[]}]"#).unwrap();
            let personal_match = |engine: &Engine| engine.suggest_with_extension(
                &personal, "konkon", Default::default(), &Default::default()).unwrap()
                .exact_candidates.iter().any(|candidate| candidate.khmer == "មិត្តកូន");
            let before = engine.sound_index.get().unwrap().browse_inputs("កិច្ចកូន");
            assert!(!personal_match(&engine));
            engine.set_pronunciations(vec![crate::Pronunciation {
                word: "មិត្ត".into(), read_as: "កូន".into(),
            }]).unwrap();
            assert_eq!(engine.sound_index.get().unwrap().browse_inputs("កិច្ចកូន"), before);
            assert!(personal_match(&engine));
            engine.set_pronunciations(vec![]).unwrap();
            assert!(!personal_match(&engine));
        });
    }

    #[test]
    fn shorthand_type_stays_whole_word_even_for_an_otherwise_reusable_rule() {
        with_configuration(|source|{
            let fixture:serde_json::Value=serde_json::from_str(include_str!("../../../test/fixtures/read-as-boundaries.json")).unwrap();
            let mut part=fixture["movedParts"][0].clone();
            part["readingType"]="shorthand".into();
            let parts=source["runtime"]["wordParts"].as_array_mut().unwrap();
            // This isolated test deliberately makes កម្ម a shorthand instead
            // of a reusable pronunciation. Remove the unrelated សកម្ម
            // respelling which depends on the normal reusable កម្ម default.
            parts.retain(|part|part["khmer"] != "សកម្ម");
            let order=parts.iter().flat_map(|part|part["cases"].as_array().unwrap()).filter(|case|case["kind"]=="syllable").count();
            part["cases"][0]["order"]=order.into();parts.push(part);
        },||{
            let core=Engine::from_json(&serde_json::json!({"schemaVersion":2,"mappings":[],
                "words":[{"id":1,"khmer":"កម្ម","rank":0,"status":"approved"},
                    {"id":2,"khmer":"កា","rank":0,"status":"approved"},
                    {"id":3,"khmer":"កម្មកា","rank":0,"status":"approved"}]
            }).to_string()).unwrap();
            let shorthand=core.suggest("kam").unwrap();
            assert!(shorthand.candidates.iter().any(|s|!s.tap_only
                && matches!(&s.candidate,crate::SuggestionCandidate::Khmer(c) if c.khmer=="កម្ម")));
            assert!(!core.suggest_with_context("kamka",Default::default(),&Default::default()).unwrap()
                .exact_candidates.iter().any(|c|c.khmer=="កម្មកា"));
        });
    }

    #[test]
    fn shorthand_case_overrides_use_the_effective_keys_for_selection() {
        with_configuration(|source|{
            let part=source["runtime"]["shorthands"].as_array_mut().unwrap().iter_mut()
                .find(|part|part["khmer"]=="ខ្ញុំ").unwrap();
            part["cases"][0]["keys"]=serde_json::json!(["tiny"]);
            part["cases"][0]["preserveMapped"]=false.into();
        },||{
            let core=Engine::from_json(&serde_json::json!({"schemaVersion":2,"mappings":[],
                "words":[{"id":1,"khmer":"ខ្ញុំ","rank":0,"status":"approved"},
                    {"id":2,"khmer":"កិច្ច","rank":0,"status":"approved"},
                    {"id":3,"khmer":"ខ្ញុំកិច្ច","rank":0,"status":"approved"}]
            }).to_string()).unwrap();
            for input in ["tiny","tinny"] {
                let result=core.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
                assert!(result.result.candidates.iter().any(|s|!s.tap_only
                    && matches!(&s.candidate,crate::SuggestionCandidate::Khmer(c) if c.khmer=="ខ្ញុំ")),"{input}");
            }
            assert!(!shorthand_match("nh","ខ្ញុំ"));
            assert!(!core.suggest_with_context("tinykich",Default::default(),&Default::default()).unwrap()
                .exact_candidates.iter().any(|c|c.khmer=="ខ្ញុំកិច្ច"));
        });
    }

    #[test]
    #[cfg(feature = "bundled-dictionaries")]
    fn local_vowel_pairs_preserve_every_unrelated_complete_lattice_and_identity() {
        let engine = Engine::bundled().unwrap();
        let binary = engine.to_binary();
        let controls = super::super::parity::fixture_words();
        let related = |word: &str| {
            let chars: Vec<_> = word.chars().collect();
            (0..chars.len()).any(|start|local_vowels::at(&chars,start,Context::new(word)).is_some())
        };
        let unrelated: Vec<_> = controls.iter().filter(|word|!related(word)).cloned().collect();
        let mut before = None;
        let mut before_controls = Vec::new();
        with_configuration(|source|source["runtime"]["localVowelPairs"] = serde_json::json!([]), || {
            before = Some(Index::new(&engine.dictionary));
            for word in &unrelated {
                before_controls.push((word_readings_mode(word,false),word_readings_mode(word,true)));
            }
        });
        let before = before.unwrap();
        let after = Index::new(&engine.dictionary);
        assert_eq!(before.words.len(),after.words.len());
        let mut unrelated_ids = BTreeSet::new();
        for (i,(old,new)) in before.words.iter().zip(&after.words).enumerate() {
            assert_eq!(old.candidate,new.candidate);
            if !related(&old.candidate.khmer) {
                unrelated_ids.insert(i);
                assert_eq!(old.readings,new.readings,"{}",old.candidate.khmer);
                assert_eq!(old.recovery_lengths,new.recovery_lengths,"{}",old.candidate.khmer);
            }
        }
        let prefixes: BTreeSet<_> = before.prefixes.keys().chain(after.prefixes.keys()).collect();
        for prefix in prefixes {
            let projected = |index: &Index|index.prefixes.get(prefix).into_iter().flatten()
                .copied().filter(|id|unrelated_ids.contains(id)).collect::<Vec<_>>();
            assert_eq!(projected(&before),projected(&after),"{prefix}");
        }
        for (word,(plain,repeated)) in unrelated.iter().zip(before_controls) {
            assert_eq!(word_readings_mode(word,false),plain,"{word}");
            assert_eq!(word_readings_mode(word,true),repeated,"repeated {word}");
        }
        assert_eq!(engine.to_binary(),binary);
    }

    #[test]
    #[cfg(feature = "bundled-dictionaries")]
    fn consonant_counts_preserve_all_canonical_lattices_except_selected_joins() {
        let engine = Engine::bundled().unwrap();
        let binary = engine.to_binary();
        let controls = super::super::parity::fixture_words();
        let related = |word: &str| {
            let chars: Vec<_> = word.chars().collect();
            (0..chars.len()).any(|start|local_vowels::at(&chars,start,Context::new(word))
                .is_some_and(|selection|selection.joins_consonants()))
        };
        let unrelated: Vec<_> = controls.iter().filter(|word|!related(word)).cloned().collect();
        let mut before = None;
        let mut before_controls = Vec::new();
        with_configuration(|source| {
            source["runtime"]["consonantCountTolerance"]["enabled"] = false.into();
            for pair in source["runtime"]["localVowelPairs"].as_array_mut().unwrap() {
                pair.as_object_mut().unwrap().remove("joinConsonants");
            }
        }, || {
            before = Some(Index::new(&engine.dictionary));
            for word in &unrelated { before_controls.push((word_readings_mode(word,false),word_readings_mode(word,true))); }
        });
        let before = before.unwrap();
        let after = Index::new(&engine.dictionary);
        assert_eq!(before.words.len(),after.words.len());
        for (old,new) in before.words.iter().zip(&after.words) {
            assert_eq!(old.candidate,new.candidate);
            if !related(&old.candidate.khmer) { assert_eq!(old.readings,new.readings,"{}",old.candidate.khmer); }
        }
        for (word,(plain,repeated)) in unrelated.iter().zip(before_controls) {
            assert_eq!(word_readings_mode(word,false),plain,"{word}");
            assert_eq!(word_readings_mode(word,true),repeated,"repeated {word}");
        }
        assert_eq!(engine.to_binary(),binary);
    }

    #[test]
    fn count_tolerance_can_be_disabled_without_changing_preferred_readings() {
        with_configuration(|source|source["runtime"]["consonantCountTolerance"]["enabled"] = false.into(),|| {
            let engine = engine_for("លទ្ធិ");
            assert!(exact(&engine,"lerthi","លទ្ធិ"));
            assert!(!exact(&engine,"lertthi","លទ្ធិ"));
            let engine = engine_for("វគ្គីករណ៍");
            assert!(exact(&engine,"vaekkikor","វគ្គីករណ៍"));
            assert!(!exact(&engine,"vaekikor","វគ្គីករណ៍"));
        });
    }

    #[test]
    fn scoped_terminal_overrides_replace_defaults_and_preserve_saved_spellings() {
        let outside_readings = word_readings("ឯកសិទ្ធិ");
        with_configuration(|source|add_case(source,"សិទ្ធិ",serde_json::json!({
            "kind":"syllable","keys":["right"],"words":["អភិសិទ្ធិ"],"positions":["end"]
        })), || {
            let engine = Engine::from_json(&serde_json::json!({"schemaVersion":2,
                "words":[{"id":31,"khmer":"អភិសិទ្ធិ","rank":4,"status":"approved"}],
                "mappings":[{"id":51,"romanized":"savedright","khmer":"អភិសិទ្ធិ","rank":3,"status":"approved"}]
            }).to_string()).unwrap();
            let personal = Engine::from_personal_words_json(r#"[
                {"word":"អភិសិទ្ធិ","spellings":["orphiset","myright"]}
            ]"#).unwrap();
            let before = (engine.to_binary(),personal.to_binary());
            for prefix in ["akpe","akphe","akphi","akpi"] {
                assert!(exact(&engine,&format!("{prefix}right"),"អភិសិទ្ធិ"));
                for old in ["set","sit"] {
                    assert!(!exact(&engine,&format!("{prefix}{old}"),"អភិសិទ្ធិ"),"{prefix}{old}");
                }
            }
            // The same stored root in an unselected word keeps its defaults.
            let outside = engine_for("ឯកសិទ្ធិ");
            assert_eq!(word_readings("ឯកសិទ្ធិ"),outside_readings);
            assert!(!exact(&outside,"aekright","ឯកសិទ្ធិ"));
            for sound_map in [true,false] {
                for input in ["savedright","orphiset","myright"] {
                    let result = engine.suggest_with_extension(&personal,input,
                        SuggestionFeatures {sound_map,..Default::default()},&Default::default()).unwrap();
                    assert!(result.exact_candidates.iter().any(|candidate|candidate.khmer == "អភិសិទ្ធិ"),"{input}");
                }
            }
            assert_eq!((engine.to_binary(),personal.to_binary()),before);
        });
    }

    #[test]
    fn standalone_only_override_does_not_leak_into_a_stored_compound_root() {
        with_configuration(|source| {
            // Retain the migrated source's standalone-only reuse policy while
            // adding an independent terminal case for this boundary test.
            source["runtime"]["wordParts"].as_array_mut().unwrap().iter_mut()
                .find(|part|part["khmer"]=="ពេជ្រ").unwrap()["reusableAsComponent"] = false.into();
            add_case(source,"ពេជ្រ",serde_json::json!({
                "kind":"terminalRoot","keys":["diamond"],"words":["កាពេជ្រ"]
            }));
        }, || {
            assert!(exact(&engine_for("ពេជ្រ"),"pech","ពេជ្រ"));
            let engine = engine_for("កាពេជ្រ");
            assert!(exact(&engine,"kadiamond","កាពេជ្រ"));
            assert!(!exact(&engine,"kapech","កាពេជ្រ"));
        });
    }

    #[test]
    fn original_positions_survive_recursive_prefix_and_root_parsing() {
        with_configuration(|source| {
            for (spelling,key,position) in [("កា","first","start"),("ខា","middle","middle"),("គា","last","end")] {
                add_case(source,spelling,serde_json::json!({"kind":"syllable","keys":[key],
                    "words":["កាខាគា"],"positions":[position]}));
            }
            add_case(source,"គា",serde_json::json!({"kind":"terminalRoot","keys":["oldlast"],"words":["កាខាគា"]}));
            add_case(source,"ខា",serde_json::json!({"kind":"terminalRoot","keys":["oldmiddle"],"words":["កាខា"]}));
        }, || {
            let engine = engine_for("កាខាគា");
            assert!(exact(&engine,"firstmiddlelast","កាខាគា"));
            for old in ["firstoldmiddlelast","firstmiddleoldlast","kakheaoldlast"] {
                assert!(!exact(&engine,old,"កាខាគា"),"{old}");
            }
        });
    }

    #[test]
    fn scoped_overrides_replace_checked_prefix_middle_and_suffix_components() {
        let word = "ភូមិសាស្ត្រ";
        let replaced = [("prefix","pumisah"),("middle","phumesah"),("suffix","phumisas")];
        let baseline = engine_for(word);
        for (_,input) in replaced {
            assert!(exact(&baseline,input,word),"the rejected reading must exist before the override: {input}");
        }
        with_configuration(|source| {
            // Each stored component has two real readings. Select one per
            // position so losing any component's scope leaves an extra input.
            add_case(source,"មិ",serde_json::json!({
                "kind":"scoped","keys":["mi","me"],"words":[word],"positions":["middle"],
                "preserveMapped":true,"checkedContextReadings":[{
                    "word":word,"prefix":"ភូ","prefixKeys":["phu","pu"],
                    "suffix":"សាស្ត្រ","suffixKeys":["sah","sas"]
                }]
            }));
            for (spelling,key,position) in [("ភូ","phu","start"),("មិ","mi","middle")] {
                add_case(source,spelling,serde_json::json!({"kind":"syllable","keys":[key],
                    "words":[word],"positions":[position]}));
            }
            // An explicit scoped choice wins over the derived Read as default.
            add_case(source,"សាស្ត្រ",serde_json::json!({"kind":"syllable",
                "keys":["sah"],"words":[word],"positions":["end"]}));
        }, || {
            let readings = checked_component_pair_readings(word,false,Context::new(word));
            assert_eq!(readings,vec![vec![vec!["phu".into()],vec!["mi".into()],vec!["sah".into()]]]);
            let engine = engine_for(word);
            assert!(exact(&engine,"phumisah",word));
            for (position,input) in replaced {
                assert!(!exact(&engine,input,word),"unreplaced {position}: {input}");
            }
        });
    }

    #[test]
    fn independently_mapped_suffix_keeps_original_word_scope() {
        with_configuration(|source|add_case(source,"ខ្វែង",serde_json::json!({
            "kind":"syllable","keys":["cross"],"words":["ពាក្យខ្វែង"],"positions":["end"]
        })), || {
            let engine = engine_for("ពាក្យខ្វែង");
            assert!(exact(&engine,"peakcross","ពាក្យខ្វែង"));
            assert!(exact(&engine,"piakcross","ពាក្យខ្វែង"));
            assert!(!exact(&engine,"peakkhvaeng","ពាក្យខ្វែង"));
        });
    }

    #[test]
    fn longest_complete_match_is_independent_of_entry_order() {
        let short = FixedSyllable {khmer:"កម្ម".into(),keys:vec!["kam".into()].into(),original_spelling_only:true,words:vec![],positions:vec![]};
        let long = FixedSyllable {khmer:"កម្មវិធី".into(),keys:vec!["program".into()].into(),original_spelling_only:true,words:vec![],positions:vec![]};
        let chars: Vec<_> = "កម្មវិធី".chars().collect();
        for units in [[&short, &long], [&long, &short]] {
            let parsed: Vec<_> = units.iter().map(|unit| FixedSyllable {
                khmer:unit.khmer.clone(),keys:unit.keys.to_vec().into(),original_spelling_only:unit.original_spelling_only,
                words:unit.words.clone(),positions:unit.positions.clone(),
            }).collect();
            assert_eq!(at(&parsed,"កម្មវិធី",&chars,0,true).unwrap().0.keys[0].text,"program");
            assert!(at(&parsed,"កម្មវិធី",&chars,0,false).is_none());
        }
    }

    #[test]
    fn stored_components_are_retained_only_for_equivalent_complete_readings() {
        let reading = vec![vec!["a".into(),"b".into()],vec!["x".into(),"y".into()]];
        assert!(same_keys(&reading,&["ax".into(),"ay".into(),"bx".into(),"by".into()]));
        assert!(!same_keys(&reading,&["ax".into(),"by".into()]));
        let ranked: Key = serde_json::from_value(serde_json::json!({"key":"ax","rankPriority":7})).unwrap();
        assert!(!same_keys(&[vec!["ax".into()]],&[ranked]));
    }

    #[test]
    fn migrated_replacements_preserve_every_original_scope_and_key() {
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../../test/fixtures/shared-override-migration.json")).unwrap();
        let cases = proof["cases"].as_array().unwrap();
        assert_eq!(cases.len(),6);
        for row in cases {
            let word = row["khmer"].as_str().unwrap();
            if ["នឹង", "ផល"].contains(&word) {
                assert!(!rules().runtime.fixed_syllables.iter().any(|unit|unit.khmer == word));
                continue; // Explicitly removed by the owner.
            }
            let unit = rules().runtime.fixed_syllables.iter().find(|unit|unit.khmer == word).unwrap();
            let old = &row["sourceCase"];
            let words: Vec<String> = serde_json::from_value(old["words"].clone()).unwrap();
            let positions: Vec<WordReadingPosition> = serde_json::from_value(
                if old["kind"] == "prefix" {serde_json::json!(["start"])} else {old["positions"].clone()}
            ).unwrap();
            // The owner later selected ទិន្ន → ទិន, retaining ten and adding tin.
            let keys: Vec<Key> = serde_json::from_value(if word == "ទិន្ន" {
                serde_json::json!(["tin","ten"])
            } else { row["keys"].clone() }).unwrap();
            assert_eq!(unit.words,words,"{word}");
            assert_eq!(unit.positions,positions,"{word}");
            let graph=factored::Graph::new(&[vec![unit.keys.to_vec()]]);
            let mut expected:Vec<_>=keys.iter().map(|key|key.text.clone()).collect();
            expected.extend(keys.iter().filter(|key|key.text.ends_with("ech")||key.text.ends_with("ich"))
                .map(|key|format!("{}x",&key.text[..key.text.len()-2])));
            expected.sort();expected.dedup();
            let (actual,more)=browse::display_inputs(&[vec![unit.keys.to_vec()]],64);
            assert!(!more);assert_eq!(actual,expected,"{word}");
            for key in &keys {assert!(graph.matching(&key.text,None,false)
                .is_some_and(|hit|hit.exact&&hit.priority==key.priority),"{word}: {}",key.text);}
            assert_eq!(unit.original_spelling_only,old["kind"] == "prefix","{word}");
            assert!(!rules().runtime.word_readings.iter().any(|unit|unit.khmer == word));
            for selected in &words {
                let chars: Vec<_> = selected.chars().collect();
                assert!((0..chars.len()).any(|start|unit.checked_end(selected,&chars,start,true).is_some()),"{selected}");
            }
            let outside = format!("{}កកក",words[0]);
            assert!(!words.contains(&outside));
            let chars: Vec<_> = outside.chars().collect();
            assert!((0..chars.len()).all(|start|unit.checked_end(&outside,&chars,start,true).is_none()));
        }
    }

    #[test]
    fn scoped_overrides_keep_position_limits_and_original_offsets() {
        let mut unit: FixedSyllable = serde_json::from_value(serde_json::json!({
            "khmer":"កា","keys":["local"],"words":["កាខា","ខាកា","ខាកាខា","កា"],"positions":["start"]
        })).unwrap();
        let matches = |unit: &FixedSyllable, word: &str, start|unit.checked_end(word,&word.chars().collect::<Vec<_>>(),start,true).is_some();
        assert!(matches(&unit,"កាខា",0));
        assert!(!matches(&unit,"ខាកា",2));
        assert!(!matches(&unit,"ខាកាខា",2));
        assert!(!matches(&unit,"កា",0));
        assert!(unit.checked_end("កាខា",&"ខាកា".chars().collect::<Vec<_>>(),2,false).is_none());
        unit.positions = vec![WordReadingPosition::End];
        assert!(matches(&unit,"ខាកា",2));
        assert!(!matches(&unit,"កាខា",0));
        assert!(!matches(&unit,"កា",0));
        unit.positions = vec![WordReadingPosition::Standalone];
        assert!(matches(&unit,"កា",0));
        assert!(!matches(&unit,"ខាកា",2));
    }

    #[test]
    fn overrides_replace_all_default_paths_and_keep_explicit_spellings() {
        with_overrides(&[("អ្នក","person"),("ជ័យ","win"),("កម្មវិធី","program"),
            ("សិទ្ធិ","right"),("សាស្ត្រ","science"),("តារា","tara")], || {
            let words = ["អ្នក","ជ័យ","កម្មវិធី","សិទ្ធិ","សាស្ត្រ","កាជ័យ",
                "គណិតសាស្ត្រ","តារាសាស្ត្រ","តារាវិទ្យាសាស្ត្រ","លោកអ្នក","អភិសិទ្ធិ"];
            let base = Engine::from_json(&serde_json::json!({"schemaVersion":2,
                "words":words.iter().enumerate().map(|(i,word)|serde_json::json!({
                    "id":i+1,"khmer":word,"rank":i,"status":"approved"
                })).collect::<Vec<_>>(),
                "mappings":[{"id":501,"romanized":"ownscience","khmer":"សាស្ត្រ","rank":7,"status":"approved"}]
            }).to_string()).unwrap();
            let personal = Engine::from_personal_words_json(r#"[
                {"word":"សាស្ត្រ","spellings":["sastro","sah"]},
                {"word":"អ្នក","spellings":["neak"]}
            ]"#).unwrap();
            let before = (base.to_binary(),personal.to_binary());
            for (word,input) in [("អ្នក","person"),("ជ័យ","win"),("កម្មវិធី","program"),
                ("សិទ្ធិ","right"),("សាស្ត្រ","science"),("កាជ័យ","kawin"),
                ("គណិតសាស្ត្រ","kaeknitscience"),("តារាសាស្ត្រ","tarascience"),
                ("តារាវិទ្យាសាស្ត្រ","taravityeascience"),("លោកអ្នក","lokperson"),
                ("អភិសិទ្ធិ","akphiright")] {
                let query = base.suggest_with_context(input,Default::default(),&Default::default()).unwrap();
                let hit = query.exact_candidates.iter().find(|candidate|candidate.khmer == word)
                    .unwrap_or_else(||panic!("{input}: {word}"));
                let index = words.iter().position(|&entry|entry == word).unwrap();
                assert_eq!(hit.mapping_id,Some(index as u64 + 1));
                assert_eq!(hit.rank,index as i32);
                assert!(!base.suggest_with_context(input,SuggestionFeatures {sound_map:false,..Default::default()},
                    &Default::default()).unwrap().exact_candidates.iter().any(|candidate|candidate.khmer == word));
            }
            for (word,input) in [("អ្នក","neak"),("ជ័យ","jey"),("ជ័យ","chey"),
                ("កម្មវិធី","kamvithi"),("សាស្ត្រ","sah"),("សាស្ត្រ","sas"),("សាស្ត្រ","sastro"),
                ("គណិតសាស្ត្រ","kaeknitsah"),("តារាវិទ្យាសាស្ត្រ","taravityeasas"),
                ("លោកអ្នក","lokneak"),("អភិសិទ្ធិ","akphiset")] {
                assert!(!base.suggest_with_context(input,Default::default(),&Default::default()).unwrap()
                    .exact_candidates.iter().any(|candidate|candidate.khmer == word),"old default {input}: {word}");
            }
            for sound_map in [true,false] {
                for (word,input) in [("សាស្ត្រ","ownscience"),("សាស្ត្រ","sastro"),("សាស្ត្រ","sah"),("អ្នក","neak")] {
                    let result = base.suggest_with_extension(&personal,input,
                        SuggestionFeatures {sound_map,..Default::default()},&Default::default()).unwrap();
                    assert!(result.exact_candidates.iter().any(|candidate|candidate.khmer == word),"saved {input}");
                    assert_eq!(base.convert_with_extension(&personal,input).unwrap().candidates[0].khmer,word);
                }
            }
            assert_eq!((base.to_binary(),personal.to_binary()),before);
        });
    }

    fn engine_for(word: &str) -> Engine {
        Engine::from_json(&serde_json::json!({"schemaVersion":2,"mappings":[],
            "words":[{"id":1,"khmer":word,"rank":0,"status":"approved"}]
        }).to_string()).unwrap()
    }

    fn exact(engine: &Engine, input: &str, word: &str) -> bool {
        engine.suggest_with_context(input,Default::default(),&Default::default()).unwrap()
            .exact_candidates.iter().any(|candidate|candidate.khmer == word)
    }

    #[test]
    fn syllable_final_so_keeps_both_keys_before_an_inline_override() {
        let before = word_readings("ទេសធម៌");
        // A terminal override uses the inline parser rather than the old
        // terminal-root split. Both routes must keep the preceding ស coda.
        with_configuration(|source| {
            let parts = source["runtime"]["wordParts"].as_array_mut().unwrap();
            let order = parts.iter().flat_map(|part|part["cases"].as_array().unwrap())
                .filter(|case|case["kind"] == "syllable")
                .map(|case|case["order"].as_u64().unwrap()).max().unwrap() + 1;
            let case = &mut parts.iter_mut().find(|part|part["khmer"] == "ធម៌").unwrap()["cases"][0];
            case["kind"] = "syllable".into();
            case["order"] = order.into();
            case["positions"] = serde_json::json!(["end","standalone"]);
            case["originalSpellingOnly"] = true.into();
            let mut roots: Vec<_> = parts.iter_mut().flat_map(|part|part["cases"].as_array_mut().unwrap())
                .filter(|case|case["kind"] == "terminalRoot").collect();
            roots.sort_by_key(|case|case["order"].as_u64().unwrap());
            for (order,case) in roots.into_iter().enumerate() { case["order"] = order.into(); }
        }, || {
            assert_eq!(word_readings("ទេសធម៌"),before);
            let engine = engine_for("ទេសធម៌");
            assert!(exact(&engine,"tehthoa","ទេសធម៌"));
            assert!(exact(&engine,"testhoa","ទេសធម៌"));
        });
    }

    #[test]
    fn crossing_an_old_terminal_root_split_replaces_the_whole_default_path() {
        with_overrides(&[("ភិសិទ្ធិ","edge")], || {
            let engine = engine_for("អភិសិទ្ធិ");
            assert!(exact(&engine,"akedge","អភិសិទ្ធិ"));
            assert!(!exact(&engine,"akphiset","អភិសិទ្ធិ"));
        });
    }

    #[test]
    fn crossing_a_checked_component_split_cannot_restore_its_default_keys() {
        with_overrides(&[("ណិតសាស្ត្រ","edge")], || {
            let engine = engine_for("គណិតសាស្ត្រ");
            assert!(exact(&engine,"koedge","គណិតសាស្ត្រ"));
            assert!(!exact(&engine,"kaeknitsah","គណិតសាស្ត្រ"));
            assert!(!exact(&engine,"kaeknitsas","គណិតសាស្ត្រ"));
        });
    }

    #[test]
    fn ineligible_overrides_do_not_disable_original_terminal_readings() {
        let words = ["ទស្សន៍","ទូរទស្សន៍"];
        let before: Vec<_> = words.iter().map(|word|word_readings(word)).collect();
        with_overrides(&[("ទស្ស","vision")], || {
            assert_eq!(overrides::whole("ទស្ស").unwrap()[0][0].text,"vision");
            for (word, expected) in words.iter().zip(&before) {
                assert_eq!(&word_readings(word),expected,"{word}");
            }
        });
    }

    #[test]
    fn ineligible_overrides_leave_stored_marked_components_intact() {
        let word = "បទពិសោធន៍";
        let before = word_readings(word);
        with_overrides(&[("ពិសោធ","experiment")], || {
            assert_eq!(whole("ពិសោធ").unwrap()[0][0].text,"experiment");
            assert_eq!(word_readings(word),before);
        });
    }

    #[test]
    fn overlap_checks_use_the_same_non_overlapping_longest_matches() {
        with_overrides(&[("កាខា","first"),("ខាគា","second")], || {
            let chars: Vec<_> = "កាខាគា".chars().collect();
            assert!(in_range("កាខាគា",&chars,0,2,true),"override crosses this default component's end");
            assert!(!in_range("កាខាគា",&chars,4,chars.len(),true),"the earlier selected override already consumed ខា");
        });
    }
    #[test]
    #[cfg(feature="bundled-dictionaries")]
    fn word_part_read_as_migration_preserves_complete_bundled_profiles_and_scoped_context() {
        let proof:serde_json::Value=serde_json::from_str(include_str!("../../../test/fixtures/word-part-read-as-migration.json")).unwrap();
        let mut engine=Engine::bundled().unwrap();
        let info:serde_json::Value=serde_json::from_str(&engine.app_data_info_json()).unwrap();
        let mut pronunciations:Vec<crate::Pronunciation>=serde_json::from_value(info["provenance"]["pronunciations"]["rules"].clone()).unwrap();
        let moved:crate::Pronunciation=serde_json::from_value(proof["pronunciation"].clone()).unwrap();
        pronunciations.retain(|rule|rule.word!=moved.word);let former_pronunciations=pronunciations.clone();pronunciations.push(moved);
        engine.set_pronunciations(pronunciations).unwrap();
        let current=Index::new(&engine.dictionary);let context=starting_rule_context(&engine.dictionary);
        let binary=engine.to_binary();
        with_configuration(|source|{
            let moved=&proof["movedPart"];
            let order=moved["cases"][0]["order"].as_u64().unwrap();
            for group in ["wordParts","shorthands"] {
                for part in source["runtime"][group].as_array_mut().unwrap() {
                    for case in part["cases"].as_array_mut().unwrap() {
                        if case["kind"]=="syllable" && case["order"].as_u64().unwrap()>=order {
                            case["order"]=(case["order"].as_u64().unwrap()+1).into();
                        }
                    }
                }
            }
            let parts=source["runtime"]["wordParts"].as_array_mut().unwrap();
            parts.push(moved.clone());
            let scoped=parts.iter_mut().find(|p|p["khmer"]==proof["reuseDisabled"]).unwrap();
            scoped.as_object_mut().unwrap().remove("reusableAsComponent");
            let mut expected = proof["keptScopedDefinition"].clone();
            expected["keys"] = serde_json::json!({"readAs":"បណ្ឌិត"});
            expected.as_object_mut().unwrap().remove("readingType");
            expected["cases"][0]["order"] = scoped["cases"][0]["order"].clone();
            assert_eq!(scoped,&expected);
        },||{
            let mut former=Engine::bundled().unwrap();former.set_pronunciations(former_pronunciations).unwrap();
            let previous=Index::new(&former.dictionary);
            assert_eq!(starting_rule_context(&former.dictionary),context);
            assert_eq!(current.words.len(),previous.words.len());
            for (after,before) in current.words.iter().zip(&previous.words) {
                assert_eq!(after.candidate,before.candidate);
                if after.readings!=before.readings {
                    let old=reading_profile(&before.readings).expect("bounded migration profile");
                    assert_eq!(reading_profile(&after.readings),Some(old),"{}",before.candidate.khmer);
                }
            }
            assert_eq!(former.to_binary(),binary);
        });
    }
    #[test]
    #[cfg(feature="bundled-dictionaries")]
    fn glottal_ou_retains_all_existing_weighted_readings_and_word_identities() {
        let engine = Engine::bundled().unwrap();
        let current = Index::new(&engine.dictionary);
        with_configuration(|source| {
            source["runtime"]["glottalOnset"]["additionalVowelKeys"] = serde_json::json!({});
        }, || {
            let previous = Index::new(&engine.dictionary);
            assert_eq!(previous.words.len(),current.words.len());
            for (old,new) in previous.words.iter().zip(&current.words) {
                assert_eq!(old.candidate,new.candidate);
                for reading in &old.readings {
                    assert!(new.readings.iter().any(|after|after.len() == reading.len()
                        && reading.iter().zip(after).all(|(before,after)|before.iter().all(|key|after.contains(key)))),
                        "lost weighted reading: {} {reading:?}",old.candidate.khmer);
                }
            }
        });
    }

    #[test]
    #[cfg(feature="bundled-dictionaries")]
    fn disabling_shared_two_dot_keeps_explicit_read_as_defaults_and_edits_usable() {
        with_configuration(|source| {
            source["runtime"]["shortInherentSign"]["enabled"] = false.into();
        }, || {
            assert_eq!(Engine::pronunciation_inputs("គៈណៈ").unwrap(), ["keaknak", "keanak"]);
            let mut engine = Engine::from_json(&serde_json::json!({
                "schemaVersion":3,"mappings":[],"englishWords":[],
                "words":[{"id":1,"khmer":"កុម្ភៈ","rank":0,"status":"approved"},
                    {"id":2,"khmer":"ស្រៈ","rank":1,"status":"approved"}]
            }).to_string()).unwrap();
            let before = engine.to_binary();
            let exact = |engine:&Engine, input:&str, word:&str| engine.suggest_with_context(
                input, Default::default(), &Default::default()).unwrap()
                .exact_candidates.iter().any(|candidate|candidate.khmer == word);
            assert!(exact(&engine,"kompheak","កុម្ភៈ"));
            assert!(!exact(&engine,"srak","ស្រៈ"),"the ordinary shared path is disabled");
            engine.set_pronunciations(vec![crate::Pronunciation { word:"ស្រៈ".into(),read_as:"រ៉ៈ".into() }]).unwrap();
            assert!(exact(&engine,"rak","ស្រៈ"));
            engine.set_pronunciations(vec![]).unwrap();
            assert!(exact(&engine,"kompheak","កុម្ភៈ"));
            assert!(!exact(&engine,"rak","ស្រៈ"));
            assert_eq!(engine.to_binary(),before);
        });
    }

    #[test]
    #[cfg(feature="bundled-dictionaries")]
    fn shared_two_dot_preserves_all_existing_weighted_readings_and_prefix_membership() {
        let engine = Engine::bundled().unwrap();
        let current = Index::new(&engine.dictionary);
        let proof: serde_json::Value = serde_json::from_str(include_str!("../../../test/fixtures/shared-two-dot.json")).unwrap();
        with_configuration(|source| {
            source["runtime"]["shortInherentSign"]["enabled"] = false.into();
            source["pronunciationSyllables"].as_array_mut().unwrap().retain(|rule|
                !proof["irregular"].as_array().unwrap().iter().any(|entry|entry["word"] == rule["word"]));
            let part = source["runtime"]["wordParts"].as_array_mut().unwrap().iter_mut()
                .find(|part|part["khmer"] == "គណៈ").unwrap();
            part["cases"][0]["words"].as_array_mut().unwrap().retain(|word|word != "គណៈកម្មាធិការ");
        }, || {
            let previous = Index::new(&engine.dictionary);
            for old in &previous.words {
                let new = current.words.iter().find(|word|word.candidate.khmer == old.candidate.khmer).unwrap();
                assert_eq!(new.candidate,old.candidate);
                for reading in &old.readings {
                    assert!(new.readings.iter().any(|after|after.len() == reading.len()
                        && reading.iter().zip(after).all(|(before,after)|before.iter().all(|key|after.contains(key)))),
                        "lost weighted lattice: {} {reading:?}",old.candidate.khmer);
                }
            }
            for (prefix, ids) in &previous.prefixes {
                let new = &current.prefixes[prefix];
                for &id in ids {
                    let candidate = &previous.words[id].candidate;
                    assert!(new.iter().any(|&id|current.words[id].candidate == *candidate),"{prefix}: {candidate:?}");
                }
            }
        });
    }

    #[test]
    #[cfg(feature="bundled-dictionaries")]
    fn read_as_boundaries_preserve_every_bundled_lattice_and_scoped_path() {
        let fixture:serde_json::Value=serde_json::from_str(include_str!("../../../test/fixtures/read-as-boundaries.json")).unwrap();
        let engine=Engine::bundled().unwrap();
        // Compare the boundary migration independently of newer whole-word
        // edits whose alternate spellings themselves use these defaults.
        let mut dictionary=engine.dictionary.clone();
        dictionary.pronunciations=std::sync::Arc::new(dictionary.pronunciations.iter().filter(|rule|
            !fixture["defaults"].as_array().unwrap().iter().any(|default|
                rule.word != default["word"].as_str().unwrap()
                    && rule.read_as.spellings().iter().any(|spelling|spelling.contains(default["word"].as_str().unwrap())))).cloned().collect());
        let dependent: BTreeSet<_> = rules().runtime.pronunciation_defaults.iter().filter(|rule|
            fixture["defaults"].as_array().unwrap().iter().any(|default|
                rule.read_as.spellings().iter().any(|spelling|spelling.contains(default["word"].as_str().unwrap()))))
            .map(|rule|rule.word.clone()).collect();
        let current=Index::new(&dictionary);
        let controls:Vec<_>=super::super::parity::fixture_words().into_iter().map(|word|{
            let readings=(word_readings_mode(&word,false),word_readings_mode(&word,true));(word,readings)
        }).collect();
        with_configuration(|source|{
            source["pronunciationSyllables"].as_array_mut().unwrap().retain(|rule|
                !fixture["defaults"].as_array().unwrap().iter().any(|default|default["word"] == rule["word"]));
            let parts=source["runtime"]["wordParts"].as_array_mut().unwrap();
            for part in parts.iter_mut() {for case in part["cases"].as_array_mut().unwrap() {
                if case["kind"]=="syllable" {case["order"]=(case["order"].as_u64().unwrap()+2).into();}
            }}
            parts.push(fixture["movedParts"][0].clone());
            let science=parts.iter_mut().find(|part|part["khmer"]=="សាស្ត្រ").unwrap();
            science.as_object_mut().unwrap().remove("reusableAsComponent");
            science["cases"].as_array_mut().unwrap().insert(0,fixture["movedParts"][1]["cases"][0].clone());
        },||{
            let mut legacy=dictionary.clone();
            legacy.pronunciations=std::sync::Arc::new(legacy.pronunciations.iter().filter(|rule|!fixture["defaults"].as_array().unwrap().iter().any(|default|default["word"]==rule.word)).cloned().collect());
            let previous=Index::new(&legacy);
            assert_eq!(current.words.len(),previous.words.len());
            for (after,before) in current.words.iter().zip(&previous.words) {
                assert_eq!(after.candidate,before.candidate);
                if dependent.contains(&before.candidate.khmer) {continue;}
                assert_eq!(after.readings,before.readings,"{}",before.candidate.khmer);
                assert_eq!(after.recovery_lengths,before.recovery_lengths);
            }
            let without_dependents = |index:&Index| index.prefixes.iter().map(|(prefix,ids)|
                (prefix.clone(),ids.iter().copied().filter(|id|!dependent.contains(&index.words[*id].candidate.khmer)).collect::<Vec<_>>()))
                .filter(|(_,ids)|!ids.is_empty()).collect::<BTreeMap<_,_>>();
            assert_eq!(without_dependents(&current),without_dependents(&previous));
            for (word,readings) in controls {
                if dependent.contains(&word) {continue;}
                assert_eq!((word_readings_mode(&word,false),word_readings_mode(&word,true)),readings,"{word}");
            }
        });
    }

    #[test]
    #[cfg(feature = "bundled-dictionaries")]
    fn dry_run_of_scoped_and_standalone_migrations_preserves_bundled_typing() {
        let engine = Engine::bundled().unwrap();
        let before = Index::new(&engine.dictionary);
        let starting_context = starting_rule_context(&engine.dictionary);
        let controls: Vec<_> = super::super::parity::fixture_words().into_iter()
            .filter(|word| word.contains("អ្នក"))
            .map(|word| {
                let readings = word_readings_mode(&word, false);
                (word, readings)
            }).collect();
        with_configuration(|source| {
            let parts = source["runtime"]["wordParts"].as_array_mut().unwrap();
            let mut migrated = Vec::new();
            for (word, read_as) in [("អ្នក", "នាក់")] {
                let index = parts.iter().position(|part| part["khmer"] == word).unwrap();
                let part = parts.remove(index);
                let [case] = part["cases"].as_array().unwrap().as_slice() else {
                    panic!("{word} must have one rule case for this migration");
                };
                assert_eq!(case["kind"], "syllable");
                let order = case["order"].as_u64().unwrap();
                for part in parts.iter_mut() {
                    for remaining in part["cases"].as_array_mut().unwrap() {
                        if remaining["kind"] == "syllable" && remaining["order"].as_u64().unwrap() > order {
                            remaining["order"] = (remaining["order"].as_u64().unwrap() - 1).into();
                        }
                    }
                }
                migrated.push(serde_json::json!({
                    "word": word, "readAs": read_as, "words": case["words"], "positions": case["positions"],
                }));
            }
            source["pronunciationSyllables"].as_array_mut().unwrap().extend(migrated);
        }, || {
            let after = Index::new(&engine.dictionary);
            assert_eq!(after.words.len(), before.words.len());
            for (new, old) in after.words.iter().zip(&before.words) {
                assert_eq!(new.candidate, old.candidate);
                assert_eq!(reading_profile(&new.readings),reading_profile(&old.readings),"{}",old.candidate.khmer);
                assert_eq!(new.recovery_lengths, old.recovery_lengths);
            }
            assert_eq!(after.prefixes, before.prefixes);
            assert_eq!(after.count_prefixes, before.count_prefixes);
            assert_eq!(starting_rule_context(&engine.dictionary), starting_context);
            for (word, readings) in controls {
                assert_eq!(word_readings_mode(&word, false), readings, "{word}");
            }
        });
    }

    #[test]
    fn migrated_pronunciation_can_initialize_with_a_reparsed_marked_syllable() {
        let expected = Engine::pronunciation_inputs("បង្អាក់").unwrap();
        with_configuration(|source| {
            source["pronunciationSyllables"].as_array_mut().unwrap().push(serde_json::json!({
                "word":"មិត្ត", "readAs":"បង្អាក់", "words":["មិត្ត"], "positions":["standalone"],
            }));
        }, || {
            let core = engine_for("មិត្ត");
            assert_eq!(core.sound_index.get().unwrap().browse_inputs("មិត្ត").0, expected);
        });
    }

}
