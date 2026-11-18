import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,readFile,writeFile,rm,rename,readdir,copyFile,symlink} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createEngine} from '../../dist/public/node.js';
import {dictionaryStore} from '../dictionary-store.mjs';
import {managerServer} from '../server.mjs';
import {loadSharedDictionary,saveSharedDictionary,parseImport,wordExport} from '../state.js';

const paths=['compiled-data/khmer.kmd','compiled-data/english.kmd','dictionary-manifest.json'];
async function fixture(t){
  const root=await mkdtemp(join(tmpdir(),'kmeow-manager-dictionary-'));t.after(()=>rm(root,{recursive:true,force:true}));
  await mkdir(join(root,'compiled-data'));
  for(const path of paths)await writeFile(join(root,path),await readFile(new URL('../../'+path,import.meta.url)));
  return root;
}
async function assertNoSavedHistory(root){
  const names=await readdir(root);
  assert(!names.some(name=>name.startsWith('.dictionary-update-')||name.startsWith('.dictionary-write-')||name.startsWith('.dictionary-manager')));
  await assert.rejects(readFile(join(root,'data/dictionary-edits.json')),{code:'ENOENT'});
}
const spellingChange=spellings=>({type:'save',original:{language:'khmer',word:'សៀវភៅ'},word:{language:'khmer',word:'សៀវភៅ',spellings}});

test('reads and unchanged saves preserve the complete pair, every identity and all typing records',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const row=before.catalog.khmer.find(word=>word.word==='សៀវភៅ');
  const after=await store.save({...spellingChange(row.romanizations),word:{...spellingChange(row.romanizations).word,rank:row.rank}},before.revision);
  assert.equal(after.revision,before.revision);assert.deepEqual(after.bytes,before.bytes);assert.deepEqual(after.source,before.source);
  assert.deepEqual(after.metadata,before.metadata);await assertNoSavedHistory(root);
});

test('a saved spelling changes the real pair, survives restart and preserves all words and unrelated records',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const row=before.catalog.khmer.find(word=>word.word==='សៀវភៅ'),request=spellingChange([...row.romanizations,'sharedbooktest']);
  const after=await store.save(request,before.revision);
  assert.notEqual(after.revision,before.revision);
  assert.deepEqual(after.source.words,before.source.words);assert.deepEqual(after.source.englishWords,before.source.englishWords);
  assert.deepEqual(after.source.structures,before.source.structures);assert.deepEqual(after.source.soundMapDisabledWords,before.source.soundMapDisabledWords);
  assert.deepEqual(after.source.mappings.filter(row=>row.romanized!=='sharedbooktest'),before.source.mappings);
  for(const language of ['khmer','english'])assert.deepEqual(after.metadata[language].words,before.metadata[language].words);
  const restarted=await dictionaryStore(root).snapshot();assert.equal(restarted.revision,after.revision);
  const engine=await createEngine({dictionary:restarted.files}),original=await createEngine({dictionary:before.files});
  try{
    assert.equal(engine.convert('sharedbooktest').candidates[0].khmer,'សៀវភៅ');
    for(const input of ['sievphov','angkor','nih','niyom','tusna','ក','សៀវភៅ','kh'])assert.deepEqual(engine.suggest(input),original.suggest(input),input);
    for(const input of ['book','books','lets','children','wrote'])assert.deepEqual(engine.suggest(input,{language:'english',correction:true}),original.suggest(input,{language:'english',correction:true}),input);
  }finally{engine.dispose();original.dispose();}
  await assert.rejects(store.save(request,before.revision),error=>error.status===409);
  assert.equal((await store.snapshot()).revision,after.revision);
  await assertNoSavedHistory(root);
});

