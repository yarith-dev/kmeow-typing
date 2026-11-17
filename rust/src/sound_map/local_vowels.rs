//! Local unwritten-vowel choices. A letter pair is eligible only in its
//! configured syllable shape, with the original spelling and marks intact.
use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Pair {
    first: String,
    second: String,
    subscript: String,
    structures: Vec<Structure>,
    keys: Vec<Key>,
    #[serde(default)]
    join_consonants: bool,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
enum Structure {
    MedialVowel,
    MedialContinuation,
    ClosedCluster,
    FinalCluster,
}

pub(super) struct Selection {
    pair: &'static Pair,
    pub(super) final_cluster: bool,
    pub(super) end: usize,
}
impl Selection {
    pub(super) fn keys(&self) -> Vec<Key> { self.pair.keys.clone() }
    pub(super) fn joins_consonants(&self) -> bool { self.pair.join_consonants }
}

pub(super) fn validate(rules: &Rules) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for pair in &rules.runtime.local_vowel_pairs {
        if [&pair.first,&pair.second,&pair.subscript].iter().any(|letter|
            letter.chars().count() != 1 || !rules.consonants.iter().any(|c|&c.khmer == *letter))
            || pair.structures.is_empty() || pair.keys.is_empty()
            || pair.keys.iter().any(|key|!matches!(key.text.as_str(),"a"|"o"|"ae"|"er")) {
            return Err("Invalid local vowel pair: require consonants, syllable structures and a/o/ae/er keys.".into());
        }
        for structure in &pair.structures {
            if !seen.insert((&pair.first,&pair.second,&pair.subscript,*structure)) {
                return Err("Duplicate local vowel pair and syllable structure.".into());
            }
        }
        if pair.join_consonants {
            let left = rules.consonants.iter().find(|c|c.khmer == pair.second).unwrap();
            let right = rules.consonants.iter().find(|c|c.khmer == pair.subscript).unwrap();
            if left.onset_keys.len() != 1 || right.onset_keys.is_empty()
                || !right.onset_keys.iter().all(|key|key.text.starts_with(&left.onset_keys[0].text)
                    && key.text != left.onset_keys[0].text) {
                return Err("Joined consonants require the left key to overlap the start of the right key.".into());
            }
        }
    }
    Ok(())
}

pub(super) fn joins_cluster(chars: &[char], start: usize, context: overrides::Context<'_>) -> bool {
    start.checked_sub(1).and_then(|first|at(chars,first,context))
        .is_some_and(|selection|selection.joins_consonants() && selection.end == start + 3)
}

pub(super) fn at(chars: &[char], start: usize, context: overrides::Context<'_>) -> Option<Selection> {
    if !context.original || start.checked_sub(1).and_then(|i|chars.get(i)) == Some(&'្') {
        return None;
    }
    let frame = chars.get(start..start.checked_add(4)?)?;
    if frame[2] != '្' { return None; }
    let original: Vec<_> = context.word.chars().collect();
    let absolute = context.offset.checked_add(start)?;
    if original.get(absolute..absolute + 4)? != frame
        || absolute.checked_sub(1).and_then(|i|original.get(i)) == Some(&'្') {
        return None;
    }
    let end = start + 4;
    // Shared replacements remain authoritative, including matches crossing
    // the frame. An attached vowel, shift or extra coeng is never discarded.
    if context.in_range(chars,start,end) { return None; }
    for pair in &rules().runtime.local_vowel_pairs {
        if !pair.first.starts_with(frame[0]) || !pair.second.starts_with(frame[1])
            || !pair.subscript.starts_with(frame[3]) { continue; }
        for shape in &pair.structures {
            let matched = match shape {
                Structure::MedialVowel => {
                    let last = end + chars[end..].iter().take_while(|&&ch|is_vowel(ch)).count();
                    last > end && !original.get(context.offset + last).is_some_and(|&ch|is_vowel(ch))
                        && original.get(absolute + 4..context.offset + last) == chars.get(end..last)
                        && vowel_keys(&chars[end..last].iter().collect::<String>(),
                            consonant(frame[3])?.series.as_str()).is_some()
                },
                Structure::MedialContinuation => chars.get(end).is_some_and(|&ch|consonant(ch).is_some())
                    && original.get(context.offset + end) == chars.get(end),
                Structure::ClosedCluster => silent_coda_at(chars,start + 1).is_some_and(|(_,last)|last == end)
                    && (chars.get(end).is_some_and(|&ch|consonant(ch).is_some())
                        && original.get(context.offset + end) == chars.get(end)
                        || end == chars.len() && context.offset + end == context.end
                            && (context.end == original.len()
                                || original.get(context.end).is_some_and(|&ch|consonant(ch).is_some()))),
                Structure::FinalCluster => frame[1] == frame[3] && end == chars.len()
                    && context.offset + end == original.len(),
            };
            if matched { return Some(Selection {pair,final_cluster:*shape == Structure::FinalCluster,end}); }
        }
    }
    None
}

