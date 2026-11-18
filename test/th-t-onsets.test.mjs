import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {WasmEngine,initSync} from '../wasm/khmer_engine.js';
import {createEngine} from '../dist/public/node.js';
const read=file=>readFileSync(new URL('../'+file,import.meta.url));
initSync({module:read('wasm/khmer_engine_bg.wasm')});
const proof=JSON.parse(read('test/fixtures/th-t-onsets.json'));

test('t onset alternatives preserve vowel series, finals, older exact choices and aliases',()=>{
  const rows=[...proof.cases,...proof.controls];
  const core=new WasmEngine(JSON.stringify({schemaVersion:3,englishWords:[],
    words:rows.map((row,i)=>({id:100+i,khmer:row.word,rank:i,status:'approved'})),
    mappings:[{id:900,romanized:'tver',khmer:'កូន',rank:100,status:'approved'}]}));
  const exact=input=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates;
  try{
    const bytes=core.exportBinary(),alias=core.convert('tver');
    for(const row of proof.cases){
      assert.deepEqual(JSON.parse(core.soundMapInputs(row.word,'')).inputs,[...row.previous,...row.additional].sort(),row.word);
      for(const input of [...row.previous,...row.additional])assert(exact(input).some(candidate=>candidate.khmer===row.word),`${input} → ${row.word}`);
    }
    for(const row of proof.controls)assert.deepEqual(JSON.parse(core.soundMapInputs(row.word,'')).inputs,row.inputs,row.word);
    for(const [input,word]of proof.collisions)assert.equal(exact(input)[0].khmer,word,input);
    assert(!JSON.parse(core.suggestWithContext('tver','{"soundMap":false}','{}')).exactCandidates.some(row=>row.khmer==='ធ្វើ'));
    assert.deepEqual(core.exportBinary(),bytes);assert.equal(core.convert('tver'),alias);
  }finally{core.free();}
});

test('older ou predictions retain precedence when followed by a new onset alternative',()=>{
  const core=new WasmEngine(JSON.stringify({schemaVersion:3,mappings:[],englishWords:[],words:[{id:100,khmer:'ប្អូនថ្លៃ',rank:0,status:'approved'}]}));
  try{
    for(const input of ["b'ount","p'ount"])
      assert.deepEqual(JSON.parse(core.suggestWithContext(input,'{}','{}')).nextLetterWeights,{h:1000});
    for(const input of ["b'ountl","p'ountl"])
      assert.deepEqual(JSON.parse(core.suggestWithContext(input,'{}','{}')).nextLetterWeights,{a:1000});
  }finally{core.free();}
});

test('large Read as alternatives survive dictionary packaging, pagination and component reuse',()=>{
  const core=new WasmEngine(JSON.stringify({schemaVersion:3,mappings:[],englishWords:[],words:['កម្ម','ករ','កម្មករ'].map((khmer,i)=>({id:100+i,khmer,rank:i,status:'approved'}))}));
  const rules=[{word:'កម្ម',readAs:['ប្រធានាធិបតី','ធូ']}];
  let loaded;
  const exact=(engine,input,word)=>JSON.parse(engine.suggestWithContext(input,'{}','{}')).exactCandidates.some(row=>row.khmer===word);
  try{
    core.setPronunciations(JSON.stringify(rules));
    let after='',inputs=[];
    do{const page=JSON.parse(core.soundMapInputs('កម្ម',after));inputs.push(...page.inputs);after=page.nextAfter??'';}while(after);
    assert.equal(inputs.length,260);
    assert.deepEqual(inputs,[...new Set(inputs)].sort());
    const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
    const compact={...JSON.parse(core.stats()),artifactHash:hash(core.exportBinary()),datasetHash:'b'.repeat(64)};
    const metadata=language=>JSON.stringify({format:'kmeow-word-metadata-v2',language,snapshotId:'a'.repeat(64),translations:[],
      words:language==='khmer'?['កម្ម','ករ','កម្មករ'].map(word=>({id:word,word,context:null,partsOfSpeech:['noun'],commonnessLevel:1,meanings:[],translations:[],sources:[]})):[]});
    const km=metadata('khmer'),en=metadata('english');
    const file=(language,text)=>({file:language+'-words.json',bytes:Buffer.byteLength(text),sha256:hash(text),words:JSON.parse(text).words.length});
    const provenance=JSON.stringify({compactManifest:compact,metadataManifest:{format:'kmeow-word-metadata-bundle-v1',snapshotId:'a'.repeat(64),
      compactArtifactHash:hash(core.exportBinary()),files:{khmer:file('khmer',km),english:file('english',en)}},
      pronunciations:{format:'kmeow-pronunciations-v1',rules},notices:[{file:'test.txt',text:'Isolated fixture'}]});
    loaded=WasmEngine.fromAppData(core.compileAppData(km,en,provenance,'khmer'),core.compileAppData(km,en,provenance,'english'));
    for(const input of inputs){assert(exact(loaded,input,'កម្ម'),input);assert(exact(loaded,input+'kor','កម្មករ'),input+'kor');}
    assert(JSON.parse(loaded.browseWords('{"query":"t","limit":100}')).words.some(row=>row.khmer==='កម្ម'));
    core.setPronunciations(JSON.stringify([{word:'កម្ម',readAs:'ថា'.repeat(40)}]));
    for(const input of ['tha'.repeat(40),'ta'.repeat(40),'tha'.repeat(20)+'ta'.repeat(20)])assert(exact(core,input,'កម្ម'));
    const before=core.suggestWithContext('tha','{}','{}');
    assert.throws(()=>core.setPronunciations(JSON.stringify([{word:'កម្ម',readAs:'មិត'.repeat(12)}])));
    assert.equal(core.suggestWithContext('tha','{}','{}'),before);
  }finally{loaded?.free();core.free();}
});

test('Read as uses the t alternatives in component words and resets reversibly',async()=>{
  const sdk=await createEngine({dictionary:{schemaVersion:3,englishWords:[],mappings:[],
    words:['មិត្ត','ភាព','មិត្តភាព'].map((khmer,i)=>({id:100+i,khmer,rank:i,status:'approved'}))}});
  const hit=(input,word)=>sdk.suggest(input,{previousWords:[]}).exactCandidates.some(row=>row.khmer===word);
  try{
    const words=sdk.getWords(),before=sdk.suggest('met',{previousWords:[]});
    for(const [readAs,previous,added]of [['ថា','tha','ta'],['ធា','thea','tea']]){
      assert(sdk.previewPronunciation(readAs).includes(added));
      sdk.setPronunciations([{word:'មិត្ត',readAs}]);
      for(const input of [previous,added]){assert(hit(input,'មិត្ត'));assert(hit(input+'peap','មិត្តភាព'));}
    }
    sdk.setPronunciations([]);
    assert.deepEqual(sdk.suggest('met',{previousWords:[]}),before);assert.deepEqual(sdk.getWords(),words);
  }finally{sdk.dispose();}
});
