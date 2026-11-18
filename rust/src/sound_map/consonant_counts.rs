//! Single/double consonants are alternate inputs for generated readings only.
//! Keep the original lattice and its preferred witness; never expand all
//! combinations, rewrite aliases, or normalize vowels and apostrophes.
use super::{Key, Match};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

// Below every configured i32 path total for a bounded 128-character word.
// Existing exact readings and saved spellings keep their ranking precedence.
pub(super) const FALLBACK_PRIORITY: i64 = -(1 << 50);

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Settings {
    enabled: bool,
    letters: String,
}
impl Settings {
    pub(super) fn validate(&self) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        if self.enabled && self.letters.is_empty() || !self.letters.bytes().all(|letter|
            b"bcdfghjklmnpqrstvwxz".contains(&letter) && seen.insert(letter)) {
            return Err("Consonant count tolerance requires unique lowercase consonants; vowels and y stay literal.".into());
        }
        Ok(())
    }
    pub(super) fn normalizer(&self) -> Option<Normalizer> {
        self.enabled.then(|| Normalizer {mask:self.letters.bytes().fold(0,|mask,b|mask | (1 << (b - b'a')))})
    }
}

#[derive(Clone, Copy)]
pub(super) struct Normalizer { mask: u32 }
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct State { last: u8, count: u8 }
impl Normalizer {
    pub(super) fn tolerates(self, byte:u8)->bool {
        byte.is_ascii_lowercase() && self.mask & (1 << (byte - b'a')) != 0
    }
    pub(super) fn piece(self, text: &str, mut state: State) -> Option<(String, State)> {
        let mut result = String::new();
        for byte in text.bytes() {
            if self.tolerates(byte) {
                if byte == state.last {
                    if state.count == 2 { return None; }
                    state.count += 1;
                    continue;
                }
                state = State {last:byte,count:1};
            } else { state = State::default(); }
            result.push(char::from(byte));
        }
        Some((result,state))
    }
    pub(super) fn input(self, input: &str) -> Option<String> {
        self.piece(input,State::default()).map(|(text,_)|text)
    }
    pub(super) fn collect_prefixes(self, chunks: &[Vec<Key>], output: &mut BTreeSet<String>) {
        self.prefixes(chunks,0,String::new(),State::default(),output,&mut BTreeSet::new());
    }
    fn prefixes(self, chunks: &[Vec<Key>], unit: usize, prefix: String, state: State,
        output: &mut BTreeSet<String>, visited: &mut BTreeSet<(usize,String,State)>) {
        if unit == chunks.len() || prefix.len() == 3 || !visited.insert((unit,prefix.clone(),state)) { return; }
        for key in &chunks[unit] {
            let Some((key,next)) = self.piece(&key.text,state) else { continue; };
            let mut prefix = prefix.clone();
            for byte in key.bytes().take(3 - prefix.len()) {
                prefix.push(char::from(byte));
                output.insert(prefix.clone());
            }
            self.prefixes(chunks,unit + 1,prefix,next,output,visited);
        }
    }
    /// `input` is normalized once per query. Carry run length across chunks,
    /// and memoize suffixes so independent choices never form an expanded list.
    pub(super) fn match_chunks(self, chunks: &[Vec<Key>], input: &str) -> Option<Match> {
        let mut ceilings = vec![0i64;chunks.len() + 1];
        for i in (0..chunks.len()).rev() {
            ceilings[i] = ceilings[i + 1].saturating_add(chunks[i].iter().map(|key|key.priority).max().unwrap_or(0));
        }
        self.suffix(chunks,input,0,0,State::default(),&ceilings,&mut BTreeMap::new())
    }
    pub(super) fn next_mask(self, chunks: &[Vec<Key>], input: &str, max_stage: u8) -> u32 {
        self.mask_suffix(chunks,input,0,0,State::default(),max_stage,&mut BTreeMap::new())
    }
    fn mask_suffix(self, chunks: &[Vec<Key>], input: &str, unit: usize, offset: usize, state: State, max_stage: u8,
        memo: &mut BTreeMap<(usize,usize,State),u32>) -> u32 {
        if unit == chunks.len() { return if offset == input.len() {crate::next_letters::STOP} else {0}; }
        let memo_key = (unit,offset,state);
        if let Some(&mask) = memo.get(&memo_key) { return mask; }
        let rest = &input[offset..];
        let mut mask = 0;
        for key in &chunks[unit] {
            if key.prediction_stage > max_stage { continue; }
            let Some((text,next)) = self.piece(&key.text,state) else { continue; };
            if rest.starts_with(&text) {
                mask |= self.mask_suffix(chunks,input,unit + 1,offset + text.len(),next,max_stage,memo);
            } else if text.starts_with(rest) {
                mask |= crate::next_letters::letter_bit(text.as_bytes()[rest.len()]);
            }
        }
        memo.insert(memo_key,mask);
        mask
    }
    fn suffix(self, chunks: &[Vec<Key>], input: &str, unit: usize, offset: usize, state: State,
        ceilings: &[i64], memo: &mut BTreeMap<(usize,usize,State),Option<Match>>) -> Option<Match> {
        if unit == chunks.len() {
            return (offset == input.len()).then(||Match {exact:true,key:String::new(),priority:0});
        }
        let memo_key = (unit,offset,state);
        if let Some(hit) = memo.get(&memo_key) { return hit.clone(); }
        let mut best: Option<Match> = None;
        let rest = &input[offset..];
        for key in &chunks[unit] {
            let Some((text,next)) = self.piece(&key.text,state) else { continue; };
            let suffix = if rest.starts_with(&text) {
                self.suffix(chunks,input,unit + 1,offset + text.len(),next,ceilings,memo)
            } else if text.starts_with(rest) {
                self.suffix(chunks,input,unit + 1,input.len(),next,ceilings,memo)
                    .map(|mut hit| {hit.exact = false;hit})
            } else { None };
            if let Some(hit) = suffix {
                let hit = Match {exact:hit.exact,key:format!("{}{}",key.text,hit.key),
                    priority:key.priority.saturating_add(hit.priority)};
                if best.as_ref().is_none_or(|old|hit.better_than(old)) { best = Some(hit); }
                if best.as_ref().is_some_and(|hit|hit.exact && hit.priority == ceilings[unit]) { break; }
            }
        }
        memo.insert(memo_key,best.clone());
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn normalizer() -> Normalizer {
        Settings {enabled:true,letters:"bcdfghjklmnprstvw".into()}.normalizer().unwrap()
    }
    fn chunks(parts: &[&str]) -> Vec<Vec<Key>> { parts.iter().map(|part|vec![Key::from(*part)]).collect() }
    #[test]
    fn counts_cross_chunks_without_changing_preferred_keys_or_vowels() {
        let normalizer = normalizer();
        for (parts,input,preferred) in [
            (vec!["l","er","th","i"],"lertthi","lerthi"),
            (vec!["v","ae","k","k","i","kor"],"vaekikor","vaekkikor"),
            (vec!["s","a","b","b","ay"],"sabay","sabbay"),
        ] {
            let hit = normalizer.match_chunks(&chunks(&parts),&normalizer.input(input).unwrap()).unwrap();
            assert!(hit.exact);assert_eq!(hit.key,preferred);
        }
        assert!(normalizer.input("lerttthi").is_none());
        assert!(normalizer.match_chunks(&chunks(&["t","t","t"]),"t").is_none());
        for input in ["lerrthi","lirthi","vaakikor","vokikor","sabbaay","sabbayy","sabb'ay"] {
            assert!(normalizer.match_chunks(&chunks(&["sabbay"]),&normalizer.input(input).unwrap()).is_none(),"{input}");
        }
        assert!(normalizer.match_chunks(&chunks(&["lerthi"]),"lerrthi").is_none()); // caller normalizes input
    }
    #[test]
    fn prefixes_ranking_and_large_lattices_stay_bounded() {
        let normalizer = normalizer();
        let mut prefixes = BTreeSet::new();
        normalizer.collect_prefixes(&chunks(&["l","l","erthi"]),&mut prefixes);
        assert_eq!(prefixes,BTreeSet::from(["l".into(),"le".into(),"ler".into()]));
        let reading = vec![vec![Key {text:"lertthi".into(),priority:-2,preserve_existing:false,prediction_stage:0,paths:None},Key::from("lerthi")]];
        let hit = normalizer.match_chunks(&reading,"lerthi").unwrap();
        assert_eq!(hit.key,"lerthi");assert_eq!(hit.priority,0);
        let reading = vec![vec![Key::from("bba"),Key::from("kka")];60];
        let hit = normalizer.match_chunks(&reading,"baka").unwrap();
        assert!(!hit.exact);assert!(hit.key.starts_with("bbakka"));
        assert!(normalizer.match_chunks(&reading,"bako").is_none());
    }
    #[test]
    fn settings_reject_vowels_duplicates_and_unknown_fields() {
        for letters in ["", "aa", "ab", "yy", "bb", "K", "'"] {
            assert!(Settings {enabled:true,letters:letters.into()}.validate().is_err());
        }
        assert!(Settings::default().validate().is_ok());
        assert!(serde_json::from_str::<Settings>(r#"{"enabled":true,"letters":"b","other":true}"#).is_err());
    }
}
