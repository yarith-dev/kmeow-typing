//! Small, read-only touch priors over the full eligible prefix range. These are
//! dictionary heuristics, not measured probabilities or new typing spellings.
use super::*;

// Include word endings/punctuation in the denominator so a complete word does
// not strongly attract a letter solely because one longer word also matches.
pub(crate) const STOP: u32 = 1 << 26;
pub(crate) fn letter_bit(byte: u8) -> u32 {
    if byte.is_ascii_lowercase() { 1 << (byte - b'a') } else { STOP }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn english(words: &[&str]) -> Engine {
        Engine::from_json(&serde_json::json!({"schemaVersion":3, "mappings":[], "words":[],
            "englishWords": words.iter().enumerate().map(|(i, word)| serde_json::json!({
                "id":i+1, "text":word, "rank":0, "status":"approved"
            })).collect::<Vec<_>>() }).to_string()).unwrap()
    }
    fn weights(engine: &Engine, input: &str, features: SuggestionFeatures) -> BTreeMap<String, u16> {
        engine.suggest_with_context(input, features, &TypingContext::default()).unwrap().next_letter_weights
    }
    #[test]
    fn endings_preferences_and_literal_inputs_fall_back_without_changing_candidates() {
        let engine = english(&["hello", "help", "helps"]);
        let features = SuggestionFeatures { mode: TypingMode::English, ..Default::default() };
        assert_eq!(weights(&engine, "HEL", features), BTreeMap::from([("l".into(), 333), ("p".into(), 666)]));
        assert_eq!(weights(&engine, "help", features), BTreeMap::from([("s".into(), 500)]));
        for input in ["", "h", "hel ", "hel lo", "123", "zzzz", "hello", "សុ"] {
            assert!(weights(&engine, input, features).is_empty(), "{input}");
        }
        assert!(weights(&engine, "hel", SuggestionFeatures { english_completion: false, ..features }).is_empty());
        let before = engine.to_binary();
        assert_eq!(engine.suggest_with_features("hel", features).unwrap().candidates,
            engine.suggest_with_context("hel", features, &TypingContext::default()).unwrap().result.candidates);
        assert_eq!(engine.to_binary(), before);
    }
    #[test]
    fn every_prefix_match_counts_even_when_the_suggestion_strip_is_full() {
        let mut words = (b'a'..=b'y').map(|c| format!("heaa{}", c as char)).collect::<Vec<_>>();
        words.push("hezebra".into());
        let engine = english(&words.iter().map(String::as_str).collect::<Vec<_>>());
        let features = SuggestionFeatures { mode: TypingMode::English, ..Default::default() };
        assert_eq!(weights(&engine, "he", features).get("z"), Some(&38));
        assert_eq!(engine.suggest_with_features("he", features).unwrap().candidates.len(), 20);
    }
    #[test]
    fn aliases_share_one_word_vote_and_generated_keys_respect_policy() {
        let engine = Engine::from_json(r#"{"schemaVersion":2,"mappings":[
            {"id":1,"romanized":"tent","khmer":"ក","rank":0,"status":"approved"},
            {"id":2,"romanized":"tenx","khmer":"ក","rank":0,"status":"approved"},
            {"id":3,"romanized":"tenxx","khmer":"ក","rank":0,"status":"approved"},
            {"id":4,"romanized":"tenz","khmer":"ខ","rank":0,"status":"approved"}],
            "words":[{"id":10,"khmer":"ក","rank":0,"status":"approved"},
            {"id":11,"khmer":"ខ","rank":0,"status":"approved"},
            {"id":12,"khmer":"សួរ","rank":0,"status":"approved"}]}"#).unwrap();
        let features = SuggestionFeatures { sound_map: false, ..Default::default() };
        assert_eq!(weights(&engine, "ten", features), BTreeMap::from([("t".into(),250), ("x".into(),250), ("z".into(),500)]));
        assert_eq!(weights(&engine, "su", SuggestionFeatures::default()).get("r"), Some(&1000));
        assert!(weights(&engine, "su", features).is_empty());
        assert!(weights(&engine, "su", SuggestionFeatures { romanized: false, ..Default::default() }).is_empty());
        let disabled = engine.with_typing_overrides_json(r#"[{"language":"khmer","word":"សួរ","soundMapEnabled":false}]"#).unwrap();
        assert!(weights(&disabled, "su", SuggestionFeatures::default()).is_empty());
        assert!(disabled.suggest_with_extension(&engine, "su", SuggestionFeatures::default(),
            &TypingContext::default()).unwrap().next_letter_weights.is_empty());
    }
}
type Votes = BTreeMap<String, (u32, u32)>;

