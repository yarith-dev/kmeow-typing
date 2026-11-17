//! Sparse component trie read directly from the compact dictionary bytes.
//! Only the build step materializes nodes; typing keeps at most 63 active states.
use crate::{compact, invalid, EngineError};
use std::{collections::{BTreeMap, BTreeSet}, sync::Arc};

pub(crate) type Score = (usize, bool);
pub(crate) type Scores = BTreeMap<String, Score>;
pub(crate) fn score(scores: Option<&Scores>, word: &str) -> Score {
    scores.and_then(|scores| scores.get(word)).copied().unwrap_or_default()
}

/// Prepared from complete, unique component analyses. Edges share prefixes;
/// querying follows at most 63 history states, without running the detector.
#[derive(Default)]
pub(crate) struct Continuations { nodes: Vec<ContinuationNode> }
#[derive(Default)]
struct ContinuationNode { children: BTreeMap<String,(usize,Score)> }
impl Continuations {
    pub(crate) fn insert(&mut self, parts: &[String], saved: bool) {
        if !(2..=MAX_WORDS + 1).contains(&parts.len()) { return; }
        if self.nodes.is_empty() { self.nodes.push(Default::default()); }
        let mut node = 0;
        for (i,part) in parts.iter().enumerate() {
            // Automatic evidence is weaker than every explicit context path,
            // but stronger than no evidence. It never changes rule priorities.
            let score = if i == 0 {(0,false)} else if saved {(i,i + 1 == parts.len())} else {(0,true)};
            let child = if let Some((child,old)) = self.nodes[node].children.get_mut(part) {
                *old = (*old).max(score);*child
            } else {
                let child = self.nodes.len();
                self.nodes.push(Default::default());
                self.nodes[node].children.insert(part.clone(),(child,score));child
            };
            node = child;
        }
    }
    pub(crate) fn scores(&self, previous: &[String]) -> Scores {
        let mut scores = Scores::new();
        if self.nodes.is_empty() { return scores; }
        let mut states = Vec::new();
        for word in previous.iter().skip(previous.len().saturating_sub(MAX_WORDS)) {
            let mut next = Vec::new();
            for node in states.into_iter().chain(std::iter::once(0)) {
                if let Some((child,_)) = self.nodes[node].children.get(word) { next.push(*child); }
            }
            states = next;
        }
        for node in states {
            for (word,(_,score)) in &self.nodes[node].children {
                scores.entry(word.clone()).and_modify(|old|*old = (*old).max(*score)).or_insert(*score);
            }
        }
        scores
    }
}

/// Reviewed starting word rules supply a single complete lexical continuation.
/// This derived index is runtime-only; it creates no saved component approvals.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RuleIndex {
    continuations: BTreeMap<String, BTreeSet<(String,String)>>,
}
impl RuleIndex {
    pub(crate) fn insert(&mut self, parent: &str, previous: &str, next: String) {
        self.continuations.entry(previous.to_owned()).or_default().insert((parent.to_owned(),next));
    }
    pub(crate) fn extend_scores(&self, previous: &[String], scores: &mut Scores, authoritative: impl Fn(&str)->bool) {
        if let Some(next) = previous.last().and_then(|word| self.continuations.get(word)) {
            for (parent,word) in next {
                if authoritative(parent) { continue; }
                let score = scores.entry(word.clone()).or_default();
                *score = (*score).max((1, true));
            }
        }
    }
}

const NONE: u32 = u32::MAX;
pub(crate) const MAX_WORDS: usize = 63;
fn number(data: &[u8], at: usize) -> Result<u32, EngineError> {
    let bytes = data
        .get(
            at..at
                .checked_add(4)
                .ok_or_else(|| invalid("Context offset overflow."))?,
        )
        .ok_or_else(|| invalid("Truncated context index."))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}
