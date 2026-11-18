import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {WasmEngine,initSync} from '../wasm/khmer_engine.js';
const read=file=>readFileSync(new URL('../'+file,import.meta.url));
initSync({module:read('wasm/khmer_engine_bg.wasm')});
const proof=JSON.parse(read('test/fixtures/word-part-read-as-migration.json'));

test('safe reading moves to Read as while the checked scoped case stays intact',()=>{
  const runtime=JSON.parse(read('data/khmer-sound-map.json')).runtime;
  assert(!(runtime.wordParts??[]).some(p=>p.khmer===proof.movedPart.khmer));
  const scoped=structuredClone(runtime.pronunciationParts.find(p=>p.word===proof.reuseDisabled));
  assert.equal(scoped.reusableAsComponent,false);delete scoped.reusableAsComponent;
  assert.equal(scoped.readAs,'បណ្ឌិត');
  assert.deepEqual(scoped.cases.map(({order,...scope})=>scope),proof.keptScopedDefinition.cases.map(({order,...scope})=>scope));
  const core=WasmEngine.fromAppData(read('compiled-data/khmer.kmd'),read('compiled-data/english.kmd'));
  try{
    assert(JSON.parse(core.appDataInfo()).provenance.pronunciations.rules.some(rule=>rule.word===proof.pronunciation.word&&rule.readAs===proof.pronunciation.readAs));
    for(const input of ['sursdei','sursdey'])assert(JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(c=>c.khmer==='សួស្តី'),input);
    for(const input of ['bondetsapea','bonditsapea'])assert(JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(c=>c.khmer==='បណ្ឌិត្យសភា'),input);
  }finally{core.free();}
});
