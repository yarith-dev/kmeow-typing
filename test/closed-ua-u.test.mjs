import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {WasmEngine,initSync} from '../wasm/khmer_engine.js';
import {createEngine} from '../dist/public/node.js';
import {ruleFamilies} from '../manager/sound-rules.js';
const read=file=>readFileSync(new URL('../'+file,import.meta.url));
initSync({module:read('wasm/khmer_engine_bg.wasm')});
const proof=JSON.parse(read('test/fixtures/closed-ua-u.json'));

test('closed ◌ួ adds u in both series while retaining open vowels, silent រ, aliases and existing choices',()=>{
  const rows=[...proof.cases,...proof.controls];
  const core=new WasmEngine(JSON.stringify({schemaVersion:3,englishWords:[],
    words:rows.map((row,i)=>({id:100+i,khmer:row.word,rank:i,status:'approved'})),
    mappings:[{id:900,romanized:'juy',khmer:'កូន',rank:1000,status:'approved'}]}));
  const exact=input=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates;
  try{
    const bytes=core.exportBinary(),alias=core.convert('juy');
    for(const row of proof.cases){
      assert.deepEqual(JSON.parse(core.soundMapInputs(row.word,'')).inputs,[...row.previous,...row.additional].sort(),row.word);
      for(const input of [...row.previous,...row.additional])assert(exact(input).some(candidate=>candidate.khmer===row.word),`${input} → ${row.word}`);
    }
    for(const row of proof.controls)assert.deepEqual(JSON.parse(core.soundMapInputs(row.word,'')).inputs,row.inputs,row.word);
    for(const [input,word]of [['mun','មុន'],['bun','បុន'],['puk','ពុក'],['rum','រុម'],['juy','កូន']])assert.equal(exact(input)[0].khmer,word,input);
    assert(!JSON.parse(core.suggestWithContext('muy','{"soundMap":false}','{}')).exactCandidates.some(row=>row.khmer==='មួយ'));
    assert.deepEqual(core.exportBinary(),bytes);assert.equal(core.convert('juy'),alias);
  }finally{core.free();}
});

test('Read as and compounds share the closing decision and retain older predictions',async()=>{
  const sdk=await createEngine({dictionary:{schemaVersion:3,englishWords:[],mappings:[],
    words:['ជួយ','កម្ម','ករ','កម្មករ'].map((khmer,i)=>({id:100+i,khmer,rank:i,status:'approved'}))}});
  const hit=(input,word)=>sdk.suggest(input,{previousWords:[]}).exactCandidates.some(row=>row.khmer===word);
  try{
    const words=sdk.getWords(),before=sdk.suggest('ju',{previousWords:[]});
    assert.deepEqual(before.nextLetterWeights,{r:1000});
    assert.deepEqual(sdk.previewPronunciation('ជួរ'),['chur','jur']);
    sdk.setPronunciations([{word:'កម្ម',readAs:'ជួយ'}]);
    for(const input of ['jury','juy','chury','chuy']){assert(hit(input,'កម្ម'));assert(hit(input+'kor','កម្មករ'));}
    sdk.setPronunciations([{word:'កម្ម',readAs:'មួយ'.repeat(30)}]);
    for(const input of ['mury'.repeat(30),'muy'.repeat(30),'mury'.repeat(15)+'muy'.repeat(15)])assert(hit(input,'កម្ម'));
    sdk.setPronunciations([]);
    assert.deepEqual(sdk.suggest('ju',{previousWords:[]}),before);assert.deepEqual(sdk.getWords(),words);
  }finally{sdk.dispose();}
});

test('the closing-vowel rule appears with Khmer examples in the existing Vowels family',()=>{
  const map=JSON.parse(read('data/khmer-sound-map.json'));
  const family=ruleFamilies(map.runtime).find(row=>row.id==='vowels');
  assert(family.rules.some(rule=>rule.id==='closedVowelAlternatives'));
  assert.deepEqual(map.dependentVowels.find(row=>row.khmer==='ួ').aSeriesKeys,['ur']);
  assert.deepEqual(map.dependentVowels.find(row=>row.khmer==='ូ').oSeriesKeys,['u','ou']);
});
