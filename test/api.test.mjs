import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import test from 'node:test';
import { createEngine, EngineError } from '@khmer-typing/engine';
import { WasmEngine } from '../wasm/khmer_engine.js';

const files = Object.fromEntries(['khmer', 'english'].map(language => [language,
  readFileSync(new URL(`../compiled-data/${language}.kmd`, import.meta.url))]));
const inputs = ['', 's', 'sa', 'sievphov', 'siev phov', 'angkor', 'niyom', 'niyum',
  'tropsambat', 'kaeknitsah', "l'ar", 'sabbay', 'ស', 'ក្ត', 'សៀវ', 'Hello',
  'bo', 'books', 'im', 'teh', 'recieve', 'lets', 'កង់', 'test ', ' SIEV   PHOV ', 'hello.world'];

test('unrelated custom ranks retain contextual exact and prefix ordering across dictionary layers',async()=>{
  const initialized=await createEngine();initialized.dispose();
  for(const saved of [false,true]){
    const words=['កា','គា','ខា','កាខា','ឃា'];
    const base=new WasmEngine(JSON.stringify({schemaVersion:2,
      words:words.map((khmer,i)=>({id:i+1,khmer,rank:i,status:'approved'})),
      mappings:[{id:900,romanized:'same',khmer:'គា',rank:0,status:'approved'}],
      structures:saved?[{wordId:4,componentIds:[1,3]}]:[]}));
    const local=WasmEngine.fromPersonalWords(JSON.stringify([
      {word:'ខា',spellings:['same','my-part']},{word:'ឃា',spellings:['other-word']}]));
    const rankedBase=base.withTypingOverrides(JSON.stringify([{language:'khmer',word:'កា',rank:123}]));
    const rankedLocal=local.withTypingOverrides(JSON.stringify([{language:'khmer',word:'ឃា',rank:456}]));
    const bytes=[Buffer.from(base.exportBinary()),Buffer.from(local.exportBinary())];
    try{
      for(const input of ['same','sa','my-part'])for(const previousWords of [[],['កា']]){
        const context=JSON.stringify({previousWords});
        const expected=JSON.parse(base.suggestWithExtension(local,input,'{}',context));
        for(const [core,extra]of [[rankedBase,local],[base,rankedLocal],[rankedBase,rankedLocal]]){
          assert.deepEqual(JSON.parse(core.suggestWithExtension(extra,input,'{}',context)),expected,`${saved}: ${input}`);
        }
        if(previousWords.length){assert.equal(expected.candidates[0].khmer,'ខា');
          if(input==='sa')assert.equal(expected.exactCandidates.length,0);
          else assert.equal(expected.exactCandidates[0].khmer,'ខា');}
      }
      assert.deepEqual(Buffer.from(rankedBase.exportBinary()),bytes[0]);
      assert.deepEqual(Buffer.from(rankedLocal.exportBinary()),bytes[1]);
    }finally{rankedBase.free();rankedLocal.free();local.free();base.free();}
  }
});

