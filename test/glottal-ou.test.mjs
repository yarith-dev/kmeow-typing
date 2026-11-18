import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {WasmEngine,initSync} from '../wasm/khmer_engine.js';
import {createEngine} from '../dist/public/node.js';
const read=file=>readFileSync(new URL('../'+file,import.meta.url));
initSync({module:read('wasm/khmer_engine_bg.wasm')});
const proof=JSON.parse(read('test/fixtures/glottal-ou.json'));

test('glottal ou reaches all five patterns while retaining older readings and protecting plain carriers',()=>{
  const rows=[...proof.cases,...proof.controls];
  const core=new WasmEngine(JSON.stringify({schemaVersion:3,englishWords:[],
    words:rows.map((row,i)=>({id:100+i,khmer:row.word,rank:i,status:'approved'})),
    mappings:[{id:900,romanized:"p'oun",khmer:'កូន',rank:100,status:'approved'}]}));
  const exact=(input,word)=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.find(row=>row.khmer===word);
  try{
    const bytes=core.exportBinary(),alias=core.convert("p'oun");
    for(const row of proof.cases){
      assert.deepEqual(JSON.parse(core.soundMapInputs(row.word,'')).inputs,[...row.previous,...row.additional].sort(),row.word);
      for(const input of row.additional){
        const candidate=exact(input,row.word);assert(candidate,`${input} → ${row.word}`);
        assert.equal(candidate.mappingId,exact(row.previous[0],row.word).mappingId);
        assert.equal(candidate.rank,exact(row.previous[0],row.word).rank);
      }
    }
    for(const row of proof.controls)assert.deepEqual(JSON.parse(core.soundMapInputs(row.word,'')).inputs,row.inputs,row.word);
    assert.equal(JSON.parse(core.suggestWithContext("p'oun",'{}','{}')).exactCandidates[0].khmer,'កូន');
    assert(!exact("p'un",'ប្អូន'));
    assert(!JSON.parse(core.suggestWithContext("b'oun",'{"soundMap":false}','{}')).exactCandidates.some(row=>row.khmer==='ប្អូន'));
    assert.equal(core.convert("p'oun"),alias);assert.deepEqual(core.exportBinary(),bytes);
  }finally{core.free();}
});

test('Read-As previews and compound edits use ou for both direct and nasal glottal paths',async()=>{
  const sdk=await createEngine({dictionary:{schemaVersion:3,englishWords:[],mappings:[],
    words:['មិត្ត','ភាព','មិត្តភាព'].map((khmer,i)=>({id:100+i,khmer,rank:i,status:'approved'}))}});
  const hit=(input,word)=>sdk.suggest(input,{previousWords:[]}).exactCandidates.some(row=>row.khmer===word);
  const queries=['met','mit','mettar',"p'on","p'oun","jong'o","jong'ou"];
  try{
    const words=sdk.getWords(),before=queries.map(input=>sdk.suggest(input,{previousWords:[]}));
    assert.deepEqual(sdk.previewPronunciation('អូន'),['on']);
    for(const [readAs,old,added]of [['ប្អូន',"p'on","p'oun"],['ចង្អូរ',"jong'o","jong'ou"]]){
      assert(sdk.previewPronunciation(readAs).includes(added));
      sdk.setPronunciations([{word:'មិត្ត',readAs}]);
      assert(hit(old,'មិត្ត'));assert(hit(added,'មិត្ត'));
      assert(hit(old+'peap','មិត្តភាព'));assert(hit(added+'peap','មិត្តភាព'));
      if(readAs==='ចង្អូរ')assert.deepEqual(sdk.suggest(old,{previousWords:[]}).nextLetterWeights,{p:500});
    }
    sdk.setPronunciations([]);
    for(const [i,input]of queries.entries())assert.deepEqual(sdk.suggest(input,{previousWords:[]}),before[i]);
    assert.deepEqual(sdk.getWords(),words);
  }finally{sdk.dispose();}
});
