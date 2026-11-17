# Kmeow Engine

Offline Latin → Khmer conversion and Khmer/English typing suggestions. The included dictionaries work immediately. Load once, then query synchronously; typing makes no network requests and needs no database, account or server.

## Start here

From this checkout, install the development tools and build the JavaScript API:

```sh
npm ci
npm run build
```

Node.js 22+ is required. The included Wasm and dictionaries are ready to use; Rust is needed only when changing the core. To install the engine into another project, run `npm pack` here, then `npm install /path/to/khmer-typing-engine-0.1.0.tgz` in that project.

```js
import { createEngine } from '@khmer-typing/engine';

const engine = await createEngine();
const result = engine.suggest('sievphov');
console.log(result.candidates[0]?.khmer); // សៀវភៅ

engine.dispose(); // when the editor/app is finished
```

Use `suggest()` for a typing UI: it includes Sound map readings, complete matches and completions. Use `convert()` for saved whole-input spellings and compositions of saved entries. `convert()` retains its original exact-conversion behavior; suggestion settings do not affect it. Among exact suggestions with equal rule priority, commonness comes before saved-spelling preference; saved spellings still win equal-commonness ties. This changes display order without editing words, aliases or stored ranks. Unknown inputs can return no candidates. Neither call inserts or changes editor text.

`engine.analyzeComponents(word)` finds partial Khmer components using the same dictionary and current personal words. It returns `source` (`saved` or `automatic`), `status` (`complete`, `partial`, `ambiguous` or `unknown`), `parts` and up to two competing `alternatives`. Each part contains its exact `text`, Unicode character `start`/`end` offsets and a `wordId` when it matches a dictionary entry. Unmatched text remains an unresolved part without a word ID. `moreAlternatives` reports additional competing splits. These are dictionary matches, not approvals of word meanings or pronunciation.

The Rust detector compares non-overlapping known spans by covered characters, allowing unknown prefixes, middles and suffixes. It never cuts an attached vowel/mark or coeng cluster, and the complete input word cannot count as its own sole component. At equal coverage, it prefers fewer recognized components; unresolved spans do not count as recognized words. Splits tied on both coverage and component count remain ambiguous, without rank or commonness guessing. For example, **ទេស + ធម៌** wins over **ទេ + ស + ធម៌**. Existing saved component definitions stay authoritative, even when an automatic split has fewer parts; multiple saved choices are retained. Available single-word Khmer entries are analyzed and cached on first use; other queries use a bounded 256-entry cache. Personal dictionary replacements invalidate their separate cache. Analysis never writes dictionary records or pronunciation rules. Clear, complete splits also supply cached context continuations: for example, សុខ + សប្បាយ can favor សប្បាយ after សុខ. Automatic continuations receive a modest boost below saved component and reviewed-rule context, without changing match priorities, exactness, stored ranks or typing inputs. Partial, unknown and ambiguous analyses contribute no detected boost; multiple saved alternatives remain preserved without guessing a choice. The manager's Khmer word details show saved or possible components, with unresolved text in brackets. **Edit word → Components** contains the compact component controls: keep the default analysis, explicitly mark a word Standalone, or enter existing Khmer parts in order with `+`, using one alternative per line. The single Save word action saves component and word-setting edits together. All saved alternatives remain editable; merely opening or saving an unchanged list is a no-op. These owner choices travel in the pair's `wordComponents` provenance and control both analysis and the runtime context cache. Standalone suppresses that parent's inferred and reviewed continuations; a saved unique split overrides automatic choices. The compact component/context bytes, generated readings, aliases and priorities remain unchanged. Resetting to default restores the original saved or automatic analysis and its eligible continuations.

Exclusions in either the base or personal dictionary keep a component unresolved; a personal duplicate cannot restore it to analysis. Saved boundaries remain visible when a part is unavailable, reported as partial rather than complete. Multiple saved alternatives remain ambiguous even if their availability differs. Ranking reuses the prepared component cache; a personal generation builds its complete union continuations once, so ambiguity introduced by a new personal part also removes an inferred boost. Replacing or clearing personal words releases the cached personal analysis immediately; a later query prepares the current view again.

## Local web manager

```sh
npm ci
npm run manage
```

The launcher builds a missing JavaScript API on the first start from a source checkout. Release archives use their included JavaScript and need no development dependencies. After editing the public TypeScript API, run `npm run build` before restarting the manager. From a project that installed the archive, use `npm --prefix node_modules/@khmer-typing/engine run manage`.

Open http://127.0.0.1:3001. The lightweight manager uses Studio’s visual style with three sections: **Typing**, **Sound map**, and **Word list**. It has no additional npm dependencies. Set `PORT` to use a different port. On phones, navigation moves to the bottom, typing options collapse, suggestions scroll horizontally, word-list entries show compact word → Romanized rows with details on tap, and dialogs open as bottom sheets. English / ខ្មែរ selects the interface language independently of the typing and word-list languages. The choice is saved in this browser; navigation, controls, dialogs, POS labels and Sound map explanations are translated without modifying words, readings, editor text or undo history. The same editor and saved data are shared across layouts.

