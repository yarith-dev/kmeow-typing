import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,readFile,writeFile,rm,readdir} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {dictionaryStore} from '../dictionary-store.mjs';
import {createEngine} from '../../dist/public/node.js';
import {managerServer} from '../server.mjs';
const rules=[{word:'មិត្ត',readAs:'មិត'}];
async function fixture(t){
  const root=await mkdtemp(join(tmpdir(),'kmeow-pronunciation-'));t.after(()=>rm(root,{recursive:true,force:true}));await mkdir(join(root,'compiled-data'));
  for(const file of ['compiled-data/khmer.kmd','compiled-data/english.kmd','dictionary-manifest.json'])await writeFile(join(root,file),await readFile(new URL('../../'+file,import.meta.url)));
  return root;
}

test('pronunciation rules persist separately from words and aliases, survive reload, and reset',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const original=await createEngine({dictionary:before.files});original.setPronunciations([]);t.after(()=>original.dispose());
  const after=await store.save({type:'pronunciations',rules},before.revision);
  assert.deepEqual(after.source,before.source);assert.deepEqual(after.metadata,before.metadata);
  assert.deepEqual(after.info.provenance.pronunciations,{format:'kmeow-pronunciations-v1',rules});
  const restarted=await dictionaryStore(root).snapshot(),core=await createEngine({dictionary:restarted.files});t.after(()=>core.dispose());
  assert.deepEqual(core.previewPronunciation('មិត'),['met','mit']);
  for(const input of ['met','mit'])assert(core.suggest(input).candidates.some(c=>c.khmer==='មិត្ត'&&!c.completion));
  for(const input of ['sievphov','angkor','books'])assert.deepEqual(core.suggest(input),original.suggest(input),input);
  for(const mapping of before.source.mappings)assert.deepEqual(core.convert(mapping.romanized),original.convert(mapping.romanized));
  const reset=await store.save({type:'pronunciations',rules:[]},restarted.revision);
  assert.equal(reset.info.provenance.pronunciations,undefined);assert.deepEqual(reset.source,before.source);
  const restored=await createEngine({dictionary:reset.files});t.after(()=>restored.dispose());
  for(const input of ['met','mit','mettar','metpeap'])assert.deepEqual(restored.suggest(input),original.suggest(input));
  assert(!(await readdir(root)).some(file=>file.startsWith('.dictionary')));
});

test('invalid and stale rule saves leave the pair unchanged',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  for(const invalid of [[{word:'មិត្ត',readAs:'latin'}],[{word:'missing',readAs:'មិត'}],[...rules,...rules]]){
    await assert.rejects(store.save({type:'pronunciations',rules:invalid},before.revision));
    assert.equal((await store.snapshot()).revision,before.revision);
  }
  const after=await store.save({type:'pronunciations',rules},before.revision);
  await assert.rejects(store.save({type:'pronunciations',rules:[]},before.revision),e=>e.status===409);
  assert.equal((await store.snapshot()).revision,after.revision);
});

test('multiple spellings persist and reset without changing dictionary records or other settings',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const stock=before.info.provenance.pronunciations?.rules??[];
  const rules=[...stock.filter(rule=>rule.word!=='ដែល'),{word:'ដែល',readAs:['ដេល','ដែល']}];
  const after=await store.save({type:'pronunciations',rules},before.revision);
  assert.deepEqual(after.source,before.source);assert.deepEqual(after.metadata,before.metadata);
  for(const key of ['shorthands','inputExclusions','sharedWords','wordComponents'])assert.deepEqual(after.info.provenance[key],before.info.provenance[key]);
  const reopened=await dictionaryStore(root).snapshot();
  assert.deepEqual(reopened.info.provenance.pronunciations.rules,rules);
  const core=await createEngine({dictionary:reopened.files});t.after(()=>core.dispose());
  assert.deepEqual(core.previewPronunciation(['ដេល','ដែល','ដេល']),['del','dael']);
  for(const input of ['del','dael'])assert(core.suggest(input).candidates.some(word=>word.khmer==='ដែល'&&!word.completion));
  for(const invalid of [[],['ដេល','latin'],{spelling:'ដេល'}]){
    await assert.rejects(store.save({type:'pronunciations',rules:[...stock.filter(rule=>rule.word!=='ដែល'),{word:'ដែល',readAs:invalid}]},reopened.revision));
    assert.equal((await store.snapshot()).revision,reopened.revision);
  }
  const reset=await store.save({type:'pronunciations',rules:stock},reopened.revision);
  assert.deepEqual(reset.source,before.source);assert.deepEqual(reset.metadata,before.metadata);
  assert.deepEqual(reset.info.provenance.pronunciations,before.info.provenance.pronunciations);
});

test('marked Read as saves preserve independent-vowel boundaries through reopen and reset',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const stock=before.info.provenance.pronunciations?.rules??[];
  const edited=await store.save({type:'pronunciations',rules:[...stock.filter(rule=>rule.word!=='មិត្ត'),
    {word:'មិត្ត',readAs:'គៈឥ'}]},before.revision);
  assert.deepEqual(edited.source,before.source);assert.deepEqual(edited.metadata,before.metadata);
  const reopened=await dictionaryStore(root).snapshot(),core=await createEngine({dictionary:reopened.files});
  try{
    assert.deepEqual(core.previewPronunciation('គៈឥ'),['keakei','keakey']);
    assert(core.suggest('keakei',{previousWords:[]}).exactCandidates.some(row=>row.khmer==='មិត្ត'));
    assert(!core.suggest('keae',{previousWords:[]}).exactCandidates.some(row=>row.khmer==='មិត្ត'));
  }finally{core.dispose();}
  await assert.rejects(store.save({type:'pronunciations',rules:[{word:'មិត្ត',readAs:'គៈឨ'}]},reopened.revision));
  assert.equal((await store.snapshot()).revision,reopened.revision);
  const reset=await store.save({type:'pronunciations',rules:stock},reopened.revision);
  assert.deepEqual(reset.catalog,before.catalog);assert.deepEqual(reset.source,before.source);
});