test('new Khmer and English words are shared, require review and never replace existing words',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const words=[{word:'សៀវភៅសៀវភៅ',language:'khmer',spellings:['fixturebook'],rank:1},{word:'kmeowfixture',language:'english',rank:1}];
  await assert.rejects(store.save({type:'import',words},before.revision),/Review new words/);
  const after=await store.save({type:'import',words,reviewed:true},before.revision);
  for(const [key,extra] of [['words',1],['englishWords',1]]){
    assert.equal(after.source[key].length,before.source[key].length+extra);
    const originalIds=new Set(before.source[key].map(word=>word.id));
    assert.deepEqual(after.source[key].filter(word=>originalIds.has(word.id)),before.source[key]);
  }
  const engine=await createEngine({dictionary:after.files});
  try{assert(engine.suggest('fixturebook').candidates.some(word=>word.khmer==='សៀវភៅសៀវភៅ'));assert(engine.suggest('kmeowfixture',{language:'english'}).candidates.some(word=>word.text==='kmeowfixture'));}
  finally{engine.dispose();}
  await assert.rejects(store.save({type:'save',original:{word:'សៀវភៅ',language:'khmer'},word:{word:'សៀវភៅសៀវភៅ',language:'khmer'}},after.revision),/keep their text/);
});

test('validation, failed commits and stale concurrent saves leave a complete pair without retained copies',async t=>{
  const root=await fixture(t);let calls=0;
  const store=dictionaryStore(root,{commitRename:async(from,to)=>{if(++calls===2)throw new Error('Injected write failure');await rename(from,to);}}),before=await store.snapshot();
  await assert.rejects(store.save(spellingChange(['!invalid']),before.revision),/Romanized/);
  await assert.rejects(store.save(spellingChange(['recoverbook']),before.revision),/Injected write failure/);
  assert.deepEqual((await dictionaryStore(root).snapshot()).bytes,before.bytes);
  await assertNoSavedHistory(root);
  const normal=dictionaryStore(root),a=normal.save(spellingChange(['firstbook']),before.revision),b=normal.save(spellingChange(['secondbook']),before.revision);
  const results=await Promise.allSettled([a,b]);assert.equal(results.filter(result=>result.status==='fulfilled').length,1);
  assert.equal(results.find(result=>result.status==='rejected').reason.status,409);
  await assertNoSavedHistory(root);
});

test('an interrupted multi-file write is restored before the next dictionary read',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const temporary=await mkdtemp(join(root,'.dictionary-write-'));
  for(let i=0;i<paths.length;i++)await writeFile(join(temporary,paths[i].replaceAll('/','__')+'.before'),before.bytes[i]);
  await writeFile(join(root,'.dictionary-manager-transaction.json'),JSON.stringify({directory:temporary.split('/').at(-1)}));
  await writeFile(join(root,paths[0]),'interrupted');
  assert.deepEqual((await dictionaryStore(root).snapshot()).bytes,before.bytes);
  await assertNoSavedHistory(root);
});

test('opening the dictionary removes abandoned transaction files without changing the pair',async t=>{
  const root=await fixture(t),before=await dictionaryStore(root).snapshot();
  const temporary=await mkdtemp(join(root,'.dictionary-write-'));
  await writeFile(join(temporary,'unpublished'),'temporary data');
  await writeFile(join(root,'.dictionary-manager-transaction.json.tmp'),'unfinished journal');
  assert.deepEqual((await dictionaryStore(root).snapshot()).bytes,before.bytes);
  await assertNoSavedHistory(root);
});