Typing uses the same engine and dictionaries in a sentence composer: click or Enter inserts the chosen suggestion at the message cursor, arrow keys choose candidates, and Space can insert eligible exact matches. Hold a suggestion for half a second to open its word-management dialog without inserting it; right-click or Shift+F10 opens the same dialog. Scrolling cancels the hold. Dictionary words show their saved spellings; generated English forms can be added independently of their base word. Shift+Space keeps a literal space in the input. Khmer suggestion tiles show Romanized spellings below the word; English tiles show the word once. Status dots stack vertically at the top right: green Match, blue Completion, amber Correction, purple Select explicitly. Fully typed spellings show green Match, including exact English and direct Khmer results; longer completions stay blue. All applicable states appear together, such as blue plus purple for a completion requiring explicit selection; tooltips and accessible labels list every state. The editable message supports native text selection, selection replacement, undo/redo, newlines and copying. Shift+arrow shortcuts select text; unmodified Up/Down chooses suggestions. Native editing preserves selection handles and direction, including backward selections restored by undo/redo. Enter and Space refresh suggestions against the current message cursor before accepting a choice, including cursor updates reported on blur. Context is derived from the message cursor, bounded to the engine’s Unicode limits, and retained for untouched Khmer choices when editing. Empty composition can show next-word suggestions; Space still accepts only eligible exact matches. IME completion creates a single undo step, and explicit-selection flags stay protected. Sound map provides searchable built-in mappings, five consonants per row at desktop widths, and inline collapsible character details. Identical A/O typing-key lists share one row with a blue/green AO badge; differing lists keep separate series rows. Word list shows every shared Khmer/English word, with language tabs, search, a starting-letter index, numbered pages and a page jump. Khmer consonants and independent vowels or English A–Z filter the current search without changing word order; All clears the letter filter, and Other keeps browser words with other initials reachable. Letters with no matches are disabled. The index scrolls horizontally on phones. The Khmer-only No Romanized spelling filter finds words with no saved, browser-personal or generated Romanized readings; it combines with search, letters and pagination. Turning it off restores the matching list, and switching word languages clears it. This filter only changes the displayed rows. Pagination shows the visible word range and brings the first rows into view after a page change. With Khmer selected, Example opens 42 topics containing 356 original sentences, with Romanized text above Khmer. Each page has a Back link; hash navigation preserves the word-list filters and typing draft. Topics open as read-only paragraphs. Edit reveals the Romanized input and updates the Khmer preview through the current engine. Add example opens a blank editor with a required title, Romanized input and live Khmer preview. Save adds it to this browser’s topic list, where it can be reopened and edited; Cancel discards the new draft without writing. Custom titles stay as entered in either interface language. Save keeps each topic’s edits in this browser; Cancel restores its last saved version. Both return to reading mode. Unsaved drafts survive navigation within the manager. Tap a word in either the Romanized or Khmer paragraph to choose an alternative from the engine and save that choice with the example. The picker also follows the cursor or a selection within the Romanized editor and queries the entire word, including the suffix after the cursor. Explicit completions replace only that word’s full spelling; exact alternatives can be saved without changing the Romanized text. Cancel restores the saved choices. Native Undo and Redo restore word choices with their text through a bounded in-memory history, including grouped and IME edits. Unrecognized words stay as typed, and unchanged words retain their existing choices, including repeated words shifted by native edits or explicit completions. If conversion fails, the raw draft stays intact across navigation and interface-language changes; stale preview choices are disabled until conversion succeeds or Cancel restores the saved paragraph. Example choices remain local to the paragraph and do not add dictionary entries, aliases or rules or change engine ranking. Add word and Edit word save directly to the shared dictionary files; imports merge words and spellings. Saving rebuilds and validates both `.kmd` files and `dictionary-manifest.json`, then reloads Typing without replacing its draft or undo history. Generated Sound map spellings stay separate from editable saved spellings. The compact word editor includes Use default Sound map, Word level, saved spellings and Advanced → Rank. Turning Sound map off disables generated Romanized readings for that exact Khmer spelling while retaining its saved aliases and direct Khmer completion. Word levels use the existing five-level scale; levels 4 and 5 require explicit selection when typing. Existing word text, IDs, POS and rules remain intact; the editor does not rename or delete words. New words require an explicit owner suitability review and default to unrated and rank 1. Spellings, Sound map policy, levels and rank change only when explicitly saved; opening a form or editing an unrelated field preserves existing policies and per-identity ratings.

The compiled dictionary pair is the authoritative editable data. The manager losslessly recovers its source through the shared Rust core before applying an edit; opening the site or saving an unchanged entry never rewrites dictionary bytes. Saves retain only the current dictionary pair and manifest, without automatic backups or persistent change history. Temporary transaction files protect a save from partial writes and are removed after success or rollback. Interrupted transactions are resolved and their temporary files removed before the next read. Saves use a same-origin local token, an exclusive writer lock and revision checks; stale pages must Refresh before saving. Each lock attempt has its own temporary claim, preventing stale-process cleanup from removing a newer writer's lock. The manager remains bound to `127.0.0.1`.

Previous browser words remain active in that browser until explicitly imported with Import browser words; they are never automatically admitted or erased. A recovery export remains available, including for unreadable old storage. After successful import, only the imported browser entries are retired. Their explicit typing preferences are embedded in the dictionary pair and loaded by the Rust core, preserving literal choices such as `lets` and capitalized personal spellings in every client. Case variants reuse existing English lexical identities; migrated preferences do not overwrite existing word or alias ranks. Personal words added later are merged into one bounded, cached runtime view. Ordinary edits and imports now persist in the shared files rather than browser storage. Export words saves editable word text, language, rank, saved spellings and Sound map policy as JSON; it includes word levels when the word’s stored identities agree, preserving differing per-identity ratings on ordinary round trips; it includes shared typing preferences and accepts the previous personal-word format too. Other open manager pages can Refresh to load a saved generation. Changes reach another application when it loads or packages this updated pair; already installed native apps are not rebuilt or modified by the manager.

