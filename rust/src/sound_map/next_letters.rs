//! Match a prefix through each original reading lattice without expanding full
//! spellings. A state is (chunk, consumed bytes), so variants stay bounded.
use super::*;

impl Index {
    pub(crate) fn visit_next_letters(&self, input: &str, policy: Option<&compact::Dictionary>,
        mut visit: impl FnMut(&Candidate, u32)) {
        if let Some(prepared) = self.part_policy(policy) {
            return prepared.as_ref().unwrap().index.visit_next_letters(input,policy,visit);
        }
        let normalizer = rules().runtime.consonant_count_tolerance.normalizer();
        let normalized = normalizer.and_then(|n|n.input(input));
        for id in self.matching_ids(input,normalized.as_deref()) {
            let word = &self.words[id];
            if !Self::allows(word, policy) { continue; }
            let mut mask = 0;
            for reading in &word.readings {
                mask |= next_mask(reading, input.as_bytes(), 0, 0, &mut BTreeMap::new());
            }
            // Keep every original prior when the typed count already fits.
            // Only a missing canonical path borrows the alternate-count view.
            if mask == 0 {
                if let (Some(n),Some(normalized)) = (normalizer,normalized.as_deref()) {
                    for reading in &word.readings { mask |= n.next_mask(reading,normalized); }
                }
            }
            if mask != 0 { visit(&word.candidate, mask); }
        }
    }
}

fn next_mask(reading: &[Vec<Key>], input: &[u8], unit: usize, offset: usize,
    memo: &mut BTreeMap<(usize, usize), u32>) -> u32 {
    if unit == reading.len() { return if offset == input.len() { crate::next_letters::STOP } else { 0 }; }
    if let Some(&mask) = memo.get(&(unit, offset)) { return mask; }
    let mut mask = 0;
    let rest = &input[offset..];
    for key in &reading[unit] {
        let bytes = key.as_bytes();
        if rest.starts_with(bytes) {
            mask |= next_mask(reading, input, unit + 1, offset + bytes.len(), memo);
        } else if bytes.starts_with(rest) {
            mask |= crate::next_letters::letter_bit(bytes[rest.len()]);
        }
    }
    memo.insert((unit, offset), mask);
    mask
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_lattice_choices_are_visible_without_expanding_full_spellings() {
        let reading = vec![vec![Key::from("a"), Key::from("b")]; 60];
        assert_eq!(next_mask(&reading, b"abba", 0, 0, &mut BTreeMap::new()), 3);
        assert_eq!(next_mask(&reading, b"abc", 0, 0, &mut BTreeMap::new()), 0);
        let reading = vec![vec![Key::from("sur"), Key::from("sor")], vec![Key::from("sdey")]];
        assert_eq!(next_mask(&reading, b"su", 0, 0, &mut BTreeMap::new()), 1 << (b'r' - b'a'));
        assert_eq!(next_mask(&reading, b"sur", 0, 0, &mut BTreeMap::new()), 1 << (b's' - b'a'));
        assert_eq!(next_mask(&reading, b"sursdey", 0, 0, &mut BTreeMap::new()), crate::next_letters::STOP);
    }
}