test('the CLI resolves a pending manager recovery before admitting another word',async t=>{
  const root=await fixture(t),before=await dictionaryStore(root).snapshot();
  const word='សៀវភៅសៀវភៅ',source=structuredClone(before.source);
  source.words.push({id:Math.max(...source.words.map(row=>row.id),...source.mappings.map(row=>row.id),...source.englishWords.map(row=>row.id))+1,khmer:word,rank:1,status:'approved'});
  const {WasmEngine}=await import('../../wasm/khmer_engine.js');
  const core=new WasmEngine(JSON.stringify(source));let inputs;
  try{inputs=JSON.parse(core.browseWords(JSON.stringify({query:word,limit:10}))).words.find(row=>row.khmer===word).soundMapInputs;}finally{core.free();}
  await mkdir(join(root,'scripts'));
  await copyFile(new URL('../../scripts/add-word.mjs',import.meta.url),join(root,'scripts/add-word.mjs'));
  for(const path of ['wasm','manager','scripts/read-compact-source.mjs'])await symlink(fileURLToPath(new URL('../../'+path,import.meta.url)),join(root,path));
  const temporary=await mkdtemp(join(root,'.dictionary-write-'));
  for(let i=0;i<paths.length;i++)await writeFile(join(temporary,paths[i].replaceAll('/','__')+'.before'),before.bytes[i]);
  await writeFile(join(root,'.dictionary-manager-transaction.json'),JSON.stringify({directory:temporary.split('/').at(-1)}));
  await writeFile(join(root,paths[0]),'interrupted');
  await writeFile(join(root,'admission.json'),JSON.stringify({word,date:'2026-11-16',partsOfSpeech:['noun'],expectedInputs:inputs,suitability:{decision:'clean',scope:'Isolated test fixture only'}}));
  const env=Object.fromEntries(Object.entries(process.env).filter(([key])=>!key.startsWith('NODE_TEST_')));
  await promisify(execFile)(process.execPath,[join(root,'scripts/add-word.mjs'),join(root,'admission.json'),'--dry-run'],{env});
  assert.deepEqual((await dictionaryStore(root).snapshot()).bytes,before.bytes);
  await assertNoSavedHistory(root);
  await promisify(execFile)(process.execPath,[join(root,'scripts/add-word.mjs'),join(root,'admission.json')],{env});
  const after=await dictionaryStore(root).snapshot();
  assert(after.source.words.some(row=>row.khmer===word));
  assert.equal(after.source.words.length,before.source.words.length+1);
  await assert.rejects(readFile(join(root,'test/fixtures/word-admissions.json')),{code:'ENOENT'});
  await assertNoSavedHistory(root);
});

test('migrated literal and capitalized preferences retain typing behavior in a fresh SDK and raw core',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const words=[{word:'lets',language:'english',rank:0},{word:'Book',language:'english',rank:0},{word:'សៀវភៅ',language:'khmer',spellings:['mybook'],rank:0}];
  const original=await createEngine({dictionary:before.files,words});
  const saved=await store.save({type:'import',words,reviewed:true,preservePersonal:true},before.revision);
  const fresh=await createEngine({dictionary:saved.files});
  const {WasmEngine}=await import('../../wasm/khmer_engine.js'),raw=WasmEngine.fromAppData(saved.files.khmer,saved.files.english);
  const normalized=value=>JSON.parse(JSON.stringify(value,(key,value)=>key==='mappingId'||key==='mappingIds'?undefined:value));
  try{
    assert.deepEqual(fresh.getWords(),[]);
    assert.deepEqual(saved.source.englishWords.filter(row=>row.text.toLowerCase()==='book'),before.source.englishWords.filter(row=>row.text.toLowerCase()==='book'));
    for(const language of ['english','khmer'])for(const input of language==='english'?['l','le','lets',"let's",'b','bo','book','Book','BOOK','books']:['s','si','sievphov','my','mybook','សៀវភៅ'])for(const context of ['', 'She ', 'Please ']){
      const options={language,correction:true,before:context,after:''};
      assert.deepEqual(normalized(fresh.suggest(input,options)),normalized(original.suggest(input,options)),`${language}: ${context}${input}`);
    }
    assert.equal(JSON.parse(raw.suggestWithContext('lets',JSON.stringify({mode:'english',englishAutocorrection:true}),JSON.stringify({previousWords:[],englishWordContext:{before:'',after:''}}))).candidates[0].text,'lets');
    fresh.addWords([{word:'localfixture',language:'english'}]);original.addWords([{word:'localfixture',language:'english'}]);
    for(const input of ['l','le','lets','local'])assert.deepEqual(normalized(fresh.suggest(input,{language:'english',before:'',after:''})),normalized(original.suggest(input,{language:'english',before:'',after:''})));
  }finally{fresh.dispose();original.dispose();raw.free();}
});

test('repeated rank saves retain only the current pair and preserve unrelated records',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const row=before.catalog.khmer.find(word=>word.word==='សៀវភៅ');
  const ranked=await store.save({...spellingChange(row.romanizations),word:{...spellingChange(row.romanizations).word,rank:row.rank+1}},before.revision);
  assert.deepEqual(ranked.metadata.khmer.words,before.metadata.khmer.words);
  assert.deepEqual(ranked.source.words.filter(word=>word.khmer!=='សៀវភៅ'),before.source.words.filter(word=>word.khmer!=='សៀវភៅ'));
  const restored=await store.save({...spellingChange(row.romanizations),word:{...spellingChange(row.romanizations).word,rank:row.rank}},ranked.revision);
  assert.equal(restored.info.compactArtifactHash,before.info.compactArtifactHash);
  assert.deepEqual(restored.source,before.source);
  await assertNoSavedHistory(root);
});