Run `npm run test:manager` after building to verify persistence and local asset serving. The optional manager’s bundled Noto Sans Khmer font uses the SIL Open Font License; see `manager/fonts/LICENSE`.

## Choose behavior

Five settings cover ordinary use:

| Setting | Default | Controls |
| --- | --- | --- |
| `language` | `'khmer'` | Khmer/Latin → Khmer, or `'english'` suggestions |
| `conversion` | `true` | Romanized suggestions in Khmer mode |
| `soundMap` | `true` | Generated pronunciation readings, alongside saved spellings |
| `completion` | `true` | Completions while typing directly in Khmer or English |
| `correction` | `false` | English spelling/contraction corrections |

```js
engine.configure({ language: 'english', correction: true });
engine.suggest('recieve');

// A request override leaves the engine's saved settings alone.
engine.suggest('សៀវ', { language: 'khmer' });
console.log(engine.settings);
```

The same settings can be passed to `createEngine()`. Latin → Khmer suggestions retain the core's existing prefix behavior through `conversion` and `soundMap`, independently of direct word completion. English correction is opt-in. Results keep the core's original order, IDs, `completion`, `correction`, `tapOnly` and `manualOnly` flags. Khmer outputs use `khmer`; English outputs use `text`. A keyboard must honor explicit-selection flags and its own Auto/Manual and protected-field rules; a first suggestion is not permission to insert text automatically.

Context is optional and never retained:

```js
engine.suggest('lets', { language: 'english', before: 'She ', after: ' us go' });
engine.suggest('ស', { language: 'khmer', previousWords: ['កូន'] });
```

## Add your own words and spellings

```js
engine.addWords([
  { word: 'សៀវភៅ', spellings: ['mybook'] },
  { word: 'kmeow', language: 'english' },
]);

engine.convert('mybook');
engine.suggest('kme', { language: 'english' });
```

Khmer is the default language. Khmer words can have several lowercase Latin spellings; English words use their own spelling. A word without aliases still participates in its language's completions. Optional `rank` is an integer from 0 to 1,000,000 (lower first, default 0). Repeated additions merge spellings. Invalid batches fail atomically. Bundled duplicate identities, exclusions and rare-word explicit-selection protections remain in force.

Personal additions are in memory and do not change bundled files or shared vocabulary. Persist them wherever your application stores settings:

```js
const savedWords = JSON.stringify(engine.getWords());
// Later, restore them during setup:
const restored = await createEngine({ words: JSON.parse(savedWords) });
engine.clearWords(); // returns to the original dictionary's behavior
```

## Browser

Serve the exported package as static files, then import its browser module:

```html
<script type="module">
  import { createEngine } from './kmeow/dist/public/browser.js';
  const engine = await createEngine();
  console.log(engine.suggest('sievphov').candidates);
</script>
```

Bundlers can use `@khmer-typing/engine` through its browser condition, or explicitly import `@khmer-typing/engine/simple/browser`. Copy `wasm/` and `compiled-data/` into your static assets and point at their parent directory, with a trailing slash:

```js
const engine = await createEngine({ assets: new URL('/kmeow/', location.origin) });
```

The browser loads the Wasm module and two dictionary files during setup. Serve Wasm as `application/wasm`; array-buffer fallback is supported. Your application controls offline caching. Supplying `dictionary: { khmer, english }` with `Uint8Array` values and optional browser `wasm` bytes lets you manage asset loading yourself. Compact bytes or a core dataset can also be supplied through `dictionary`.

## Native Rust

Use the Rust crate directly:

```toml
[dependencies]
khmer-engine = { path = "path/to/kmeow/rust", features = ["bundled-dictionaries"] }
```

```rust
use khmer_engine::Engine;

let engine = Engine::bundled()?;
let result = engine.suggest("sievphov")?;
```

The bundled feature is opt-in so existing native apps keep supplying their own dictionary bytes. Native customization uses `Engine::from_personal_words`, `convert_with_extension` and `suggest_with_extension`; the same Rust implementation powers JavaScript. `cargo test --locked -p khmer-engine --features bundled-dictionaries` runs the source tests.

## Where to customize rules

Ordinary apps need only the settings and personal-word API above. Contributors changing language rules can start with:

| Change | Source |
| --- | --- |
| Letter/vowel mappings and finite rule data | `data/khmer-sound-map.json` |
| Parsing and pronunciation rules | `rust/src/sound_map.rs`, `rust/src/sound_map/` |
| Personal-word schema, validation and merging | `rust/src/personal.rs` |
| English correction and contextual choices | `rust/src/english_correction.rs`, `rust/src/english_word_choices.rs` |
| English inflection rules and finite compatibility choices | `rust/src/english_surface_forms.rs`, `rust/src/english_surface_forms/` |
| Public settings and API | `src/public/` |
| Expected readings and scope boundaries | `test/fixtures/`, `rust/tests/` |

