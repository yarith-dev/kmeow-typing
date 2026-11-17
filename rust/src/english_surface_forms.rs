//! Retain finite, fully typed inflections of an available approved lemma.
//! These are typing identities, never new lexical entries or prefix proposals.
use crate::{
    contains_profanity, Engine, EnglishCandidate, ProfanityLanguage, Suggestion,
    SuggestionCandidate,
};

// Original spelling rules with finite compatibility choices per approved base.
// The choices keep existing behavior closed: a grammatical possibility alone
// does not admit a new form. No source dictionary or expanded form table loads.
// Rule bits: +s, +es, y→ies, +ed, +d, y→ied, +ing, e→ing,
// double-final+ed, double-final+ing, ie→ying.
const POLICY: &str = include_str!("english_surface_forms/policy.tsv");
const IRREGULARS: &str = include_str!("english_surface_forms/irregulars.tsv");
const MAX_BASE_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Lemmas([Option<&'static str>; 2]);

impl Lemmas {
    fn insert(&mut self, base: &'static str) {
        if self.0.contains(&Some(base)) {
            return;
        }
        if let Some(slot) = self.0.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(base);
        } else {
            // The finite compatibility policy is checked to have at most two
            // bases per spelling. User input cannot add a third policy entry.
            debug_assert!(false, "Too many English inflection bases");
        }
    }
    fn iter(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.0.iter().flatten().copied()
    }
}

fn table_entry(table: &'static str, key: &str) -> Option<(&'static str, &'static str)> {
    let mut remaining = table;
    while !remaining.is_empty() {
        let middle = remaining.len() / 2;
        let start = remaining[..middle].rfind('\n').map_or(0, |i| i + 1);
        let end = remaining[middle..]
            .find('\n')
            .map_or(remaining.len(), |i| middle + i);
        let (entry, value) = remaining[start..end].split_once('\t')?;
        match key.cmp(entry) {
            std::cmp::Ordering::Equal => return Some((entry, value)),
            std::cmp::Ordering::Less => remaining = &remaining[..start],
            std::cmp::Ordering::Greater => {
                remaining = remaining.get(end + 1..).unwrap_or("");
            }
        }
    }
    None
}

fn rule_base(result: &mut Lemmas, candidate: &str, bit: u8) {
    if let Some((base, flags)) = table_entry(POLICY, candidate) {
        if flags
            .parse::<u16>()
            .is_ok_and(|flags| flags & (1 << bit) != 0)
        {
            result.insert(base);
        }
    }
}

fn appended_base(result: &mut Lemmas, prefix: &str, ending: &[u8], bit: u8) {
    let length = prefix.len() + ending.len();
    if length > MAX_BASE_BYTES {
        return;
    }
    let mut bytes = [0u8; MAX_BASE_BYTES];
    bytes[..prefix.len()].copy_from_slice(prefix.as_bytes());
    bytes[prefix.len()..length].copy_from_slice(ending);
    if let Ok(base) = std::str::from_utf8(&bytes[..length]) {
        rule_base(result, base, bit);
    }
}

fn doubled_base(result: &mut Lemmas, stem: &str, bit: u8) {
    let bytes = stem.as_bytes();
    if bytes.len() >= 2 && bytes[bytes.len() - 1] == bytes[bytes.len() - 2] {
        rule_base(result, &stem[..stem.len() - 1], bit);
    }
}

fn lemmas(key: &str) -> Option<Lemmas> {
    if key.len() < 2
        || key.len() > MAX_BASE_BYTES + 4
        || !key.bytes().all(|ch| ch.is_ascii_lowercase())
    {
        return None;
    }
    let mut result = Lemmas::default();
    if let Some((_, bases)) = table_entry(IRREGULARS, key) {
        for base in bases.split(',') {
            result.insert(base);
        }
    }
    if let Some(base) = key.strip_suffix('s') {
        rule_base(&mut result, base, 0);
    }
    if let Some(base) = key.strip_suffix("es") {
        rule_base(&mut result, base, 1);
    }
    if let Some(stem) = key.strip_suffix("ies") {
        appended_base(&mut result, stem, b"y", 2);
    }
    if let Some(base) = key.strip_suffix("ed") {
        rule_base(&mut result, base, 3);
        doubled_base(&mut result, base, 8);
    }
    if let Some(base) = key.strip_suffix('d') {
        rule_base(&mut result, base, 4);
    }
    if let Some(stem) = key.strip_suffix("ied") {
        appended_base(&mut result, stem, b"y", 5);
    }
    if let Some(base) = key.strip_suffix("ing") {
        rule_base(&mut result, base, 6);
        appended_base(&mut result, base, b"e", 7);
        doubled_base(&mut result, base, 9);
    }
    if let Some(stem) = key.strip_suffix("ying") {
        appended_base(&mut result, stem, b"ie", 10);
    }
    result.0[0].is_some().then_some(result)
}

impl Engine {
    fn available_english_surface_base(&self, bases: &Lemmas, extension: Option<&Engine>) -> bool {
        bases.iter().any(|base| {
            !std::iter::once(self)
                .chain(extension)
                .any(|engine| engine.dictionary.excluded_english.contains(base))
                && std::iter::once(self)
                    .chain(extension)
                    .any(|engine| engine.dictionary.english.contains(base))
        })
    }

    pub(crate) fn excluded_english_surface(&self, key: &str, extension: Option<&Engine>) -> bool {
        let Some(bases) = lemmas(key) else {
            return false;
        };
        // An exclusion in either view stays authoritative. A second documented
        // base can still support a homographic form if that base is available.
        std::iter::once(self)
            .chain(extension)
            .any(|engine| engine.dictionary.excluded_english.contains(key))
            || !self.available_english_surface_base(&bases, extension)
    }

    pub(crate) fn known_english_surface(
        &self,
        input: &str,
        key: &str,
        extension: Option<&Engine>,
    ) -> bool {
        if input.is_empty()
            || !input.bytes().all(|ch| ch.is_ascii_alphabetic())
            || !input.eq_ignore_ascii_case(key)
            || contains_profanity(key, ProfanityLanguage::English)
        {
            return false;
        }
        let Some(bases) = lemmas(key) else {
            return false;
        };
        if std::iter::once(self)
            .chain(extension)
            .any(|engine| engine.dictionary.excluded_english.contains(key))
        {
            return false;
        }
        self.available_english_surface_base(&bases, extension)
    }

    pub(crate) fn preserve_english_surface(
        &self,
        candidates: &mut Vec<Suggestion>,
        input: &str,
        key: &str,
    ) {
        // A real exact entry retains its identity, approved spelling and rank.
        if self.dictionary.english.contains(key) || !self.known_english_surface(input, key, None) {
            return;
        }
        candidates.insert(
            0,
            Suggestion {
                tap_only: false, preferred_input: None,
                completion: Some(input.to_owned()),
                correction: None,
                candidate: SuggestionCandidate::English(EnglishCandidate {
                    mapping_id: 0,
                    text: input.to_owned(),
                    rank: 0,
                    manual_only: false,
                    word_alternative: false,
                }),
            },
        );
    }

    pub(crate) fn runtime_english_exact(hit: &Suggestion, key: &str) -> bool {
        hit.correction.is_none()
            && matches!(&hit.candidate, SuggestionCandidate::English(word)
                if word.mapping_id == 0 && word.text.eq_ignore_ascii_case(key))
    }
}

#[cfg(test)]
mod tests {
    use super::{lemmas, IRREGULARS, POLICY};

    #[test]
    fn rules_preserve_regular_irregular_and_homographic_forms() {
        for (form, expected) in [
            ("books", vec!["book"]),
            ("children", vec!["child"]),
            ("wrote", vec!["write"]),
            ("studied", vec!["study"]),
            ("studying", vec!["study"]),
            ("travelled", vec!["travel"]),
            ("issued", vec!["issue"]),
            ("issuing", vec!["issue"]),
            ("tying", vec!["tie"]),
            ("is", vec!["be"]),
            ("were", vec!["be"]),
            ("leaves", vec!["leaf", "leave"]),
            ("staves", vec!["staff", "stay"]),
        ] {
            let mut actual: Vec<_> = lemmas(form)
                .unwrap_or_else(|| panic!("Missing {form}"))
                .iter()
                .collect();
            actual.sort_unstable();
            assert_eq!(actual, expected, "{form}");
        }
    }

    #[test]
    fn compatibility_policy_is_finite_sorted_and_does_not_guess_new_forms() {
        for table in [POLICY, IRREGULARS] {
            assert!(table.is_ascii());
            let mut previous = "";
            for line in table.lines() {
                let (key, value) = line.split_once('\t').unwrap();
                assert!(key > previous && key.bytes().all(|ch| ch.is_ascii_lowercase()));
                assert!(!value.is_empty());
                previous = key;
            }
        }
        assert_eq!(POLICY.lines().count(), 3_344);
        assert_eq!(IRREGULARS.lines().count(), 220);
        for key in [
            "", "a", "zzzzzzzz", "word", "words ", "words!", "WORDs", "isn", "ve", "hafta",
            "knowed", "readed", "bookes", "havings", "queened",
        ] {
            assert!(lemmas(key).is_none(), "unsupported spelling {key}");
        }
    }
}