test('clear components rank existing choices while saved spellings and personal state stay intact',async()=>{
  const words=['សួរ','សុខ','សប្បាយ','សុខសប្បាយ'];
  const dictionary={schemaVersion:2,words:words.map((khmer,i)=>({id:i+1,khmer,rank:i,status:'approved'})),
    mappings:['សួរ','សប្បាយ'].map((khmer,i)=>({id:900+i,romanized:'same',khmer,rank:i,status:'approved'}))};
  const engine=await createEngine({dictionary});
  try{
    const plain=engine.suggest('same',{previousWords:[]});
    assert.equal(plain.exactCandidates[0].khmer,'សួរ');
    for(let i=0;i<3;i++){
      const ranked=engine.suggest('same',{previousWords:['សុខ']});
      assert.equal(ranked.exactCandidates[0].khmer,'សប្បាយ');
      assert.deepEqual(new Set(ranked.exactCandidates.map(c=>c.khmer)),new Set(plain.exactCandidates.map(c=>c.khmer)));
      assert(ranked.candidates.filter(c=>ranked.exactCandidates.some(exact=>exact.khmer===c.khmer)).every(c=>!c.completion&&!c.correction));
    }
    assert.deepEqual(engine.suggest('same',{previousWords:[]}),plain);
    engine.addWords([{word:'សប្បាយ',spellings:['my-happy','llerrttthhi']}]);
    const saved=engine.getWords();
    for(const soundMap of [true,false])for(const input of saved[0].spellings){
      const result=engine.suggest(input,{soundMap,previousWords:['សុខ']});
      assert(result.exactCandidates.some(c=>c.khmer==='សប្បាយ'));assert(!result.candidates[0].preferredInput);
    }
    assert.deepEqual(engine.getWords(),saved);
    engine.clearWords();assert.deepEqual(engine.getWords(),[]);
    assert.equal(engine.suggest('same',{previousWords:['សុខ']}).exactCandidates[0].khmer,'សប្បាយ');
  }finally{engine.dispose();}
});

test('a personal component can make the cached base split ambiguous without changing base inputs',async()=>{
  const words=['សួរ','កា','កាខា','ខាគា','កាខាគា'];
  const dictionary={schemaVersion:2,words:words.map((khmer,i)=>({id:i+1,khmer,rank:i,status:'approved'})),
    mappings:['សួរ','ខាគា'].map((khmer,i)=>({id:900+i,romanized:'same',khmer,rank:i,status:'approved'}))};
  const engine=await createEngine({dictionary});
  try{
    const options={previousWords:['កា']};
    assert.equal(engine.suggest('same',options).exactCandidates[0].khmer,'ខាគា');
    engine.addWords([{word:'គា',spellings:['my-component']}]);
    assert.equal(engine.analyzeComponents('កាខាគា').status,'ambiguous');
    assert.equal(engine.suggest('same',options).exactCandidates[0].khmer,'សួរ');
    assert(engine.suggest('my-component',options).exactCandidates.some(c=>c.khmer==='គា'));
    engine.clearWords();
    assert.equal(engine.suggest('same',options).exactCandidates[0].khmer,'ខាគា');
  }finally{engine.dispose();}
});

test('the default API loads its local dictionaries independently of the working directory', async () => {
  const original = process.cwd();
  process.chdir(tmpdir());
  let engine;
  try {
    engine = await createEngine();
    assert.equal(engine.suggest('sievphov').candidates[0].khmer, 'សៀវភៅ');
    assert.deepEqual(engine.getWords(), []);
  } finally { engine?.dispose(); process.chdir(original); }
});

test('settings, context and results preserve the underlying core behavior', async () => {
  const engine = await createEngine();
  const core = WasmEngine.fromAppData(files.khmer, files.english);
  try {
    for (const input of inputs) assert.deepEqual(engine.convert(input), JSON.parse(core.convert(input)));
    for (const language of ['khmer', 'english']) for (const conversion of [true, false])
    for (const soundMap of [true, false]) for (const completion of [true, false])
    for (const correction of [true, false]) {
      engine.configure({ language, conversion, soundMap, completion, correction });
      const features = JSON.stringify({ mode: language, romanized: conversion, soundMap,
        khmerCompletion: completion, englishCompletion: completion, englishAutocorrection: correction });
      for (const input of inputs) assert.deepEqual(engine.suggest(input), JSON.parse(core.suggestWithFeatures(input, features)));
      const context = { previousWords: [], englishWordContext: { before: 'She ', after: ' us go' } };
      assert.deepEqual(engine.suggest('lets', { before: 'She ', after: ' us go' }),
        JSON.parse(core.suggestWithContext('lets', features, JSON.stringify(context))));
    }
  } finally { engine.dispose(); core.free(); }
});

