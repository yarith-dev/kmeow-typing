//! Two self-contained language bundles. Hashes detect corruption/mismatched pairs,
//! not malicious publishers. No filesystem or platform dependencies.
use crate::{compact, invalid, word_metadata::Lexicon, Engine, EngineError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

const MAGIC: &[u8; 8] = b"KMEOWA01";
const HEADER: usize = 24;
const MAX_MANIFEST: usize = 2 * 1024 * 1024;
const MAX_SECTION: usize = 32 * 1024 * 1024;
pub(crate) const MAX_PLAIN_APP_DATA_BYTES: usize = HEADER + MAX_MANIFEST + 2 * MAX_SECTION + 32;
pub const MAX_APP_DATA_BYTES: usize = MAX_PLAIN_APP_DATA_BYTES;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    format: String,
    language: String,
    pair_id: String,
    dataset_hash: String,
    compact_artifact_hash: String,
    snapshot_id: String,
    // Retain exact JSON bytes for reproducible pair hashing, including notices.
    provenance_json: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataHeader {
    format: String, language: String, snapshot_id: String,
    words: serde::de::IgnoredAny,
}
pub(crate) struct Loaded {
    manifest: Manifest,
    pub(crate) metadata: Option<(String, String)>,
    lexicon: OnceLock<Lexicon>,
}
fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn valid_hash(value: &str) -> bool { value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) }
fn pair_id(kc: &[u8], ec: &[u8], km: &[u8], em: &[u8], manifest: &Manifest) -> String {
    let mut digest = Sha256::new();
    for section in [kc, ec, km, em, manifest.dataset_hash.as_bytes(), manifest.compact_artifact_hash.as_bytes(),
        manifest.snapshot_id.as_bytes(), manifest.provenance_json.as_bytes()] {
        digest.update((section.len() as u64).to_le_bytes()); digest.update(section);
    }
    format!("{:x}", digest.finalize())
}
fn metadata_header(json: &str, language: &str) -> Result<String, EngineError> {
    let header: MetadataHeader = serde_json::from_str(json).map_err(|e| invalid(&e.to_string()))?;
    let _ = header.words;
    if !matches!(header.format.as_str(), "kmeow-word-metadata-v1" | "kmeow-word-metadata-v2") || header.language != language || !valid_hash(&header.snapshot_id) {
        return Err(invalid("Invalid app metadata header."));
    }
    Ok(header.snapshot_id)
}
fn encode(manifest: &Manifest, compact: &[u8], metadata: &[u8]) -> Result<Vec<u8>, EngineError> {
    let json = serde_json::to_vec(manifest).map_err(|e| invalid(&e.to_string()))?;
    if json.len() > MAX_MANIFEST || compact.len() > MAX_SECTION || metadata.len() > MAX_SECTION {
        return Err(invalid("App data exceeds section limits."));
    }
    let mut result = MAGIC.to_vec();
    result.extend_from_slice(&(if manifest.language == "khmer" { 1u32 } else { 2u32 }).to_le_bytes());
    for size in [json.len(), compact.len(), metadata.len()] { result.extend_from_slice(&(size as u32).to_le_bytes()); }
    result.extend(json); result.extend_from_slice(compact); result.extend_from_slice(metadata);
    result.extend_from_slice(&Sha256::digest(&result));
    Ok(result)
}
fn decode<'a>(bytes: &'a [u8], language: &str) -> Result<(Manifest, &'a [u8], &'a str), EngineError> {
    if bytes.len() < HEADER + 32 || bytes.len() > MAX_PLAIN_APP_DATA_BYTES || &bytes[..8] != MAGIC {
        return Err(invalid("Unsupported app data format."));
    }
    let end = bytes.len() - 32;
    if Sha256::digest(&bytes[..end])[..] != bytes[end..] { return Err(invalid("App data checksum mismatch.")); }
    let number = |offset| u32::from_le_bytes(bytes[offset..offset+4].try_into().unwrap()) as usize;
    let expected = if language == "khmer" { 1 } else { 2 };
    let sizes = [number(12), number(16), number(20)];
    if number(8) != expected || sizes[0] > MAX_MANIFEST || sizes[1] > MAX_SECTION || sizes[2] > MAX_SECTION
        || HEADER + sizes.iter().sum::<usize>() != end { return Err(invalid("Invalid app data sections.")); }
    let compact_start = HEADER + sizes[0];
    let metadata_start = compact_start + sizes[1];
    let manifest: Manifest = serde_json::from_slice(&bytes[HEADER..compact_start]).map_err(|e| invalid(&e.to_string()))?;
    let metadata = std::str::from_utf8(&bytes[metadata_start..end]).map_err(|e| invalid(&e.to_string()))?;
    if manifest.format != "kmeow-app-data-v1" || manifest.language != language
        || !valid_hash(&manifest.pair_id) || !valid_hash(&manifest.dataset_hash) || !valid_hash(&manifest.compact_artifact_hash)
        || metadata_header(metadata, language)? != manifest.snapshot_id {
        return Err(invalid("Invalid app data manifest."));
    }
    Ok((manifest, &bytes[compact_start..metadata_start], metadata))
}
fn verify_provenance(engine: &Engine, manifest: &Manifest) -> Result<Value, EngineError> {
    let provenance: Value = serde_json::from_str(&manifest.provenance_json).map_err(|e| invalid(&e.to_string()))?;
    let compact = &provenance["compactManifest"];
    let stats: Value = serde_json::from_str(&engine.stats_json()).unwrap();
    if compact["artifactHash"] != manifest.compact_artifact_hash || compact["datasetHash"] != manifest.dataset_hash
        || provenance["metadataManifest"]["snapshotId"] != manifest.snapshot_id
        || provenance["metadataManifest"]["compactArtifactHash"] != manifest.compact_artifact_hash
        || ["mappings", "words", "englishWords", "encodedBytes"].iter().any(|key| compact[key] != stats[key])
        || provenance["notices"].as_array().is_none_or(|v| v.is_empty()) {
        return Err(invalid("App data provenance does not match its payload."));
    }
    Ok(provenance)
}
fn verify_metadata(manifest: &Manifest, khmer: &str, english: &str) -> Result<(), EngineError> {
    let provenance: Value = serde_json::from_str(&manifest.provenance_json).map_err(|e| invalid(&e.to_string()))?;
    let metadata = &provenance["metadataManifest"];
    if metadata["format"] != "kmeow-word-metadata-bundle-v1" { return Err(invalid("Invalid embedded metadata manifest.")); }
    for (language, json) in [("khmer", khmer), ("english", english)] {
        let file = &metadata["files"][language];
        if file["file"] != format!("{language}-words.json") || file["bytes"].as_u64() != Some(json.len() as u64)
            || file["sha256"] != hash(json.as_bytes()) {
            return Err(invalid("Embedded metadata does not match its source manifest."));
        }
    }
    Ok(())
}
fn verify_typing_words(engine: &Engine, lexicon: &Lexicon) -> Result<(), EngineError> {
    for (language, index) in [("khmer", &engine.dictionary.khmer), ("english", &engine.dictionary.english)] {
        for word in lexicon.entries(language)?.values() {
            // English lookup is case-insensitive; Khmer spellings must match exactly.
            let key = if language == "english" { word.word.to_ascii_lowercase() } else { word.word.clone() };
            if !index.contains(&key) {
                return Err(invalid(&format!("Metadata word is missing from the {language} typing dictionary: {}", word.word)));
            }
        }
    }
    Ok(())
}
impl Engine {
    /// Build only from a pinned compact release and an independently approved metadata pair.
    pub fn compile_app_data(&self, khmer: &str, english: &str, provenance: &str) -> Result<(Vec<u8>, Vec<u8>), EngineError> {
        let lexicon = Lexicon::from_pair(khmer, english)?;
        verify_typing_words(self, &lexicon)?;
        let source: Value = serde_json::from_str(provenance).map_err(|e| invalid(&e.to_string()))?;
        let mut manifest = Manifest {
            format: "kmeow-app-data-v1".into(), language: "khmer".into(), pair_id: String::new(),
            dataset_hash: source["compactManifest"]["datasetHash"].as_str().unwrap_or("").into(),
            compact_artifact_hash: hash(self.compiled_bytes()), snapshot_id: metadata_header(khmer, "khmer")?,
            provenance_json: provenance.into(),
        };
        if !valid_hash(&manifest.dataset_hash) { return Err(invalid("Invalid app dataset hash.")); }
        verify_provenance(self, &manifest)?;
        verify_metadata(&manifest, khmer, english)?;
        let (kc, ec) = compact::split(&self.dictionary);
        manifest.pair_id = pair_id(&kc, &ec, khmer.as_bytes(), english.as_bytes(), &manifest);
        let kb = encode(&manifest, &kc, khmer.as_bytes())?;
        manifest.language = "english".into();
        let eb = encode(&manifest, &ec, english.as_bytes())?;
        Engine::from_app_data(&kb, &eb)?;
        Ok((kb, eb))
    }
    pub fn from_app_data(khmer: &[u8], english: &[u8]) -> Result<Self, EngineError> {
        // Plain dictionary bundles retain checksum, section, pair and provenance validation.
        let (km, kc, kj) = decode(khmer, "khmer")?;
        let (em, ec, ej) = decode(english, "english")?;
        let mut mirror = em; mirror.language = "khmer".into();
        if km != mirror || pair_id(kc, ec, kj.as_bytes(), ej.as_bytes(), &km) != km.pair_id {
            return Err(invalid("App data files are not a matching pair."));
        }
        let bytes = compact::join(kc.to_vec(), ec.to_vec())?;
        if hash(&bytes) != km.compact_artifact_hash { return Err(invalid("App typing release hash mismatch.")); }
        let mut engine = Self::from_binary(bytes)?;
        let provenance = verify_provenance(&engine, &km)?;
        verify_metadata(&km, kj, ej)?;
        engine.set_commonness(crate::word_metadata::Commonness::from_pair(kj, ej)?);
        engine.load_shared_words(&provenance)?;
        engine.load_word_components(&provenance)?;
        engine.load_pronunciations(&provenance)?;
        engine.load_input_exclusions(&provenance)?;
        engine.load_shorthands(&provenance)?;
        engine.app_data = Some(Loaded { manifest: km, metadata: Some((
            kj.to_owned(), ej.to_owned()
        )), lexicon: OnceLock::new() });
        Ok(engine)
    }
    pub(crate) fn word_lexicon(&self) -> Result<&Lexicon, EngineError> {
        if let Some(app) = &self.app_data {
            if let Some((khmer, english)) = &app.metadata {
                if let Some(lexicon) = app.lexicon.get() { return Ok(lexicon); }
                let lexicon = Lexicon::from_pair(khmer, english)?;
                let _ = app.lexicon.set(lexicon);
                return Ok(app.lexicon.get().expect("initialized lexical metadata"));
            }
        }
        Ok(&self.lexicon)
    }
    pub fn app_data_info_json(&self) -> String {
        self.app_data.as_ref().map_or_else(|| "null".into(), |app| {
            let mut value = serde_json::to_value(&app.manifest).unwrap();
            let provenance = serde_json::from_str::<Value>(&app.manifest.provenance_json).unwrap();
            value.as_object_mut().unwrap().remove("language");
            value.as_object_mut().unwrap().remove("provenanceJson");
            value["provenance"] = provenance;
            value.to_string()
        })
    }
}
