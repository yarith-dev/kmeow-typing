import test from 'node:test';
import assert from 'node:assert/strict';
import {createEngine} from '../dist/public/node.js';

test('SDK exact exclusions override all sources and clearing restores identical results',async()=>{
  const core=await createEngine({words:[{word:'កូន',spellings:['fixturekid']}]});
  try{
    const inputs=['kon','fixturekid','ko','ke','កូន'];
    const before=inputs.map(input=>core.suggest(input));
    core.setInputExclusions([{word:'កូន',inputs:[' KON ','fixturekid']}]);
    for(const input of ['kon','fixturekid']){
      assert(!core.suggest(input).candidates.some(hit=>hit.khmer==='កូន'));
      assert(!core.suggest(input,{previousWords:[]}).candidates.some(hit=>hit.khmer==='កូន'));
      assert(!core.convert(input).candidates.some(hit=>hit.khmer==='កូន'));
    }
    assert.throws(()=>core.setInputExclusions([{word:'កូន',inputs:['!']}]),/Romanized/);
    assert(!core.suggest('kon').candidates.some(hit=>hit.khmer==='កូន'));
    core.setInputExclusions([]);
    for(const [index,input]of inputs.entries())assert.deepEqual(core.suggest(input),before[index]);
  }finally{core.dispose();}
});

test('personal-only exclusions cover saved and generated inputs and survive add, clear and re-add',async()=>{
  const word='មិត្តកូន',personal=[{word,spellings:['fixtureword'],rank:7}];
  const core=await createEngine({words:personal});
  try{
    const queries=['fixtureword','metkon','kon','ke','កូន'];
    const before=queries.map(input=>({converted:core.convert(input),suggested:core.suggest(input)}));
    for(const input of ['fixtureword','metkon'])assert(core.suggest(input).candidates.some(hit=>hit.khmer===word),input);
    const stored=core.getWords();
    core.setInputExclusions([{word,inputs:[' FIXTUREWORD ','metkon']},{word:'កូន',inputs:['kon']}]);
    function assertBlocked(){
      for(const [input,blocked]of [['fixtureword',word],['metkon',word],['kon','កូន']]){
        assert(!core.convert(input).candidates.some(hit=>hit.khmer===blocked),input);
        const result=core.suggest(input,{previousWords:[]});
        assert(!result.candidates.some(hit=>hit.khmer===blocked),input);
        assert(!result.exactCandidates.some(hit=>hit.khmer===blocked),input);
      }
    }
    assertBlocked();assert.deepEqual(core.getWords(),stored);
    core.addWords([{word,spellings:['fixturealt']}]);
    assertBlocked();assert.equal(core.getWords()[0].rank,7);
    assert.deepEqual(core.getWords()[0].spellings,['fixtureword','fixturealt']);
    assert(core.convert('fixturealt').candidates.some(hit=>hit.khmer===word));
    for(const entries of [[{word,inputs:['!']}],[{word:'មិត្តកូនមិត្តកូន',inputs:['unknownword']}],[[word,['fixtureword']]]]){
      assert.throws(()=>core.setInputExclusions(entries));assertBlocked();
    }
    core.clearWords();assert.deepEqual(core.getWords(),[]);
    assert(!core.convert('fixtureword').candidates.some(hit=>hit.khmer===word));
    assert(!core.suggest('kon').candidates.some(hit=>hit.khmer==='កូន'));
    core.addWords(stored);assertBlocked();
    core.setInputExclusions([]);
    for(const [index,input]of queries.entries()){
      assert.deepEqual(core.convert(input),before[index].converted,input);
      assert.deepEqual(core.suggest(input),before[index].suggested,input);
    }
    assert.deepEqual(core.getWords(),stored);
  }finally{core.dispose();}
});

test('words added after setup can be excluded without changing unrelated typing',async()=>{
  const core=await createEngine();
  try{
    const baseline=core.suggest('ke',{previousWords:[]});
    core.addWords([{word:'មិត្តកូន',spellings:['fixtureword']}]);
    assert(core.convert('fixtureword').candidates.some(hit=>hit.khmer==='មិត្តកូន'));
    core.setInputExclusions([{word:'មិត្តកូន',inputs:['fixtureword']}]);
    assert(!core.convert('fixtureword').candidates.some(hit=>hit.khmer==='មិត្តកូន'));
    assert.deepEqual(core.suggest('ke',{previousWords:[]}),baseline);
    core.setInputExclusions([]);
    assert(core.convert('fixtureword').candidates.some(hit=>hit.khmer==='មិត្តកូន'));
  }finally{core.dispose();}
});

test('editable shorthands stay separate from spellings with normal acceptance including personal-only words',async()=>{
  const core=await createEngine({words:[{word:'មិត្តកូន',spellings:['personalword']}]});
  try{
    const before=core.suggest('nh'),words=core.getWords();
    assert.deepEqual(core.wordShorthands('ខ្ញុំ'),['nh']);
    assert(!core.soundMapInputs('ខ្ញុំ').inputs.includes('nh'));
    core.setShorthands([{word:'ខ្ញុំ',inputs:[]},{word:'កូន',inputs:[' BB ']},{word:'មិត្តកូន',inputs:['mm']}]);
    assert(!core.suggest('nh').candidates.some(hit=>hit.khmer==='ខ្ញុំ'&&hit.completion===undefined));
    for(const [input,word] of [['b','កូន'],['bb','កូន'],['mm','មិត្តកូន']]){
      const hit=core.suggest(input).candidates.find(hit=>hit.khmer===word);assert(hit,input);assert(!hit.tapOnly);
    }
    assert(!core.convert('bb').candidates.some(hit=>hit.khmer==='កូន'));
    assert(!core.suggest('bbmet').candidates.some(hit=>hit.khmer==='កូនមិត្ត'));
    core.addWords([{word:'មិត្តកូន',spellings:['morepersonal']}]);
    assert(core.suggest('mm').candidates.some(hit=>hit.khmer==='មិត្តកូន'&&!hit.tapOnly));
    core.setInputExclusions([{word:'កូន',inputs:['bb']}]);assert(!core.suggest('bb').candidates.some(hit=>hit.khmer==='កូន'));
    assert.deepEqual(core.getWords()[0].spellings,[...words[0].spellings,'morepersonal']);
    core.setShorthands([]);core.setInputExclusions([]);
    assert.deepEqual(core.suggest('nh'),before);
  }finally{core.dispose();}
});