test('personal additions are atomic, reversible and leave bundled files unchanged', async () => {
  const engine = await createEngine();
  const before = engine.suggest('mybook');
  try {
    engine.addWords([{word: 'សៀវភៅ', spellings: ['mybook']}, {word: 'kmeow', language: 'english'}]);
    assert.equal(engine.convert('mybook').candidates[0].khmer, 'សៀវភៅ');
    assert.equal(engine.suggest('kme', {language: 'english'}).candidates[0].text, 'kmeow');
    const saved = engine.getWords();
    assert.throws(() => engine.addWords([{word: 'good', language: 'english'}, {word: 'bad', spellings: ['!']}]), EngineError);
    assert.deepEqual(engine.getWords(), saved);
    engine.clearWords();
    assert.deepEqual(engine.suggest('mybook'), before);
    assert.throws(() => engine.configure({completion: 'yes'}), EngineError);
    assert.throws(() => engine.configure({unknown: true}), EngineError);
  } finally { engine.dispose(); }
});

test('partial component analysis reports ambiguity and preserves typing and personal spellings', async () => {
  const words=['កា','កាខា','ខាគា','គា','កាខាគា'];
  const dataset={schemaVersion:2,mappings:[{id:900,romanized:'myword',khmer:'កាខាគា',rank:77,status:'approved'}],
    words:words.map((khmer,i)=>({id:i+1,khmer,rank:i,status:'approved'}))};
  const engine=await createEngine({dictionary:dataset});
  const saved=await createEngine({dictionary:{...dataset,structures:[{wordId:5,componentIds:[1,3]}]}});
  try{
    const before=inputs.map(input=>engine.suggest(input));
    const ambiguous=engine.analyzeComponents('កាខាគា');
    assert.equal(ambiguous.status,'ambiguous');assert.deepEqual(ambiguous.parts,[]);
    assert.equal(ambiguous.alternatives.length,2);
    const resolved=saved.analyzeComponents('កាខាគា');
    assert.equal(resolved.source,'saved');assert.deepEqual(resolved.parts.map(part=>part.text),['កា','ខាគា']);
    const partial=engine.analyzeComponents('ឃាកា');
    assert.equal(partial.status,'partial');assert.deepEqual(partial.parts.map(part=>part.text),['ឃា','កា']);
    assert.equal(partial.parts[0].wordId,undefined);assert.equal(partial.parts[1].wordId,1);
    assert.deepEqual(inputs.map(input=>engine.suggest(input)),before);
    assert.equal(engine.convert('myword').candidates[0].khmer,'កាខាគា');
    engine.addWords([{word:'ឃា',spellings:['myfragment']},{word:'កាខាគា',spellings:['ownword']}]);
    const personal=engine.getWords();
    assert.equal(engine.analyzeComponents('ឃាកា').status,'complete');
    assert.equal(engine.convert('ownword').candidates[0].khmer,'កាខាគា');
    assert.deepEqual(engine.getWords(),personal);
    engine.clearWords();assert.equal(engine.analyzeComponents('ឃាកា').status,'partial');
    assert.throws(()=>engine.analyzeComponents('books'),EngineError);
    assert.throws(()=>engine.analyzeComponents('កា ខា'),EngineError);
    engine.analyzeComponents('ឃាកា').parts[0].text='changed';
    assert.equal(engine.analyzeComponents('ឃាកា').parts[0].text,'ឃា');
  }finally{engine.dispose();saved.dispose();}
  assert.throws(()=>engine.analyzeComponents('កា'),EngineError);
});

