import test from 'node:test';
import assert from 'node:assert/strict';
import {createEngine} from '../dist/public/node.js';

function allInputs(engine,word){
  const inputs=[];
  let after;
  do{
    const page=engine.soundMapInputs(word,after);
    assert(page.inputs.length<=64);
    assert(page.inputs.every(input=>!after||input>after));
    inputs.push(...page.inputs);
    after=page.nextAfter;
    assert(inputs.length<100_000,'paging must finish for this fixture');
  }while(after);
  assert.deepEqual(inputs,[...new Set(inputs)].sort());
  return inputs;
}

test('all generated spelling pages use the live engine and preserve its typing results',async()=>{
  const engine=await createEngine();
  try{
    const probes=['brap','prap','books','angkor','sievphov','បដិសណ្ឋារកិច្ច'];
    const before=probes.map(input=>engine.suggest(input));
    const settings=engine.settings,personal=engine.getWords();
    assert.deepEqual(allInputs(engine,'ប្រាប់'),['brab','brap','prab','prap']);
    const inputs=allInputs(engine,'បដិសណ្ឋារកិច្ច');
    assert(inputs.length>64);
    for(const input of inputs){
      assert(engine.suggest(input,{completion:false,previousWords:[]}).exactCandidates.some(row=>row.khmer==='បដិសណ្ឋារកិច្ច'),input);
    }
    assert.deepEqual(probes.map(input=>engine.suggest(input)),before);
    assert.deepEqual(engine.settings,settings);assert.deepEqual(engine.getWords(),personal);
    assert.throws(()=>engine.soundMapInputs('book'));
    assert.throws(()=>engine.soundMapInputs('ប្រាប់','x'.repeat(513)));
  }finally{engine.dispose();}
});

test('generated spelling inspection includes disabled and excluded keys and omits saved aliases',async()=>{
  const engine=await createEngine({dictionary:{schemaVersion:2,
    mappings:[{id:90,romanized:'saved-only',khmer:'កិ',rank:0,status:'approved'}],
    words:[{id:1,khmer:'កិ',rank:0,status:'approved'}],
    soundMapDisabledWords:['កិ'],
  }});
  try{
    engine.setInputExclusions([{word:'កិ',inputs:['ki']}]);
    const before=engine.suggest('ki',{previousWords:[]});
    assert.deepEqual(allInputs(engine,'កិ'),['ke','ki']);
    assert(!before.exactCandidates.some(row=>row.khmer==='កិ'));
    assert.deepEqual(engine.suggest('ki',{previousWords:[]}),before);
    assert(engine.convert('saved-only').candidates.some(row=>row.khmer==='កិ'));
  }finally{engine.dispose();}
});

test('overlapping shorthands keep default spellings visible without changing typing',async()=>{
  for(const disabled of [false,true]){
    const engine=await createEngine({dictionary:{schemaVersion:3,mappings:[],englishWords:[],
      words:[{id:1,khmer:'កម្ម',rank:0,status:'approved'}],
      soundMapDisabledWords:disabled?['កម្ម']:[],
    }});
    try{
      const baseline=engine.suggest('kam',{previousWords:[]});
      assert.equal(baseline.exactCandidates.some(row=>row.khmer==='កម្ម'),!disabled);
      for(const inputs of [['kam','shortcut'],['kam'],[]]){
        engine.setShorthands([{word:'កម្ម',inputs}]);
        const before=engine.suggest('kam',{previousWords:[]});
        assert.deepEqual(allInputs(engine,'កម្ម'),['kam']);
        assert.deepEqual(engine.suggest('kam',{previousWords:[]}),before);
        assert.deepEqual(before,baseline);
      }
      engine.setShorthands([{word:'កម្ម',inputs:['kam','shortcut']}]);
      engine.setInputExclusions([{word:'កម្ម',inputs:['kam']}]);
      const excluded=engine.suggest('kam',{previousWords:[]});
      assert.deepEqual(allInputs(engine,'កម្ម'),['kam']);
      assert.deepEqual(engine.suggest('kam',{previousWords:[]}),excluded);
      assert(!excluded.exactCandidates.some(row=>row.khmer==='កម្ម'));
    }finally{engine.dispose();}
  }
});