test('HTTP saves recover after a token change and still require a local origin and matching revision',async t=>{
  const root=await fixture(t),server=managerServer({dictionaryRoot:root});
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));t.after(()=>new Promise(resolve=>server.close(resolve)));
  const base=`http://127.0.0.1:${server.address().port}`,fetcher=(url,options)=>fetch(base+url,options);
  const loaded=await loadSharedDictionary({fetcher,createEngine,words:[{word:'សៀវភៅ',spellings:['browserbook']}]});
  try{
    const {catalog}=loaded;
    assert.equal(loaded.engine.convert('browserbook').candidates[0].khmer,'សៀវភៅ');
    const request=spellingChange([...catalog.khmer.find(word=>word.word==='សៀវភៅ').romanizations,'httpbook']);
    assert.equal((await fetcher('/api/words',{method:'POST',body:'{}'})).status,403);
    assert.equal((await fetcher('/api/words',{method:'POST',headers:{'X-Kmeow-Token':catalog.token,Origin:'https://other.example','Content-Type':'application/json'},body:JSON.stringify(request)})).status,403);
    const calls=[];
    const browserFetch=(url,options)=>{
      calls.push([url,options?.method??'GET']);
      return fetcher(url,options?.method==='POST'?{
        ...options,headers:{...options.headers,Origin:base,'Sec-Fetch-Site':'same-origin'},
      }:options);
    };
    const expired={...catalog,token:'expired-manager-token'};
    const saved=await saveSharedDictionary({fetcher:browserFetch,catalog:expired,request});assert.notEqual(saved.revision,catalog.revision);
    assert.deepEqual(calls,[['/api/words','POST'],['/api/words','GET'],['/api/words','POST']]);
    assert.equal((await fetcher('/compiled-data/khmer.kmd?revision='+catalog.revision)).status,409);
    const other=await loadSharedDictionary({fetcher,createEngine});
    try{assert.equal(other.engine.convert('httpbook').candidates[0].khmer,'សៀវភៅ');assert.deepEqual(other.engine.getWords(),[]);}
    finally{other.engine.dispose();}
    await assert.rejects(saveSharedDictionary({fetcher:browserFetch,catalog:expired,request}),error=>error.status===409&&/Refresh/.test(error.message));
    assert.equal(loaded.engine.convert('browserbook').candidates[0].khmer,'សៀវភៅ');
    const exported=parseImport(wordExport(saved));assert.equal(exported.length,saved.khmer.length+saved.english.length);
  }finally{loaded.engine.dispose();}
});