test('Wasm retains saved boundaries while excluded parts stay unresolved through personal overlays', async()=>{
  const initialized=await createEngine();
  const raw=new WasmEngine(JSON.stringify({schemaVersion:2,mappings:[],
    words:['កា','ខា','កាខា'].map((khmer,i)=>({id:i+1,khmer,rank:i,status:'approved'})),
    structures:[{wordId:3,componentIds:[1,2]}]}));
  const hidden=raw.withTypingOverrides(JSON.stringify([{language:'khmer',word:'កា',excluded:true}]));
  const personal=WasmEngine.fromPersonalWords(JSON.stringify([{word:'កា',spellings:['mypart']}]));
  try{
    const before=hidden.exportBinary();
    for(const result of [JSON.parse(hidden.analyzeComponents('កាខា')),JSON.parse(hidden.analyzeComponentsWithExtension(personal,'កាខា'))]){
      assert.equal(result.source,'saved');assert.equal(result.status,'partial');
      assert.deepEqual(result.parts.map(part=>part.text),['កា','ខា']);
      assert.equal(result.parts[0].wordId,undefined);assert.equal(result.parts[1].wordId,2);
    }
    hidden.clearComponentExtensionCache();
    assert.equal(JSON.parse(hidden.analyzeComponentsWithExtension(personal,'កាខា')).status,'partial');
    assert.deepEqual(hidden.exportBinary(),before);
  }finally{personal.free();hidden.free();raw.free();initialized.dispose();}
});

test('equal component coverage prefers fewer parts without changing typing or saved definitions',async()=>{
  const engine=await createEngine();
  const words=['កា','ខា','គា','កាខា','ខាគា','កាខាគា'];
  const dictionary={schemaVersion:2,mappings:[],words:words.map((khmer,i)=>({id:i+1,khmer,rank:i,status:'approved'}))};
  const tied=await createEngine({dictionary});
  const saved=await createEngine({dictionary:{...dictionary,structures:[{wordId:6,componentIds:[1,2,3]}]}});
  try{
    const inputs=['tehthoa','testhoa','ke','sievphov','books'];
    const before=inputs.map(input=>engine.suggest(input));
    const result=engine.analyzeComponents('ទេសធម៌');
    assert.equal(result.source,'automatic');assert.equal(result.status,'complete');
    assert.deepEqual(result.parts.map(part=>part.text),['ទេស','ធម៌']);
    assert.deepEqual(result.alternatives,[]);assert.equal(result.moreAlternatives,false);
    assert.equal(tied.analyzeComponents('កាខាគា').status,'ambiguous');
    assert.equal(tied.analyzeComponents('កាខាគា').alternatives.length,2);
    const chosen=saved.analyzeComponents('កាខាគា');
    assert.equal(chosen.source,'saved');assert.deepEqual(chosen.parts.map(part=>part.text),['កា','ខា','គា']);
    engine.addWords([{word:'ទេសធម៌',spellings:['myteh']}]);
    const personal=engine.getWords();
    assert.deepEqual(engine.analyzeComponents('ទេសធម៌').parts.map(part=>part.text),['ទេស','ធម៌']);
    assert.equal(engine.convert('myteh').candidates[0].khmer,'ទេសធម៌');
    assert.deepEqual(engine.getWords(),personal);
    engine.clearWords();assert.deepEqual(inputs.map(input=>engine.suggest(input)),before);
  }finally{engine.dispose();tied.dispose();saved.dispose();}
});

test('shared word-part overrides preserve explicit personal spellings', async () => {
  const engine = await createEngine();
  const core = WasmEngine.fromAppData(files.khmer, files.english);
  try {
    assert(!JSON.parse(core.suggestWithContext('sastro', '{}', '{}')).exactCandidates
      .some(candidate => candidate.khmer === 'សាស្ត្រ'));
    const before = engine.convert('sastro');
    engine.addWords([
      {word: 'សាស្ត្រ', spellings: ['sastro', 'myscience']},
      {word: 'កាសាស្ត្រ', spellings: ['mycompound']},
    ]);
    const personal = engine.getWords();
    for (const soundMap of [true, false]) {
      for (const [input, word] of [['sastro', 'សាស្ត្រ'], ['myscience', 'សាស្ត្រ'], ['mycompound', 'កាសាស្ត្រ']]) {
        assert.equal(engine.convert(input).candidates[0].khmer, word);
        assert(engine.suggest(input, {soundMap}).candidates.some(candidate => candidate.khmer === word), input);
      }
    }
    assert.deepEqual(engine.getWords(), personal);
    engine.clearWords();
    assert.deepEqual(engine.convert('sastro'), before);
  } finally { engine.dispose(); core.free(); }
});

