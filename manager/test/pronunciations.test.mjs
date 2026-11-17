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