Word-part definitions live in `runtime.wordParts`: one Khmer spelling and shared keys, with ordered cases for default-reading overrides, short endings, terminal roots and additive scoped readings. A case retains its own word list, position limits, fallback behavior and optional key-order override. The Rust configuration loader builds the phase-specific indexes from this single registry; do not combine word lists and positions across cases. `runtime.nasalBoundaries` similarly groups written and unwritten nasal-boundary patterns while retaining each pattern’s initial letters, clusters and final restrictions.

A `syllable` case replaces letter-by-letter readings wherever the complete part matches a parsed syllable boundary, without listing every compound. **សាស្ត្រ** uses this shared case with only `sah` / `sas`; `sastro` is no longer generated. `originalSpellingOnly: true` prevents a cancelled tail from exposing a new match, and attached vowels, marks and subscripts keep their separate spellings (for example **សាស្ត្រាចារ្យ**). Existing checked prefix choices stay independent. The native and Wasm tests share `test/fixtures/checked-science-ending.json`, including unlisted and medial compounds; after a matched part, following syllables use their ordinary readings.

This is one generic default-reading override rule, backed by the `runtime.wordParts` list. Add another entry's Khmer spelling, shared keys and `syllable` case to use the same matcher; no word-specific Rust code or compound list is needed. It replaces only generated readings. Explicit dictionary spellings and user-defined personal spellings remain accepted unchanged, including a user-saved `sastro` for **សាស្ត្រ**. The personal-word API remains a separate layer; these shared rules never rewrite a user's saved words or spellings.

Override cases can optionally restrict `words` to exact complete word identities and `positions` to `start`, `middle`, `end` or `standalone`. Omitted limits keep a reusable pattern unrestricted. Restricted cases also verify the original character offsets and attached marks, so a shortened parser view cannot expose a new terminal match. Stored components, independently parsed suffixes and nested terminal roots retain the parent word identity and absolute character positions; a standalone-only case never becomes eligible merely because a component is parsed separately. The existing replacements for **អ្នក, ពេជ្រ, ទិន្ន, នឹង, លទ្ធ, ផល, សួរ, សេរី, សាមី, សាលា and សិលា** use this same matcher with their original word and position limits; the four former prefix cases additionally retain `originalSpellingOnly: true`. Their previous scopes and keys are pinned in `test/fixtures/shared-override-migration.json`. The separate prefix matcher is retired. Additive cases and terminal-root rules remain separate.

The longest complete override match wins independently of entry order. Whole-word overrides run before default scoped readings, marked-rhyme shortcuts and terminal-root rules. Matched parts also take precedence inside generated compounds, including across stored component boundaries, so another default path cannot restore the replaced keys. Overlapping matches follow the same left-to-right selection in every default path. Overrides retain their original-spelling and attached-mark protections; an ineligible match leaves the fallback intact, including when a marked ending parses a retained form. Native override tests configure additional entries for **អ្នក, ជ័យ and កម្មវិធី** without changing the bundled rule list, and verify saved and personal spellings with Sound map both on and off.

Nasal-boundary IDs are `ntr`, `ndr` and `nhR` for written shapes, and `stop` and `aspirated` for unwritten shapes. Loading rejects unknown IDs and duplicates within each family; removing a case intentionally disables that shape. Sound map searches narrow the matching word-part entries and their examples while retaining each entry's complete scope. Case details separate reusable patterns, selected word groups and single-word exceptions; this presentation does not change engine behavior.

Mappings, pronunciation rules and exact word exceptions have separate scopes. Editing one must not silently broaden another. Keep existing fixtures passing and add an input/output and boundary test for a changed rule. English inflection choices stay finite: the ending rules use per-base compatibility flags and a small irregular list, preserving approved-base availability, exclusions, personal exact entries and existing complete-form behavior. No source dictionary or expanded inflection catalog is loaded.

## License

The engine code, original documentation, standalone typing dictionaries and English inflection rules use [MIT](LICENSE), copyright 2026 Chhayarith Tes; see [Dictionary license](DICTIONARY-LICENSE.md) for the scope. No exclusive ownership is claimed in ordinary words or unprotectable linguistic facts. Dependency copyright notices, MIT license options and required Unicode-data notices are retained in `NOTICES.txt`; the Rust standard-library copyright report is in `licenses/`.

The exported dictionaries carry their own `compiled-data/LICENSE`, and the Sound map and content-policy lists carry `data/LICENSE`, both containing the same full MIT grant. The dictionary manifest and embedded bundle notices point to these license files. MIT covers Kmeow's original code and compilation/contributions to the extent those rights are held; it does not claim third-party rights or exclusive ownership of ordinary facts.

The standalone release keeps every current word, mapping, ID, rank, priority, commonness level, typing policy and conventional POS code. It omits source-linked definitions, POS evidence, meaning summaries, translation catalogs, reference excerpts and research provenance. POS labels are descriptive only and do not change conversion, suggestion order or acceptance. The richer original dictionaries and private Studio evidence remain unchanged and retain their applicable terms outside this release. The package includes no source-research or reference-verification tools and needs no source downloads or local evidence files. The loader retains checksums, matching-pair, vocabulary and internal payload validation. Compact typing bytes remain exact; public package checksums and pair identity change to reflect the projected metadata.

## Development

```sh
npm test                 # JavaScript API, browser initialization and dictionary checks
npm run test:rust        # Rust unit tests and typing regression fixtures
```

For a reviewed Khmer word addition, put the word, date, POS, expected generated inputs and the completed suitability decision in a JSON file, then run:

```sh
npm run dict:add -- /path/to/admission.json --dry-run
npm run dict:add -- /path/to/admission.json
npm run test:dictionary
```

The input fields are `word`, `date` (YYYY-MM-DD), `partsOfSpeech` (array), `expectedInputs` (sorted array), and `suitability` (`decision: "clean"` plus the reviewed `scope`). Optional `preserveFirstChoices` maps inputs to their expected first Khmer choice. The command resolves any interrupted manager transaction under the shared writer lock before reading the pair. It checks generated inputs, preserves the complete existing compact source, compiles and validates the matching pair, and publishes only the current dictionaries and manifest. It keeps no automatic backup or admission history. New entries use rank 1 and stay unrated. It refuses existing words and mismatched readings. It does not approve content or invent aliases.

Routine data additions need no Wasm build, full native suite, manager suite or exhaustive input sweep. If a rule changes, rebuild Wasm, run the relevant native regression and `test:dictionary`. Expand checks when a change crosses subsystems, a focused check fails, or a review requests broader coverage. Use Refresh in the manager after an external data update; restart its process when the loaded Wasm changes. Counts are checked against the shipped data rather than duplicated in UI tests.

To change Rust rules or rule data, install the toolchain pinned in `rust-toolchain.toml` and `wasm-bindgen-cli` 0.2.104, then rebuild Wasm:

```sh
cargo install wasm-bindgen-cli --version 0.2.104 --locked
npm run build:wasm
npm test
npm run test:rust
```

For a broad rule refactor, run `npm run test:rust -- --lib complete_reading_fingerprint -- --ignored --nocapture` before and after. Its `RULE_PARITY` line hashes complete generated reading lattices, signed priorities, prefix indexes, recovery lengths and fixture controls, including all supported paths; it does not truncate to the browsing preview. `rule-consolidation.json` independently pins the ordered word-part cases, total case count and boundary combinations for the current rules. Refresh affected table fingerprints only after reviewing intentional rule changes; snapshot-only fixes must preserve the complete reading fingerprint and dictionary bytes. These audits are for refactors, not routine word additions.

The lockfiles pin dependencies. The first Rust build downloads missing packages; after caching them, add `-- --offline` to `npm run build:wasm` to disable downloads.

This checkout contains the portable Rust core, thin Node/browser APIs, MIT typing dictionaries with POS labels, licenses and regression fixtures. The optional local manager uses the public engine and a local server for shared dictionary edits; there is no full Studio, app service, native app UI, SQLite, artwork or model dependency. Applications own their editors, storage, keyboard integration and user interface.

The dictionaries contain **4,560 Khmer words**, **7,063 English words** and **229 saved Romanized spellings**. POS labels are present for 4,558 Khmer and 7,062 English words; the remaining three stay unknown. Definitions, translations and source research are excluded. Tests validate the current pair against its manifest, check complete vocabulary and metadata alignment, and exercise fixed typing cases. Words containing **អាចម៍**, including **អាចម៍រណារ**, are absent. The manager has no quarantine management feature.

Current word identities, commonness, POS and generated typing keys are tested in `test/fixtures/dictionary-cases.json`. These are regression expectations, not a record of previous dictionary versions. `scripts/read-compact-source.mjs` recovers the current compact source only after a byte-for-byte Rust recompilation check; encoding and validation remain in Rust. The affirmative spelling is **ចា៎** with the existing `jah` input; **ចាស** and **ចា៎ស** remain absent.

The shortened unwritten vowel with **់** uses **`o` in both consonant series**, including medial syllables: **រលត់ → `rolot`**, **រត់ → `rot`**, **យប់ → `yop`** and **ផ្គត់ផ្គង់ → `phkotphkong`**. Written vowels, configured ending choices such as final **ក** (`u` / `ua` / `uo`), and independently specified word-part readings remain separate. Saved dictionary aliases and personal spellings remain authoritative. Native and Wasm tests share `test/fixtures/short-inherent-o.json`; this changes generated readings without modifying dictionary bytes.

The November 15 vowel selection makes **ោះ → `oh` / `os` in A-series and `uh` / `us` in O-series**. **តោះ** accepts `toh` / `tos`; **ទោះ** accepts `tuh` / `tus`. The existing effective syllable-series logic handles supported consonant clusters and register marks, so the split applies throughout generated readings. Earlier syllable boundaries are preserved when vowel typing keys differ. Saved whole-input aliases retain their independent choices; other vowel combinations retain their own mappings. Dictionary bytes, identities, commonness and ranks stay intact. The shared native/Wasm proof is `test/fixtures/vowel-series.json`; historical admission records retain their original readings.

The owner-selected compact reading **ទស្សនា → `tusna`** uses a local Sound map rule: after a consumed **ស្ស** coda, plain **នា** uses the A-series `a` key. This replaces the generated `nea` / `nia` ending and also composes in **ទស្សនាវដ្ដី**. The coda's existing `h` / `s` alternatives remain. `followingVowelGovernance` on the silent-coda rule configures its onset, vowel and target series; no saved word alias is added. Other vowels, onsets, register marks and cancellation-exposed spellings retain their existing handling. The rule change leaves dictionary bytes and saved aliases unchanged. `test/fixtures/silent-coda-series.json` is shared by the native and Wasm tests; its synthetic កស្សនា row verifies composition without admitting a dictionary word.

