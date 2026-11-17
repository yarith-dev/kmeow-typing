import assert from 'node:assert/strict';import fs from 'node:fs';import {createHash} from 'node:crypto';
import {initSync,WasmEngine} from '../wasm/khmer_engine.js';
import {fileURLToPath} from 'node:url';
import {resolve} from 'node:path';
import {readCompactSource} from './read-compact-source.mjs';
import {acquireDictionaryLock,recoverDictionaryTransaction,commitDictionaryFiles} from '../manager/dictionary-store.mjs';
// Input is an already reviewed admission, not an automatic vocabulary approval.
const [input, flag] = process.argv.slice(2);
if (!input || (flag && flag !== '--dry-run')) throw new Error('Usage: npm run dict:add -- admission.json [--dry-run]');
const spec=JSON.parse(fs.readFileSync(resolve(input),'utf8'));
assert.equal(typeof spec.word,'string');assert(spec.word.length && spec.word===spec.word.trim());
assert(/^\d{4}-\d{2}-\d{2}$/.test(spec.date));
assert.equal(spec.suitability?.decision,'clean');assert(spec.suitability.scope?.trim());
assert(Array.isArray(spec.partsOfSpeech));
assert(spec.partsOfSpeech.every(tag=>'noun verb adj adv prep pron conj det intj num particle classifier aux article'.split(' ').includes(tag)));
assert(Array.isArray(spec.expectedInputs) && spec.expectedInputs.length);
process.chdir(fileURLToPath(new URL('../',import.meta.url)));
const release=await acquireDictionaryLock(process.cwd());
try{
await recoverDictionaryTransaction(process.cwd());
const sha=v=>createHash('sha256').update(v).digest('hex');
initSync({module:fs.readFileSync('wasm/khmer_engine_bg.wasm')});
const files=Object.fromEntries(['khmer','english'].map(l=>[l,fs.readFileSync(`compiled-data/${l}.kmd`)]));
const old=WasmEngine.fromAppData(files.khmer,files.english);const data=readCompactSource(old),original=structuredClone(data);
const word=spec.word;assert(!data.words.some(w=>w.khmer===word));const typing={id:Math.max(...data.words.map(w=>w.id),...data.englishWords.map(w=>w.id),...data.mappings.map(w=>w.id))+1,khmer:word,rank:1,status:'approved'};data.words.push(typing);
const next=new WasmEngine(JSON.stringify(data));const compactSha256=sha(next.exportBinary());
const row=JSON.parse(next.browseWords(JSON.stringify({query:word,limit:100}))).words.find(w=>w.khmer===word);assert.deepEqual(row?.soundMapInputs,spec.expectedInputs,'Generated inputs differ; review the rule before admitting the word');
const metadata=Object.fromEntries(Object.entries(files).map(([l,b])=>[l,JSON.parse(b.subarray(24+b.readUInt32LE(12)+b.readUInt32LE(16),-32))]));
metadata.khmer.words.push({id:sha(`kmeow:khmer:${word}:${spec.date}`),word,context:null,commonnessLevel:null,partsOfSpeech:spec.partsOfSpeech,meanings:[],translations:[],sources:[]});
const datasetHash=sha(JSON.stringify(data));const snapshot=sha(JSON.stringify({compactHash:compactSha256,words:[metadata.khmer.words,metadata.english.words]}));
const json={};for(const l of ['khmer','english']){metadata[l].snapshotId=snapshot;json[l]=JSON.stringify(metadata[l]);}
const provenance=JSON.parse(old.appDataInfo()).provenance;Object.assign(provenance.compactManifest,{artifactHash:compactSha256,datasetHash,...JSON.parse(next.stats())});Object.assign(provenance.metadataManifest,{snapshotId:snapshot,compactArtifactHash:compactSha256});for(const l of ['khmer','english'])Object.assign(provenance.metadataManifest.files[l],{bytes:Buffer.byteLength(json[l]),sha256:sha(json[l]),words:metadata[l].words.length});
const packed=Object.fromEntries(['khmer','english'].map(l=>[l,Buffer.from(next.compileAppData(json.khmer,json.english,JSON.stringify(provenance),l))]));
const checked=WasmEngine.fromAppData(packed.khmer,packed.english);const decoded=readCompactSource(checked);decoded.words=decoded.words.filter(w=>w.id!==typing.id);assert.deepEqual(decoded,original);
for(const input of spec.expectedInputs) assert(JSON.parse(checked.suggestWithContext(input,'{}','{}')).exactCandidates.some(c=>c.khmer===word),input);
for(const [input,expected] of Object.entries(spec.preserveFirstChoices??{})) assert.equal(JSON.parse(checked.suggestWithContext(input,'{}','{}')).exactCandidates[0]?.khmer,expected,input);
const manifest=JSON.parse(fs.readFileSync('dictionary-manifest.json'));const info=JSON.parse(checked.appDataInfo());
for(const key of ['pairId','datasetHash','snapshotId'])manifest[key]=info[key];
for(const lang of ['khmer','english'])manifest[lang]=sha(packed[lang]);
if(flag!=='--dry-run'){
  await commitDictionaryFiles(process.cwd(),[packed.khmer,packed.english,Buffer.from(JSON.stringify(manifest,null,2)+'\n')]);
}
console.log({dryRun:flag==='--dry-run',word,inputs:row.soundMapInputs,stats:JSON.parse(checked.stats())});
checked.free();next.free();old.free();

}finally{await release();}