test('syllable-final ស accepts h and s inside compounds and preserves onset and saved spellings', async () => {
  const initialized = await createEngine();
  const proof = JSON.parse(readFileSync(new URL('./fixtures/syllable-final-so.json', import.meta.url)));
  const rows = [...proof.cases, ...proof.onsets];
  const words = [...rows.map(row=>row.word), ...proof.parts];
  const core = new WasmEngine(JSON.stringify({schemaVersion:2,
    words:words.map((khmer,i)=>({id:98000+i,khmer,rank:i,status:'approved'})),
    mappings:[{id:99000,romanized:'mycoda',khmer:'កាសកា',rank:77,status:'approved'}],
  }));
  try {
    const before = core.exportBinary();
    for (const [i,row] of rows.entries()) {
      for (const input of row.inputs) {
        const hit = JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.find(candidate=>candidate.khmer===row.word);
        assert(hit,`${input}: ${row.word}`);
        assert.equal(hit.mappingId,98000+i);
        assert.equal(hit.rank,i);
      }
      for (const input of row.rejected??[]) {
        assert(!JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(candidate=>candidate.khmer===row.word),`${input}: ${row.word}`);
      }
    }
    for (const soundMap of [true,false]) {
      const hit = JSON.parse(core.suggestWithContext('mycoda',JSON.stringify({soundMap}),'{}')).exactCandidates.find(candidate=>candidate.mappingId===99000);
      assert.equal(hit?.khmer,'កាសកា');
      assert.equal(hit?.rank,77);
    }
    assert.deepEqual(core.exportBinary(),before);
    initialized.addWords([{word:'សុខសប្បាយ',spellings:['mygreeting']}]);
    const partsOnly = WasmEngine.fromPersonalWords(JSON.stringify([{word:'សុខសប្បាយ',spellings:['mygreeting']}]));
    try {
      for (const input of ['sokhohbbay','sukhohbbay']) {
        assert(!JSON.parse(core.suggestWithExtension(partsOnly,input,'{}','{}')).exactCandidates.some(candidate=>candidate.khmer==='សុខសប្បាយ'),`personal duplicate ${input}`);
        assert(!initialized.suggest(input).candidates.some(candidate=>candidate.khmer==='សុខសប្បាយ'&&!candidate.correction&&!candidate.completion),`public duplicate ${input}`);
      }
    } finally { partsOnly.free(); }
    initialized.addWords([{word:'សុខសប្បាយ',spellings:['sokhohbbay','sukhohbbay','mygreeting']}]);
    const saved = initialized.getWords();
    for (const soundMap of [true,false]) for (const input of ['sokhohbbay','sukhohbbay','mygreeting']) {
      assert(initialized.suggest(input,{soundMap}).candidates.some(candidate=>candidate.khmer==='សុខសប្បាយ'),`saved ${input}`);
    }
    assert.deepEqual(initialized.getWords(),saved);
  } finally { core.free(); initialized.dispose(); }
});

test('browser initialization loads only its local Wasm and dictionary assets', async () => {
  const {createEngine: browserEngine} = await import('../dist/public/browser.js');
  const originalFetch = globalThis.fetch, requested = [];
  globalThis.fetch = async input => {
    const url = new URL(input.url ?? input);
    assert.equal(url.protocol, 'file:');
    requested.push(url.pathname);
    return new Response(readFileSync(url), {headers: {'content-type': url.pathname.endsWith('.wasm') ? 'application/wasm' : 'application/octet-stream'}});
  };
  let browser, node;
  try {
    browser = await browserEngine(); node = await createEngine();
    assert.equal(requested.filter(path => path.endsWith('.kmd')).length, 2);
    for (const input of inputs) assert.deepEqual(browser.suggest(input), node.suggest(input));
  } finally { browser?.dispose(); node?.dispose(); globalThis.fetch = originalFetch; }
});
