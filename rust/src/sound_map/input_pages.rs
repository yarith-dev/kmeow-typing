//! Page through a word's existing reading lattices without expanding every combination.
use super::{Index, Key};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct Node {
    edges: Vec<(u8, usize)>,
    empty: Vec<usize>,
    terminal: bool,
}

struct Graph {
    nodes: Vec<Node>,
    minimum: Vec<usize>,
}

impl Graph {
    fn new(readings: &[Vec<Vec<Key>>]) -> Self {
        let mut graph = Self { nodes: vec![Node::default()], minimum: Vec::new() };
        graph.paths(0,None,readings);
        graph.minimum = vec![usize::MAX; graph.nodes.len()];
        graph.minimum_remaining();
        graph
    }

    fn paths(&mut self,from:usize,to:Option<usize>,readings:&[Vec<Vec<Key>>]) {
        for reading in readings {
            let mut start = self.node();
            self.nodes[from].empty.push(start);
            for chunk in reading {
                let end = self.node();
                for key in chunk {
                    if let Some(paths)=&key.paths {self.paths(start,Some(end),paths);continue;}
                    let mut current = start;
                    for byte in key.text.bytes() {
                        let next = self.node();
                        self.nodes[current].edges.push((byte, next));
                        current = next;
                    }
                    self.nodes[current].empty.push(end);
                }
                start = end;
            }
            if let Some(end)=to {self.nodes[start].empty.push(end);}
            else {self.nodes[start].terminal = true;}
        }
    }

    fn node(&mut self) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node::default());
        index
    }

    fn minimum_remaining(&mut self) {
        let mut incoming=vec![0;self.nodes.len()];
        for node in &self.nodes {for &next in &node.empty {incoming[next]+=1;}for &(_,next) in &node.edges {incoming[next]+=1;}}
        let mut ready:std::collections::VecDeque<_>=incoming.iter().enumerate().filter_map(|(i,&n)|(n==0).then_some(i)).collect();
        let mut order=Vec::new();
        while let Some(i)=ready.pop_front() {
            order.push(i);
            for next in self.nodes[i].empty.iter().copied().chain(self.nodes[i].edges.iter().map(|&(_,next)|next)) {
                incoming[next]-=1;if incoming[next]==0 {ready.push_back(next);}
            }
        }
        for i in order.into_iter().rev() {
            let mut length=if self.nodes[i].terminal {0}else{usize::MAX-1};
            for &next in &self.nodes[i].empty {length=length.min(self.minimum[next]);}
            for &(_,next) in &self.nodes[i].edges {length=length.min(self.minimum[next].saturating_add(1));}
            self.minimum[i]=length;
        }
    }

    fn closure(&self, states: &BTreeSet<usize>) -> BTreeSet<usize> {
        let mut found = states.clone();
        let mut pending: Vec<_> = states.iter().copied().collect();
        while let Some(index) = pending.pop() {
            for &next in &self.nodes[index].empty {
                if found.insert(next) { pending.push(next); }
            }
        }
        found
    }

    fn collect(&self, states: &BTreeSet<usize>, prefix: &mut String,
        after: &str, limit: usize, inputs: &mut Vec<String>) {
        if inputs.len() == limit { return; }
        // Skip whole lexical subtrees before the cursor, including on very large lattices.
        if prefix.as_str() < after && !after.starts_with(prefix.as_str()) { return; }
        let states = self.closure(states);
        if states.iter().all(|&index| self.minimum[index].saturating_add(prefix.len()) > 512) { return; }
        if !prefix.is_empty() && prefix.as_str() > after && states.iter().any(|&index| self.nodes[index].terminal) {
            inputs.push(prefix.clone());
            if inputs.len() == limit { return; }
        }
        if prefix.len() == 512 { return; }
        let mut edges: BTreeMap<u8, BTreeSet<usize>> = BTreeMap::new();
        for index in states {
            for &(byte, next) in &self.nodes[index].edges {
                if !byte.is_ascii_lowercase() && byte != b'\'' { continue; }
                if byte == b'\'' && (prefix.is_empty() || prefix.ends_with('\'')) { continue; }
                edges.entry(byte).or_default().insert(next);
            }
        }
        for (byte, next) in edges {
            prefix.push(char::from(byte));
            self.collect(&next, prefix, after, limit, inputs);
            prefix.pop();
            if inputs.len() == limit { break; }
        }
    }
}

pub(super) fn input_page(readings: &[Vec<Vec<Key>>], after: &str, limit: usize) -> (Vec<String>, Option<String>) {
    let graph = Graph::new(readings);
    let mut inputs = Vec::new();
    graph.collect(&BTreeSet::from([0]), &mut String::new(), after, limit + 1, &mut inputs);
    let next_after = if inputs.len() > limit {
        inputs.pop();
        inputs.last().cloned()
    } else { None };
    (inputs, next_after)
}

impl Index {
    pub(crate) fn input_page(&self, word: &str, after: &str, limit: usize) -> (Vec<String>, Option<String>) {
        let Ok(index) = self.words.binary_search_by(|entry| entry.candidate.khmer.as_str().cmp(word)) else {
            return (Vec::new(), None);
        };
        input_page(self.words[index].ordinary_readings(), after, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_sorted_unique_and_keep_independent_paths_separate() {
        let readings = vec![
            vec![vec![Key::from("a"), Key::from("ab")], vec![Key::from("b"), Key::from("")]],
            vec![vec![Key::from("ab"), Key::from("z")]],
            vec![vec![Key::from("'bad"), Key::from("a''bad"), Key::from("l'ar")]],
        ];
        let (first, cursor) = input_page(&readings, "", 2);
        assert_eq!(first, ["a", "ab"]);
        let (second, cursor) = input_page(&readings, cursor.as_deref().unwrap(), 2);
        assert_eq!(second, ["abb", "l'ar"]);
        assert_eq!(input_page(&readings, cursor.as_deref().unwrap(), 2), (vec!["z".into()], None));
    }

    #[test]
    fn large_lattices_can_jump_to_the_last_page_without_expanding_earlier_inputs() {
        let readings = vec![vec![vec![Key::from("a"), Key::from("b")]; 32]];
        let (first, next) = input_page(&readings, "", 64);
        assert_eq!(first.len(), 64);
        assert_eq!(next, first.last().cloned());
        let after = format!("{}a", "b".repeat(31));
        assert_eq!(input_page(&readings, &after, 64), (vec!["b".repeat(32)], None));
        assert_eq!(input_page(&readings, &"b".repeat(32), 64), (Vec::new(), None));
    }

    #[test]
    fn output_length_is_bounded_without_hiding_shorter_alternatives() {
        let readings = vec![vec![vec![Key::from("a")]; 513], vec![vec![Key::from("short")]]];
        assert_eq!(input_page(&readings, "", 64), (vec!["short".into()], None));
    }
}
