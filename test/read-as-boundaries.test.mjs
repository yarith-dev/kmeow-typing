import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {WasmEngine,initSync} from '../wasm/khmer_engine.js';
const read=file=>readFileSync(new URL('../'+file,import.meta.url));
initSync({module:read('wasm/khmer_engine_bg.wasm')});
const proof=JSON.parse(read('test/fixtures/read-as-boundaries.json'));
const config=JSON.parse(read('data/khmer-sound-map.json'));
test('migrated Read as defaults preserve the exact scoped cases and compound suggestions in Wasm',()=>{
  assert.deepEqual(config.pronunciationSyllables.filter(rule=>proof.defaults.some(old=>old.word===rule.word)),proof.defaults);
  assert(!(config.runtime.wordParts??[]).some(part=>part.khmer==='កម្ម'));
  const scoped=config.runtime.pronunciationParts.find(part=>part.word==='សាស្ត្រ');
  assert.equal(scoped.reusableAsComponent,false);
  assert.deepEqual(scoped.cases.map(({order,...scope})=>scope),proof.movedParts[1].cases.filter(row=>row.kind==='scoped').map(({order,...scope})=>scope));
  const core=WasmEngine.fromAppData(read('compiled-data/khmer.kmd'),read('compiled-data/english.kmd'));
  try{
    const rules=JSON.parse(core.appDataInfo()).provenance.pronunciations.rules;
    for(const {word,readAs} of proof.defaults)assert(rules.some(rule=>rule.word===word&&rule.readAs===readAs));
    for(const [input,word] of [...proof.examples,...proof.standalone]){
      assert(JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(candidate=>candidate.khmer===word),`${input} → ${word}`);
    }
    const before=core.exportBinary();
    core.setPronunciations(JSON.stringify([...rules.filter(rule=>rule.word!=='កម្ម'),{word:'កម្ម',readAs:'មិត'}]));
    for(const input of ['metkor','mitkor'])assert(JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(candidate=>candidate.khmer==='កម្មករ'));
    core.setPronunciations(JSON.stringify(rules.filter(rule=>rule.word!=='កម្ម')));
    assert(JSON.parse(core.suggestWithContext('kamkor','{}','{}')).exactCandidates.some(candidate=>candidate.khmer==='កម្មករ'));
    assert.deepEqual(core.exportBinary(),before);
    core.setPronunciations(JSON.stringify([...rules.filter(rule=>rule.word!=='សាស្ត្រ'),{word:'សាស្ត្រ',readAs:'កូន'}]));
    for(const input of ['kaeknitsah','kaeknitsas'])assert(JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(candidate=>candidate.khmer==='គណិតសាស្ត្រ'),input);
    assert(JSON.parse(core.suggestWithContext('ekokon','{}','{}')).exactCandidates.some(candidate=>candidate.khmer==='អេកូសាស្ត្រ'));
    assert(!JSON.parse(core.suggestWithContext('kaeknitkon','{}','{}')).exactCandidates.some(candidate=>candidate.khmer==='គណិតសាស្ត្រ'));
    assert.deepEqual(core.exportBinary(),before);
  }finally{core.free();}
});

test('Read as never inherits an original-word-only checked prefix through a longer component split',()=>{
  const words=['គណិតសាស្ត្រ','កូន','គណិតសាស្ត្រកូន'];
  const core=new WasmEngine(JSON.stringify({schemaVersion:3,mappings:[],englishWords:[],words:words.map((khmer,i)=>({id:i+1,khmer,rank:i,status:'approved'}))}));
  try{
    core.setPronunciations(JSON.stringify([{word:'កូន',readAs:'មិត'}]));
    const exact=(input,word)=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(candidate=>candidate.khmer===word);
    assert(exact('kaeknitsah','គណិតសាស្ត្រ'));
    assert(exact('konetsahmet','គណិតសាស្ត្រកូន'));
    assert(!exact('kaeknitsahmet','គណិតសាស្ត្រកូន'));
  }finally{core.free();}
});