/// Checked component keys are generated defaults, not saved aliases. Change
/// only their leading unwritten vowel when the complete original pair fits;
/// retain their reviewed suffix, key order, ranking and full-word scope.
pub(super) fn component(spelling: &str, keys: &[Key], context: overrides::Context<'_>) -> Option<Vec<Key>> {
    let chars: Vec<_> = spelling.chars().collect();
    let selection = at(&chars,0,context)?;
    let onset = consonant(chars[0])?;
    let defaults: Vec<_> = rules().runtime.inherent.get(&onset.series).into_iter()
        .chain(rules().runtime.closed_inherent.get(&onset.series)).flatten().collect();
    let mut result = Vec::new();
    for key in keys {
        let replacement = onset.onset_keys.iter().find_map(|initial|
            defaults.iter().find_map(|vowel|
                key.text.strip_prefix(&format!("{}{}",initial.text,vowel.text)).map(|tail|(initial,tail))));
        if let Some((initial,tail)) = replacement {
            for vowel in &selection.pair.keys {
                let tail = if selection.joins_consonants() {
                    let left = &consonant(chars[1])?.onset_keys[0];
                    let right = &consonant(chars[3])?.onset_keys;
                    if right.iter().any(|key|tail.starts_with(&format!("{}{}",left.text,key.text))) {
                        &tail[left.len()..]
                    } else { tail }
                } else { tail };
                let key = Key {text:format!("{}{}{tail}",initial.text,vowel.text),priority:key.priority};
                if !result.contains(&key) { result.push(key); }
            }
        } else if !result.contains(key) { result.push(key.clone()); }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vowels_shifts_marks_extra_subscripts_and_exposed_ends_keep_their_own_rules() {
        for word in ["កក","កកាយ","លក់","កាញ្ញា","ក៉ញ្ញា","ស៊ប្បាយ","វគ្គ្រា","លក្ខ៉ណៈ","វគ្គក៍"] {
            let chars: Vec<_> = word.chars().collect();
            assert!(at(&chars,0,overrides::Context::new(word)).is_none(),"{word}");
        }
        let word = "វគ្គក៍";
        let exposed: Vec<_> = "វគ្គ".chars().collect();
        assert!(at(&exposed,0,overrides::Context::new(word)).is_none());
        let original: Vec<_> = "វគ្គ".chars().collect();
        assert!(at(&original,0,overrides::Context::new("វគ្គ").with_original(false)).is_none());
        assert!(at(&original,0,overrides::Context::new("វគ្គ")).unwrap().final_cluster);
    }

    #[test]
    fn checked_fragments_keep_original_offsets_and_suffixes() {
        let context = overrides::Context::new("លក្ខខណ្ឌ").part(0,4);
        assert_eq!(component("លក្ខ",&[Key::from("lok")],context),Some(vec![Key::from("laek")]));
        assert!(component("លក្ខ",&[Key::from("lok")],overrides::Context::new("លក់")).is_none());
        let context = overrides::Context::new("កាលក្ខខណ្ឌ").part(2,6);
        assert_eq!(component("លក្ខ",&[Key::from("lok")],context),Some(vec![Key::from("laek")]));
    }
}