test('per-word Sound map saves preserve every word and alias, keep completion, survive restart and reverse exactly',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const row=before.catalog.khmer.find(row=>row.soundMapEnabled&&row.romanizations.length&&row.soundMapInputs.some(input=>!row.romanizations.includes(input)));
  const generated=row.soundMapInputs.find(input=>!row.romanizations.includes(input));
  assert(generated);assert.equal(row.soundMapEnabled,true);
  const request=enabled=>({type:'save',original:{word:row.word,language:'khmer'},word:{word:row.word,language:'khmer',soundMapEnabled:enabled}});
  const disabled=await store.save(request(false),before.revision);
  assert.deepEqual(disabled.source.words,before.source.words);assert.deepEqual(disabled.source.englishWords,before.source.englishWords);
  assert.deepEqual(disabled.source.mappings,before.source.mappings);
  for(const language of ['khmer','english'])assert.deepEqual(disabled.metadata[language].words,before.metadata[language].words);
  assert.deepEqual(disabled.source.structures,before.source.structures);
  const restarted=await dictionaryStore(root).snapshot();
  const changed=restarted.catalog.khmer.find(value=>value.word===row.word);
  assert.equal(changed.soundMapEnabled,false);assert.deepEqual(changed.soundMapInputs,[]);
  const original=await createEngine({dictionary:before.files}),engine=await createEngine({dictionary:restarted.files});
  try{
    assert(original.suggest(generated).candidates.some(hit=>hit.khmer===row.word));
    assert(!engine.suggest(generated).candidates.some(hit=>hit.khmer===row.word));
    for(const alias of row.romanizations)assert(engine.suggest(alias).candidates.some(hit=>hit.khmer===row.word));
    assert(engine.suggest(row.word).candidates.some(hit=>hit.khmer===row.word));
    for(const input of ['angkor','nih','niyom','tusna','ke'])assert.deepEqual(engine.suggest(input),original.suggest(input),input);
    for(const input of ['books','lets'])assert.deepEqual(engine.suggest(input,{language:'english'}),original.suggest(input,{language:'english'}),input);
  }finally{engine.dispose();original.dispose();}
  const noop=await store.save(request(false),restarted.revision);assert.equal(noop.revision,restarted.revision);
  const restored=await store.save(request(true),noop.revision);
  assert.deepEqual(restored.source,before.source);
  for(const language of ['khmer','english'])assert.deepEqual(restored.metadata[language].words,before.metadata[language].words);
  await assertNoSavedHistory(root);
});

test('word levels change only selected metadata and acceptance, preserve compact bytes, and export with Sound map policies',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const word='សៀវភៅ',row=before.catalog.khmer.find(row=>row.word===word);
  const request=level=>({type:'save',word:{word,language:'khmer',commonnessLevel:level}});
  const after=await store.save(request(4),before.revision);
  assert.deepEqual(after.source,before.source);assert.deepEqual(after.metadata.english.words,before.metadata.english.words);
  assert.deepEqual(after.metadata.khmer.words.filter(row=>row.word!==word),before.metadata.khmer.words.filter(row=>row.word!==word));
  assert(after.metadata.khmer.words.filter(row=>row.word===word).every(row=>row.commonnessLevel===4));
  assert.deepEqual(after.files.khmer.subarray(24+after.files.khmer.readUInt32LE(12),24+after.files.khmer.readUInt32LE(12)+after.files.khmer.readUInt32LE(16)),
    before.files.khmer.subarray(24+before.files.khmer.readUInt32LE(12),24+before.files.khmer.readUInt32LE(12)+before.files.khmer.readUInt32LE(16)));
  const engine=await createEngine({dictionary:after.files});
  try{
    const hit=engine.suggest(row.soundMapInputs[0]).candidates.find(candidate=>candidate.khmer===word);
    assert(hit);assert.equal(hit.tapOnly,true);
  }finally{engine.dispose();}
  const exported=parseImport(wordExport(after.catalog));
  const entry=exported.find(row=>row.word===word&&row.language==='khmer');
  assert.equal(entry.commonnessLevel,4);assert.equal(entry.soundMapEnabled,true);
  const noop=await store.save(request(4),after.revision);assert.equal(noop.revision,after.revision);
  const restored=await store.save(request(row.commonnessLevel),after.revision);
  assert.deepEqual(restored.source,before.source);
  for(const language of ['khmer','english'])assert.deepEqual(restored.metadata[language].words,before.metadata[language].words);
  const english=await store.save({type:'save',word:{word:'book',language:'english',commonnessLevel:5}},restored.revision);
  assert.deepEqual(english.source,restored.source);
  assert.deepEqual(english.metadata.khmer.words,restored.metadata.khmer.words);
  const englishEngine=await createEngine({dictionary:english.files});
  try{assert.equal(englishEngine.suggest('book',{language:'english'}).candidates.find(hit=>hit.text==='book').tapOnly,true);}
  finally{englishEngine.dispose();}
  const bad=[{soundMapEnabled:'no'},{language:'english',word:'book',soundMapEnabled:false},{commonnessLevel:0},{commonnessLevel:6},{commonnessLevel:'2'}];
  for(const patch of bad)await assert.rejects(store.save({type:'save',word:{word,language:'khmer',...patch}},english.revision));
  assert.deepEqual((await store.snapshot()).bytes,english.bytes);await assertNoSavedHistory(root);
});