fn put(bytes: &mut Vec<u8>, value: usize) -> Result<(), EngineError> {
    bytes.extend(
        u32::try_from(value)
            .map_err(|_| invalid("Context index exceeds 4 GiB."))?
            .to_le_bytes(),
    );
    Ok(())
}
#[derive(Default)]
struct Node {
    terminal: Option<usize>,
    children: BTreeMap<usize, usize>,
}
pub(crate) fn compile(sequences: &[(usize, Vec<usize>)]) -> Result<Vec<u8>, EngineError> {
    let mut nodes = vec![Node::default()];
    for (parent, components) in sequences {
        let mut node = 0;
        for word in components {
            node = if let Some(child) = nodes[node].children.get(word) {
                *child
            } else {
                let child = nodes.len();
                nodes.push(Node::default());
                nodes[node].children.insert(*word, child);
                child
            };
        }
        nodes[node].terminal = Some(*parent);
    }
    let mut records = Vec::new();
    let mut offsets = vec![0];
    for node in nodes {
        if let Some(terminal) = node.terminal {
            put(&mut records, terminal)?;
        } else {
            records.extend(NONE.to_le_bytes());
        }
        for (word, child) in node.children {
            put(&mut records, word)?;
            put(&mut records, child)?;
        }
        offsets.push(records.len());
    }
    let mut bytes = Vec::new();
    put(&mut bytes, offsets.len() - 1)?;
    for offset in offsets {
        put(&mut bytes, offset)?;
    }
    bytes.extend(records);
    Ok(bytes)
}
#[derive(Clone)]
pub(crate) struct Index {
    data: Arc<Vec<u8>>,
    start: usize,
    records: usize,
}
impl Index {
    fn bounds(&self, node: usize) -> (usize, usize) {
        let start = number(&self.data, self.start + 4 + node * 4).unwrap() as usize;
        let end = number(&self.data, self.start + 8 + node * 4).unwrap() as usize;
        (self.records + start, self.records + end)
    }
    fn terminal(&self, node: usize) -> u32 {
        number(&self.data, self.bounds(node).0).unwrap()
    }
    pub(crate) fn parse(
        data: Arc<Vec<u8>>,
        start: usize,
        len: usize,
        pool: &compact::Pool,
        words: &compact::Index,
    ) -> Result<Self, EngineError> {
        let section = data
            .get(
                start
                    ..start
                        .checked_add(len)
                        .ok_or_else(|| invalid("Context section overflow."))?,
            )
            .ok_or_else(|| invalid("Truncated context section."))?;
        let count = number(section, 0)? as usize;
        // Every node requires an offset and a terminal field, plus the final offset.
        if count < 3 || count > len.saturating_sub(8) / 8 {
            return Err(invalid("Invalid context node count."));
        }
        let records = start + 4 + (count + 1) * 4;
        let index = Self {
            data,
            start,
            records,
        };
        let payload_len = start + len - records;
        if number(&index.data, start + 4)? != 0
            || number(&index.data, start + 4 + count * 4)? as usize != payload_len
        {
            return Err(invalid("Invalid context directory."));
        }
        let mut seen = vec![0u8; count.div_ceil(8)];
        for node in 0..count {
            if number(&index.data, start + 4 + node * 4)? as usize > payload_len
                || number(&index.data, start + 8 + node * 4)? as usize > payload_len
            {
                return Err(invalid("Context offset exceeds section."));
            }
            let (begin, end) = index.bounds(node);
            if begin > end || end > start + len || end - begin < 4 || (end - begin - 4) % 8 != 0 {
                return Err(invalid("Invalid context record."));
            }
            let terminal = index.terminal(node);
            if terminal != NONE && terminal as usize >= pool.count() {
                return Err(invalid("Invalid context output."));
            }
            let mut previous = None;
            for at in (begin + 4..end).step_by(8) {
                let word = number(&index.data, at)?;
                let child = number(&index.data, at + 4)? as usize;
                if word as usize >= pool.count()
                    || previous.is_some_and(|p| p >= word)
                    || child <= node
                    || child >= count
                    || seen[child / 8] & (1 << (child % 8)) != 0
                {
                    return Err(invalid("Invalid context edge."));
                }
                if !words.contains(pool.get(word as u128)) {
                    return Err(invalid("Context component is not a word."));
                }
                seen[child / 8] |= 1 << (child % 8);
                previous = Some(word);
            }
        }
        if (1..count).any(|node| seen[node / 8] & (1 << (node % 8)) == 0) {
            return Err(invalid("Unreachable context node."));
        }
        index.validate_path(0, 0, &mut String::new(), pool, words)?;
        Ok(index)
    }
    fn validate_path(
        &self,
        node: usize,
        depth: usize,
        text: &mut String,
        pool: &compact::Pool,
        words: &compact::Index,
    ) -> Result<(), EngineError> {
        if depth > MAX_WORDS + 1 || text.chars().count() > 240 {
            return Err(invalid("Context sequence exceeds word limits."));
        }
        let terminal = self.terminal(node);
        let (begin, end) = self.bounds(node);
        if terminal != NONE
            && (depth < 2 || pool.get(terminal as u128) != text || !words.contains(text))
        {
            return Err(invalid("Context components do not form an approved word."));
        }
        if begin + 4 == end && terminal == NONE {
            return Err(invalid("Context path has no connected word."));
        }
        for at in (begin + 4..end).step_by(8) {
            let len = text.len();
            text.push_str(pool.get(number(&self.data, at)? as u128));
            self.validate_path(
                number(&self.data, at + 4)? as usize,
                depth + 1,
                text,
                pool,
                words,
            )?;
            text.truncate(len);
        }
        Ok(())
    }
    #[cfg(test)]
    fn next(&self, node: usize, word: usize) -> Option<usize> {
        let (begin, end) = self.bounds(node);
        let begin = begin + 4;
        let mut lo = 0;
        let mut hi = (end - begin) / 8;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if (number(&self.data, begin + mid * 8).unwrap() as usize) < word {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let at = begin + lo * 8;
        (at < end && number(&self.data, at).unwrap() as usize == word)
            .then(|| number(&self.data, at + 4).unwrap() as usize)
    }
    #[cfg(test)]
    pub(crate) fn states(&self, previous: &[String], pool: &compact::Pool) -> Vec<(usize, usize)> {
        let mut states = Vec::new();
        for text in previous {
            let Some(word) = pool.find(text) else {
                states.clear();
                continue;
            };
            let mut next = Vec::new();
            for (node, depth) in states {
                if let Some(child) = self.next(node, word) {
                    next.push((child, depth + 1));
                }
            }
            if let Some(child) = self.next(0, word) {
                next.push((child, 1));
            }
            states = next;
        }
        states
    }
    #[cfg(test)]
    pub(crate) fn score(
        &self,
        states: &[(usize, usize)],
        text: &str,
        pool: &compact::Pool,
    ) -> (usize, bool) {
        let Some(word) = pool.find(text) else {
            return (0, false);
        };
        states
            .iter()
            .filter_map(|(node, depth)| {
                self.next(*node, word)
                    .map(|child| (*depth, self.terminal(child) != NONE))
            })
            .max()
            .unwrap_or_default()
    }
    pub(crate) fn definitions(&self, pool: &compact::Pool) -> BTreeMap<String,Vec<Vec<String>>> {
        let mut definitions: BTreeMap<String,Vec<Vec<String>>> = BTreeMap::new();
        let mut pending = vec![(0,Vec::new())];
        while let Some((node,path)) = pending.pop() {
            let parent = self.terminal(node);
            if parent != NONE { definitions.entry(pool.get(parent as u128).to_owned()).or_default().push(path.clone()); }
            let (begin,end) = self.bounds(node);
            for at in (begin + 4..end).step_by(8) {
                let mut next = path.clone();
                next.push(pool.get(number(&self.data,at).unwrap() as u128).to_owned());
                pending.push((number(&self.data,at + 4).unwrap() as usize,next));
            }
        }
        for alternatives in definitions.values_mut() { alternatives.sort(); alternatives.dedup(); }
        definitions
    }
}