The November 16 owner-selected addition **ទស្សនៈ** is a noun with rank 1 and no numerical commonness rating. Its generated **`tusnak` / `tuhnak`** inputs use the existing `a` / `ak` convention for final **ៈ**, with the same local ស្ស → ន series rule. This is limited to the original terminal mark on the listed onset; bare O-series នៈ, shifted forms and cancellation-exposed endings retain their prior handling. The shorter `tusna` / `tuhna` inputs also remain available, with the existing Common (3) **ទស្សនា** retaining first choice. The existing **ទស្សន** entry, every other word and all saved aliases stay intact. No definition, translation or source payload is added to the dictionary.

The November 16 correction uses `u` for the original simple initial **ទស្ស** frame, producing **ទស្សនា → `tusna` / `tuhna`** and **ទស្សនៈ → `tusnak` / `tuhnak`**. Later syllables retain their independent rules. Written vowels, clusters and cancellation-exposed spellings are excluded; saved aliases and dictionary bytes are unchanged.

The reviewed terminal **ទស្សន៍** unit uses the retained **ទស្ស** reading (`tuh` / `tus`) in **ទស្សន៍, ទូរទស្សន៍ and ស្ថានីយទូរទស្សន៍**. Match the complete original ending before cancellation and preserve the preceding syllable boundary. The rule uses `terminalReadingUnits` with an explicit word scope; it does not change other cancelled endings or the separate សន្ទស្សន៍ context.

The November 16 selection replaces plain **ឹ → `eu`** with **`er`** in both series, retaining **`oeu`**. Examples: **ទឹក → `terk` / `toeuk`**, **ព្រឹក → `prerk` / `proeuk`**. Other vowel signs, explicitly mapped combined endings and saved aliases retain their independent mappings.

For a plain parsed **សិ + ស្ស** syllable, the silent-coda rule substitutes the vowel **ិ → ឹ** through `vowelSubstitutions`. **សិស្ស** and **កូនសិស្ស** therefore read like **សឹស្ស** and **កូនសឹស្ស**, using the current ឹ keys automatically. Written spellings stay intact; attached-vowel, shifted, clustered and cancellation-exposed forms retain their existing handling.

The local Sound map organizes all configurable Khmer pronunciation rules from `data/khmer-sound-map.json` into nine searchable families: Vowels; Consonants & endings; Syllable splitting; A/O changes; Silent letters & marks; Word-part readings; Shorthands; Repetition; Ranking. Families have one expandable level with compact rule rows. Word parts show their keys in a bounded list, with two columns on desktop and one on phones. Selecting a rule or word part opens one detail dialog; child rows replace its current view and Back returns to the parent. Rule explanations and Khmer → Romanized examples appear in that dialog. Character details keep their existing inline presentation. A rare shape without a bundled example uses an explicitly labeled pattern illustration, checked in an isolated engine without admitting the word. Full keys, conditions, retained readings, word scopes and priorities remain reachable in the dialog, with examples where available. The Examples row retains the word-part overview demonstrations, while each part can show its own existing reading alongside its scoped examples. Back restores the parent scroll position and selected row. Language switching retains the current detail path, and closing returns focus to its row. New configuration groups appear automatically; `manager/sound-rules.js` supplies the guides. `node --test manager/test/sound-rules.test.mjs manager/test/sound-collapse.test.mjs` verifies examples, exact readings and dialog access to every configured scope. This view is read-only and does not change typing.

Both **ផ** and **ភ** accept `ph` and `p` at syllable ends as well as beginnings. Their `codaFallbackKeys` lists both choices, which also appear in Character details. **លាភ** retains `leap` / `liap` and `leaph` / `liaph`. Native and Wasm tests use isolated spellings to check word-final and medial syllable endings without adding dictionary words; dictionary bytes and saved aliases remain unchanged.

Parsed syllable-final **ស** accepts both `h` and `s`, including before another syllable or a matched word-part override inside a compound. **សាសនា** accepts `sahnea` / `sasnea` and `sahnia` / `sasnia`; **ទេសធម៌** retains `tehthoa` / `testhoa`. A ស that starts the next complete word part retains `s`: **សុខសប្បាយ** uses `soksabbay` / `suksabbay` with the selected local vowel pair, without `sokhohbbay` / `sukhohbbay`. Onset guards use existing scoped part matches or complete dictionary prefix/part matches; middle parts also require a complete dictionary suffix. They only restrict generated `h`, without importing component readings or aliases. Initial and subscript ស remain unchanged. Dictionary records and saved or personal spellings are preserved, including explicit spellings containing `h`.

The owner-selected **`nh` → ខ្ញុំ** is an additive standalone Sound map reading. Existing `khnhom` / `khnyom` readings, word identity and rank remain; the shorthand does not propagate into compounds.

The owner removed automatic parent-word doubling for **ៗ**. Repeated-form words remain in the dictionary and retain saved aliases and Khmer completion, but repeating a parent’s generated Romanized input no longer creates a reading for its ៗ form. Repeated initial-letter handling (such as ពពក) remains separate.

The default unwritten vowel uses A-series `or` or `ar` and O-series `o` (for example ក → `kor`/`kar`, គ → `ko`, កកាយ → `korkay`/`karkay`). The shared override selects `sak` + `kam` → `sakkam` only for standalone សកម្ម; compounds retain their existing rules. Written vowels, short/closed inherent vowels and explicitly scoped readings retain their separate rules. Saved whole-word spellings remain independent; this changes generated readings without rewriting dictionary records.

