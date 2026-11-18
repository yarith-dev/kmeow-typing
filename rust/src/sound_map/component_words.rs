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
pub(super) struct Reading { pub khmer: String, pub keys: read_as::Keys }
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
struct Path { groups:Vec<Vec<Key>>, reused:bool, ending:bool }

pub(super) fn protected_parent(word:&str)->bool {
    if overrides::Context::new(word).whole(word).is_some() {return true;}
    rules().runtime.word_readings.iter().any(|unit|unit.words.iter().any(|selected|selected==word)
        && (unit.checked_suffix_readings.iter().chain(&unit.checked_prefix_readings).any(|neighbor|neighbor.word==word)
            || unit.checked_context_readings.iter().any(|context|context.word==word)))
}
// Only a respelling actually used by the selected split can conflict with
// the checked parent. Matching text inside a different component is irrelevant.
pub(super) fn agrees_with_checked_neighbor(
    parent: &str, part: &str, readings: &[Vec<Vec<Key>>],
) -> bool {
    rules().runtime.word_readings.iter()
        .filter(|unit| unit.words.iter().any(|word| word == parent))
        .flat_map(|unit| unit.checked_suffix_readings.iter().chain(&unit.checked_prefix_readings))
        .any(|neighbor| neighbor.word == parent && neighbor.khmer == part
            && readings.iter().all(|reading| overrides::same_keys(reading, &neighbor.keys)))
}
// A complete component can use a scoped reading only at its original
// word and position. Checked pairs are matched as whole paths elsewhere.
pub(super) fn append_scoped_part(text: &str, context: overrides::Context<'_>, readings: &mut Vec<Vec<Vec<Key>>>) {
    if !context.original { return; }
    let original: Vec<_> = context.word.chars().collect();
    let chars: Vec<_> = text.chars().collect();
    if context.in_range(&chars, 0, chars.len()) { return; }
    for unit in &rules().runtime.word_readings {
        if unit.khmer != text
            || unit.checked_end(context.word, &original, context.offset) != Some(context.end)
            || unit.checked_suffix_readings.iter().chain(&unit.checked_prefix_readings)
                .any(|neighbor| neighbor.word == context.word)
            || unit.checked_context_readings.iter().any(|checked| checked.word == context.word)
            || has_shorthand_rule(text)
        {
            continue;
        }
        let missing: Vec<_> = unit.keys.iter().filter(|key| {
            !readings.iter().any(|reading| {
                let mut scratch = vec![0; (reading.len() + 1) * (key.len() + 1)];
                exact_recovery(reading, &key.text, 0, 0, &mut scratch)
            })
        }).cloned().collect();
        if !missing.is_empty() {
            readings.push(vec![missing]);
        }
    }
}

pub(super) fn append(word:&str, splits:&crate::components::ReadingSplits, read_as:&[Reading], context:overrides::Context<'_>, readings:&mut Vec<Vec<Vec<Key>>>) {
    if protected_parent(word) {return;}
    let has_reusable_rule=rules().runtime.component_readings.iter().chain(read_as).any(|rule|word.contains(&rule.khmer))
        || rules().runtime.default_component_parts.iter().any(|part|word.contains(&part.khmer));
    // Prepare the established paths first, with their original bounded budget.
    // New ending choices must never consume slots needed by an older path.
    for endings in [false,true] {
        if !endings && (!has_reusable_rule || !rules().runtime.word_rule_reuse.enabled) || endings && !rules().runtime.ending_alternatives.enabled {continue;}
        for mut path in component_paths(word,splits,read_as,context,0,endings) {
            if !path.reused || !rules().runtime.word_rule_reuse.enabled && !path.ending || readings.contains(&path.groups) { continue; }
            if let Some(first)=path.groups.first_mut() {
                let priority=FALLBACK_PRIORITY.saturating_add(i64::from(rules().runtime.word_rule_reuse.rank_priority));
                for key in first {key.priority=key.priority.saturating_add(priority);}
            }
            if !readings.contains(&path.groups) { readings.push(path.groups); }
        }
    }
}

