//! Alternatives at complete word/component ends, never arbitrary Latin
//! substrings. Take a right quotient of the reading lattice so suffixes can
//! cross parser chunks or large Read as choices without expanding products.
use super::*;

#[derive(Default, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub(super) struct Settings {
    pub enabled: bool,
    pub rank_priority: i32,
    pub pairs: Vec<Pair>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pair { ending: String, alternative: String }

pub(super) fn validate(settings: &Settings) -> Result<(), String> {
    if settings.enabled && (settings.rank_priority >= 0 || settings.pairs.len() != 2
        || ![("ech","ex"),("ich","ix")].iter().all(|&(ending,alternative)|
            settings.pairs.iter().filter(|pair|pair.ending==ending && pair.alternative==alternative).count()==1)) {
        return Err("Ending alternatives require ech → ex and ich → ix with a negative additive priority.".into());
    }
    Ok(())
}

pub(super) fn alternatives(readings: &[Vec<Vec<Key>>]) -> Vec<Vec<Vec<Key>>> {
    let settings=&rules().runtime.ending_alternatives;
    if !settings.enabled {return Vec::new();}
    let mut result=Vec::new();
    for pair in &settings.pairs {
        let prefixes:Vec<_>=readings.iter().filter_map(|reading|
            trim_path(reading,&pair.ending,&mut BTreeMap::new()).remove(&0)).collect();
        if prefixes.is_empty() {continue;}
        let mut marker=Key::additive_path(settings.rank_priority);
        marker.prediction_stage=ranking::LATEST_PREDICTION_STAGE;
        result.push(vec![vec![marker],prefixes,vec![Key::from(pair.alternative.clone())]]);
    }
    result
}
pub(super) fn append(readings: &mut Vec<Vec<Vec<Key>>>) {
    for path in alternatives(readings) {if !readings.contains(&path) {readings.push(path);}}
}

fn factor(paths: Vec<Vec<Vec<Key>>>) -> Key {
    if paths.len()==1 && paths[0].len()==1 && paths[0][0].len()==1 {
        return paths[0][0][0].clone();
    }
    Key::factored(paths)
}

// The key is the length of the suffix still to consume from earlier groups.
// Memoization bounds work by (group, suffix length), including empty markers.
fn trim_path(reading: &[Vec<Key>], suffix: &str,
    memo: &mut BTreeMap<(usize,usize),BTreeMap<usize,Key>>,
) -> BTreeMap<usize,Key> {
    let state=(reading.len(),suffix.len());
    if let Some(found)=memo.get(&state) {return found.clone();}
    let Some((last,prefix))=reading.split_last() else {
        return BTreeMap::from([(suffix.len(),Key::from(""))]);
    };
    let mut paths:BTreeMap<usize,Vec<Vec<Vec<Key>>>>=BTreeMap::new();
    for key in last {
        for (remaining,tail) in trim_key(key,suffix) {
            if remaining==0 {
                let mut groups=prefix.to_vec();groups.push(vec![tail]);
                paths.entry(0).or_default().push(groups);
            } else {
                for (remaining,head) in trim_path(prefix,&suffix[..remaining],memo) {
                    paths.entry(remaining).or_default().push(vec![vec![head],vec![tail.clone()]]);
                }
            }
        }
    }
    let result:BTreeMap<_,_>=paths.into_iter().map(|(remaining,paths)|(remaining,factor(paths))).collect();
    memo.insert(state,result.clone());result
}
fn trim_key(key: &Key, suffix: &str) -> BTreeMap<usize,Key> {
    if suffix.is_empty() {return BTreeMap::from([(0,key.clone())]);}
    if let Some(paths)=&key.paths {
        let mut choices:BTreeMap<usize,Vec<Vec<Vec<Key>>>>=BTreeMap::new();
        for path in paths.iter() {
            for (remaining,mut prefix) in trim_path(path,suffix,&mut BTreeMap::new()) {
                prefix.priority=prefix.priority.saturating_add(key.priority);
                prefix.preserve_existing|=key.preserve_existing;
                prefix.prediction_stage=prefix.prediction_stage.max(key.prediction_stage);
                choices.entry(remaining).or_default().push(vec![vec![prefix]]);
            }
        }
        return choices.into_iter().map(|(remaining,paths)|(remaining,factor(paths))).collect();
    }
    let count=key.text.len().min(suffix.len());
    if key.text[key.text.len()-count..]!=suffix[suffix.len()-count..] {return BTreeMap::new();}
    let mut prefix=key.clone();prefix.text.truncate(prefix.text.len()-count);
    BTreeMap::from([(suffix.len()-count,prefix)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;
    #[test]
    fn saved_standalone_blocks_internal_endings_and_component_edits_refresh_the_cache() {
        let words=["តិច","ខា","តិចខា"];
        let mut core=Engine::from_json(&serde_json::json!({"schemaVersion":2,"mappings":[],
            "words":words.iter().enumerate().map(|(i,word)|serde_json::json!({"id":i+1,"khmer":word,"rank":0,"status":"approved"})).collect::<Vec<_>>()
        }).to_string()).unwrap();
        let bytes=core.to_binary();
        for (components,expected) in [(serde_json::json!([]),false),(serde_json::json!([["តិច","ខា"]]),true)] {
            core.load_word_components(&serde_json::json!({"wordComponents":{"format":"kmeow-word-components-v1",
                "words":[{"word":"តិចខា","components":components}]}})).unwrap();
            assert_eq!(core.suggest_with_context("tixkha",Default::default(),&Default::default()).unwrap()
                .exact_candidates.iter().any(|row|row.khmer=="តិចខា"),expected);
            assert_eq!(core.to_binary(),bytes);
        }
    }
    #[test]
    fn suffix_transform_crosses_chunks_and_factored_choices_without_mixing_paths() {
        let paths=vec![vec![vec![Key::from("k")],vec![Key::from("e"),Key::from("i")],vec![Key::from("c")],vec![Key::from("h")]],
            vec![vec![Key::from("doch")]],vec![vec![Key::from("echka")]]];
        for source in [paths.clone(),vec![vec![vec![Key::factored(paths)]]]] {
            let additions=alternatives(&source);let graph=factored::Graph::new(&additions);
            for input in ["kex","kix"] {assert!(graph.matching(input,None,false).is_some_and(|hit|hit.exact),"{input}");}
            for input in ["dox","exka","kech","kich"] {assert!(graph.matching(input,None,false).is_none(),"{input}");}
            assert_eq!(graph.next_mask("ke",None,3),0);
            assert_eq!(graph.next_mask("ke",None,4),crate::next_letters::letter_bit(b'x'));
        }
        let huge=vec![vec![vec![Key::factored(vec![vec![vec![Key::from("a"),Key::from("b")];60]])],vec![Key::from("ech")]]];
        let graph=factored::Graph::new(&alternatives(&huge));
        assert!(graph.matching(&format!("{}ex","ab".repeat(30)),None,false).is_some_and(|hit|hit.exact));
    }
}
