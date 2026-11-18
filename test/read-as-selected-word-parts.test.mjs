import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {WasmEngine,initSync} from '../wasm/khmer_engine.js';
import {readCompactSource} from '../scripts/read-compact-source.mjs';
import {createEngine} from '../dist/public/node.js';
const read=file=>readFileSync(new URL('../'+file,import.meta.url));
initSync({module:read('wasm/khmer_engine_bg.wasm')});
const proof=JSON.parse(read('test/fixtures/read-as-selected-word-parts.json'));
const map=JSON.parse(read('data/khmer-sound-map.json'));
const scope=part=>part.cases.map(({order,keys,...rest})=>rest);
function catalog(core){
  const words=[];let after;
  do{const page=JSON.parse(core.browseWords(JSON.stringify({query:'',limit:100,after})));words.push(...page.words);after=page.nextAfter;}while(after);
  return words;
}

test('គណៈ reads keaknak and retains keanak through explicit គៈណៈ without changing its identity or aliases',async()=>{
  const sdk=await createEngine();
  const core=WasmEngine.fromAppData(read('compiled-data/khmer.kmd'),read('compiled-data/english.kmd'));
  const exact=(input,word)=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.find(candidate=>candidate.khmer===word);
  try{
    assert.deepEqual(sdk.previewPronunciation('គៈណៈ'),['keaknak','keanak']);
    const source=readCompactSource(core),word=source.words.find(row=>row.khmer==='គណៈ');
    assert.equal(exact('keanak','គណៈ').mappingId,word.id);
    assert.equal(exact('keaknak','គណៈ').mappingId,word.id);
    assert(exact('keaknakroatmontrei','គណៈរដ្ឋមន្ត្រី'));
    assert(exact('keanakroatmontrei','គណៈរដ្ឋមន្ត្រី'));
    assert(!(map.runtime.wordParts??[]).some(rule=>rule.khmer==='គណៈ'));
    assert(map.runtime.pronunciationParts.some(rule=>rule.word==='គណៈ'&&rule.readAs==='គៈណៈ'));
    const rules=JSON.parse(core.appDataInfo()).provenance.pronunciations.rules;
    const before=catalog(core);
    core.setPronunciations(JSON.stringify([...rules,{word:'គណៈ',readAs:'គៈណៈ'}]));
    assert.deepEqual(catalog(core),before);
    core.setPronunciations(JSON.stringify([...rules,{word:'គណៈ',readAs:'មិត'}]));
    assert(exact('met','គណៈ'));assert(!exact('keanak','គណៈ'));
    core.setPronunciations(JSON.stringify(rules));
    assert.deepEqual(catalog(core),before);
    assert.deepEqual(readCompactSource(core),source);
    for(const mapping of source.mappings)assert(JSON.parse(core.convert(mapping.romanized)).candidates.some(candidate=>candidate.khmer===mapping.khmer),mapping.romanized);
  }finally{sdk.dispose();core.free();}
});

test('selected word parts use Khmer Read as spellings and retain their original scopes',()=>{
  assert.deepEqual(map.runtime.pronunciationParts,proof.pronunciations);
  for(const rule of proof.pronunciations){
    const old=proof.formerParts.find(part=>part.khmer===rule.word);
    assert.deepEqual(scope(rule),scope(old),rule.word);
    assert(!('keys' in rule));
    assert(!(map.runtime.wordParts??[]).some(part=>part.khmer===rule.word));
    assert.equal(rule.reusableAsComponent,old.reusableAsComponent,rule.word);
  }
  for(const word of proof.deferred){
    const part=(map.runtime.wordParts??[]).find(part=>part.khmer===word);
    assert(part,word);
  }
  assert(!(map.runtime.wordParts??[]).some(part=>part.khmer===proof.removedPart));
  for(const part of proof.removedParts??[]){
    assert(!(map.runtime.wordParts??[]).some(row=>row.khmer===part.khmer));
    assert(!map.runtime.pronunciationParts.some(row=>row.word===part.khmer));
  }
});

