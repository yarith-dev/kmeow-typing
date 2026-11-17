//! Immutable, front-coded blocks. The runtime searches the encoded bytes directly.
//! No I/O, platform dependencies, or dataset-sized decoded string index.
use super::{invalid, Candidate, EngineError, EnglishCandidate, MAX_SAFE_INTEGER};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

pub(crate) const BLOCK: usize = 16;
const MAGIC: &[u8; 8] = b"KMEOWC01";
const JOINED_MAGIC: &[u8; 8] = b"KMEOWC02";
const CONTEXT_MAGIC: &[u8; 8] = b"KMEOWC03";
const POLICY_MAGIC: &[u8; 8] = b"KMEOWC04";
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Romanized = 0,
    Khmer = 1,
    English = 2,
}
#[derive(Clone)]
pub(crate) struct Index {
    data: Arc<Vec<u8>>,
    start: usize,
    entries: usize,
    end: usize,
    pub(crate) len: usize,
    pub(crate) kind: Kind,
    joined_forms: bool,
    word_policy: bool,
    local_ranks: Arc<BTreeMap<u64, u32>>,
    local_rank_words: Arc<BTreeSet<String>>,
    local_sounds: Arc<BTreeMap<String, bool>>,
    // Runtime-only lower bounds including the current reviewed commonness.
    // One tuple per encoded block; compact bytes and saved ranks stay intact.
    completion_bounds: Arc<Vec<(u8, u32, usize)>>,
}
#[derive(Clone)]
pub(crate) struct Pool {
    data: Arc<Vec<u8>>,
    offsets: usize,
    strings: usize,
    count: usize,
}
#[derive(Clone)]
pub(crate) struct Dictionary {
    pub(crate) data: Arc<Vec<u8>>,
    pub(crate) romanized: Index,
    pub(crate) khmer: Index,
    pub(crate) english: Index,
    pub(crate) pool: Pool,
    pub(crate) max_key_bytes: usize,
    pub(crate) mapping_count: usize,
    pub(crate) context: Option<crate::context::Index>,
    pub(crate) excluded_khmer: Arc<BTreeSet<String>>,
    pub(crate) excluded_english: Arc<BTreeSet<String>>,
    // Owner choices used by runtime component readings; never serialized here.
    pub(crate) component_definitions: Arc<crate::components::Definitions>,
    pub(crate) pronunciations: Arc<Vec<crate::Pronunciation>>,
}
pub(crate) struct Row<'a> {
    pub key: &'a str,
    pub id: u64,
    pub rank: u32,
    pub target: u128,
    /// Position of the canonical Romanized row; absent for an approved source.
    pub source: Option<usize>,
}
pub(crate) struct Cursor<'a> {
    index: &'a Index,
    pub next: usize,
    pos: usize,
    key: Vec<u8>,
    previous_id: i64,
}
fn number(bytes: &[u8], offset: usize) -> Result<usize, EngineError> {
    let b = bytes
        .get(
            offset
                ..offset
                    .checked_add(4)
                    .ok_or_else(|| invalid("Compact offset overflow."))?,
        )
        .ok_or_else(|| invalid("Truncated compact dictionary."))?;
    Ok(u32::from_le_bytes(b.try_into().unwrap()) as usize)
}
fn put32(out: &mut Vec<u8>, value: usize) -> Result<(), EngineError> {
    out.extend(
        u32::try_from(value)
            .map_err(|_| invalid("Compact section exceeds 4 GiB."))?
            .to_le_bytes(),
    );
    Ok(())
}
fn put(out: &mut Vec<u8>, mut value: u128) {
    while value >= 128 {
        out.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    out.push(value as u8);
}
fn read(bytes: &[u8], pos: &mut usize) -> Result<u128, EngineError> {
    let mut value = 0u128;
    for shift in (0..128).step_by(7) {
        let byte = *bytes
            .get(*pos)
            .ok_or_else(|| invalid("Truncated compact integer."))?;
        *pos += 1;
        if shift == 126 && byte > 3 {
            return Err(invalid("Compact integer overflow."));
        }
        value |= u128::from(byte & 127) << shift;
        if byte < 128 {
            if shift > 0 && byte == 0 {
                return Err(invalid("Noncanonical compact integer."));
            }
            return Ok(value);
        }
    }
    Err(invalid("Compact integer overflow."))
}
fn small(bytes: &[u8], pos: &mut usize, max: u128) -> Result<usize, EngineError> {
    let v = read(bytes, pos)?;
    if v > max {
        return Err(invalid("Compact integer out of range."));
    }
    Ok(v as usize)
}
// Integrity check, not authentication. Distribution manifests also supply SHA-256.
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}
impl Index {
    pub(crate) fn has_local_ranks(&self) -> bool { !self.local_ranks.is_empty() }
    pub(crate) fn has_local_rank_for_word(&self, word: &str) -> bool { self.local_rank_words.contains(word) }
    pub(crate) fn contains(&self, text: &str) -> bool {
        self.cursor(self.bound(text, false))
            .next()
            .expect("validated index")
            .is_some_and(|row| row.key == text)
    }
    fn parse(
        data: Arc<Vec<u8>>,
        start: usize,
        len: usize,
        kind: Kind,
        joined_forms: bool,
    ) -> Result<Self, EngineError> {
        let end = start
            .checked_add(len)
            .ok_or_else(|| invalid("Compact section overflow."))?;
        if len < 16
            || end > data.len()
            || number(&data, start)? != kind as usize
            || number(&data, start + 12)? != BLOCK
        {
            return Err(invalid("Invalid compact index header."));
        }
        let count = number(&data, start + 4)?;
        let size = number(&data, start + 8)?;
        let directory = count
            .div_ceil(BLOCK)
            .checked_mul(12)
            .ok_or_else(|| invalid("Compact directory overflow."))?;
        let entries = start
            .checked_add(16 + directory)
            .ok_or_else(|| invalid("Compact directory overflow."))?;
        if entries.checked_add(size) != Some(end) {
            return Err(invalid("Compact section size mismatch."));
        }
        let mut previous = None;
        for block in 0..count.div_ceil(BLOCK) {
            let offset = number(&data, start + 16 + block * 12)?;
            if offset >= size
                || (block == 0 && offset != 0)
                || previous.is_some_and(|p| offset <= p)
            {
                return Err(invalid("Invalid compact block offset."));
            }
            previous = Some(offset);
        }
        let word_policy = &data[..8] == POLICY_MAGIC;
        Ok(Self {
            data,
            start,
            entries,
            end,
            len: count,
            kind,
            joined_forms,
            word_policy,
            local_ranks: Default::default(),
            local_rank_words: Default::default(),
            local_sounds: Default::default(),
            completion_bounds: Default::default(),
        })
    }
    fn block_offset(&self, block: usize) -> usize {
        self.entries
            + number(&self.data, self.start + 16 + block * 12).expect("validated directory")
    }
    pub fn block_min(&self, block: usize) -> (u32, usize) {
        (
            number(&self.data, self.start + 20 + block * 12).unwrap() as u32,
            number(&self.data, self.start + 24 + block * 12).unwrap(),
        )
    }
    pub(crate) fn completion_block_min(&self, block: usize, commonness: bool) -> Option<(u8, u32, usize)> {
        if commonness { self.completion_bounds.get(block).copied() }
        else { let (rank, length) = self.block_min(block); Some((6, rank, length)) }
    }
    pub fn cursor(&self, from: usize) -> Cursor<'_> {
        let mut cursor = Cursor {
            index: self,
            next: from / BLOCK * BLOCK,
            pos: self.entries,
            key: Vec::new(),
            previous_id: 0,
        };
        while cursor.next < from {
            cursor.next().expect("validated compact row");
        }
        cursor
    }
    pub fn bound(&self, key: &str, upper: bool) -> usize {
        // Probe independently encoded block heads, then decode just one block.
        // Probing arbitrary rows repeatedly decodes up to BLOCK earlier rows
        // on every binary-search step (especially costly during recovery).
        let (mut lo, mut hi) = (0, self.len.div_ceil(BLOCK));
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let mut cursor = self.cursor(mid * BLOCK);
            let row = cursor.next().expect("validated compact row").unwrap();
            if row.key < key || (upper && row.key == key) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 { return 0; }
        // The preceding block may contain the first match, including when
        // duplicate keys extend across several block boundaries.
        let end = (lo * BLOCK).min(self.len);
        let mut cursor = self.cursor((lo - 1) * BLOCK);
        while cursor.next < end {
            let position = cursor.next;
            let row = cursor.next().expect("validated compact row").unwrap();
            if row.key > key || (!upper && row.key == key) { return position; }
        }
        end
    }
    pub fn prefix(&self, prefix: &str) -> (usize, usize) {
        // Every stored key is ASCII or Khmer; U+10FFFF sorts after any legal suffix.
        (
            self.bound(prefix, false),
            self.bound(&format!("{prefix}\u{10ffff}"), false),
        )
    }
}
impl<'a> Cursor<'a> {
    pub fn next(&mut self) -> Result<Option<Row<'_>>, EngineError> {
        if self.next >= self.index.len {
            return Ok(None);
        }
        if self.next % BLOCK == 0 {
            self.pos = self.index.block_offset(self.next / BLOCK);
            self.key.clear();
            self.previous_id = 0;
        }
        let data = &self.index.data[..self.index.end];
        let prefix = small(data, &mut self.pos, 720)?;
        let suffix = small(data, &mut self.pos, 720)?;
        if prefix > self.key.len() || prefix + suffix > 720 {
            return Err(invalid("Invalid compact prefix."));
        }
        let text = data
            .get(self.pos..self.pos + suffix)
            .ok_or_else(|| invalid("Truncated compact spelling."))?;
        self.pos += suffix;
        self.key.truncate(prefix);
        self.key.extend(text);
        let key = std::str::from_utf8(&self.key).map_err(|_| invalid("Invalid compact UTF-8."))?;
        let encoded = read(data, &mut self.pos)?;
        if encoded > u128::from(u64::MAX) {
            return Err(invalid("Compact ID delta overflow."));
        }
        let encoded = encoded as u64;
        let delta = ((encoded >> 1) as i64) ^ -((encoded & 1) as i64);
        let id = self
            .previous_id
            .checked_add(delta)
            .filter(|id| *id > 0 && *id as u64 <= MAX_SAFE_INTEGER)
            .ok_or_else(|| invalid("Invalid compact mapping ID."))?;
        self.previous_id = id;
        let stored_rank = small(data, &mut self.pos, 1_000_000)? as u32;
        let rank = self.index.local_ranks.get(&(id as u64)).copied().unwrap_or(stored_rank);
        let mut target = if self.index.kind == Kind::Khmer && !self.index.word_policy {
            0
        } else {
            read(data, &mut self.pos)?
        };
        if self.index.kind == Kind::Khmer {
            if let Some(enabled) = self.index.local_sounds.get(key) { target = u128::from(!enabled); }
        }
        let source = if self.index.joined_forms && self.index.kind == Kind::Romanized {
            small(data, &mut self.pos, u32::MAX as u128)?.checked_sub(1)
        } else {
            None
        };
        self.next += 1;
        Ok(Some(Row {
            key,
            id: id as u64,
            rank,
            target,
            source,
        }))
    }
}
impl Pool {
    fn parse(data: Arc<Vec<u8>>, start: usize, len: usize) -> Result<Self, EngineError> {
        if len < 8 || start.checked_add(len).is_none_or(|end| end > data.len()) {
            return Err(invalid("Invalid compact string pool."));
        }
        let count = number(&data, start)?;
        let strings = start
            .checked_add(4)
            .and_then(|v| {
                count
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(4))
                    .and_then(|n| v.checked_add(n))
            })
            .ok_or_else(|| invalid("Compact pool overflow."))?;
        if strings > start + len {
            return Err(invalid("Truncated compact string directory."));
        }
        let mut previous = 0;
        let mut previous_text = "";
        for i in 0..=count {
            let offset = number(&data, start + 4 + i * 4)?;
            if offset < previous || offset > start + len - strings || (i == 0 && offset != 0) {
                return Err(invalid("Invalid compact string offset."));
            }
            if i > 0 {
                let s = std::str::from_utf8(&data[strings + previous..strings + offset])
                    .map_err(|_| invalid("Invalid compact output UTF-8."))?;
                if !super::valid_khmer(s) {
                    return Err(invalid("Invalid compact Khmer output."));
                }
                if s <= previous_text {
                    return Err(invalid("Unsorted or duplicate compact output pool."));
                }
                previous_text = s;
            }
            previous = offset;
        }
        if strings + previous != start + len {
            return Err(invalid("Compact string pool size mismatch."));
        }
        Ok(Self {
            data,
            offsets: start + 4,
            strings,
            count,
        })
    }
    pub(crate) fn count(&self) -> usize {
        self.count
    }
    #[cfg(test)]
    pub(crate) fn find(&self, text: &str) -> Option<usize> {
        let mut lo = 0;
        let mut hi = self.count;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.get(mid as u128) < text {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        (lo < self.count && self.get(lo as u128) == text).then_some(lo)
    }
    pub fn get(&self, id: u128) -> &str {
        let i = id as usize;
        let start = number(&self.data, self.offsets + i * 4).unwrap();
        let end = number(&self.data, self.offsets + (i + 1) * 4).unwrap();
        std::str::from_utf8(&self.data[self.strings + start..self.strings + end])
            .expect("validated compact output")
    }
}
pub(crate) fn english_text<'a>(row: &Row<'a>) -> std::borrow::Cow<'a, str> {
    if row.target == 0 {
        return std::borrow::Cow::Borrowed(row.key);
    }
    let mut bytes = row.key.as_bytes().to_vec();
    for (i, byte) in bytes.iter_mut().enumerate() {
        if row.target & (1u128 << i) != 0 {
            *byte = byte.to_ascii_uppercase();
        }
    }
    std::borrow::Cow::Owned(String::from_utf8(bytes).expect("validated ASCII"))
}
impl Dictionary {
    pub(crate) fn same_reading_policy(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data,&other.data)
            && Arc::ptr_eq(&self.excluded_khmer,&other.excluded_khmer)
            && Arc::ptr_eq(&self.khmer.local_sounds,&other.khmer.local_sounds)
            && Arc::ptr_eq(&self.component_definitions,&other.component_definitions)
            && Arc::ptr_eq(&self.pronunciations,&other.pronunciations)
    }
    pub(crate) fn set_completion_commonness(&mut self, commonness: &crate::word_metadata::Commonness) {
        for index in [&mut self.romanized, &mut self.khmer, &mut self.english] {
            let language = if index.kind == Kind::English { "english" } else { "khmer" };
            let mut bounds = Vec::new();
            if commonness.has_levels(language) {
                let mut cursor = index.cursor(0);
                while cursor.next < index.len {
                    let block = cursor.next / BLOCK;
                    let row = cursor.next().expect("validated compact dictionary").unwrap();
                    let target = if index.kind == Kind::Romanized { self.pool.get(row.target) } else { row.key };
                    let value = (commonness.level(language, target), row.rank,
                        if index.kind == Kind::English { target.len() } else { 0 });
                    if block == bounds.len() { bounds.push(value); }
                    else { bounds[block] = bounds[block].min(value); }
                }
            }
            index.completion_bounds = Arc::new(bounds);
        }
    }
    /// Sparse runtime settings share the immutable compact bytes. Every alias,
    /// joined lookup and completion for a spelling receives the same local rank.
    pub(crate) fn with_local_settings(&self, settings: &[crate::personal::TypingOverride]) -> Result<Self, EngineError> {
        // Rebuild views from the immutable source, so clearing a local setting
        // also restores rows hidden by a previous prepared view.
        let mut copy = Self::from_bytes(self.data.as_ref().clone())?;
        copy.excluded_khmer = Arc::new(settings.iter().filter(|s| s.language == "khmer" && s.excluded == Some(true))
            .map(|s| s.word.clone()).collect());
        copy.excluded_english = Arc::new(settings.iter().filter(|s| s.language == "english" && s.excluded == Some(true))
            .map(|s| s.word.clone()).collect());
        for index in [&mut copy.romanized, &mut copy.khmer, &mut copy.english] {
            let language = if index.kind == Kind::English { "english" } else { "khmer" };
            let blocked: BTreeSet<&str> = settings.iter().filter(|s|
                s.language == language && s.excluded == Some(true)).map(|s| s.word.as_str()).collect();
            if blocked.is_empty() { continue; }
            let mut rows = Vec::new();
            let mut positions = BTreeMap::new();
            let mut cursor = index.cursor(0);
            while cursor.next < index.len {
                let position = cursor.next;
                let row = cursor.next()?.expect("validated row");
                let word = if index.kind == Kind::Romanized { copy.pool.get(row.target) } else { row.key };
                if blocked.contains(word) { continue; }
                positions.insert(position, rows.len());
                rows.push((row.key.to_owned(), row.id, row.rank, row.target, row.source));
            }
            let mut builder = Builder::new(index.kind, index.joined_forms);
            builder.word_policy = index.word_policy;
            for (key, id, rank, target, source) in rows {
                let source = source.map(|old| positions.get(&old).copied()
                    .ok_or_else(|| invalid("A local alias lost its canonical source."))).transpose()?;
                builder.push(&key, id, rank, target, source);
            }
            let section = builder.finish()?;
            let mut bytes = if index.word_policy { POLICY_MAGIC } else if index.joined_forms { JOINED_MAGIC } else { MAGIC }.to_vec();
            bytes.extend(&section);
            *index = Index::parse(Arc::new(bytes), 8, section.len(), index.kind, index.joined_forms)?;
        }
        let mut khmer = BTreeMap::new();
        let mut english = BTreeMap::new();
        let mut sounds = BTreeMap::new();
        for setting in settings {
            if setting.language == "khmer" {
                if let Some(rank) = setting.rank { khmer.insert(setting.word.as_str(), rank); }
                if let Some(enabled) = setting.sound_map_enabled { sounds.insert(setting.word.clone(), enabled); }
            } else if let Some(rank) = setting.rank { english.insert(setting.word.as_str(), rank); }
        }
        for index in [&mut copy.romanized, &mut copy.khmer, &mut copy.english] {
            let mut ranks = BTreeMap::new();
            let mut cursor = index.cursor(0);
            while let Some(row) = cursor.next()? {
                let word = if index.kind == Kind::Romanized { copy.pool.get(row.target) } else { row.key };
                let values = if index.kind == Kind::English { &english } else { &khmer };
                if let Some(rank) = values.get(word) { ranks.insert(row.id, *rank); }
            }
            index.local_ranks = Arc::new(ranks);
            let values = if index.kind == Kind::English { &english } else { &khmer };
            index.local_rank_words = Arc::new(values.keys().map(|word| (*word).to_owned()).collect());
        }
        copy.khmer.local_sounds = Arc::new(sounds);
        Ok(copy)
    }
    // Missing personal words retain their own defaults; only an explicit
    // disabled base spelling constrains an overlapping extension reading.
    pub(crate) fn sound_map_disabled(&self, word: &str) -> bool {
        self.khmer.cursor(self.khmer.bound(word, false)).next().expect("validated dictionary")
            .is_some_and(|row| row.key == word && row.target != 0)
    }
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, EngineError> {
        if bytes.len() < 36
            || (&bytes[..8] != MAGIC && &bytes[..8] != JOINED_MAGIC && &bytes[..8] != CONTEXT_MAGIC && &bytes[..8] != POLICY_MAGIC)
        {
            return Err(invalid("Unsupported compact dictionary format."));
        }
        let policy = &bytes[..8] == POLICY_MAGIC;
        let contextual = &bytes[..8] == CONTEXT_MAGIC || policy;
        let joined_forms = &bytes[..8] == JOINED_MAGIC || contextual;
        let end = bytes.len() - 8;
        if checksum(&bytes[..end]) != u64::from_le_bytes(bytes[end..].try_into().unwrap()) {
            return Err(invalid("Compact dictionary checksum mismatch."));
        }
        let schema = number(&bytes, 8)?;
        if !(1..=3).contains(&schema) || (joined_forms && schema < 2) {
            return Err(invalid("Invalid compact source schema."));
        }
        let data = Arc::new(bytes);
        let mut start = if contextual { 32usize } else { 28usize };
        let mut ranges = Vec::with_capacity(5);
        for offset in (12..start).step_by(4) {
            let len = number(&data, offset)?;
            let next = start
                .checked_add(len)
                .ok_or_else(|| invalid("Compact section overflow."))?;
            if next > end {
                return Err(invalid("Truncated compact section."));
            }
            ranges.push((start, len));
            start = next;
        }
        if start != end {
            return Err(invalid("Unexpected compact trailing bytes."));
        }
        let romanized = Index::parse(
            data.clone(),
            ranges[0].0,
            ranges[0].1,
            Kind::Romanized,
            joined_forms,
        )?;
        let khmer = Index::parse(
            data.clone(),
            ranges[1].0,
            ranges[1].1,
            Kind::Khmer,
            joined_forms,
        )?;
        let english = Index::parse(
            data.clone(),
            ranges[2].0,
            ranges[2].1,
            Kind::English,
            joined_forms,
        )?;
        let pool = Pool::parse(data.clone(), ranges[3].0, ranges[3].1)?;
        if (schema == 1 && khmer.len > 0) || (schema < 3 && english.len > 0) {
            return Err(invalid("Compact indexes conflict with source schema."));
        }
        let mut max_key_bytes = 0;
        let mut mapping_count = 0;
        let mut joined_count = 0;
        for index in [&romanized, &khmer, &english] {
            // Dense release IDs need only bits. Sparse external IDs retain their
            // original values and use a temporary sorted overflow vector.
            let dense_limit = index.len.saturating_mul(2).max(1024);
            let mut dense_ids = vec![0u8; dense_limit.div_ceil(8)];
            let mut ids = Vec::new();
            let mut previous = String::new();
            let mut previous_order = None;
            let mut previous_case: Option<u128> = None;
            let mut targets = HashSet::new();
            let mut previous_end = index.entries;
            for block in 0..index.len.div_ceil(BLOCK) {
                if index.block_offset(block) != previous_end {
                    return Err(invalid("Noncontiguous compact block."));
                }
                let mut cursor = index.cursor(block * BLOCK);
                let mut min = (u32::MAX, usize::MAX);
                for position in block * BLOCK..((block + 1) * BLOCK).min(index.len) {
                    let row = cursor.next()?.unwrap();
                    if row.key < previous.as_str() || row.key.is_empty() {
                        return Err(invalid("Unsorted compact keys."));
                    }
                    if row.key != previous {
                        previous_order = None;
                        previous_case = None;
                        targets.clear();
                    }
                    match index.kind {
                        Kind::Romanized => {
                            if !super::valid_romanized(row.key) || row.target >= pool.count as u128
                            {
                                return Err(invalid("Invalid compact Romanized mapping."));
                            }
                            if !targets.insert(row.target)
                                || previous_order.is_some_and(|last| last >= (row.rank, row.id))
                            {
                                return Err(invalid(
                                    "Invalid compact candidate order or duplicate output.",
                                ));
                            }
                            previous_order = Some((row.rank, row.id));
                            max_key_bytes = max_key_bytes.max(row.key.len());
                        }
                        Kind::Khmer => {
                            if !super::valid_khmer(row.key) || row.key == previous || row.target > u128::from(policy) {
                                return Err(invalid("Invalid or duplicate compact Khmer word."));
                            }
                        }
                        Kind::English => {
                            if !super::valid_english(row.key)
                                || row.key.bytes().any(|b| b.is_ascii_uppercase())
                                || row.target >> row.key.len() != 0
                                || row.key.bytes().enumerate().any(|(i, b)| {
                                    row.target & (1u128 << i) != 0 && !b.is_ascii_lowercase()
                                })
                                || previous_case
                                    .is_some_and(|last| last <= row.target.reverse_bits())
                            {
                                return Err(invalid(
                                    "Invalid or duplicate compact English spelling.",
                                ));
                            }
                            previous_case = Some(row.target.reverse_bits());
                        }
                    }
                    if let Some(source) = row.source {
                        // The referenced row is earlier in sorted order, so its
                        // encoding has already passed validation. Decode only its
                        // bounded block; no additional runtime index is retained.
                        if source >= position {
                            return Err(invalid("Invalid compact joined source reference."));
                        }
                        let mut source_cursor = index.cursor(source);
                        let original = source_cursor.next()?.unwrap();
                        if original.source.is_some()
                            || original.id != row.id
                            || original.rank != row.rank
                            || original.target != row.target
                            || !original.key.contains(' ')
                            || original
                                .key
                                .bytes()
                                .filter(|byte| *byte != b' ')
                                .ne(row.key.bytes())
                        {
                            return Err(invalid("Invalid compact joined lookup."));
                        }
                        joined_count += 1;
                    } else if row.id < dense_limit as u64 {
                        let position = row.id as usize;
                        let mask = 1u8 << (position % 8);
                        if dense_ids[position / 8] & mask != 0 {
                            return Err(invalid("Duplicate compact mapping ID."));
                        }
                        dense_ids[position / 8] |= mask;
                    } else {
                        ids.push(row.id);
                    }
                    if index.kind == Kind::Romanized && row.source.is_none() {
                        mapping_count += 1;
                    }
                    min = min.min((row.rank, row.key.len()));
                    previous = row.key.to_owned();
                }
                previous_end = cursor.pos;
                if min != index.block_min(block) {
                    return Err(invalid("Compact block ranking summary mismatch."));
                }
            }
            if previous_end != index.end {
                return Err(invalid("Unexpected compact row bytes."));
            }
            ids.sort_unstable();
            if ids.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(invalid("Duplicate compact mapping ID."));
            }
        }
        if joined_forms && !contextual && joined_count == 0 {
            return Err(invalid("Compact v2 requires a derived joined lookup."));
        }
        let context = if contextual && !(policy && ranges[4].1 == 0) {
            Some(crate::context::Index::parse(
                data.clone(),
                ranges[4].0,
                ranges[4].1,
                &pool,
                &khmer,
            )?)
        } else {
            None
        };
        Ok(Self {
            data,
            romanized,
            khmer,
            english,
            pool,
            max_key_bytes,
            mapping_count,
            context,
            excluded_khmer: Default::default(),
            excluded_english: Default::default(),
            component_definitions: Default::default(),
            pronunciations: Default::default(),
        })
    }
}
struct Builder {
    kind: Kind,
    count: usize,
    bytes: Vec<u8>,
    blocks: Vec<(usize, u32, usize)>,
    previous: String,
    previous_id: i64,
    joined_forms: bool,
    word_policy: bool,
}
impl Builder {
    fn new(kind: Kind, joined_forms: bool) -> Self {
        Self {
            kind,
            count: 0,
            bytes: Vec::new(),
            blocks: Vec::new(),
            previous: String::new(),
            previous_id: 0,
            joined_forms,
            word_policy: false,
        }
    }
    fn push(&mut self, key: &str, id: u64, rank: u32, target: u128, source: Option<usize>) {
        if self.count % BLOCK == 0 {
            self.blocks.push((self.bytes.len(), rank, key.len()));
            self.previous.clear();
            self.previous_id = 0;
        }
        let prefix = key
            .bytes()
            .zip(self.previous.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        put(&mut self.bytes, prefix as u128);
        put(&mut self.bytes, (key.len() - prefix) as u128);
        self.bytes.extend(&key.as_bytes()[prefix..]);
        let delta = id as i64 - self.previous_id;
        put(
            &mut self.bytes,
            ((delta << 1) ^ (delta >> 63)) as u64 as u128,
        );
        put(&mut self.bytes, rank as u128);
        if self.kind != Kind::Khmer || self.word_policy {
            put(&mut self.bytes, target);
        }
        if self.joined_forms && self.kind == Kind::Romanized {
            put(
                &mut self.bytes,
                source.map_or(0, |position| position as u128 + 1),
            );
        }
        self.previous.clear();
        self.previous.push_str(key);
        self.previous_id = id as i64;
        let block = self.blocks.last_mut().unwrap();
        let min = (block.1, block.2).min((rank, key.len()));
        block.1 = min.0;
        block.2 = min.1;
        self.count += 1;
    }
    fn finish(self) -> Result<Vec<u8>, EngineError> {
        let mut out = Vec::new();
        put32(&mut out, self.kind as usize)?;
        put32(&mut out, self.count)?;
        put32(&mut out, self.bytes.len())?;
        put32(&mut out, BLOCK)?;
        for (offset, rank, len) in self.blocks {
            put32(&mut out, offset)?;
            put32(&mut out, rank as usize)?;
            put32(&mut out, len)?;
        }
        out.extend(self.bytes);
        Ok(out)
    }
}
pub(crate) fn compile(
    schema: u32,
    romanized: BTreeMap<String, Vec<Candidate>>,
    khmer: BTreeMap<String, Candidate>,
    english: BTreeMap<String, Vec<EnglishCandidate>>,
    joined: BTreeSet<(String, u64)>,
    structures: BTreeSet<(String, Vec<String>)>,
    disabled: BTreeSet<String>,
) -> Result<Dictionary, EngineError> {
    let policy = !disabled.is_empty();
    let contextual = !structures.is_empty() || policy;
    let joined_forms = !joined.is_empty() || contextual;
    let sources: BTreeMap<u64, usize> = romanized
        .iter()
        .flat_map(|(key, rows)| rows.iter().map(move |row| (key, row.mapping_id.unwrap())))
        .enumerate()
        .filter(|(_, (key, id))| !joined.contains(&((*key).clone(), *id)))
        .map(|(position, (_, id))| (id, position))
        .collect();
    let outputs: BTreeMap<String, usize> = romanized
        .values()
        .flatten()
        .map(|row| row.khmer.clone())
        .chain(structures.iter().flat_map(|(parent, children)| {
            std::iter::once(parent.clone()).chain(children.iter().cloned())
        }))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .enumerate()
        .map(|(i, text)| (text, i))
        .collect();
    let mut pool: Vec<u8> = Vec::new();
    let mut offsets = vec![0];
    for text in outputs.keys() {
        pool.extend(text.as_bytes());
        offsets.push(pool.len());
    }
    let mut mappings = Builder::new(Kind::Romanized, joined_forms);
    for (key, rows) in romanized {
        for row in rows {
            let target = outputs[&row.khmer];
            let id = row.mapping_id.unwrap();
            let source = joined.contains(&(key.clone(), id)).then(|| sources[&id]);
            mappings.push(&key, id, row.rank, target as u128, source);
        }
    }
    let mut words = Builder::new(Kind::Khmer, joined_forms);
    words.word_policy = policy;
    for (key, row) in khmer {
        words.push(&key, row.mapping_id.unwrap(), row.rank, u128::from(disabled.contains(&key)), None);
    }
    let mut english_words = Builder::new(Kind::English, joined_forms);
    for (key, rows) in english {
        for row in rows {
            let mask = row.text.bytes().enumerate().fold(0u128, |mask, (i, b)| {
                if b.is_ascii_uppercase() {
                    mask | (1u128 << i)
                } else {
                    mask
                }
            });
            english_words.push(&key, row.mapping_id, row.rank, mask, None);
        }
    }
    let mut strings = Vec::new();
    put32(&mut strings, outputs.len())?;
    for offset in offsets {
        put32(&mut strings, offset)?;
    }
    strings.extend(pool);
    let mut sections = vec![
        mappings.finish()?,
        words.finish()?,
        english_words.finish()?,
        strings,
    ];
    if contextual && structures.is_empty() {
        sections.push(Vec::new());
    } else if contextual {
        let sequences = structures
            .into_iter()
            .map(|(parent, children)| {
                (
                    outputs[&parent],
                    children.iter().map(|word| outputs[word]).collect(),
                )
            })
            .collect::<Vec<_>>();
        sections.push(crate::context::compile(&sequences)?);
    }
    let mut bytes = if policy {
        POLICY_MAGIC
    } else if contextual {
        CONTEXT_MAGIC
    } else if joined_forms {
        JOINED_MAGIC
    } else {
        MAGIC
    }
    .to_vec();
    put32(&mut bytes, schema as usize)?;
    for section in &sections {
        put32(&mut bytes, section.len())?;
    }
    for section in sections {
        bytes.extend(section);
    }
    let hash = checksum(&bytes);
    bytes.extend(hash.to_le_bytes());
    Dictionary::from_bytes(bytes)
}

pub(crate) fn compile_sorted_english(
    words: impl IntoIterator<Item = super::ApprovedEnglishWord>,
) -> Result<Dictionary, EngineError> {
    let mut builder = Builder::new(Kind::English, false);
    let mut previous: Option<(String, String)> = None;
    for word in words {
        if word.id == 0
            || word.id > MAX_SAFE_INTEGER
            || word.rank > 1_000_000
            || !super::valid_english(&word.text)
        {
            return Err(invalid("Invalid approved English word fields."));
        }
        let key = word.text.to_ascii_lowercase();
        if previous
            .as_ref()
            .is_some_and(|(k, t)| (k.as_str(), t.as_str()) >= (key.as_str(), word.text.as_str()))
        {
            return Err(invalid("Streaming English input must have distinct spellings sorted by lowercase key and exact spelling."));
        }
        let mask = word.text.bytes().enumerate().fold(0u128, |mask, (i, b)| {
            if b.is_ascii_uppercase() {
                mask | (1u128 << i)
            } else {
                mask
            }
        });
        builder.push(&key, word.id, word.rank, mask, None);
        previous = Some((key, word.text));
    }
    let sections = [
        Builder::new(Kind::Romanized, false).finish()?,
        Builder::new(Kind::Khmer, false).finish()?,
        builder.finish()?,
        vec![0u8; 8],
    ];
    let mut bytes = MAGIC.to_vec();
    put32(&mut bytes, 3)?;
    for section in &sections {
        put32(&mut bytes, section.len())?;
    }
    for section in sections {
        bytes.extend(section);
    }
    let hash = checksum(&bytes);
    bytes.extend(hash.to_le_bytes());
    Dictionary::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_search_matches_exhaustive_bounds_across_duplicate_keys_and_partial_blocks() {
        for count in [0, 1, 15, 16, 17, 31, 32, 33, 100, 257] {
            // Duplicate runs exceed a block and start away from its boundary.
            let rows: Vec<_> = (0..count).map(|i|
                (format!("a{}", char::from(b'a' + (i / 35) as u8)), (i + 1) as u64, 0, 0, None)).collect();
            let mut builder = Builder::new(Kind::Romanized, false);
            for (key, id, rank, target, source) in &rows { builder.push(key, *id, *rank, *target, *source); }
            let bytes = builder.finish().unwrap();
            let index = Index::parse(Arc::new(bytes.clone()), 0, bytes.len(), Kind::Romanized, false).unwrap();
            for key in ["", "a", "aa", "aaa", "ab", "ac", "ad", "ah", "ai", "z"] {
                for upper in [false, true] {
                    let expected = rows.partition_point(|row| row.0.as_str() < key || (upper && row.0 == key));
                    assert_eq!(index.bound(key, upper), expected, "{count}: {key}/{upper}");
                }
            }
        }
    }

    #[test]
    fn reviewed_completion_bounds_match_full_scans_after_ranking_replacement() {
        use crate::{Engine, SuggestionFeatures, TypingContext, TypingMode};
        use serde_json::json;
        let rows: Vec<_> = (0..129).map(|i| (
            format!("x{}{}", char::from(b'a' + (i / 26) as u8), char::from(b'a' + (i % 26) as u8)),
            format!("ក{}{}", char::from_u32(0x1780 + i / 33).unwrap(), char::from_u32(0x1780 + i % 33).unwrap())
        )).collect();
        let dataset = json!({"schemaVersion":3,
            "mappings":rows.iter().enumerate().map(|(i,(key,word))| json!({"id":i+1,"romanized":key,"khmer":word,"rank":i%7,"status":"approved"})).collect::<Vec<_>>(),
            "words":rows.iter().enumerate().map(|(i,(_,word))| json!({"id":i+1,"khmer":word,"rank":i%7,"status":"approved"})).collect::<Vec<_>>(),
            "englishWords":rows.iter().enumerate().map(|(i,(word,_))| json!({"id":i+1,"text":word,"rank":i%7,"status":"approved"})).collect::<Vec<_>>()});
        let mut optimized = Engine::from_json(&dataset.to_string()).unwrap();
        let original = optimized.to_binary();
        let mut full_scan = Engine::from_binary(original.clone()).unwrap();
        for epoch in 0..4 {
            let level = |i:usize| if epoch == 0 { 3 } else { ((i * 7 + epoch * 13) % 5 + 1) as u8 };
            let ranking = json!({"format":"kmeow-word-ranking-v1",
                "khmer":rows.iter().enumerate().map(|(i,(_,word))| json!({"word":word,"level":level(i)})).collect::<Vec<_>>(),
                "english":rows.iter().enumerate().map(|(i,(word,_))| json!({"word":word,"level":level(i)})).collect::<Vec<_>>()});
            optimized.load_word_ranking_json(&ranking.to_string()).unwrap();
            full_scan.load_word_ranking_json(&ranking.to_string()).unwrap();
            for index in [&mut full_scan.dictionary.romanized, &mut full_scan.dictionary.khmer, &mut full_scan.dictionary.english] {
                index.completion_bounds = Default::default();
            }
            for mode in [TypingMode::Khmer, TypingMode::English] {
                let features = SuggestionFeatures { mode, sound_map:false, khmer_completion:true, ..Default::default() };
                for input in ["x", "xa", "xaa", "xb", "xe", "ក", "កក", rows[0].1.as_str()] {
                    assert_eq!(optimized.suggest_with_context(input, features, &TypingContext::default()).unwrap(),
                        full_scan.suggest_with_context(input, features, &TypingContext::default()).unwrap(), "epoch={epoch}, input={input}");
                }
            }
            assert_eq!(optimized.to_binary(), original);
        }
    }

    fn encoded_rows(rows: &[(&str, u64, u32, u128, Option<usize>)], joined: bool) -> Vec<u8> {
        let mut romanized = Builder::new(Kind::Romanized, joined);
        for &(key, id, rank, target, source) in rows {
            romanized.push(key, id, rank, target, source);
        }
        let mut pool = Vec::new();
        let outputs = ["ក", "ខ"];
        put32(&mut pool, outputs.len()).unwrap();
        put32(&mut pool, 0).unwrap();
        put32(&mut pool, outputs[0].len()).unwrap();
        put32(&mut pool, outputs.iter().map(|text| text.len()).sum()).unwrap();
        for output in outputs {
            pool.extend(output.as_bytes());
        }
        let sections = [
            romanized.finish().unwrap(),
            Builder::new(Kind::Khmer, joined).finish().unwrap(),
            Builder::new(Kind::English, joined).finish().unwrap(),
            pool,
        ];
        let mut bytes = if joined { JOINED_MAGIC } else { MAGIC }.to_vec();
        put32(&mut bytes, 2).unwrap();
        for section in &sections {
            put32(&mut bytes, section.len()).unwrap();
        }
        for section in sections {
            bytes.extend(section);
        }
        bytes.extend(checksum(&bytes).to_le_bytes());
        bytes
    }

    #[test]
    fn joined_rows_require_a_matching_canonical_source_and_keep_unique_source_ids() {
        for id in [1, MAX_SAFE_INTEGER] {
            let original = ("a b", id, 2, 0, None);
            let derived = ("ab", id, 2, 0, Some(0));
            let dictionary =
                Dictionary::from_bytes(encoded_rows(&[original, derived], true)).unwrap();
            assert_eq!(dictionary.mapping_count, 1);
            assert_eq!(dictionary.romanized.len, 2);
            for bad in [
                ("ab", id, 2, 0, None),
                ("ab", id, 2, 0, Some(1)),
                ("ab", id, 2, 0, Some(2)),
                ("ab", id, 3, 0, Some(0)),
                ("ab", id, 2, 1, Some(0)),
                ("ab", if id == 1 { 2 } else { 1 }, 2, 0, Some(0)),
                ("ac", id, 2, 0, Some(0)),
            ] {
                assert!(Dictionary::from_bytes(encoded_rows(&[original, bad], true)).is_err());
            }
            assert!(Dictionary::from_bytes(encoded_rows(
                &[original, derived, ("ac", id, 2, 0, None)],
                true
            ))
            .is_err());
            assert!(Dictionary::from_bytes(encoded_rows(
                &[original, derived, ("ac", id, 2, 0, Some(1))],
                true
            ))
            .is_err());
            assert!(
                Dictionary::from_bytes(encoded_rows(&[original, derived, derived], true)).is_err()
            );
            // Legacy binaries still reject every repeated source ID.
            assert!(Dictionary::from_bytes(encoded_rows(&[original, derived], false)).is_err());
        }
        assert!(Dictionary::from_bytes(encoded_rows(&[("ab", 1, 2, 0, None)], true)).is_err());
    }

    #[test]
    fn joined_source_references_cross_blocks_without_retaining_a_decoded_index() {
        let mut rows = vec![("a z".to_owned(), 1, 0, 0, None)];
        for letter in b'a'..=b'y' {
            rows.push((
                format!("a{}", char::from(letter)),
                u64::from(letter),
                0,
                0,
                None,
            ));
        }
        rows.push(("az".to_owned(), 1, 0, 0, Some(0)));
        let borrowed: Vec<_> = rows
            .iter()
            .map(|(key, id, rank, target, source)| (key.as_str(), *id, *rank, *target, *source))
            .collect();
        let bytes = encoded_rows(&borrowed, true);
        let engine = super::super::Engine::from_binary(bytes.clone()).unwrap();
        assert_eq!(
            engine.convert("a z").unwrap().candidates,
            engine.convert("az").unwrap().candidates
        );
        for position in 8..bytes.len() - 8 {
            let mut corrupt = bytes.clone();
            corrupt[position] ^= 0x80;
            let end = corrupt.len() - 8;
            let hash = checksum(&corrupt[..end]);
            corrupt[end..].copy_from_slice(&hash.to_le_bytes());
            assert!(
                std::panic::catch_unwind(|| Dictionary::from_bytes(corrupt)).is_ok(),
                "parser panicked at {position}"
            );
        }
    }
}

// Split encoded sections without rebuilding entries, ranks, IDs or context.
// All inputs have already passed Dictionary::from_bytes validation.
fn sections(data: &[u8]) -> Vec<&[u8]> {
    let mut start = if &data[..8] == CONTEXT_MAGIC || &data[..8] == POLICY_MAGIC { 32 } else { 28 };
    let header = start;
    (12..header).step_by(4).map(|offset| {
        let length = number(data, offset).expect("validated compact section");
        let section = &data[start..start + length];
        start += length;
        section
    }).collect()
}
fn assemble(magic: &[u8], schema: &[u8], sections: &[&[u8]]) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend_from_slice(magic);
    result.extend_from_slice(schema);
    for section in sections { result.extend_from_slice(&(section.len() as u32).to_le_bytes()); }
    for section in sections { result.extend_from_slice(section); }
    result.extend_from_slice(&checksum(&result).to_le_bytes());
    result
}
pub(crate) fn split(dictionary: &Dictionary) -> (Vec<u8>, Vec<u8>) {
    let data = &dictionary.data;
    let sections = sections(data);
    let romanized = Builder::new(Kind::Romanized, false).finish().expect("empty index");
    let khmer = Builder::new(Kind::Khmer, false).finish().expect("empty index");
    let english = Builder::new(Kind::English, false).finish().expect("empty index");
    let pool = [0u8; 8];
    let mut km = sections.clone();
    km[2] = &english;
    (assemble(&data[..8], &data[8..12], &km),
     assemble(MAGIC, &3u32.to_le_bytes(), &[&romanized, &khmer, sections[2], &pool]))
}
pub(crate) fn join(khmer: Vec<u8>, english: Vec<u8>) -> Result<Vec<u8>, EngineError> {
    let km = Dictionary::from_bytes(khmer)?;
    let en = Dictionary::from_bytes(english)?;
    if km.english.len != 0 || en.romanized.len != 0 || en.khmer.len != 0
        || en.pool.count != 0 || en.context.is_some() || &en.data[..8] != MAGIC {
        return Err(invalid("App typing sections are not language-specific."));
    }
    let mut sections = sections(&km.data);
    sections[2] = self::sections(&en.data)[2];
    let result = assemble(&km.data[..8], &km.data[8..12], &sections);
    Dictionary::from_bytes(result.clone())?;
    Ok(result)
}