test('component editing persists analysis only, preserves all typing paths and personal choices, and reverses cleanly',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const word='កូនសិស្ស',request=components=>({type:'save',original:{word,language:'khmer'},word:{word,language:'khmer',components}});
  const row=before.catalog.khmer.find(row=>row.word===word);assert(row);
  const unchanged=await store.save(request(row.components),before.revision);
  assert.deepEqual(unchanged.bytes,before.bytes);
  const saved=await store.save(request([['កូន','សិស្ស']]),before.revision);
  assert.deepEqual(saved.source,before.source);
  for(const language of ['khmer','english'])assert.deepEqual(saved.metadata[language].words,before.metadata[language].words);
  assert.deepEqual(saved.info.provenance.sharedWords,before.info.provenance.sharedWords);
  const restarted=await dictionaryStore(root).snapshot();
  assert.deepEqual(restarted.catalog.khmer.find(row=>row.word===word).components,[['កូន','សិស្ស']]);
  const raw=await import('../../wasm/khmer_engine.js');
  const core=raw.WasmEngine.fromAppData(restarted.files.khmer,restarted.files.english);
  const originalCore=raw.WasmEngine.fromAppData(before.files.khmer,before.files.english);
  try{
    assert.deepEqual(core.exportBinary(),originalCore.exportBinary());
    const analysis=JSON.parse(core.analyzeComponents(word));
    assert.equal(analysis.source,'saved');assert.deepEqual(analysis.parts.map(part=>part.text),['កូន','សិស្ស']);
  }finally{core.free();originalCore.free();}
  const multiple=await store.save({type:'save',word:{word:'ទេសធម៌',language:'khmer',components:[['ទេស','ធម៌'],['ទេ','ស','ធម៌']]}},saved.revision);
  const standalone=await store.save(request([]),multiple.revision);
  assert.deepEqual(standalone.catalog.khmer.find(row=>row.word==='ទេសធម៌').components,[['ទេស','ធម៌'],['ទេ','ស','ធម៌']]);
  const original=await createEngine({dictionary:before.files,words:[{word,spellings:['mychildpupil']}]}),engine=await createEngine({dictionary:standalone.files,words:[{word,spellings:['mychildpupil']}]});
  try{
    assert.deepEqual(engine.analyzeComponents(word).parts,[]);assert.equal(engine.analyzeComponents(word).source,'saved');
    const inputs=['mychildpupil','kon','កូន','សិស្ស','konsers','ke','tos','angkor','niyom'];
    for(const input of inputs){
      assert.deepEqual(engine.suggest(input),original.suggest(input),input);
      assert.deepEqual(engine.suggest(input,{previousWords:['កូន']}),original.suggest(input,{previousWords:['កូន']}),input);
      assert.deepEqual(engine.convert(input),original.convert(input),input);
    }
    for(const input of ['books','lets','Book'])assert.deepEqual(engine.suggest(input,{language:'english',correction:true}),original.suggest(input,{language:'english',correction:true}),input);
    assert.deepEqual(engine.getWords(),original.getWords());
  }finally{engine.dispose();original.dispose();}
  const exported=parseImport(wordExport(standalone.catalog));
  assert.deepEqual(exported.find(row=>row.word===word&&row.language==='khmer').components,[]);
  const restored=await store.save(request(null),standalone.revision);
  assert.deepEqual(restored.source,before.source);assert.deepEqual(restored.catalog.khmer.find(row=>row.word===word).components,row.components);
  for(const value of [[[word]],[['កូន','ខា']],[['កូនសិ','ស្ស']],[['កូន','សិស្ស'],['កូន','សិស្ស']],false])
    await assert.rejects(store.save(request(value),restored.revision));
  await assert.rejects(store.save({type:'save',word:{word:'book',language:'english',components:[]}},restored.revision));
  assert.deepEqual((await store.snapshot()).bytes,restored.bytes);await assertNoSavedHistory(root);
});


