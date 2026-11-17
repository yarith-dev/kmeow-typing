//! Finite English word choices and literal-context guards, independent of punctuation.
use crate::{contains_profanity, english_correction::manual_alternative, suggestion_output,
    Engine, EnglishCandidate, EnglishWordContext, ProfanityLanguage, Suggestion, SuggestionCandidate};

fn word(value: &str) -> String {
    value.trim_matches(|c: char| !c.is_ascii_alphabetic() && c != '\'').to_ascii_lowercase()
}

// These cues protect explicit literal uses, not every quoted sentence or name.
// Only the caller's bounded, already-present text is inspected; no text is saved.
fn literal_context(input: &str, context: Option<&EnglishWordContext>) -> bool {
    if input.len() > 1 && input.bytes().all(|ch| ch.is_ascii_uppercase()) { return true; }
    let Some(context) = context else { return false; };
    let mut fence = None;
    for line in context.before.lines() {
        let line = line.trim_start();
        let marker = if line.starts_with("```") { Some('`') }
            else if line.starts_with("~~~") { Some('~') } else { None };
        if let Some(marker) = marker {
            if fence == Some(marker) { fence = None; }
            else if fence.is_none() { fence = Some(marker); }
        }
    }
    if fence.is_some() { return true; }
    let line = context.before.rsplit(['\n', '\r']).next().unwrap_or("");
    if line.chars().filter(|ch| *ch == '`').count() % 2 == 1 { return true; }
    let before = line.trim_end_matches(|ch: char| ch.is_whitespace()
        || matches!(ch, '"' | '\'' | '“' | '‘' | '(' | '[' | '{'));
    let after = context.after.trim_start_matches(|ch: char| ch.is_whitespace()
        || matches!(ch, '"' | '\'' | '”' | '’' | ')' | ']' | '}'));
    if before.ends_with('=') || after.starts_with(['=', ':']) { return true; }
    let mut words = before.split_whitespace().rev().map(word);
    let previous = words.next().unwrap_or_default();
    let cue = if matches!(previous.as_str(), "is" | "was" | "as") {
        words.next().unwrap_or_default()
    } else { previous };
    if before.ends_with(['.', '!', '?'])
        && !matches!(cue.as_str(), "mr" | "mrs" | "ms" | "dr" | "prof") { return false; }
    matches!(cue.as_str(), "abbreviation" | "acronym" | "initials" | "token" | "string"
        | "identifier" | "variable" | "literal" | "spelling" | "word" | "command"
        | "parameter" | "argument" | "username" | "handle" | "name" | "surname"
        | "nickname" | "named" | "called" | "mr" | "mrs" | "ms" | "dr" | "prof"
        | "a" | "an" | "the" | "my" | "your" | "his" | "her" | "our" | "their" | "its")
}

fn literal_suggestion(input: &str) -> Suggestion {
    Suggestion { tap_only: false, preferred_input: None, completion: Some(input.to_owned()), correction: None,
        candidate: SuggestionCandidate::English(EnglishCandidate { mapping_id: 0,
            text: input.to_owned(), rank: 0, manual_only: false, word_alternative: true }) }
}

// Bounded grammar cues; an undecided comparison retains the typed spelling.
fn prefer_contraction(bare: &str, context: &EnglishWordContext) -> Option<bool> {
    let before = context.before.trim_end_matches([' ', '\t', '"', '\'', '(', '[', '{', '“', '‘']);
    let start = before.is_empty() || before.ends_with(['.', '!', '?', '\n', '\r']);
    let previous = before.split_whitespace().next_back().map(word).unwrap_or_default();
    let following = context.after.split_whitespace().next().map(word).unwrap_or_default();
    let introductory_cues = before.rsplit(['.', '!', '?', '\n', '\r']).next().unwrap_or("")
        .split_whitespace().all(|token| matches!(word(token).as_str(),
            "please" | "okay" | "ok" | "now" | "then" | "so" | "well"));
    let subject = matches!(previous.as_str(), "he" | "she" | "it" | "who" | "someone" | "somebody" | "everyone" | "everybody" | "nobody");
    let possessive_noun = matches!(following.as_str(), "name" | "house" | "home" | "car" | "cat" | "dog" | "book" | "phone" | "color" | "colour" | "owner" | "way" | "family" | "friend" | "friends" | "idea" | "ideas");
    let predicate = matches!(following.as_str(), "a" | "an" | "the" | "not" | "really" | "very" | "so" | "too" | "going" | "been" | "being" | "ready" | "here" | "there" | "raining" | "working" | "welcome" | "right" | "wrong" | "late" | "early");
    match bare {
        "lets" if start => Some(true),
        "lets" if subject => Some(false),
        "lets" if introductory_cues => Some(true),
        "were" if matches!(previous.as_str(), "we" | "you" | "they" | "there" | "these" | "those") => Some(false),
        "were" if start && predicate => Some(true),
        "ill" if matches!(previous.as_str(), "feel" | "feels" | "feeling" | "felt" | "very" | "seriously" | "gravely" | "became" | "become") => Some(false),
        "ill" if start && matches!(following.as_str(), "be" | "go" | "do" | "have" | "take" | "see" | "try" | "call" | "help") => Some(true),
        "well" if matches!(previous.as_str(), "very" | "quite" | "really" | "as" | "so" | "doing" | "did" | "done" | "went" | "sleep" | "slept" | "feel" | "feeling") => Some(false),
        "well" | "wed" | "hell" | "shell" | "shed" | "id" if start && matches!(following.as_str(), "be" | "go" | "do" | "have" | "take" | "see" | "try" | "call" | "help" | "like") => Some(true),
        "well" | "wed" | "hell" | "shell" | "shed" | "id" if matches!(previous.as_str(), "a" | "an" | "the" | "this" | "that" | "my" | "your" | "his" | "her" | "our" | "their" | "to") => Some(false),
        "its" | "your" if possessive_noun => Some(false),
        "its" | "your" if predicate => Some(true),
        _ => None,
    }
}

