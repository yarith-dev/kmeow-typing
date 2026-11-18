import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {WasmEngine,initSync} from '../wasm/khmer_engine.js';
import {createEngine} from '../dist/public/node.js';
import {readCompactSource} from '../scripts/read-compact-source.mjs';
import {ruleFamilies} from '../manager/sound-rules.js';
const read=file=>readFileSync(new URL('../'+file,import.meta.url));
initSync({module:read('wasm/khmer_engine_bg.wasm')});
const proof=JSON.parse(read('test/fixtures/shared-two-dot.json'));
const map=JSON.parse(read('data/khmer-sound-map.json'));
const exact=(core,input,word)=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.find(row=>row.khmer===word);

test('shared ៈ covers regular syllables and keeps every earlier marked-word input and saved alias',()=>{
  const core=WasmEngine.fromAppData(read('compiled-data/khmer.kmd'),read('compiled-data/english.kmd'));
  try{
    const bytes=core.exportBinary(),source=readCompactSource(core);
    for(const row of proof.previous)for(const input of row.inputs??[])assert(exact(core,input,row.word),`${input} → ${row.word}`);
    for(const [input,word] of proof.examples)assert(exact(core,input,word),`${input} → ${word}`);
    for(const row of proof.previous)assert(JSON.parse(core.soundMapInputs(row.word,'')).inputs.length,row.word);
    for(const mapping of source.mappings)assert(JSON.parse(core.convert(mapping.romanized)).candidates.some(row=>row.khmer===mapping.khmer));
    assert.deepEqual(core.exportBinary(),bytes);
    assert.deepEqual(readCompactSource(core),source);
  }finally{core.free();}
});

test('irregular syllables stay editable as Read as and reset without changing dictionary bytes',async()=>{
  const sdk=await createEngine();
  const core=WasmEngine.fromAppData(read('compiled-data/khmer.kmd'),read('compiled-data/english.kmd'));
  try{
    const bytes=core.exportBinary();
    const saved=JSON.parse(core.appDataInfo()).provenance.pronunciations.rules;
    for(const rule of proof.irregular){
      assert.deepEqual(map.pronunciationSyllables.find(row=>row.word===rule.word),rule);
      const inputs=sdk.previewPronunciation(rule.readAs);
      for(const input of inputs)assert(exact(core,input,rule.word),`${input} → ${rule.word}`);
      core.setPronunciations(JSON.stringify([...saved.filter(row=>row.word!==rule.word),{word:rule.word,readAs:rule.readAs}]));
      for(const input of inputs)assert(exact(core,input,rule.word));
      core.setPronunciations(JSON.stringify([...saved.filter(row=>row.word!==rule.word),{word:rule.word,readAs:'មិត'}]));
      assert(exact(core,'met',rule.word));
      core.setPronunciations(JSON.stringify(saved));
      for(const input of inputs)assert(exact(core,input,rule.word));
    }
    assert.deepEqual(sdk.previewPronunciation('គៈណៈ'),['keaknak','keanak']);
    assert.deepEqual(core.exportBinary(),bytes);
  }finally{sdk.dispose();core.free();}
});

test('shared mark paths preserve alias priority, exclusions and disabled words',()=>{
  const core=new WasmEngine(JSON.stringify({schemaVersion:3,englishWords:[],
    words:['កូន','ស្រៈ','រយៈ'].map((khmer,i)=>({id:100+i,khmer,rank:i,status:'approved'})),
    mappings:[{id:200,romanized:'srak',khmer:'កូន',rank:0,status:'approved'}],soundMapDisabledWords:['រយៈ']}));
  try{
    assert.equal(JSON.parse(core.suggestWithContext('srak','{}','{}')).exactCandidates[0].khmer,'កូន');
    assert(exact(core,'srak','ស្រៈ'));
    assert(!exact(core,'royeak','រយៈ'));
    core.setInputExclusions(JSON.stringify([{word:'ស្រៈ',inputs:['srak']}]));
    assert(!exact(core,'srak','ស្រៈ'));assert(exact(core,'srak','កូន'));
  }finally{core.free();}
});