test('component imports resolve parents listed before reviewed children and keep unrelated entries intact',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const imported=await store.save({type:'import',reviewed:true,words:[
    {word:'កូនកូនសិស្ស',language:'khmer',components:[['កូនកូន','សិស្ស']]},
    {word:'កូនកូន',language:'khmer',components:[['កូន','កូន']]},
  ]},before.revision);
  assert.deepEqual(imported.catalog.khmer.find(row=>row.word==='កូនកូនសិស្ស').components,[['កូនកូន','សិស្ស']]);
  assert.deepEqual(imported.source.mappings,before.source.mappings);assert.deepEqual(imported.source.structures,before.source.structures);
  const ids=new Set(before.source.words.map(row=>row.id));
  assert.deepEqual(imported.source.words.filter(row=>ids.has(row.id)),before.source.words);
  const row=imported.catalog.khmer.find(row=>row.word==='កូនសិស្ស');
  const last=await store.save({type:'import',words:[{word:row.word,language:'khmer',components:[]},{word:row.word,language:'khmer',components:row.components}]},imported.revision);
  assert.deepEqual(last.bytes,imported.bytes,'the final explicit choice wins without leaving an intermediate override');
  await assertNoSavedHistory(root);
});

test('exact input exclusions persist without changing words, aliases or unrelated typing and can be cleared',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const row=before.catalog.khmer.find(row=>row.word==='សៀវភៅ');
  const inputs=[...new Set([row.romanizations[0],row.soundMapInputs[0]].filter(Boolean))].sort();
  assert(inputs.length>0);
  const original=await createEngine({dictionary:before.files,words:[{word:row.word,spellings:inputs}]});
  try{
    for(const input of inputs)assert(original.suggest(input).candidates.some(hit=>hit.khmer===row.word));
    await assert.rejects(store.save({type:'save',word:{word:row.word,excludedInputs:['!bad']}},before.revision),/excluded inputs/);
    assert.deepEqual((await store.snapshot()).bytes,before.bytes);
    const after=await store.save({type:'save',word:{word:row.word,excludedInputs:inputs.map(input=>' '+input.toUpperCase()+' ')}},before.revision);
    assert.deepEqual(after.source,before.source);
    for(const language of ['khmer','english'])assert.deepEqual(after.metadata[language].words,before.metadata[language].words);
    assert.deepEqual(after.catalog.khmer.find(item=>item.word===row.word).excludedInputs,inputs);
    const restarted=await dictionaryStore(root).snapshot(),engine=await createEngine({dictionary:restarted.files,words:[{word:row.word,spellings:inputs}]});
    try{
      for(const input of inputs){
        assert(!engine.suggest(input).candidates.some(hit=>hit.khmer===row.word));
        assert(!engine.convert(input).candidates.some(hit=>hit.khmer===row.word));
        assert(!engine.suggest(input,{previousWords:[]}).candidates.some(hit=>hit.khmer===row.word));
      }
      for(const input of ['ke','angkor','nih','niyom','tusna','ក','សៀវភៅ'])assert.deepEqual(engine.suggest(input,{previousWords:[]}),original.suggest(input,{previousWords:[]}),input);
      for(const input of ['book','books','lets','children','wrote'])assert.deepEqual(engine.suggest(input,{language:'english',previousWords:[]}),original.suggest(input,{language:'english',previousWords:[]}),input);
      const exported=parseImport(wordExport(restarted.catalog));
      assert.deepEqual(exported.find(item=>item.word===row.word).excludedInputs,inputs);
      assert.equal((await store.save({type:'import',words:exported,reviewed:true},after.revision)).revision,after.revision);
      const cleared=await store.save({type:'save',word:{word:row.word,excludedInputs:[]}},after.revision);
      assert.deepEqual(cleared.source,before.source);
      const restored=await createEngine({dictionary:cleared.files,words:[{word:row.word,spellings:inputs}]});
      try{for(const input of inputs)assert.deepEqual(restored.suggest(input),original.suggest(input));}
      finally{restored.dispose();}
    }finally{engine.dispose();}
    await assertNoSavedHistory(root);
  }finally{original.dispose();}
});

