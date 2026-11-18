//! Bounded graph for Read as choices whose added spellings exceed the flat-key
//! budget. Nodes follow parser boundaries; no full spelling product is built.
use super::{Key, Match, consonant_counts::{Normalizer, State}};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Default)]
struct Node { edges: Vec<Edge>, terminal: bool }
struct Edge {
    text: String, next: usize, priority: i64,
    preserve_existing: bool, prediction_stage: u8,
    choice: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound_map::{match_chunks, rules};

    #[test]
    fn optional_apostrophes_compose_with_counts_without_erasing_run_boundaries() {
        let graph=Graph::new(&[vec![vec![Key::from("b")],vec![Key::from("b")],vec![Key::from("'")],vec![Key::from("bba")]]]);
        let n=rules().runtime.consonant_count_tolerance.normalizer().unwrap();
        for input in ["bba","bbba","bbbba"] {assert!(graph.matching_optional_apostrophes(input,Some(n)).is_some(),"{input}");}
        for input in ["ba","bbbbba","bbbaa"] {assert!(graph.matching_optional_apostrophes(input,Some(n)).is_none(),"{input}");}
        assert!(graph.matching_optional_apostrophes("bba",None).is_none());
        let mut prefixes=BTreeSet::new();graph.optional_apostrophe_prefixes(Some(n),&mut prefixes);
        assert!(prefixes.contains("bbb"));assert!(prefixes.iter().all(|prefix|prefix.len()<=3));
    }

    #[test]
    fn compact_graph_matches_flat_lattices_for_prefixes_counts_stages_and_priority() {
        let mut vowel=Key::from("ou");vowel.prediction_stage=1;vowel.priority=-100;
        let mut onset=Key::from("t");onset.prediction_stage=2;onset.priority=-200;onset.preserve_existing=true;
        let paths=vec![vec![vec![Key::from("p'")],vec![Key::from("o"),vowel],vec![Key::from("th"),onset],vec![Key::from("lai"),Key::from("lay")]],
            vec![vec![Key::from("pp'o")],vec![Key::from("n")]]];
        let graph=Graph::new(&[vec![vec![Key::factored(paths.clone())]]]);
        let normalizer=rules().runtime.consonant_count_tolerance.normalizer();
        let inputs=["p'othlai","p'othlay","p'oulai","p'outlai","p'outhlai","pp'on","ppp'on","p'otlaix"];
        for full in inputs {for end in 0..=full.len() {
            let input=&full[..end];
            let mut expected:Option<Match>=None;
            for reading in &paths {if let Some(hit)=match_chunks(reading,0,input,&mut BTreeMap::new()) {if expected.as_ref().is_none_or(|old|hit.better_than(old)) {expected=Some(hit);}}}
            assert_eq!(graph.matching(input,None,false),expected,"strict {input}");
            if let Some(n)=normalizer {if let Some(input)=n.input(input) {
                let mut expected:Option<Match>=None;
                for reading in &paths {if let Some(hit)=n.match_chunks(reading,&input) {if expected.as_ref().is_none_or(|old|hit.better_than(old)) {expected=Some(hit);}}}
                assert_eq!(graph.matching(&input,Some(n),false),expected,"counts {input}");
            }}
        }}
        assert_eq!(graph.next_mask("p'out",None,1),crate::next_letters::letter_bit(b'h'));
        assert_eq!(graph.next_mask("p'out",None,2),crate::next_letters::letter_bit(b'h')|crate::next_letters::letter_bit(b'l'));
        assert!(graph.matching("p'otlai",None,true).is_none());
        assert!(graph.contains(b"outl"));assert!(!graph.contains(b"ouon"));
        assert!(graph.nodes.len()<40);
    }
}
pub(super) struct Graph { nodes: Vec<Node>, order: Vec<usize> }

pub(super) fn has_paths(readings: &[Vec<Vec<Key>>]) -> bool {
    readings.iter().flatten().flatten().any(|key|key.paths.is_some())
}

