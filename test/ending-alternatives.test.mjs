import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {createEngine} from '../dist/public/node.js';
import {ruleFamilies} from '../manager/sound-rules.js';
import {setUiLanguage} from '../manager/i18n.js';
import {spaceChoice} from '../manager/composer.js';

const words=['តិច','ភ្លេច','កិច្ច','ការ','កិច្ចការ','កូន','តូច','ដូច','ឆា'];
const dictionary={schemaVersion:3,englishWords:[],
  words:words.map((khmer,i)=>({id:100+i,khmer,rank:i,status:'approved'})),
  mappings:[{id:900,romanized:'tex',khmer:'កូន',rank:1000,status:'approved'},
    {id:901,romanized:'plex',khmer:'ភ្លេច',rank:0,status:'approved'},
    {id:902,romanized:'savedich',khmer:'កូន',rank:0,status:'approved'}]};

test('Wasm and the public API accept final ex/ix and keep existing saved choices and acceptance',async()=>{
  const engine=await createEngine({dictionary});
  const query=input=>engine.suggest(input,{previousWords:[]});
  try{
    const stored=engine.getWords();
    for(const [word,inputs]of [['តិច',['tech','tich','tex','tix']],
      ['ភ្លេច',['phlech','plech','phlex','plex']],['កិច្ចការ',['kechka','kichka','kexka','kixka']]]){
      for(const input of inputs)assert(query(input).exactCandidates.some(row=>row.khmer===word),`${input} → ${word}`);
    }
    assert.equal(query('tex').exactCandidates[0].mappingId,900);
    assert.equal(query('plex').exactCandidates[0].mappingId,901);
    assert.equal(spaceChoice(query('tix'),0).khmer,'តិច');
    assert.equal(query('tix').candidates.find(row=>row.khmer==='តិច').completion,undefined);
    for(const [input,word]of [['tox','តូច'],['dox','ដូច'],['xa','ឆា'],['savedix','កូន']]){
      assert(!query(input).exactCandidates.some(row=>row.khmer===word));
    }
    for(const features of [{soundMap:false},{conversion:false},{language:'english'}]){
      assert(!engine.suggest('tix',{...features,previousWords:[]}).exactCandidates.some(row=>row.khmer==='តិច'));
    }
    engine.setInputExclusions([{word:'តិច',inputs:['tix']}]);
    assert(!query('tix').exactCandidates.some(row=>row.khmer==='តិច'));
    assert(query('tich').exactCandidates.some(row=>row.khmer==='តិច'));
    engine.setInputExclusions([]);assert.deepEqual(engine.getWords(),stored);
  }finally{engine.dispose();}
});

test('Read as retains complete component endings and shorthands remain literal',async()=>{
  const engine=await createEngine({dictionary});
  const exact=(input,word)=>engine.suggest(input,{previousWords:[]}).exactCandidates.some(row=>row.khmer===word);
  try{
    const original=engine.suggest('kechka',{previousWords:[]});
    engine.setPronunciations([{word:'កិច្ច',readAs:'តិច'}]);
    for(const input of ['tech','tich','tex','tix']){
      assert(exact(input,'កិច្ច'));assert(exact(input+'ka','កិច្ចការ'));
    }
    engine.setShorthands([{word:'កូន',inputs:['shortich']}]);
    assert(exact('shortich','កូន'));assert(!exact('shortix','កូន'));
    engine.setShorthands([]);engine.setPronunciations([]);
    assert.deepEqual(engine.suggest('kechka',{previousWords:[]}),original);
  }finally{engine.dispose();}
});

test('the ending rule appears in Consonants & endings with bilingual examples',()=>{
  const {runtime}=JSON.parse(readFileSync(new URL('../data/khmer-sound-map.json',import.meta.url)));
  try{
    for(const language of ['en','km']){
      setUiLanguage(language);
      const rule=ruleFamilies(runtime).find(row=>row.id==='consonants').rules.find(row=>row.id==='endingAlternatives');
      assert(rule);assert(rule.description.includes('កិច្ចការ'));assert(rule.description.includes('kexka'));
      assert(rule.description.includes('ech')&&rule.description.includes('ich'));
    }
  }finally{setUiLanguage('en');}
});

test('multiple valid component endings combine without a Cartesian spelling limit',async()=>{
  const word='តិច'.repeat(8);
  const engine=await createEngine({dictionary:{schemaVersion:3,englishWords:[],mappings:[],
    words:['តិច',word].map((khmer,i)=>({id:100+i,khmer,rank:i,status:'approved'})),
    structures:[{wordId:101,componentIds:Array(8).fill(100)}]}});
  try{
    for(const input of ['tich'.repeat(8),'tix'.repeat(8),'tex'.repeat(4)+'tich'.repeat(4)]){
      assert(engine.suggest(input,{previousWords:[]}).exactCandidates.some(row=>row.khmer===word),input);
    }
  }finally{engine.dispose();}
});