test('shorthand edits persist, keep aliases and words, export and restore the built-in nh reading',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  assert.deepEqual(before.catalog.khmer.find(row=>row.word==='ខ្ញុំ').shorthands,['nh']);
  const original=await createEngine({dictionary:before.files});
  try{
    const after=await store.save({type:'import',words:[{word:'ខ្ញុំ',shorthands:[]},{word:'សៀវភៅ',shorthands:[' SB ']}]},before.revision);
    assert.deepEqual(after.source,before.source);
    for(const lang of ['khmer','english'])assert.deepEqual(after.metadata[lang].words,before.metadata[lang].words);
    assert.deepEqual(after.catalog.khmer.find(row=>row.word==='ខ្ញុំ').shorthands,[]);
    assert.deepEqual(after.catalog.khmer.find(row=>row.word==='សៀវភៅ').shorthands,['sb']);
    const restarted=await dictionaryStore(root).snapshot(),core=await createEngine({dictionary:restarted.files});
    try{
      assert(!core.suggest('nh').candidates.some(hit=>hit.khmer==='ខ្ញុំ'&&hit.completion===undefined));
      assert(core.suggest('sb').candidates.some(hit=>hit.khmer==='សៀវភៅ'&&!hit.tapOnly));
      assert(!core.convert('sb').candidates.some(hit=>hit.khmer==='សៀវភៅ'));
      for(const input of ['khnhom','khnyom','sievphov','ke','kon'])assert.deepEqual(core.suggest(input),original.suggest(input),input);
      const exported=parseImport(wordExport(restarted.catalog));assert.deepEqual(exported.find(row=>row.word==='សៀវភៅ').shorthands,['sb']);
      assert.equal((await store.save({type:'import',words:exported,reviewed:true},after.revision)).revision,after.revision);
      const restored=await store.save({type:'import',words:[{word:'ខ្ញុំ',shorthands:['nh']},{word:'សៀវភៅ',shorthands:[]}]},after.revision);
      assert.deepEqual(restored.source,before.source);
      const final=await createEngine({dictionary:restored.files});
      try{assert.deepEqual(final.suggest('nh'),original.suggest('nh'));}finally{final.dispose();}
    }finally{core.dispose();}
    await assertNoSavedHistory(root);
  }finally{original.dispose();}
});

test('signed rank saves survive restart and export without losing any word or alias',async t=>{
  const root=await fixture(t),store=dictionaryStore(root),before=await store.snapshot();
  const word='ជិត',old=before.source.words.find(row=>row.khmer===word);assert(old);
  const after=await store.save({type:'save',word:{word,rank:-1}},before.revision);
  assert.deepEqual(after.source.words,before.source.words.map(row=>row.khmer===word?{...row,rank:-1}:row));
  assert.deepEqual(after.source.englishWords,before.source.englishWords);
  assert.deepEqual(after.source.mappings,before.source.mappings.map(row=>row.khmer===word?{...row,rank:-1}:row));
  for(const language of ['khmer','english'])assert.deepEqual(after.metadata[language].words,before.metadata[language].words);
  const restarted=await dictionaryStore(root).snapshot();assert.equal(restarted.revision,after.revision);
  assert.equal(restarted.catalog.khmer.find(row=>row.word===word).rank,-1);
  const exported=parseImport(wordExport(restarted.catalog));assert.equal(exported.find(row=>row.word===word).rank,-1);
  assert.equal((await store.save({type:'import',words:exported,reviewed:true},restarted.revision)).revision,restarted.revision);
  const core=await createEngine({dictionary:restarted.files});
  try{
    for(const input of ['chet','chit','jet','jit'])assert.equal(core.suggest(input,{previousWords:[]}).candidates.find(hit=>hit.khmer===word)?.rank,-1,input);
  }finally{core.dispose();}
  const restored=await store.save({type:'save',word:{word,rank:old.rank}},restarted.revision);
  assert.deepEqual(restored.source,before.source);
  const original=await createEngine({dictionary:before.files}),reverted=await createEngine({dictionary:restored.files});
  try{for(const input of ['chet','chit','jet','jit','ke','nh','tt','kon'])assert.deepEqual(reverted.suggest(input),original.suggest(input),input);}
  finally{original.dispose();reverted.dispose();}
  await assert.rejects(store.save({type:'save',word:{word,rank:-1000001}},restored.revision),/Rank must/);
  assert.equal((await store.snapshot()).revision,restored.revision);
  await assertNoSavedHistory(root);
});