test('server exposes and saves rules through the authenticated revision-checked API',async t=>{
  const root=await fixture(t),server=managerServer({dictionaryRoot:root});await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));t.after(()=>new Promise(resolve=>server.close(resolve)));
  const base=`http://127.0.0.1:${server.address().port}`,catalog=await(await fetch(base+'/api/words')).json();
  assert.deepEqual(catalog.pronunciations,(await dictionaryStore(root).snapshot()).info.provenance.pronunciations?.rules??[]);
  assert.equal((await fetch(base+'/api/words',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({type:'pronunciations',rules})})).status,403);
  const response=await fetch(base+'/api/words',{method:'POST',headers:{'Content-Type':'application/json','X-Kmeow-Token':catalog.token,'If-Match':catalog.revision},body:JSON.stringify({type:'pronunciations',rules})});
  assert.equal(response.status,200);assert.deepEqual((await response.json()).pronunciations,rules);
});

test('editing a general Read as rule preserves checked compound keys after saving and reopening the pair',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const rules=[...(before.info.provenance.pronunciations?.rules??[]).filter(rule=>rule.word!=='សាស្ត្រ'),{word:'សាស្ត្រ',readAs:'កូន'}];
  const after=await store.save({type:'pronunciations',rules},before.revision);
  assert.deepEqual(after.source,before.source);assert.deepEqual(after.metadata,before.metadata);
  const reopened=await dictionaryStore(root).snapshot(),core=await createEngine({dictionary:reopened.files});t.after(()=>core.dispose());
  const exact=(input,word)=>core.suggest(input).candidates.some(candidate=>candidate.khmer===word&&!candidate.completion);
  assert(exact('kaeknitsah','គណិតសាស្ត្រ'));assert(exact('kaeknitsas','គណិតសាស្ត្រ'));
  assert(exact('ekokon','អេកូសាស្ត្រ'));assert(!exact('kaeknitkon','គណិតសាស្ត្រ'));
});

test('stock saves and terminal edits survive reopening without changing words, aliases or reset results',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const stock=before.info.provenance.pronunciations?.rules??[];
  const unchanged=await store.save({type:'pronunciations',rules:[...stock,{word:'ព័ន្ធ',readAs:'ពាន់'}]},before.revision);
  assert.deepEqual(unchanged.catalog,before.catalog);
  assert.deepEqual(unchanged.source,before.source);
  const reopened=await dictionaryStore(root).snapshot();
  assert.deepEqual(reopened.catalog,before.catalog);
  const edited=await store.save({type:'pronunciations',rules:[...stock,{word:'ចក្រ',readAs:'មិត'}]},reopened.revision);
  const core=await createEngine({dictionary:(await dictionaryStore(root).snapshot()).files});
  try{
    for(const input of ['anamet','anamit'])assert(core.suggest(input).candidates.some(candidate=>candidate.khmer==='អាណាចក្រ'&&!candidate.completion),input);
  }finally{core.dispose();}
  assert.deepEqual(edited.source,before.source);
  assert.deepEqual(edited.metadata,before.metadata);
  const reset=await store.save({type:'pronunciations',rules:stock},edited.revision);
  assert.deepEqual(reset.catalog,before.catalog);
  assert.deepEqual(reset.source,before.source);
});

test('saved Read as edits retain builtin nh across reload and preserve explicit shorthand removal',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const retained=(before.info.provenance.pronunciations?.rules??[]).filter(rule=>rule.word!=='ខ្ញុំ');
  const after=await store.save({type:'pronunciations',rules:[...retained,{word:'ខ្ញុំ',readAs:'មិត'}]},before.revision);
  assert.deepEqual(after.source,before.source);
  for(const language of ['khmer','english'])assert.deepEqual(after.metadata[language].words,before.metadata[language].words);
  const restarted=await dictionaryStore(root).snapshot(),core=await createEngine({dictionary:restarted.files});
  try{
    assert.deepEqual(core.wordShorthands('ខ្ញុំ'),['nh']);
    for(const input of ['nh','met','mit']){
      const result=core.suggest(input,{previousWords:[]});
      assert(result.exactCandidates.some(hit=>hit.khmer==='ខ្ញុំ'),input);
      assert(result.candidates.some(hit=>hit.khmer==='ខ្ញុំ'&&!hit.tapOnly),input);
    }
    assert(!core.convert('nh').candidates.some(hit=>hit.khmer==='ខ្ញុំ'));
  }finally{core.dispose();}
  const disabled=await store.save({type:'save',word:{word:'ខ្ញុំ',shorthands:[]}},after.revision);
  const edited=await store.save({type:'pronunciations',rules:[...retained,{word:'ខ្ញុំ',readAs:'កូន'}]},disabled.revision);
  const core2=await createEngine({dictionary:edited.files});
  try{
    assert.deepEqual(core2.wordShorthands('ខ្ញុំ'),[]);
    assert(!core2.suggest('nh',{previousWords:[]}).exactCandidates.some(hit=>hit.khmer==='ខ្ញុំ'));
    assert(core2.suggest('kon',{previousWords:[]}).exactCandidates.some(hit=>hit.khmer==='ខ្ញុំ'));
  }finally{core2.dispose();}
});