impl Engine {
    pub(crate) fn guard_english_contractions(candidates: &mut Vec<Suggestion>, input: &str,
        key: &str, context: Option<&EnglishWordContext>) {
        if !literal_context(input, context) || !candidates.iter().any(|hit|
            hit.correction.is_some() && matches!(&hit.candidate, SuggestionCandidate::English(word)
                if word.mapping_id == 0 && word.word_alternative && word.text.contains('\''))) { return; }
        // Keeping the original first prevents Auto from replacing it. The
        // contraction remains an explicit tappable choice in either mode.
        candidates.retain(|hit| !suggestion_output(hit).eq_ignore_ascii_case(key));
        candidates.insert(0, literal_suggestion(input));
    }

    pub(crate) fn prioritize_english_corrections(candidates: &mut Vec<Suggestion>, key: &str) {
        let literal = candidates.iter().position(|hit| hit.correction.is_none() &&
            matches!(&hit.candidate, SuggestionCandidate::English(word) if
                word.mapping_id == 0 && word.word_alternative && word.text.eq_ignore_ascii_case(key)));
        let finite = candidates.iter().position(|hit| hit.correction.is_some() &&
            matches!(&hit.candidate, SuggestionCandidate::English(word) if
                word.mapping_id == 0 && word.word_alternative));
        if let Some(index) = literal.or(finite) {
            let chosen = candidates.remove(index);
            candidates.insert(0, chosen);
        }
    }
    pub(crate) fn english_word_choices(&self, candidates: &mut Vec<Suggestion>, input: &str,
        key: &str, context: Option<&EnglishWordContext>, extension: Option<&Engine>) {
        if input.chars().any(char::is_whitespace) { return; }
        Self::prioritize_english_corrections(candidates, key);
        let Some(alternative) = manual_alternative(key) else { return; };
        if contains_profanity(key, ProfanityLanguage::English) || contains_profanity(alternative, ProfanityLanguage::English) { return; }
        // Preserve existing finite function-word choices and the owner's
        // lets/let's pair. Other bare words require screened dictionary data.
        if !alternative.contains('\'') && !matches!(alternative, "lets" | "its" | "your" | "were") && !std::iter::once(self).chain(extension)
            .any(|engine| engine.dictionary.english.contains(alternative)) { return; }
        let bare = if key.contains('\'') { alternative } else { key };
        let literal_use = literal_context(input, context);
        let prefer = if literal_use || self.has_shared_english_exact(key)
            || extension.is_some_and(|engine| engine.dictionary.english.contains(key)) {
            None // Preserve an explicit personal exact spelling.
        } else { context.and_then(|context| prefer_contraction(bare, context)) };
        let canonical = if key == "i'll" { "I'll" } else if key == "i'd" { "I'd" } else { key };
        let choice = |text: &str| {
            let mut hit = candidates.iter().find(|hit| suggestion_output(hit).eq_ignore_ascii_case(text)).cloned()
                .unwrap_or_else(|| Suggestion { tap_only: false, preferred_input: None, completion: Some(text.to_owned()), correction: None,
                    candidate: SuggestionCandidate::English(EnglishCandidate { mapping_id: 0,
                        text: text.to_owned(), rank: 0, manual_only: false, word_alternative: true }) });
            if let SuggestionCandidate::English(candidate) = &mut hit.candidate {
                candidate.manual_only = false; candidate.word_alternative = true;
            }
            hit
        };
        let mut literal = choice(canonical);
        if literal_use {
            if let SuggestionCandidate::English(word) = &mut literal.candidate {
                word.text = input.to_owned();
            }
            literal.completion = Some(input.to_owned());
            literal.correction = None;
        }
        let alternative = choice(alternative);
        let alternative_first = prefer.is_some_and(|contracted| contracted == suggestion_output(&alternative).contains('\''));
        candidates.retain(|hit| !suggestion_output(hit).eq_ignore_ascii_case(canonical)
            && !suggestion_output(hit).eq_ignore_ascii_case(suggestion_output(&alternative)));
        let (first, second) = if alternative_first { (alternative, literal) } else { (literal, alternative) };
        candidates.insert(0, second); candidates.insert(0, first);
    }
}