impl Graph {
    /// A separate completion view keeps optional apostrophes out of exact
    /// lookup, saved spellings, spelling repair and touch prediction.
    pub(super) fn apostrophe_completions(readings:&[Vec<Vec<Key>>])->Option<Self> {
        fn contains(key:&Key)->bool {
            key.text.contains('\'') || key.paths.as_ref().is_some_and(|paths|
                paths.iter().flatten().flatten().any(contains))
        }
        readings.iter().flatten().flatten().any(contains).then(||Self::new(readings))
    }
    pub(super) fn new(readings: &[Vec<Vec<Key>>]) -> Self {
        let mut graph=Self {nodes:vec![Node::default(),Node {terminal:true,..Default::default()}],order:Vec::new()};
        graph.paths(0,1,readings);
        let mut incoming=vec![0;graph.nodes.len()];
        for node in &graph.nodes {for edge in &node.edges {incoming[edge.next]+=1;}}
        let mut ready:VecDeque<_>=incoming.iter().enumerate().filter_map(|(i,&count)|(count==0).then_some(i)).collect();
        while let Some(index)=ready.pop_front() {
            graph.order.push(index);
            for edge in &graph.nodes[index].edges {incoming[edge.next]-=1;if incoming[edge.next]==0 {ready.push_back(edge.next);}}
        }
        debug_assert_eq!(graph.order.len(),graph.nodes.len());
        graph
    }
    fn node(&mut self)->usize {let index=self.nodes.len();self.nodes.push(Node::default());index}
    fn paths(&mut self,start:usize,end:usize,paths:&[Vec<Vec<Key>>]) {
        for (choice,reading) in paths.iter().enumerate() {
            let mut current=self.node();
            self.nodes[start].edges.push(Edge {text:String::new(),next:current,priority:0,preserve_existing:false,prediction_stage:0,choice:Some(choice)});
            for group in reading {
                let next=self.node();
                for (choice,key) in group.iter().enumerate() {
                    let begin=self.node();
                    self.nodes[current].edges.push(Edge {text:String::new(),next:begin,priority:key.priority,
                        preserve_existing:key.preserve_existing,prediction_stage:key.prediction_stage,choice:Some(choice)});
                    if let Some(paths)=&key.paths {self.paths(begin,next,paths);}
                    else {self.nodes[begin].edges.push(Edge {text:key.text.clone(),next,priority:0,preserve_existing:false,prediction_stage:0,choice:None});}
                }
                current=next;
            }
            self.nodes[current].edges.push(Edge {text:String::new(),next:end,priority:0,preserve_existing:false,prediction_stage:0,choice:None});
        }
    }
    fn piece(edge:&Edge,state:State,normalizer:Option<Normalizer>)->Option<(String,State)> {
        normalizer.map_or_else(||Some((edge.text.clone(),state)),|n|n.piece(&edge.text,state))
    }
    pub(super) fn matching(&self,input:&str,normalizer:Option<Normalizer>,existing_only:bool)->Option<Match> {
        // Forward dynamic programming avoids recursive stacks even when many
        // independently customized components are joined in one word.
        let mut states:Vec<BTreeMap<(usize,State,bool),(i64,String,Vec<usize>)>>=(0..self.nodes.len()).map(|_|BTreeMap::new()).collect();
        states[0].insert((0,State::default(),false),(0,String::new(),Vec::new()));
        let mut best:Option<(Match,Vec<usize>)>=None;
        for &index in &self.order {
            for ((offset,state,completion),(priority,key,order)) in std::mem::take(&mut states[index]) {
                if self.nodes[index].terminal && offset==input.len() {
                    let hit=Match {exact:!completion,key:key.clone(),priority};
                    if best.as_ref().is_none_or(|(old,previous)|hit.better_than(old)
                        || hit.exact==old.exact && hit.priority==old.priority && order<*previous) {best=Some((hit,order.clone()));}
                }
                for edge in &self.nodes[index].edges {
                    if existing_only && edge.preserve_existing {continue;}
                    let Some((text,next_state))=Self::piece(edge,state,normalizer) else {continue;};
                    let rest=&input[offset..];
                    let (next_offset,completion)=if rest.starts_with(&text) {(offset+text.len(),completion)}
                        else if text.starts_with(rest) {(input.len(),true)} else {continue;};
                    let priority=priority.saturating_add(edge.priority);
                    let mut order=order.clone();if let Some(choice)=edge.choice {order.push(choice);}
                    let target=states[edge.next].entry((next_offset,next_state,completion));
                    match target {
                        std::collections::btree_map::Entry::Vacant(entry)=>{entry.insert((priority,format!("{key}{}",edge.text),order));},
                        std::collections::btree_map::Entry::Occupied(mut entry)=>{
                            if priority>entry.get().0 || priority==entry.get().0 && order<entry.get().2 {entry.insert((priority,format!("{key}{}",edge.text),order));}
                        }
                    }
                }
            }
        }
        best.map(|(hit,_)|hit)
    }
    pub(super) fn next_mask(&self,input:&str,normalizer:Option<Normalizer>,max_stage:u8)->u32 {
        let mut states:Vec<BTreeSet<(usize,State)>>=(0..self.nodes.len()).map(|_|BTreeSet::new()).collect();
        states[0].insert((0,State::default()));let mut mask=0;
        for &index in &self.order {
            for (offset,state) in std::mem::take(&mut states[index]) {
                if self.nodes[index].terminal && offset==input.len() {mask|=crate::next_letters::STOP;}
                for edge in &self.nodes[index].edges {
                    if edge.prediction_stage>max_stage {continue;}
                    let Some((text,next_state))=Self::piece(edge,state,normalizer) else {continue;};
                    let rest=&input[offset..];
                    if rest.starts_with(&text) {states[edge.next].insert((offset+text.len(),next_state));}
                    else if text.starts_with(rest) {mask|=crate::next_letters::letter_bit(text.as_bytes()[rest.len()]);}
                }
            }
        }
        mask
    }
    /// Match only by omitting apostrophes from a generated reading. Retain
    /// its canonical spelling as the completion witness, including any
    /// apostrophes the caller did type. Never form a Cartesian spelling list.
    pub(super) fn matching_optional_apostrophes(&self,input:&str,normalizer:Option<Normalizer>)->Option<Match> {
        let mut states:Vec<BTreeMap<(usize,State,bool,bool),(i64,String,Vec<usize>)>>=(0..self.nodes.len()).map(|_|BTreeMap::new()).collect();
        states[0].insert((0,State::default(),false,false),(0,String::new(),Vec::new()));
        let mut best:Option<(Match,Vec<usize>)>=None;
        for &index in &self.order {
            for ((offset,state,omitted,completion),(priority,key,order)) in std::mem::take(&mut states[index]) {
                if self.nodes[index].terminal && offset==input.len() && omitted {
                    let hit=Match {exact:!completion,key:key.clone(),priority};
                    if best.as_ref().is_none_or(|(old,previous)|hit.better_than(old)
                        || hit.exact==old.exact && hit.priority==old.priority && order<*previous) {best=Some((hit,order.clone()));}
                }
                for edge in &self.nodes[index].edges {
                    let mut positions=BTreeSet::from([(offset,state,omitted,completion)]);
                    for byte in edge.text.bytes() {
                        let mut next=BTreeSet::new();
                        for (at,state,omitted,completion) in positions {
                            // Normalize the source a byte at a time, keeping
                            // apostrophe boundaries even when they are omitted.
                            let Some((text,state))=normalizer.map_or_else(||Some((char::from(byte).to_string(),state)),
                                |n|n.piece(&char::from(byte).to_string(),state)) else {continue;};
                            if text.is_empty() {next.insert((at,state,omitted,completion));continue;}
                            if at==input.len() {next.insert((at,state,omitted,true));}
                            else {
                                if input.as_bytes()[at]==byte {
                                    next.insert((at+1,state,omitted,completion));
                                    if normalizer.is_some_and(|n|n.tolerates(byte)) && input.as_bytes().get(at+1)==Some(&byte) {
                                        next.insert((at+2,state,omitted,completion));
                                    }
                                }
                                if byte==b'\'' {next.insert((at,state,true,completion));}
                            }
                        }
                        positions=next;
                        if positions.is_empty() {break;}
                    }
                    let priority=priority.saturating_add(edge.priority);
                    let mut order=order.clone();if let Some(choice)=edge.choice {order.push(choice);}
                    for position in positions {
                        let target=states[edge.next].entry(position);
                        match target {
                            std::collections::btree_map::Entry::Vacant(entry)=>{entry.insert((priority,format!("{key}{}",edge.text),order.clone()));},
                            std::collections::btree_map::Entry::Occupied(mut entry)=>{
                                if priority>entry.get().0 || priority==entry.get().0 && order<entry.get().2 {entry.insert((priority,format!("{key}{}",edge.text),order.clone()));}
                            }
                        }
                    }
                }
            }
        }
        best.map(|(hit,_)|hit)
    }
    pub(super) fn optional_apostrophe_prefixes(&self,normalizer:Option<Normalizer>,out:&mut BTreeSet<String>) {
        let mut states:Vec<BTreeSet<(String,State)>>=(0..self.nodes.len()).map(|_|BTreeSet::new()).collect();
        states[0].insert((String::new(),State::default()));
        for &index in &self.order {
            for (prefix,state) in std::mem::take(&mut states[index]) {
                for edge in &self.nodes[index].edges {
                    let mut prefixes=BTreeSet::from([(prefix.clone(),state)]);
                    for byte in edge.text.bytes() {
                        let mut next=BTreeSet::new();
                        for (prefix,state) in prefixes {
                            let Some((text,state))=normalizer.map_or_else(||Some((char::from(byte).to_string(),state)),
                                |n|n.piece(&char::from(byte).to_string(),state)) else {continue;};
                            if text.is_empty() {next.insert((prefix,state));continue;}
                            if byte==b'\'' {next.insert((prefix.clone(),state));}
                            let mut prefix=prefix;
                            for _ in 0..if normalizer.is_some_and(|n|n.tolerates(byte)) {2}else{1} {
                                prefix.push(char::from(byte));out.insert(prefix.clone());
                                if prefix.len()==3 {break;}
                                next.insert((prefix.clone(),state));
                            }
                        }
                        prefixes=next;
                        if prefixes.is_empty() {break;}
                    }
                    states[edge.next].extend(prefixes);
                }
            }
        }
    }
    pub(super) fn prefixes(&self,normalizer:Option<Normalizer>,out:&mut BTreeSet<String>) {
        let mut states:Vec<BTreeSet<(String,State)>>=(0..self.nodes.len()).map(|_|BTreeSet::new()).collect();
        states[0].insert((String::new(),State::default()));
        for &index in &self.order {
            for (prefix,state) in std::mem::take(&mut states[index]) {
                for edge in &self.nodes[index].edges {
                    let Some((text,next_state))=Self::piece(edge,state,normalizer) else {continue;};
                    let mut prefix=prefix.clone();
                    for byte in text.bytes().take(3-prefix.len()) {prefix.push(char::from(byte));out.insert(prefix.clone());}
                    if prefix.len()<3 {states[edge.next].insert((prefix,next_state));}
                }
            }
        }
    }
    pub(super) fn contains(&self,query:&[u8])->bool {
        if query.is_empty() {return true;}
        let mut failure=vec![0;query.len()];
        for i in 1..query.len() {let mut j=failure[i-1];while j>0 && query[i]!=query[j] {j=failure[j-1];}if query[i]==query[j] {j+=1;}failure[i]=j;}
        let mut states:Vec<BTreeSet<usize>>=(0..self.nodes.len()).map(|_|BTreeSet::new()).collect();states[0].insert(0);
        for &index in &self.order {
            for state in std::mem::take(&mut states[index]) {for edge in &self.nodes[index].edges {
                let mut state=state;
                for byte in edge.text.bytes() {
                    while state>0 && byte!=query[state] {state=failure[state-1];}
                    if byte==query[state] {state+=1;}
                    if state==query.len() {return true;}
                }
                states[edge.next].insert(state);
            }}
        }
        false
    }
}
