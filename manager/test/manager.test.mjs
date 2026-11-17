import test from 'node:test';
import assert from 'node:assert/strict';
import { once } from 'node:events';
import { createEngine } from '../../dist/public/node.js';
import { managerServer } from '../server.mjs';
import {parseImport,parseImportSpec,wordExport,loadSharedDictionary,saveSharedDictionary} from '../state.js';

test('word exports accept both previous personal files and shared word files',()=>{
  const words=[{word:'សៀវភៅ',language:'khmer',spellings:['mybook'],rank:0}];
  const catalog={khmer:[{word:'សៀវភៅ',romanizations:['mybook'],rank:0}],english:[]};
  assert.deepEqual(parseImport(wordExport(catalog)),words);
  assert.deepEqual(parseImport(JSON.stringify({format:'kmeow-personal-v1',words})),words);
  assert.equal(parseImportSpec(JSON.stringify({format:'kmeow-personal-v1',words})).preservePersonal,true);
  const shared=[{word:'Book',language:'english',rank:0,spellings:[]}];
  const restored=parseImportSpec(wordExport({...catalog,sharedWords:shared}));
  assert.equal(restored.preservePersonal,false);assert.deepEqual(restored.preferences,shared);
  for(const bad of ['null','{}','{"format":"other","words":[]}',JSON.stringify(Array(20001).fill({}))])assert.throws(()=>parseImport(bad));
});

test('exports preserve Sound map policies and avoid flattening different identity ratings',()=>{
  const catalog={khmer:[
    {word:'ក',rank:0,commonnessLevel:2,soundMapEnabled:false,romanizations:['k']},
    {word:'ខ',rank:0,commonnessLevel:2,soundMapEnabled:true},
    {word:'ខ',rank:0,commonnessLevel:4,soundMapEnabled:true},
  ],english:[{word:'book',rank:0,commonnessLevel:1}]};
  const words=parseImport(wordExport(catalog));
  assert.equal(words[0].soundMapEnabled,false);assert.equal(words[0].commonnessLevel,2);
  assert(words.filter(row=>row.word==='ខ').every(row=>!Object.hasOwn(row,'commonnessLevel')));
  assert.equal(words.find(row=>row.language==='english').commonnessLevel,1);
});

test('failed loading or saving leaves the current engine and browser words intact',async()=>{
  const engine=await createEngine({words:[{word:'សៀវភៅ',spellings:['mybook']}]});
  const before=engine.suggest('mybook');
  try{
    await assert.rejects(loadSharedDictionary({createEngine,fetcher:async()=>({ok:false,status:500,json:async()=>({error:'Read failed'})})}),/Read failed/);
    await assert.rejects(saveSharedDictionary({catalog:{token:'token',revision:'old'},request:{},fetcher:async()=>({ok:false,status:409,json:async()=>({error:'Refresh before saving'})})}),/Refresh/);
    assert.deepEqual(engine.suggest('mybook'),before);assert.equal(engine.getWords().length,1);
  }finally{engine.dispose();}
});

test('local server serves the interface and engine assets, rejecting private files and writes',async()=>{
  const server=managerServer();server.listen(0,'127.0.0.1');await once(server,'listening');
  const base=`http://127.0.0.1:${server.address().port}`;
  try{
    const page=await fetch(base);assert.equal(page.status,200);const html=await page.text();
    assert.match(html,/Word list/);assert.match(html,/Sound map/);
    for(const path of ['/manager/app.js','/manager/field-help.js','/manager/i18n.js','/manager/khmer-ui.js','/manager/examples.js','/manager/sound-rules.js','/manager/suggestion-view.js','/manager/suggestion-gesture.js','/manager/word-list.js','/manager/state.js','/manager/composer.js','/manager/style.css','/manager/mobile.css','/dist/public/browser.js','/wasm/khmer_engine.js','/wasm/khmer_engine_bg.wasm','/compiled-data/khmer.kmd','/compiled-data/english.kmd','/data/khmer-sound-map.json','/manager/fonts/noto-sans-khmer-khmer-400-normal.woff2']){
      const response=await fetch(base+path);assert.equal(response.status,200,path);assert((await response.arrayBuffer()).byteLength>0);
    }
    for(const path of ['/package.json','/.git','/manager/server.mjs','/manager/dictionary.mjs','/manager/dictionary-store.mjs','/.dictionary-manager.lock','/manager/test/manager.test.mjs','/dist/%2e%2e/package.json','/data/profanity.json'])assert.equal((await fetch(base+path)).status,404,path);
    const catalog=await (await fetch(base+'/api/words')).json();
    assert(catalog.khmer.some(row=>row.word==='សៀវភៅ'));assert(catalog.english.some(row=>row.word==='book'));
    assert.equal((await fetch(base+'/api/words',{method:'POST',body:'test'})).status,403);
    assert.equal((await fetch(base,{method:'POST',body:'test'})).status,405);
    assert.equal((await fetch(base,{method:'HEAD'})).status,200);
    assert.match(page.headers.get('content-security-policy'),/frame-ancestors 'none'/);
  }finally{await new Promise(resolve=>server.close(resolve));}
});