fn component_paths(word:&str,splits:&crate::components::ReadingSplits,read_as:&[Reading],
    context:overrides::Context<'_>,depth:usize,endings:bool)->Vec<Path> {
    if depth>=MAX_DEPTH {return Vec::new();}
    let Some(parts)=splits.parts(word) else {return Vec::new();};
    let chars:Vec<_>=word.chars().collect();
    let cuts:Vec<_>=parts.iter().skip(1).map(|part|part.start).collect();
    let ending_boundaries_valid=!context.with_original(overrides::original_view(word)).crosses_reviewed_part(&chars,&cuts);
    if endings && !ending_boundaries_valid {return Vec::new();}
    let mut ending_parts=Vec::new();
    let configured=&rules().runtime.component_readings;
    let mut paths=vec![Path{groups:Vec::new(),reused:false,ending:false}];
    for part in parts {
        let mut child = context.part(part.start, part.end);
        if splits.is_standalone(&part.text) {
            child = child.with_stock_pronunciations();
        }
        let mut choices=if let Some(rule)=read_as.iter().chain(configured).find(|rule|rule.khmer==part.text) {
            let keys = child.word_part_keys(&rule.khmer).unwrap_or(&rule.keys);
            vec![Path { groups: vec![keys.to_vec()], reused: true, ending:false }]
        } else if rules().runtime.default_component_parts.iter().any(|default|default.khmer==part.text) {
            // This is only a boundary hint. Derive its keys from the live
            // sound map instead of keeping a duplicate spelling list.
            mapped_chunks_in_context(&part.text,false,child).into_iter()
                .map(|groups|Path{groups,reused:true,ending:false}).collect()
        } else {
            let mut choices=Vec::new();
            // Keep the previous ordinary component path first. Nested reuse
            // supplements it; it never replaces a previously accepted path.
            if let Some(groups)=mapped_chunks_in_context(&part.text,false,child) {
                choices.push(Path{groups,reused:false,ending:false});
            }
            // Each child is strictly shorter. Cache only segmentation because
            // reading scopes depend on the original outer identity/offsets.
            if overrides::Context::new(&part.text).whole(&part.text).is_none() {
                let nested=component_paths(&part.text,splits,read_as,child,depth+1,false).into_iter()
                    .chain(endings.then(||component_paths(&part.text,splits,read_as,child,depth+1,true)).into_iter().flatten());
                for nested in nested {
                    if !nested.reused {continue;}
                    if let Some(old)=choices.iter_mut().find(|old|old.groups==nested.groups) {
                        old.reused=true;old.ending|=nested.ending;
                    } else {choices.push(nested);}
                }
            }
            choices
        };
        if endings && ending_boundaries_valid {
            let original:Vec<_>=choices.iter().map(|path|path.groups.clone()).collect();
            choices.extend(ending_alternatives::alternatives(&original).into_iter()
                .map(|groups|Path{groups,reused:true,ending:true}));
        }
        if endings {ending_parts.push(choices);continue;}
        let mut next=Vec::new();
        'paths: for prefix in &paths {
            for choice in &choices {
                if prefix.groups.len()+choice.groups.len()>128 || choice.groups.iter().any(Vec::is_empty) {continue;}
                let mut groups=prefix.groups.clone();groups.extend(choice.groups.clone());
                next.push(Path{groups,reused:prefix.reused||choice.reused,ending:prefix.ending||choice.ending});
                if next.len()==MAX_PATHS {break 'paths;}
            }
        }
        paths=next;
        if paths.is_empty() {break;}
    }
    if !endings {return paths;}
    // Choose the first changed component. Later components keep both choices,
    // so all combinations remain a lattice rather than consuming MAX_PATHS.
    // Requiring that first change prevents any new non-x component reading.
    let mut additions=Vec::new();
    for first in 0..ending_parts.len() {
        if !ending_parts[first].iter().any(|path|path.ending) {continue;}
        let mut groups=Vec::new();
        for (index,choices) in ending_parts.iter().enumerate() {
            let choices:Vec<_>=choices.iter().filter(|path|
                if index<first {!path.ending} else if index==first {path.ending} else {true})
                .map(|path|path.groups.clone()).collect();
            if choices.is_empty() {groups.clear();break;}
            groups.push(vec![Key::factored(choices)]);
        }
        if !groups.is_empty() {additions.push(groups);}
    }
    if additions.is_empty() {Vec::new()}
    else {vec![Path{groups:vec![vec![Key::factored(additions)]],reused:true,ending:true}]}
}