test('removed ផល and ផលិត rules restore mapped readings while ពណ៌ uses its approved Read as',()=>{
  const core=WasmEngine.fromAppData(read('compiled-data/khmer.kmd'),read('compiled-data/english.kmd'));
  const exact=(input,word)=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(row=>row.khmer===word);
  try{
    for(const [input,word] of [['phol','ផល'],['pol','ផល'],['parlet','ផលិត'],['parlit','ផលិត'],
      ['lertphol','លទ្ធផល'],['lertpol','លទ្ធផល'],['poa','ពណ៌'],['por','ពណ៌']])assert(exact(input,word),`${input} → ${word}`);
    for(const [input,word] of [['paletkam','ផលិតកម្ម'],['paletphol','ផលិតផល'],['polproyoch','ផលប្រយោជន៍']])assert(!exact(input,word),`${input} → ${word}`);
    const source=readCompactSource(core);
    for(const word of ['ផល','ផលិត','ពណ៌','ផលិតកម្ម','ផលិតផល','លទ្ធផល'])assert(source.words.some(row=>row.khmer===word));
  }finally{core.free();}
});

test('saving stock Read as rows unchanged preserves every bundled row and suggestion, including after reset',()=>{
  const core=WasmEngine.fromAppData(read('compiled-data/khmer.kmd'),read('compiled-data/english.kmd'));
  try{
    const source=readCompactSource(core),before=catalog(core);
    const stock=JSON.parse(core.appDataInfo()).provenance.pronunciations.rules;
    const inputs=['bakpoan','bakporn','pakpoan','pakporn','tinphol','kakech','kakejjar','kejjarka','kechka','kech','គេ','ke','books'];
    const results=inputs.map(input=>JSON.parse(core.suggestWithContext(input,'{}','{}')));
    core.setPronunciations(JSON.stringify([...new Map([...stock,...map.runtime.pronunciationParts.map(({word,readAs})=>({word,readAs}))].map(rule=>[rule.word,rule])).values()]));
    assert.deepEqual(catalog(core),before);
    assert.deepEqual(inputs.map(input=>JSON.parse(core.suggestWithContext(input,'{}','{}'))),results);
    core.setPronunciations(JSON.stringify(stock));
    assert.deepEqual(catalog(core),before);
    assert.deepEqual(readCompactSource(core),source);
  }finally{core.free();}
});

test('edited terminal Read as rules work with partial component analysis and preserve original scope',()=>{
  const words=['អាណាចក្រ','អាណាចក្រា','អាណាចក្រ៍','អាណាចក្រកូន','យុត្តិធម៌','កាតព្វកិច្ច'];
  const core=new WasmEngine(JSON.stringify({schemaVersion:3,mappings:[],englishWords:[],words:words.map((khmer,i)=>({id:i+1,khmer,rank:i,status:'approved'}))}));
  const exact=(input,word)=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(candidate=>candidate.khmer===word);
  try{
    const bytes=core.exportBinary(),before=catalog(core);
    core.setPronunciations(JSON.stringify([{word:'ចក្រ',readAs:'មិត'},{word:'ធម៌',readAs:'មិត'},{word:'កិច្ច',readAs:'មិត'}]));
    for(const [input,word] of [['anamet','អាណាចក្រ'],['anamit','អាណាចក្រ'],['yottimet','យុត្តិធម៌'],['katpvomet','កាតព្វកិច្ច']])assert(exact(input,word),`${input} → ${word}`);
    for(const word of words.slice(1,4))assert(!exact('anamet',word),word);
    assert.deepEqual(core.exportBinary(),bytes);
    core.setPronunciations('[]');
    assert.deepEqual(catalog(core),before);
  }finally{core.free();}
});