`runtime.localVowelPairs` chooses an unwritten vowel from the first two consonants, the following subscript and its syllable structure. The selected ក/ស/ប + ញ and ស + ប medial pairs use `a`; ប + ណ uses `o`; វ + គ and ល + ក use `ae`; ល + ទ uses `er`. Each row lists its allowed medial-vowel, medial-continuation, closed-cluster or bare final-cluster shapes. A vowel/shift after the first letter, a mismatched or extra subscript, and a shortening mark keep their existing rules. A bare final វគ្គ closes with `k`; the same pair before written ី keeps its medial consonants. Original character spans prevent a cancelled tail from exposing a final-cluster match. Shared overrides remain authoritative. Checked component defaults retain their selected suffix and ranking while using the same local vowel; saved aliases and personal spellings never pass through this layer. Recognized pairs establish local syllable starts without enabling automatic word-component analysis. The `joinConsonants` option merges the overlapping `t` + `th` for the selected ល + ទ + ធ shapes: លទ្ធិ prefers `lerthi`, while វគ្គីករណ៍ retains `vaekkikor`. Shared overrides and saved spellings remain authoritative.


The shared syllable **ស្តី** uses `sdei` / `sdey`, including in **សួស្តី** (`sursdei` / `sursdey`). It is a complete parsed word-part override, not a change to ត's `t` mapping or other ស្ត vowels. Dictionary identities and saved aliases are unchanged.

`runtime.consonantCountTolerance` accepts single or doubled configured Latin consonants in generated Sound map readings (`lerthi` / `lertthi`, `vaekkikor` / `vaekikor`). It matches the original chunk lattices with bounded memoization; it does not expand or store alternate word lists. Browse data keeps canonical readings. An exact alternate count returns `preferredInput` for display, without a completion/correction flag, and remains eligible for the existing Space/Enter policy. Existing strict and saved exact matches rank ahead of new tolerant matches. Vowels (including `y`), apostrophes and user-defined spellings stay literal; runs of three or more are outside this tolerance. Disabling the group restores strict count matching.

The vowel readings `oa` and `or` accept each other as additive typing alternatives in the configured vowel rules and selected word-part readings: កកាយ → `korkay` / `koakay`, គាំ → `korm` / `koam`, មាន់ → `moan` / `morn`, ជ័រ → `joa` / `jor`, ពណ៌ → `poa` / `por`. The mappings list both choices explicitly, including retained longer readings and checked compound suffixes. Existing keys keep their order and priority; newly added alternatives use `rankPriority: -1000000` to keep existing exact matches ahead of newly overlapping words. The earlier `koat` / `kort` pair retains its existing priorities. A real consonant រ stays separate: គរ uses `kor`, បរទេស uses `borteh`, and the អក្សរ part retains its final `sor`; these do not gain `koa`, `boateh` or final `soa`. Saved aliases and personal spellings stay literal. The shared `oa-or-vowels.json` fixture checks these boundaries in native and Wasm engines.

Alternative word-part rules live in `runtime.wordParts`; suggestion-only shorthands live separately in `runtime.shorthands`. Alternative keys can be reused for complete recognized component words under `runtime.wordRuleReuse`. Reuse adds separate paths after existing readings; it never imports saved aliases. Component boundaries use the same dictionary-based detector as component analysis: require a complete unique split with every part available, and honor saved component definitions, explicit Standalone decisions, exclusions and disabled Sound map settings. Attached signs, coeng/subscripts, unknown spans and ambiguous splits do not establish new boundaries. Standalone-only rule cases remain local even when their reading type is Alternative; `reusableAsComponent: false` can explicitly opt another word rule out. The `nh` reading of ខ្ញុំ is Shorthand: it is offered only as a suggestion requiring explicit selection, never reused inside another word. Existing complete reading lattices remain intact; added component paths occupy a fallback tier below original strict and single/double-consonant matches. The configured priority orders paths within that tier. Complete unique nested components are resolved recursively, with earlier ordinary component paths retained alongside nested alternatives; preparation is bounded to 64 levels, 64 lattices and 128 chunks per lattice, with a bounded segmentation cache. Shorthand selection uses each case’s effective keys, including inherited keys and case-specific overrides. Detection and reading generation happen during bounded index preparation, with a cached view for personal words using the base dictionary. Saved component edits invalidate that view; compact dictionary bytes stay unchanged.

The word-part registry contains 52 reuse-enabled alternatives, the scoped-only បណ្ឌិត្យ and សាស្ត្រ definitions, and two standalone-only rules (សកម្ម and ពេជ្រ). The separate Shorthands registry contains `nh` → ខ្ញុំ. The safe ស្តី reading moved to **Read as** on the existing សួស្តី word, using សួស្ដី; no standalone ស្តី entry is added. Redundant general component reuse for បណ្ឌិត្យ is disabled while its checked បណ្ឌិត្យសភា case stays intact. Other scoped cases, checked neighbors, retained readings and priorities remain unchanged. The complete weighted profiles and prefix witnesses for the bundled vocabulary are covered by `word_part_read_as_migration_preserves_complete_bundled_profiles_and_scoped_context`; the general កម្ម → កាំ and សាស្ត្រ → សាស readings now live in Read as, while the checked សាស្ត្រ cases remain scoped. The owner removed the 35 additional overrides whose standalone spellings matched the ordinary sound map; their compound readings, completion choices and priorities now follow the remaining rules. Dictionary words and saved aliases stay intact. Four other default-reading components—ក្រដាស, ខុស, ច្បាស់ and ម្ចាស់—use `runtime.defaultComponentParts` boundary hints without custom Romanized keys. Their `startingWords` hints retain existing next-word context ordering. The former four definitions exist only in a regression fixture; `npm run test:rust -- --lib redundant_reading_cleanup` checks their weighted matching profiles and unlisted component frames against the default-reading implementation.

