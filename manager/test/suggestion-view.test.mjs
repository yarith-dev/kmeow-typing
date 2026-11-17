import test from 'node:test';
import assert from 'node:assert/strict';
import {suggestionStatuses,suggestionStatus,suggestionReading} from '../suggestion-view.js';
import {createEngine} from '../../dist/public/node.js';

test('dots show every applicable state without hiding explicit selection or corrections',()=>{
  const kinds=candidate=>suggestionStatus(candidate,{normalizedInput:''}).states.map(status=>status.kind);
  assert.deepEqual(suggestionStatuses.map(s=>s.kind),['match','completion','correction','explicit']);
  assert.deepEqual(kinds({}),['match']);
  assert.deepEqual(kinds({completion:'frang'}),['completion']);
  assert.deepEqual(kinds({correction:'the'}),['correction']);
  assert.deepEqual(kinds({completion:'the',correction:'the'}),['completion','correction']);
  for(const flag of ['tapOnly','manualOnly']){
    const status=suggestionStatus({completion:'frang',[flag]:true},{normalizedInput:'f'});
    assert.deepEqual(status.states.map(s=>s.kind),['completion','explicit']);
    assert.equal(status.description,'Completion · Select explicitly');
    assert.deepEqual(kinds({[flag]:true}),['match','explicit']);
    assert.deepEqual(kinds({completion:'the',correction:'the',[flag]:true}),['completion','correction','explicit']);
  }
  assert.deepEqual(kinds({tapOnly:true,manualOnly:true}),['match','explicit']);
});

test('Romanized subtext uses completed and corrected inputs before the typed prefix',()=>{
  assert.equal(suggestionReading({khmer:'ហ្វ្រាំង',completion:'frang'},{normalizedInput:'f'},'khmer'),'frang');
  assert.equal(suggestionReading({khmer:'សៀវភៅ'},{normalizedInput:'sievphov'},'khmer'),'sievphov');
  assert.equal(suggestionReading({khmer:'សៀវភៅ',completion:'sievphov',correction:'sievphov'},{normalizedInput:'sievpho'},'khmer'),'sievphov');
});

test('direct Khmer and next-word suggestions use existing readings or personal spellings',()=>{
  assert.equal(suggestionReading({khmer:'សៀវភៅ',completion:'សៀវភៅ'},{normalizedInput:'សៀវ'},'khmer',['sievphov']),'sievphov');
  assert.equal(suggestionReading({khmer:'សៀវភៅ'},{normalizedInput:''},'khmer',[],[{word:'សៀវភៅ',spellings:['mybook']}]),'mybook');
  assert.equal(suggestionReading({khmer:'ក'},{normalizedInput:'ក'},'khmer'), '');
  assert.equal(suggestionReading({text:'book',completion:'book'},{normalizedInput:'bo'},'english'),'book');
});

test('alternate consonant counts display the preferred input and remain exact matches',async()=>{
  const engine=await createEngine();
  try{
    for(const [input,word,preferred]of [['lertthi','លទ្ធិ','lerthi'],['vaekikor','វគ្គីករណ៍','vaekkikor']]){
      const result=engine.suggest(input,{previousWords:[]});
      const before=JSON.stringify(result),candidate=result.candidates.find(c=>c.khmer===word);
      assert(candidate,input);assert(result.exactCandidates.some(c=>c.khmer===word));
      assert.equal(candidate.preferredInput,preferred);
      assert.equal(suggestionReading(candidate,result,'khmer'),preferred);
      assert(suggestionStatus(candidate,result).states.some(s=>s.kind==='match'));
      assert(!suggestionStatus(candidate,result).states.some(s=>['correction','completion'].includes(s.kind)));
      assert.equal(JSON.stringify(result),before);
    }
    assert.equal(suggestionReading({khmer:'លទ្ធិ'},{normalizedInput:'lertthi'},'khmer'),'lertthi');
  }finally{engine.dispose();}
});

test('presentation leaves real suggestion results, flags and ordering unchanged',async()=>{
  const engine=await createEngine();
  try{
    for(const [input,language] of [['f','khmer'],['siev','khmer'],['សៀវ','khmer'],['angkor','khmer'],['recieve','english']]){
      const result=engine.suggest(input,{language,correction:true}),before=JSON.stringify(result);
      for(const candidate of result.candidates){suggestionStatus(candidate,result);suggestionReading(candidate,result,language);}
      assert.equal(JSON.stringify(result),before);
      assert.deepEqual(engine.suggest(input,{language,correction:true}),result);
    }
  }finally{engine.dispose();}
});

test('fully typed English and Khmer spellings show Match while longer suggestions remain Completion',async()=>{
  const engine=await createEngine();
  try{
    for(const [input,language,word] of [['book','english','book'],['books','english','books'],['Books','english','Books'],['BOOK','english','book'],['សៀវភៅ','khmer','សៀវភៅ'],['sievphov','khmer','សៀវភៅ'],['angkor','khmer','អង្គរ']]){
      const result=engine.suggest(input,{language}),before=JSON.stringify(result);
      const candidate=result.candidates.find(c=>(c.text??c.khmer)===word);assert(candidate,input);
      const kinds=suggestionStatus(candidate,result).states.map(s=>s.kind);
      assert(kinds.includes('match'),input);assert(!kinds.includes('completion'),input);
      for(const longer of result.candidates.filter(c=>c.completion&&c.completion.toLowerCase()!==result.normalizedInput)){
        assert(suggestionStatus(longer,result).states.some(s=>s.kind==='completion'));
      }
      assert.equal(JSON.stringify(result),before);assert.deepEqual(engine.suggest(input,{language}),result);
    }
  }finally{engine.dispose();}
});

test('an exact protected word keeps Match plus explicit selection, without changing correction dots',async()=>{
  const engine=await createEngine();
  try{
    const exact=engine.suggest('សៀវ',{previousWords:[]}),word=exact.candidates.find(c=>c.khmer==='សៀវ');
    assert(word?.tapOnly);assert.deepEqual(suggestionStatus(word,exact).states.map(s=>s.kind),['match','explicit']);
    const repair=engine.suggest('recieve',{language:'english',correction:true});
    const corrected=repair.candidates.find(c=>c.text==='receive');assert(corrected?.correction);
    assert.deepEqual(suggestionStatus(corrected,repair).states.map(s=>s.kind),['completion','correction']);
    assert.deepEqual(suggestionStatus({...corrected,manualOnly:true},repair).states.map(s=>s.kind),['completion','correction','explicit']);
  }finally{engine.dispose();}
});
