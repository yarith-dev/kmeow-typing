import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {WasmEngine,initSync} from '../wasm/khmer_engine.js';
initSync({module:readFileSync(new URL('../wasm/khmer_engine_bg.wasm',import.meta.url))});

test('component alternatives are shared across native/Wasm boundaries and shorthands require selection',()=>{
  for(const [word,input] of [['កិច្ចកា','kichka'],['កាកិច្ច','kakich'],['កាកិច្ចខា','kakichkha']]){
    const core=new WasmEngine(JSON.stringify({schemaVersion:2,mappings:[],words:
      ['កា','ខា','កិច្ច',word].map((khmer,i)=>({id:99000+i,khmer,rank:i,status:'approved'}))}));
    try{
      const hit=JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.find(c=>c.khmer===word);
      assert.equal(hit?.mappingId,99003,input);assert.equal(hit.rank,3);
      assert(!JSON.parse(core.suggestWithContext(input,'{"soundMap":false}','{}')).exactCandidates.some(c=>c.khmer===word));
    }finally{core.free();}
  }
  const core=WasmEngine.fromAppData(readFileSync(new URL('../compiled-data/khmer.kmd',import.meta.url)),readFileSync(new URL('../compiled-data/english.kmd',import.meta.url)));
  try{
    const result=JSON.parse(core.suggestWithContext('nh','{}','{}'));
    assert(result.candidates.some(c=>c.khmer==='ខ្ញុំ'&&c.tapOnly));
    for(const input of ['khnhom','khnyom']){
      const hit=JSON.parse(core.suggestWithContext(input,'{}','{}')).candidates.find(c=>c.khmer==='ខ្ញុំ');
      assert(hit&&!hit.tapOnly,input);
    }
  }finally{core.free();}
});

test('nested component readings retain count-match precedence and earlier component paths',()=>{
  const words=['កា','កិច្ច','ខា','កាកិច្ច','កាកិច្ចខា','កាកិច្ចកិច្ច','កាក្កិច'];
  const core=new WasmEngine(JSON.stringify({schemaVersion:2,mappings:[],words:
    words.map((khmer,i)=>({id:99000+i,khmer,rank:i,status:'approved'}))}));
  const exact=input=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates;
  try{
    assert.equal(exact('kakich')[0]?.khmer,'កាក្កិច');
    assert(exact('kakich').some(c=>c.khmer==='កាកិច្ច'));
    assert(exact('kakichkha').some(c=>c.khmer==='កាកិច្ចខា'));
    for(const input of ['kakichkich','kakijjorkich'])assert(exact(input).some(c=>c.khmer==='កាកិច្ចកិច្ច'),input);
  }finally{core.free();}
});
