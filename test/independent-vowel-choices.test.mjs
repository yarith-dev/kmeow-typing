import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {initSync,WasmEngine} from '../wasm/khmer_engine.js';

const read=path=>readFileSync(new URL('../'+path,import.meta.url));
initSync({module:read('wasm/khmer_engine_bg.wasm')});
const proof=JSON.parse(read('test/fixtures/independent-vowel-choices.json'));

test('ឥ uses ei and ey in Wasm while other vowels and explicit spellings retain their identities',()=>{
  const source={schemaVersion:2,mappings:[],words:proof.words.map((khmer,i)=>({
    id:996500+i,khmer,rank:i,status:'approved'
  }))};
  const core=new WasmEngine(JSON.stringify(source));
  const saved=new WasmEngine(JSON.stringify({...source,mappings:[
    {id:996004,romanized:'e',khmer:'ឥ',rank:9,status:'approved'}
  ]}));
  const personal=WasmEngine.fromPersonalWords(JSON.stringify([{word:'ឥ',spellings:['e','myvowel']}]));
  const exact=(engine,input,features={})=>JSON.parse(engine.suggestWithContext(input,JSON.stringify(features),'{}')).exactCandidates;
  try{
    const bytes=core.exportBinary();
    for(const field of ['yes','mappingOnly','letterUnits','compatibility','no'])for(const [input,word] of proof[field]){
      const hit=exact(core,input).find(row=>row.khmer===word);
      assert.equal(!!hit,field!=='no',`${input} → ${word}`);
      if(hit){
        const index=proof.words.indexOf(word);
        assert.equal(hit.mappingId,996500+index);assert.equal(hit.rank,index);
      }
    }
    for(const soundMap of [true,false]){
      assert(exact(saved,'e',{soundMap}).some(row=>row.mappingId===996004&&row.rank===9));
      for(const input of ['e','myvowel'])assert(JSON.parse(core.suggestWithExtension(personal,input,JSON.stringify({soundMap}),'{}'))
        .exactCandidates.some(row=>row.khmer==='ឥ'));
    }
    assert(!exact(core,'ei',{soundMap:false}).some(row=>row.khmer==='ឥ'));
    assert.deepEqual(core.exportBinary(),bytes);
  }finally{core.free();saved.free();personal.free();}
});