Open the Pronunciation page from the button above the Sound map rules. Search there by Khmer word, alternate spelling or Romanized reading. Add **Word → Read as** using Khmer for both fields; the preview shows the alternate spelling’s normal generated Latin readings, with no manually entered Latin keys. For example, មិត្ត → មិត derives `met` / `mit` while suggestions still insert មិត្ត. A saved rule replaces that word’s generated paths, and applies inside complete, unique component splits (including nested components); it does not replace arbitrary substrings, unknown or ambiguous spans, or a parent explicitly marked Standalone. The alternate uses the same word-part boundaries as the parser, with its keys generated from the Khmer spelling. A Romanized override or shorthand attached to the alternate word stays with that word. Stock Khmer respellings remain available without chaining caller rules. Its previewed reading lattice is retained when joining complete components, so a following part cannot change its vowels or expose its silent letters again. Existing saved spellings, dictionary identities, ranks, ratings and disabled Sound map policies remain authoritative. Edit or remove a custom rule to restore ordinary readings. Migrated built-in readings use Reset to restore their original pronunciation. No example is enabled automatically. Rules travel in the paired bundle’s `pronunciations` provenance (`kmeow-pronunciations-v1`), separately from lexical records; saving needs no Wasm rebuild. The SDK also offers `previewPronunciation(readAs)` and `setPronunciations([{word, readAs}])` for caller-managed runtime customization; native callers use `Engine::pronunciation_inputs` and `Engine::set_pronunciations`. These runtime overrides are not serialized into compact-only `exportBinary()` output; paired bundles carry them explicitly.

Read as retains the parsed-syllable boundaries of migrated built-in rules in top-level `pronunciationSyllables`, outside the grouped runtime rules. កម្ម → កាំ derives `kam`; សាស្ត្រ → សាស derives `sah` / `sas`. These definitions store Khmer respellings and the original-spelling boundary policy, without duplicated Latin keys. Saved pronunciation settings override the defaults; resetting restores the stock reading. The same prepared syllable choices feed the parser and complete-component reuse, including partial dictionary analyses where the old syllable rule already applied. Longer standalone matches, original offsets, cancellation limits and checked prefix/suffix readings remain authoritative. Editing a general Read as reading changes its mapped paths without rewriting explicit checked compound keys; for example, changing សាស្ត្រ still retains `kaeknitsah` / `kaeknitsas` for គណិតសាស្ត្រ. Original-word-only checked paths and endings stay with their approved word. A standalone ending can cross a component boundary only through keys also approved for component reuse. Word parts marked as scoped only do not gain general component reuse from a Read as edit. Read as can also use an existing fixed or scoped word-part boundary, even if the dictionary cannot split the full compound. The parser keeps the original word and position restrictions and derives the part’s keys from the Khmer respelling. Explicitly checked compound paths keep their selected keys. Unreviewed reuse still requires a complete unique component split. A conflicting component respelling cannot rebuild a scoped parent; agreement with an existing checked neighbor preserves its previous paths. The migration keeps `kamkor` → កម្មករ, `kamsit` → កម្មសិទ្ធិ, `kaeknitsah` → គណិតសាស្ត្រ and `sakkam` → សកម្ម. Native regression compares every bundled lattice, priority, recovery length and prefix index, plus the original-spelling fixture controls, against the former rule definitions. Runtime tests cover edits, resets, personal-cache invalidation and suggestion-only shorthands.

The November 17 safe pronunciation pass enables ten silent-letter respellings in the paired bundle: ការ → កា, ក្ដារ → ក្ដា, ពុម្ព → ពុម, ពេទ្យ → ពេទ, មិត្ត → មិត, លីត្រ → លីត, សួរ → សួ, សំបុត្រ → សំបុត, ហេតុ → ហេត and រូបធាតុ → រូបធាត. ធាតុ is not a standalone dictionary entry, so its silent final vowel is handled only on the existing រូបធាតុ word. No words or saved aliases are added or removed. Vowel substitutions, consonant substitutions and context-dependent choices from the other removed word-part rules remain on the ordinary Sound map. `node --test test/reviewed-pronunciations.test.mjs` verifies the chosen readings, component examples, complete vocabulary and saved aliases.

## Hosted website

GitHub Pages serves the browser manager at `https://yarith-dev.github.io/kmeow-typing/`.
Run `npm ci`, `npm run build`, and `node scripts/build-pages.mjs` to generate `.build/pages`.
The Pages workflow rebuilds and deploys on pushes to `main`; repository Pages settings must use GitHub Actions.
The hosted site includes typing, Sound map, word browsing/export and browser-local example edits.
Shared dictionary mutations require the local manager and are unavailable on the static site.
The build copies the exact dictionary pair and Wasm and derives the browsing catalog from that pair.