impl Engine {
    pub(crate) fn next_letter_weights(&self, input: &str, features: SuggestionFeatures,
        extension: Option<&Engine>) -> BTreeMap<String, u16> {
        // No bias at boundaries, on short/unknown prefixes, or in direct Khmer.
        // Completion preferences also control this use of the dictionary.
        let english = features.mode == TypingMode::English;
        if !(2..=64).contains(&input.len())
            || !input.bytes().all(|b| b.is_ascii_alphabetic() || b == b'\'' || english && b == b'-')
            || input.starts_with('\'') || input.contains("''")
            || if english { !features.english_completion } else { !features.romanized || !features.khmer_completion }
        { return BTreeMap::new(); }
        let prefix = input.to_ascii_lowercase();
        if contains_profanity(&prefix, ProfanityLanguage::Auto) { return BTreeMap::new(); }
        let mut votes = Votes::new();
        self.letter_votes(&prefix, features, None, &mut votes);
        if let Some(extra) = extension {
            extra.letter_votes(&prefix, features, Some(&self.dictionary), &mut votes);
        }
        if !english { votes.retain(|word, _| !self.input_excluded(&prefix, word, extension.map(|extra| &extra.dictionary))); }
        // Each distinct output contributes once, shared equally among its next
        // choices. Many aliases/readings cannot multiply one word's influence.
        let mut mass = [0u64; 27];
        for (weight, mask) in votes.values() {
            let share = u64::from(*weight) * 1_000_000 / u64::from(mask.count_ones());
            for (i, value) in mass.iter_mut().enumerate() {
                if mask & (1 << i) != 0 { *value += share; }
            }
        }
        let total: u64 = mass.iter().sum();
        if total == 0 { return BTreeMap::new(); }
        (0..26).filter_map(|i| {
            let weight = (mass[i] * 1000 / total) as u16;
            (weight > 0).then(|| ((char::from(b'a' + i as u8)).to_string(), weight))
        }).collect()
    }

    fn letter_votes(&self, prefix: &str, features: SuggestionFeatures,
        base_policy: Option<&compact::Dictionary>, votes: &mut Votes) {
        let english = features.mode == TypingMode::English;
        let index = if english { &self.dictionary.english } else { &self.dictionary.romanized };
        let (start, end) = index.prefix(prefix);
        let mut cursor = index.cursor(start);
        while cursor.next < end {
            let row = cursor.next().expect("validated compact dictionary").unwrap();
            let target = if english { compact::english_text(&row).into_owned() }
                else {
                    if self.blocked_mapping(&row) { continue; }
                    self.dictionary.pool.get(row.target).to_owned()
                };
            if contains_profanity(&target, if english { ProfanityLanguage::English } else { ProfanityLanguage::Khmer }) { continue; }
            let mask = row.key.as_bytes().get(prefix.len()).map_or(STOP, |&b| letter_bit(b));
            self.add_letter_vote(votes, target, english, mask);
        }
        if !english && features.sound_map {
            let sounds = self.sound_index.get_or_init(|| sound_map::Index::new(&self.dictionary));
            sounds.visit_next_letters(prefix, base_policy, |candidate, mask| {
                self.add_letter_vote(votes, candidate.khmer.clone(), false, mask);
            });
        }
    }

    fn add_letter_vote(&self, votes: &mut Votes, word: String, english: bool, mask: u32) {
        if mask == 0 { return; }
        // Commonness is ordinal. These deliberately modest weights are tuning
        // constants, not frequency estimates; saved ranks remain untouched.
        let weight = match self.commonness.level(if english { "english" } else { "khmer" }, &word) {
            1 => 16, 2 => 8, 3 => 4, 4 => 2, _ => 1,
        };
        let vote = votes.entry(word).or_insert((weight, 0));
        vote.1 |= mask;
    }
}
