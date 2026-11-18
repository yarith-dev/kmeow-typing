# Dictionary formats

Applications normally load the included pair through `createEngine()` or Rust's `Engine::bundled()`. Custom datasets should be compiled by the Rust API, which owns encoding and validation. The dictionaries are unencrypted offline data; they require no database or network connection.

## Two language files

`compiled-data/khmer.kmd` and `compiled-data/english.kmd` form one matching `KMEOWA01` pair. Each contains:

| Offset | Contents |
| --- | --- |
| 0–7 | ASCII `KMEOWA01` |
| 8–11 | Little-endian u32 language code: 1 Khmer, 2 English |
| 12–23 | Three little-endian u32 lengths: manifest, compact section, metadata |
| 24 onward | Manifest JSON, language-specific compact bytes, word-metadata JSON |
| Last 32 bytes | SHA-256 of all preceding bytes |

The manifests share the pair identity, dataset identity, snapshot identity and combined compact hash. Pair hashing includes both compact sections, both metadata sections and the exact provenance JSON. The loader rejects corruption, mismatched pairs and invalid payloads before queries. SHA-256 protects integrity, not distributor authenticity: a party able to replace both data and manifest can replace their hashes.

The MIT metadata includes every word's original identity, spelling, commonness and available conventional POS codes. Meanings, translation catalogs and source evidence are absent. POS is descriptive and does not participate in suggestion ordering or automatic acceptance.

See `rust/src/app_data.rs` for the encoder and validator, and `rust/src/word_metadata.rs` for metadata contracts.

## Compact typing sections

The core accepts `KMEOWC01` through `KMEOWC05`. The original forms contain four sections: Romanized index, Khmer index, English index and output pool. Later forms add component structures and per-word Sound map policy. Each compact artifact ends with a little-endian FNV-1a64 checksum.

Indexes use sorted, prefix-compressed blocks of 16 rows. Their directories record byte offsets and ranking bounds. Keys are reconstructed only for rows visited during lookup; the runtime does not retain a decoded tree of all strings. IDs, ranks, alternate readings, explicit-selection policies and original English capitalization survive compilation. Joined Romanized forms retain their canonical source identities and do not create new words or aliases.

Rust checks section boundaries, checksums, canonical variable integers, UTF-8, sorted keys, IDs, references and ranking summaries. The native byte-vector loader takes ownership; the Wasm loader copies caller-provided bytes. Exact layouts and version-specific rules are maintained in `rust/src/compact.rs` and its regression tests.

`Engine::from_binary` loads compact bytes; `Engine::new` compiles a validated `ApprovedDataset`. `Engine::to_binary` returns a copy of the packed dictionary. Loading never implicitly renumbers words or upgrades their behavior.

### Signed ranks

Approved word and alias ranks are signed 32-bit integers restricted to −1,000,000…1,000,000. Lower ranks sort earlier within the existing ranking groups; commonness, rule priorities and explicit-selection policies keep their current roles.

The compiler keeps the original format and bytes for datasets containing no negative ranks. A dataset with a negative rank uses `KMEOWC05`, with the same five-section header and per-word policy layout as `KMEOWC04`. Positive rank codes remain 0…1,000,000; codes 1,000,001…2,000,000 decode as `1,000,000 − code` (−1…−1,000,000). Block-directory rank bounds are little-endian signed i32 values. Decoders validate the rank codes, candidate ordering and exact block minima. Older readers reject the new version instead of silently misinterpreting it.

Both language sections in a signed-rank pair use `KMEOWC05`, including empty context sections where required. Splitting and joining retain the encoded ranks and combined compact bytes. Existing `KMEOWA01` pair integrity and metadata checks are unchanged. Runtime-only rank overrides remain outside compact exports.
