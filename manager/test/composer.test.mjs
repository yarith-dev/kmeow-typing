import test from 'node:test';
import assert from 'node:assert/strict';
import {emptyDraft,replace,editMessage,contextFor,spaceChoice,literalSpace,history,suggestDraft} from '../composer.js';
import {createEngine} from '../../dist/public/node.js';

test('sentence choices insert at the cursor, replace selections and preserve surrounding text',()=>{
  let draft={...emptyDraft(),text:'Hello !',start:6,end:6,input:'sievphov'};
  draft=replace(draft,'សៀវភៅ',{});assert.equal(draft.text,'Hello សៀវភៅ!');assert.equal(draft.input,'');
  assert.equal(draft.start,6+'សៀវភៅ'.length);assert.equal(draft.end,draft.start);
  draft=replace({...draft,start:0,end:5},'Hi');assert.equal(draft.text,'Hi សៀវភៅ!');
  assert.throws(()=>replace({...emptyDraft(),text:'a'.repeat(20000)},'b'),/full/);
});

test('context follows intact choices at the cursor and clears after literal edits',()=>{
  let draft=replace(emptyDraft(),'សៀវភៅ',{});draft=replace(draft,'កូន',{});
  assert.deepEqual(contextFor(draft,'khmer'),{previousWords:['សៀវភៅ','កូន']});
  assert.deepEqual(contextFor({...draft,start:0,end:0},'khmer'),{previousWords:[]});
  assert.deepEqual(contextFor(editMessage(draft,draft.text,0,0),'khmer'),{previousWords:[]});
  assert.deepEqual(contextFor({...emptyDraft(),text:'She lets us go',start:4,end:8},'english'),{before:'She ',after:' us go'});
});

test('undo and redo restore exact text, composition, selections and context',()=>{
  const initial={...emptyDraft(),input:'sievphov'};const state=history(initial);
  state.push(replace(state.current,'សៀវភៅ',{}));const inserted=state.current;
  assert.deepEqual(state.undo(),initial);assert.deepEqual(state.redo(),inserted);
  state.undo();state.push(replace(state.current,'literal'));assert.equal(state.canRedo,false);
});

test('Space never accepts completions or explicit-selection-only words',()=>{
  const word={mappingId:1,khmer:'កូន'},result={candidates:[word],exactCandidates:[word]};
  assert.equal(spaceChoice(result,0),word);
  for(const flag of ['tapOnly','manualOnly','completion'])assert.equal(spaceChoice({...result,candidates:[{...word,[flag]:true}],exactCandidates:[{...word,[flag]:true}]},0),undefined);
  assert.equal(spaceChoice({candidates:[{...word,tapOnly:true}],exactCandidates:[word]},0),undefined);
  assert.equal(literalSpace('hello'),'hello ');assert.equal(literalSpace('កូន'),'កូន');assert.equal(literalSpace(''),' ');
});

test('real engine suggestions compose a message without altering typing results',async()=>{
  const engine=await createEngine();try{
    const before=engine.suggest('sievphov');const selected=before.candidates[0];
    const draft=replace({...emptyDraft(),input:'sievphov'},selected.khmer,selected);
    assert.equal(draft.text,'សៀវភៅ');assert.deepEqual(engine.suggest('sievphov'),before);
  }finally{engine.dispose();}
});

test('English context stays within Unicode limits and retains nearby text',async()=>{
  const engine=await createEngine();try{
    const before='😀'.repeat(600)+' She ',after=' us go '+'😀'.repeat(200);
    const draft={...emptyDraft(),text:before+'lets'+after,start:before.length,end:before.length+4,input:'lets'};
    const context=contextFor(draft,'english');
    assert.equal([...context.before].length,512);assert.equal([...context.after].length,128);
    assert(context.before.endsWith(' She '));assert(context.after.startsWith(' us go '));
    const settings={language:'english',correction:true};
    assert.deepEqual(suggestDraft(engine,draft,settings),engine.suggest('lets',{...settings,before:[...before].slice(-512).join(''),after:[...after].slice(0,128).join('')}));
    assert.doesNotThrow(()=>suggestDraft(engine,{...emptyDraft(),text:'a'.repeat(20000),start:10000,end:10000,input:'bo'},settings));
  }finally{engine.dispose();}
});

test('IME completion plus final input creates one undo step in either field',()=>{
  for(const field of ['input','message']){
    const initial=emptyDraft(),state=history(initial);
    const finish=()=>field==='input'?{...state.current,input:'ក'}:editMessage(state.current,'ក',1,1);
    state.push(finish());state.push(finish());
    const completed=state.current;
    assert.deepEqual(state.undo(),initial);assert.equal(state.canUndo,false);
    // A redundant browser input/selection event must not destroy redo either.
    state.push({...state.current});assert.equal(state.canRedo,true);
    assert.deepEqual(state.redo(),completed);
  }
});

test('edits shift intact Khmer choices and retain their suggestion ordering',async()=>{
  const engine=await createEngine();try{
    let draft=replace({...emptyDraft(),text:'Hello ',start:6,end:6},'ពាក្យ',{});
    draft={...draft,input:'s'};
    const before=suggestDraft(engine,draft,{language:'khmer'});
    const text='Hi '+draft.text.slice(6);
    const edited=editMessage(draft,text,text.length,text.length);
    assert.deepEqual(contextFor(edited,'khmer'),{previousWords:['ពាក្យ']});
    assert.equal(edited.spans[0].start,3);assert.equal(edited.input,'s');
    assert.deepEqual(suggestDraft(engine,edited,{language:'khmer'}),before);
    const changed=editMessage(edited,'Hi កូន','Hi កូន'.length,'Hi កូន'.length);
    assert.deepEqual(changed.spans,[]);
    assert.deepEqual(contextFor(changed,'khmer'),{previousWords:[]});
    const appended=editMessage(edited,text+' ',text.length+1,text.length+1);
    assert.equal(appended.spans.length,1);assert.deepEqual(contextFor(appended,'khmer'),{previousWords:[]});
    assert.deepEqual(editMessage(edited,text,0,0).spans,edited.spans);
  }finally{engine.dispose();}
});

test('empty composition exposes next-word results without enabling Space acceptance',async()=>{
  const engine=await createEngine();try{
    const draft=replace(emptyDraft(),'ពាក្យ',{});
    const result=suggestDraft(engine,draft,{language:'khmer'});
    assert.deepEqual(result,engine.suggest('',{language:'khmer',previousWords:['ពាក្យ']}));
    assert(result.candidates.length>0);assert(result.candidates.some(c=>c.khmer==='សុំ'));
    assert.equal(spaceChoice(result,0),undefined);
    const choice=result.candidates[0];const inserted=replace(draft,choice.khmer,choice);
    assert.equal(inserted.text,'ពាក្យ'+choice.khmer);
    const state=history(draft);state.push(inserted);assert.deepEqual(state.undo(),draft);
    assert.deepEqual(suggestDraft(engine,state.current,{language:'khmer'}),result);
  }finally{engine.dispose();}
});