test('Read as accepts independent vowel boundaries, retains closures and rejects obsolete vowels atomically',async()=>{
  const sdk=await createEngine();
  const core=new WasmEngine(JSON.stringify({schemaVersion:3,englishWords:[],
    words:['មិត្ត','ភាព','មិត្តភាព'].map((khmer,i)=>({id:100+i,khmer,rank:i,status:'approved'})),
    mappings:[{id:200,romanized:'friend',khmer:'មិត្ត',rank:0,status:'approved'}]}));
  try{
    const bytes=core.exportBinary(),saved=core.convert('friend');
    assert.deepEqual(sdk.previewPronunciation('គៈឥ'),['keakei','keakey']);
    assert.deepEqual(sdk.previewPronunciation('កៈឯ'),['kakae']);
    assert.deepEqual(sdk.previewPronunciation('ស្រៈឥ'),['srakei','srakey']);
    core.setPronunciations(JSON.stringify([{word:'មិត្ត',readAs:'គៈឥ'}]));
    assert(exact(core,'keakei','មិត្ត'));assert(exact(core,'keakeipeap','មិត្តភាព'));
    assert(!exact(core,'keae','មិត្ត'));
    assert.throws(()=>core.setPronunciations(JSON.stringify([{word:'មិត្ត',readAs:'គៈឨ'}])));
    assert(exact(core,'keakei','មិត្ត'));
    assert.equal(core.convert('friend'),saved);
    core.setPronunciations('[]');
    assert.deepEqual(core.exportBinary(),bytes);
  }finally{sdk.dispose();core.free();}
});

test('SDK edits built-in Read as parts in a small dictionary and refreshes personal compound readings',async()=>{
  const dictionary={schemaVersion:3,englishWords:[],
    words:['កម្មករ','កុម្ភៈកូន','កូន'].map((khmer,i)=>({id:100+i,khmer,rank:i,status:'approved'})),
    mappings:[{id:200,romanized:'savedcompound',khmer:'កម្មករ',rank:0,status:'approved'}]};
  const sdk=await createEngine({dictionary,words:[{word:'កម្មកូន'}]});
  const hit=(input,word)=>sdk.suggest(input,{previousWords:[]}).exactCandidates.some(row=>row.khmer===word);
  const queries=['kamkor','kamkon','keakeikor','keakeikon','metkon','savedcompound'];
  try{
    const words=sdk.getWords(),before=queries.map(input=>sdk.suggest(input,{previousWords:[]}));
    const alias=sdk.convert('savedcompound');
    assert(hit('kamkor','កម្មករ'));assert(hit('kamkon','កម្មកូន'));
    sdk.setPronunciations([{word:'កម្ម',readAs:'គៈឥ'},{word:'កុម្ភៈ',readAs:'មិត'}]);
    assert(hit('keakeikor','កម្មករ'));assert(hit('keakeikon','កម្មកូន'));
    assert(!hit('keakei','កម្ម'));assert(!hit('metkon','កុម្ភៈកូន'));
    const edited=sdk.suggest('keakeikor',{previousWords:[]});
    assert.throws(()=>sdk.setPronunciations([{word:'មិនស្គាល់',readAs:'មិត'}]));
    assert.deepEqual(sdk.suggest('keakeikor',{previousWords:[]}),edited);
    assert.deepEqual(sdk.convert('savedcompound'),alias);assert.deepEqual(sdk.getWords(),words);
    sdk.setPronunciations([]);
    for(const [i,input]of queries.entries())assert.deepEqual(sdk.suggest(input,{previousWords:[]}),before[i]);
  }finally{sdk.dispose();}
});

test('the shared mark rule appears with examples in the existing collapsible mark family',()=>{
  const family=ruleFamilies(map.runtime).find(row=>row.id==='silence');
  const rule=family.rules.find(row=>row.id==='shortInherentSign');
  assert.match(rule.description,/keaknak/);assert.match(rule.description,/Read[ -]as/i);
});