test('the second batch derives approved spellings, keeps scoped neighbors and removes only the redundant ភូមិ part',()=>{
  const expected={
    'ពាក្យ':['peak','piak'],'ពាណិជ្ជ':['peanech','peanej','peanich','peanij','pianech','pianej','pianich','pianij'],
    'ពិនិត្យ':['penet','penit','pinet','pinit'],'ពិភព':['piphop','piphup','pipop','pipup'],
    'ពិសោធន៍':['pesaot','pisaot'],'ពេជ្រ':['pech'],'មន្ត':['mon'],'មេឃ':['mek'],'ម៉ែត្រ':['maet'],
    'យន្ត':['yon'],'រដ្ឋ':['roat','rort'],'លទ្ធ':['lert'],'លាភ':['leap','liap'],'សង្ឃ':['song'],
    'សញ្ញា':['sanhnhea','sanhnhia','sanhnyea','sanhnyia','sanynhea','sanynhia','sanynyea','sanynyia'],'សព្ទ':['sap'],
  };
  const core=new WasmEngine(JSON.stringify({schemaVersion:3,englishWords:[],mappings:[],
    words:Object.keys(expected).map((khmer,i)=>({id:991000+i,khmer,rank:i,status:'approved'}))}));
  try{
    for(const row of catalog(core))assert.deepEqual(row.soundMapInputs,expected[row.khmer],row.khmer);
  }finally{core.free();}
  assert(!(map.runtime.wordParts??[]).some(part=>part.khmer==='ភូមិ'));
  assert(!map.runtime.pronunciationParts.some(part=>part.word==='ភូមិ'));
  assert.equal(map.runtime.pronunciationParts.find(part=>part.word==='សញ្ញា').readAs,'សាញ្ញា');
  const bundled=WasmEngine.fromAppData(read('compiled-data/khmer.kmd'),read('compiled-data/english.kmd'));
  try{
    const settings=JSON.parse(bundled.appDataInfo()).provenance.pronunciations.rules;
    assert(settings.some(rule=>rule.word==='ភូមិ'&&rule.readAs==='ភូម'));
    for(const [input,word] of [['peaksao','ពាក្យសោ'],['mepeak','មេពាក្យ'],['maetkare','ម៉ែត្រការ៉េ'],
      ['khansanha','ខណ្ឌសញ្ញា'],['khansanhnhea','ខណ្ឌសញ្ញា'],['keanakroatmontri','គណៈរដ្ឋមន្ត្រី']]){
      assert(JSON.parse(bundled.suggestWithContext(input,'{}','{}')).exactCandidates.some(row=>row.khmer===word),`${input} → ${word}`);
    }
    // An earlier independently checked neighbor remains available alongside
    // the newly approved pronunciation, rather than being rewritten by it.
    assert(JSON.parse(bundled.suggestWithContext('nh','{}','{}')).exactCandidates.some(row=>row.khmer==='ខ្ញុំ'));
  }finally{bundled.free();}
});

