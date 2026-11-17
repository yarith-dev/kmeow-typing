//! Add complete component-word readings beside the original full-word paths.
//! Saved aliases, partial spellings and ambiguous segmentations never feed it.
use super::*;

#[derive(Default, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(super) struct Settings {
    pub enabled: bool,
    pub rank_priority: i32,
}
#[derive(Deserialize)]
pub(super) struct Reading { pub khmer: String, pub keys: Vec<Key> }
#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
pub(super) struct DefaultPart {
    pub khmer:String,
    #[serde(default)]
    pub starting_words:Vec<String>,
}

// Below original strict AND original consonant-count matches, including every
// possible i32-key total in a bounded 128-chunk reading. Apply once per path.
pub(super) const FALLBACK_PRIORITY:i64=-(1 << 52);
// Leave room for positive per-key i32 boosts within a component path.
pub(super) const FALLBACK_CEILING:i64=FALLBACK_PRIORITY/2;
const MAX_DEPTH:usize=64;
const MAX_PATHS:usize=64;
struct Path { groups:Vec<Vec<Key>>, reused:bool }

pub(super) fn protected_parent(word:&str)->bool {
    if overrides::Context::new(word).whole(word).is_some() {return true;}
    rules().runtime.word_readings.iter().any(|unit|unit.words.iter().any(|selected|selected==word)
        && (unit.checked_suffix_readings.iter().chain(&unit.checked_prefix_readings).any(|neighbor|neighbor.word==word)
            || unit.checked_context_readings.iter().any(|context|context.word==word)))
}
// A respelling agreeing with an existing checked neighbor is already part of
// that scoped path. Keep its earlier compound behavior; conflicting edits must
// not bypass a selected prefix, suffix or middle-word context.
pub(super) fn agrees_with_checked_neighbors(word:&str,pronunciations:&[crate::Pronunciation])->bool {
    let relevant:Vec<_>=pronunciations.iter().filter(|rule|word.contains(&rule.word)).collect();
    !relevant.is_empty() && relevant.iter().all(|rule|{
        let Some(readings)=word_readings_in_context(&rule.read_as,false,overrides::Context::new(&rule.read_as).for_pronunciation()) else {return false;};
        rules().runtime.word_readings.iter().filter(|unit|unit.words.iter().any(|selected|selected==word))
            .flat_map(|unit|unit.checked_suffix_readings.iter().chain(&unit.checked_prefix_readings))
            .any(|neighbor|neighbor.word==word && neighbor.khmer==rule.word
                && readings.iter().all(|reading|overrides::same_keys(reading,&neighbor.keys)))
    })
}
pub(super) fn append(word:&str, splits:&crate::components::ReadingSplits, read_as:&[Reading], context:overrides::Context<'_>, readings:&mut Vec<Vec<Vec<Key>>>) {
    if protected_parent(word) {return;}
    if !rules().runtime.component_readings.iter().chain(read_as).any(|rule|word.contains(&rule.khmer))
        && !rules().runtime.default_component_parts.iter().any(|part|word.contains(&part.khmer)) { return; }
    for mut path in component_paths(word,splits,read_as,context,0) {
        if !path.reused || readings.contains(&path.groups) { continue; }
        if let Some(first)=path.groups.first_mut() {
            let priority=FALLBACK_PRIORITY.saturating_add(i64::from(rules().runtime.word_rule_reuse.rank_priority));
            for key in first {key.priority=key.priority.saturating_add(priority);}
        }
        if !readings.contains(&path.groups) { readings.push(path.groups); }
    }
}

fn component_paths(word:&str,splits:&crate::components::ReadingSplits,read_as:&[Reading],
    context:overrides::Context<'_>,depth:usize)->Vec<Path> {
    if depth>=MAX_DEPTH {return Vec::new();}
    let Some(parts)=splits.parts(word) else {return Vec::new();};
    let configured=&rules().runtime.component_readings;
    let mut paths=vec![Path{groups:Vec::new(),reused:false}];
    for part in parts {
        let child=context.part(part.start,part.end);
        let choices=if let Some(rule)=read_as.iter().chain(configured).find(|rule|rule.khmer==part.text) {
            let keys = child.word_part_keys(&rule.khmer).unwrap_or(&rule.keys);
            vec![Path { groups: vec![keys.to_vec()], reused: true }]
        } else if rules().runtime.default_component_parts.iter().any(|default|default.khmer==part.text) {
            // This is only a boundary hint. Derive its keys from the live
            // sound map instead of keeping a duplicate spelling list.
            mapped_chunks_in_context(&part.text,false,child).into_iter()
                .map(|groups|Path{groups,reused:true}).collect()
        } else {
            let mut choices=Vec::new();
            // Keep the previous ordinary component path first. Nested reuse
            // supplements it; it never replaces a previously accepted path.
            if let Some(groups)=mapped_chunks_in_context(&part.text,false,child) {
                choices.push(Path{groups,reused:false});
            }
            // Each child is strictly shorter. Cache only segmentation because
            // reading scopes depend on the original outer identity/offsets.
            if overrides::Context::new(&part.text).whole(&part.text).is_none() {
                for nested in component_paths(&part.text,splits,read_as,child,depth+1) {
                    if !nested.reused {continue;}
                    if let Some(old)=choices.iter_mut().find(|old|old.groups==nested.groups) {
                        old.reused=true;
                    } else {choices.push(nested);}
                }
            }
            choices
        };
        let mut next=Vec::new();
        'paths: for prefix in &paths {
            for choice in &choices {
                if prefix.groups.len()+choice.groups.len()>128 || choice.groups.iter().any(Vec::is_empty) {continue;}
                let mut groups=prefix.groups.clone();groups.extend(choice.groups.clone());
                next.push(Path{groups,reused:prefix.reused||choice.reused});
                if next.len()==MAX_PATHS {break 'paths;}
            }
        }
        paths=next;
        if paths.is_empty() {break;}
    }
    paths
}
