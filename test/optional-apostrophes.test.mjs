import test from 'node:test';
import assert from 'node:assert/strict';
import {createEngine} from '../dist/public/node.js';
import {spaceChoice,replace,emptyDraft,history} from '../manager/composer.js';
import {suggestionStatus,suggestionReading} from '../manager/suggestion-view.js';

const dictionary={schemaVersion:3,englishWords:[],mappings:[{id:900,romanized:"q'foo",khmer:'កូន',rank:0,status:'approved'}],
  words:['ពូន','ប្អូន','ផ្អែម','ស្អាត','ចង្អុល','កូន','មិត្ត','ភាព','មិត្តភាព'].map((khmer,i)=>({id:100+i,khmer,rank:i,status:'approved'}))};

test('missing apostrophes show Completion and selection/Space/undo preserve the exact choice',async()=>{
  const engine=await createEngine({dictionary});
  const query=input=>engine.suggest(input,{previousWords:[]});
  try{
    const words=engine.getWords(),explicit=query("p'oun"),result=query('poun');
    const index=result.candidates.findIndex(row=>row.khmer==='ប្អូន'),hit=result.candidates[index];assert(index>0);
    assert.equal(result.candidates[0].khmer,'ពូន');assert.equal(hit.completion,"p'oun");assert.equal(hit.correction,undefined);
    assert.deepEqual(suggestionStatus(hit,result).states.map(s=>s.kind),['completion']);
    assert.equal(suggestionReading(hit,result,'khmer'),"p'oun");
    assert(!result.exactCandidates.some(row=>row.khmer==='ប្អូន'));
    assert.equal(spaceChoice(result,index).khmer,'ពូន');
    assert.equal(spaceChoice(query('phaem'),0)?.khmer,'ផ្អែម');
    assert.equal(spaceChoice(query('phaem'),0,{acceptance:'manual'}),undefined);
    const draft={...emptyDraft(),text:'Hi !',start:3,end:3,input:'poun'},state=history(draft);
    state.push(replace(state.current,hit.khmer,hit));assert.equal(state.current.text,'Hi ប្អូន!');
    const inserted=state.current;assert.deepEqual(state.undo(),draft);assert.deepEqual(state.redo(),inserted);
    for(const [input,word]of [['po','ប្អូន'],['phaem','ផ្អែម'],['sat','ស្អាត'],['jongol','ចង្អុល'],
      ['ppoun','ប្អូន'],['ppou','ប្អូន'],['phhaem','ផ្អែម'],['jonggol','ចង្អុល']]){
      const result=query(input),hit=result.candidates.find(row=>row.khmer===word);assert(hit,input);
      assert(hit.completion.includes("'"));assert(!result.exactCandidates.some(row=>row.khmer===word));
    }
    assert(!query('qfoo').candidates.some(row=>row.khmer==='កូន'));
    assert(!engine.suggest('poun',{soundMap:false,previousWords:[]}).candidates.some(row=>row.khmer==='ប្អូន'));
    assert.deepEqual(query("p'oun"),explicit);assert.deepEqual(engine.getWords(),words);
  }finally{engine.dispose();}
});

test('personal omission completions stay below ordinary prefixes and keep duplicate witnesses',async()=>{
  for(const target of ['កូន','ប្អូន']){
    const engine=await createEngine({dictionary:{schemaVersion:3,englishWords:[],
      words:['កូន','ប្អូន'].map((khmer,i)=>({id:100+i,khmer,rank:i,status:'approved'})),
      mappings:[{id:900,romanized:'pounx',khmer:target,rank:0,status:'approved'}]},
      words:[{word:'ប្អូន',spellings:['my-sibling']}]});
    try{
      const result=engine.suggest('poun',{previousWords:[]});
      assert.equal(result.candidates[0].khmer,target);assert.equal(result.candidates[0].completion,'pounx');
      assert.equal(result.candidates[0].mappingId,900);
      assert.deepEqual(spaceChoice(result,0),result.candidates[0]);
      assert.equal(spaceChoice(result,0,{acceptance:'manual'}),undefined);
      assert(result.candidates.every(row=>suggestionStatus(row,result).states.some(status=>status.kind==='completion')));
      if(target==='កូន')assert.equal(result.candidates[1].khmer,'ប្អូន');
      else assert.equal(result.candidates.length,1);
    }finally{engine.dispose();}
  }
});

test('Read as applies optional apostrophes to complete compounds without creating aliases',async()=>{
  const engine=await createEngine({dictionary,words:[{word:'មិត្តកូន',spellings:['personal-reading']}]});
  const query=input=>engine.suggest(input,{previousWords:[]});
  try{
    const words=engine.getWords(),before=query('met'),alias=query('personal-reading');
    engine.setPronunciations([{word:'មិត្ត',readAs:'ប្អូន'}]);
    for(const [input,word]of [['poun','មិត្ត'],['pounpeap','មិត្តភាព'],['pounkon','មិត្តកូន']]){
      const result=query(input);assert(result.candidates.some(row=>row.khmer===word&&row.completion?.includes("'")),input);
      assert(!result.exactCandidates.some(row=>row.khmer===word));
    }
    assert.deepEqual(query('personal-reading'),alias);
    engine.setPronunciations([]);assert.deepEqual(query('met'),before);assert.deepEqual(engine.getWords(),words);
  }finally{engine.dispose();}
});