test('the final batch uses the marked spellings and leaves no duplicate word-part entries',()=>{
  assert.deepEqual((map.runtime.wordParts??[]),[]);
  const expected={
    'សកម្ម':['sakkam'],'សភា':['sakpea','sakphea','sakphia','sakpia'],
    'សមុទ្រ':['samot','samut'],'សម្បត្តិ':['sombat'],'សាន្ត':['san'],
    'សាស្ត្រ':['sah','sas'],'សិទ្ធិ':['set','sit'],'ស្រាន្ត':['sran'],'ហត្ថ':['hat'],
    'អក្សរ':['aksor'],'អាទិត្យ':['atet','atit'],'អ្នក':['neak'],'ឱសថ':['aosot'],
  };
  const core=new WasmEngine(JSON.stringify({schemaVersion:3,englishWords:[],mappings:[
    {id:999201,romanized:'saved-food',khmer:'អាហារ',rank:7,status:'approved'},
    {id:999202,romanized:'saved-sound',khmer:'សូរ',rank:8,status:'approved'},
  ],words:[...Object.keys(expected),'អាហារ','សូរ'].map((khmer,i)=>({id:999000+i,khmer,rank:i,status:'approved'}))}));
  try{
    const bytes=core.exportBinary();
    for(const row of catalog(core)){
      const inputs=expected[row.khmer]??(row.khmer==='អាហារ'?['aha']:['so']);
      assert.deepEqual(row.soundMapInputs,inputs,row.khmer);
    }
    for(const [input,word] of [['saved-food','អាហារ'],['saved-sound','សូរ']]){
      assert(JSON.parse(core.convert(input)).candidates.some(row=>row.khmer===word));
    }
    assert.deepEqual(core.exportBinary(),bytes);
  }finally{core.free();}
  for(const word of ['ប្រព័ន្ធ','សូរ','អាហារ'])assert(!map.runtime.pronunciationParts.some(rule=>rule.word===word));
  assert.equal(map.runtime.pronunciationParts.find(rule=>rule.word==='សភា').readAs,'សាក់ភា');
  assert.equal(map.runtime.pronunciationParts.find(rule=>rule.word==='ព័ន្ធ').readAs,'ពាន់');
});

test('approved Read as alternatives preserve checked neighbors, aliases and dictionary bytes',()=>{
  const core=WasmEngine.fromAppData(read('compiled-data/khmer.kmd'),read('compiled-data/english.kmd'));
  const before=readCompactSource(core),bytes=core.exportBinary();
  const stock=JSON.parse(core.appDataInfo()).provenance.pronunciations.rules;
  const exact=(input,word)=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(candidate=>candidate.khmer===word);
  try{
    for(const [word,inputs] of Object.entries({
      'កប៉ាល់':['kapal'],'កិច្ច':['kech','kich'],'ក្សាន្ត':['ksan'],'ខណ្ឌ':['khan'],
      'ចក្រ':['jak'],'ចិត្ត':['jet','jit'],'ទ្រព្យ':['trop'],'បារម្ភ':['barom'],
      'បង្គន់':['bongkon','bongkun'],'បន្ទប់':['bontop','bontup'],
      'ប្រៃសណីយ៍':['braisnei','braisney','praisnei','praisney'],
      'បក្ស':['pak'],'ប័ណ្ណ':['ban'],
      'ទិន្នផល':['tenphol','tinphol'],'គណបក្ស':['konpak'],
      'ខណ្ឌសញ្ញា':['khansanha'],'បក្សព័ន្ធ':['pakpoan','pakporn'],
      'ការកិច្ច':['kakech','kakich','kakejjar'],'ប្រព័ន្ធ':['bropoan','broporn'],
      'បន្ទប់រៀន':['bontoprean','bontuprean'],
      'ប័ណ្ណប្រៃសណីយ៍':['banbraisnei','banbraisney'],
    }))for(const input of inputs)assert(exact(input,word),`${input} → ${word}`);
    for(const [input,word] of [['jettor','ចិត្ត'],['kijjor','កិច្ច'],['korpal','កប៉ាល់'],['neungno','នឹងន'],['ningno','នឹងន']]){
      assert(!exact(input,word),`${input} must follow the selected pronunciation or removed part`);
    }
    // Scoped fragments remain editable without adding a dictionary word.
    core.setPronunciations(JSON.stringify([...stock,{word:'ដ្ឋាន',readAs:'មិត'}]));
    assert.deepEqual(readCompactSource(core),before);
    core.setPronunciations(JSON.stringify([...stock,{word:'កិច្ច',readAs:'មិត'}]));
    assert(exact('met','កិច្ច'));assert(!exact('kech','កិច្ច'));
    core.setPronunciations(JSON.stringify(stock));
    assert(exact('kech','កិច្ច'));assert(!exact('met','កិច្ច'));
    assert.deepEqual(core.exportBinary(),bytes);
    assert.deepEqual(readCompactSource(core),before);
  }finally{core.free();}
});
