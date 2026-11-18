import test from 'node:test';
import assert from 'node:assert/strict';
import {createEngine} from '../dist/public/node.js';
import {WasmEngine} from '../wasm/khmer_engine.js';
import {readCompactSource} from '../scripts/read-compact-source.mjs';

const dataset=rank=>({schemaVersion:3,
  mappings:[{id:10,romanized:'fixture',khmer:'កា',rank,status:'approved'},{id:11,romanized:'fixture',khmer:'ខា',rank:0,status:'approved'}],
  words:[{id:1,khmer:'កា',rank,status:'approved'},{id:2,khmer:'ខា',rank:0,status:'approved'}],
  englishWords:[{id:20,text:'alpha',rank:0,status:'approved'},{id:21,text:'alpine',rank,status:'approved'}],
  structures:[],soundMapDisabledWords:['ខា']});

test('Wasm supports signed ranks and lossless source extraction while retaining legacy bytes',async()=>{
  const initialized=await createEngine();initialized.dispose();
  for(const rank of [-1000000,-1,0,1000000]){
    const core=new WasmEngine(JSON.stringify(dataset(rank))),binary=core.exportBinary();
    const restored=WasmEngine.fromBinary(binary);
    try{
      assert.equal(Buffer.from(binary.subarray(0,8)).toString(),rank<0?'KMEOWC05':'KMEOWC04');
      const source=readCompactSource(core);
      assert.equal(source.words.find(word=>word.khmer==='កា').rank,rank);
      assert.equal(source.mappings.find(word=>word.khmer==='កា').rank,rank);
      assert.equal(source.englishWords.find(word=>word.text==='alpine').rank,rank);
      for(const input of ['fixture','fix','ka','ក']){
        assert.deepEqual(JSON.parse(restored.suggestWithContext(input,'{}','{}')),JSON.parse(core.suggestWithContext(input,'{}','{}')));
      }
      if(rank<0){
        assert.equal(JSON.parse(core.convert('fixture')).candidates[0].khmer,'កា');
        assert.equal(JSON.parse(core.suggestWithFeatures('al','{"mode":"english"}')).candidates[0].text,'alpine');
      }
    }finally{restored.free();core.free();}
  }
});

test('SDK accepts signed personal ranks, preserves spellings and rejects invalid edits atomically',async()=>{
  const core=await createEngine({words:[{word:'កា',spellings:['personalword'],rank:-1}]});
  try{
    assert.equal(core.convert('personalword').candidates[0].rank,-1);
    core.addWords([{word:'កា',spellings:['anotherword'],rank:-1000000}]);
    assert.equal(core.convert('anotherword').candidates[0].rank,-1000000);
    assert.deepEqual(core.getWords()[0].spellings,['personalword','anotherword']);
    const before=core.getWords(),result=core.suggest('personalword');
    for(const rank of [-1000001,1000001,-1.5,NaN,Infinity]){
      assert.throws(()=>core.addWords([{word:'កា',rank}]),/rank/);
      assert.deepEqual(core.getWords(),before);assert.deepEqual(core.suggest('personalword'),result);
    }
  }finally{core.dispose();}
});
