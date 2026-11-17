import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { initSync, WasmEngine, wasmBuildFingerprint } from '../wasm/khmer_engine.js';
import {readCompactSource} from '../scripts/read-compact-source.mjs';

const sha = value => createHash('sha256').update(value).digest('hex');
const read = path => readFileSync(new URL('../' + path, import.meta.url));
const dictionaryCases = JSON.parse(read('test/fixtures/dictionary-cases.json'));
const caseWords = new Set(dictionaryCases.map(row=>row.word));
const vowelChoice = JSON.parse(read('test/fixtures/vowel-series.json'));
const silentCodaSeries = JSON.parse(read('test/fixtures/silent-coda-series.json'));
const manifest = JSON.parse(read('dictionary-manifest.json'));
const wasm = read('wasm/khmer_engine_bg.wasm');
initSync({module: wasm});
const files = Object.fromEntries(['khmer', 'english'].map(language => [language, read(`compiled-data/${language}.kmd`)]));

test('oa and or are vowel alternatives while real consonant r stays literal',()=>{
  const proof=JSON.parse(read('test/fixtures/oa-or-vowels.json'));
  const core=WasmEngine.fromAppData(files.khmer,files.english);
  const exact=input=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates;
  try{
    for(const [word,original,alternative] of proof.pairs){
      const before=exact(original).find(c=>c.khmer===word),after=exact(alternative).find(c=>c.khmer===word);
      assert(before&&after,`${original} / ${alternative} → ${word}`);
      assert.equal(after.mappingId,before.mappingId);assert.equal(after.rank,before.rank);
    }
    for(const [word,original,alternative] of proof.consonantR){
      assert(exact(original).some(c=>c.khmer===word),`${original} → ${word}`);
      assert(!exact(alternative).some(c=>c.khmer===word),`${alternative} must not replace consonant r in ${word}`);
    }
    assert.equal(exact('jor')[0]?.khmer,'ជរ');
    assert.equal(exact('poa')[0]?.khmer,'ពណ៌');
  }finally{core.free();}
});

test('shortened unwritten vowels use o in Wasm while saved and personal spellings remain exact',()=>{
  const proof=JSON.parse(read('test/fixtures/short-inherent-o.json'));
  const rows=[...proof.readings,...proof.unchanged];
  const data={schemaVersion:2,mappings:[],words:rows.map(([word],i)=>({
    id:99100+i,khmer:word,rank:i,status:'approved'
  }))};
  const core=new WasmEngine(JSON.stringify(data));
  data.mappings=[{id:99900,romanized:'rolut',khmer:'រលត់',rank:77,status:'approved'}];
  const saved=new WasmEngine(JSON.stringify(data));
  const personal=WasmEngine.fromPersonalWords(JSON.stringify([{word:'រលត់',spellings:['rolut','myreading']}]));
  const bytes=[core,saved,personal].map(engine=>Buffer.from(engine.exportBinary()));
  try{
    for(const [i,[word,yes,no]]of rows.entries()){
      for(const input of yes){
        const hit=JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.find(row=>row.khmer===word);
        assert(hit,`${input}: ${word}`);assert.equal(hit.mappingId,99100+i);assert.equal(hit.rank,i);
      }
      for(const input of no)assert(!JSON.parse(core.suggestWithContext(input,'{}','{}'))
        .exactCandidates.some(row=>row.khmer===word),`${input}: ${word}`);
    }
    for(const features of [{soundMap:false},{romanized:false},{mode:'english'}])assert(!JSON.parse(
      core.suggestWithContext('rolot',JSON.stringify(features),'{}')).exactCandidates.some(row=>row.khmer==='រលត់'));
    for(const soundMap of [true,false]){
      assert(JSON.parse(saved.suggestWithContext('rolut',JSON.stringify({soundMap}),'{}'))
        .exactCandidates.some(row=>row.mappingId===99900&&row.rank===77));
      for(const input of ['rolut','myreading'])assert(JSON.parse(
        core.suggestWithExtension(personal,input,JSON.stringify({soundMap}),'{}'))
        .exactCandidates.some(row=>row.khmer==='រលត់'));
    }
    for(const [i,engine]of [core,saved,personal].entries())assert.deepEqual(Buffer.from(engine.exportBinary()),bytes[i]);
  }finally{core.free();saved.free();personal.free();}
});

test('O-series ាត់ accepts oa and or through the same bundled word identities',()=>{
  const core=WasmEngine.fromAppData(files.khmer,files.english);
  const exact=input=>JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates;
  try{
    for(const [word,oldInput,newInput] of [['គាត់','koat','kort'],['មាត់','moat','mort'],
      ['ទាត់','toat','tort'],['ឃាត់','khoat','khort'],['ពួកគាត់','purkkoat','purkkort']]){
      const old=exact(oldInput).find(row=>row.khmer===word);
      const added=exact(newInput).find(row=>row.khmer===word);
      assert(old&&added,`${word}: ${oldInput} / ${newInput}`);
      assert.equal(added.mappingId,old.mappingId);
      assert.equal(added.rank,old.rank);
    }
    for(const [word,input] of [['កាត់','kort']]){
      assert(!exact(input).some(row=>row.khmer===word),`${input} must not add ${word}`);
    }
  }finally{core.free();}
});

test('the current dictionary pair matches its manifest and preserves complete typing identities and POS', () => {
  assert.equal(sha(wasm), wasmBuildFingerprint);
  assert.equal(sha(wasm), manifest.wasm);
  const core = WasmEngine.fromAppData(files.khmer, files.english);
  try {
    const source=readCompactSource(core), info=JSON.parse(core.appDataInfo()), stats=JSON.parse(core.stats());
    assert.equal(sha(core.exportBinary()),info.compactArtifactHash);
    assert.equal(info.pairId, manifest.pairId);
    assert.equal(source.mappings.length,stats.mappings);
    for (const language of ['khmer', 'english']) {
      const bytes=files[language];
      assert.equal(sha(bytes),manifest[language]);
      assert.equal(bytes.subarray(0,8).toString(),'KMEOWA01');
      assert.equal(sha(bytes.subarray(0,-32)),bytes.subarray(-32).toString('hex'));
      const metadata=JSON.parse(bytes.subarray(24+bytes.readUInt32LE(12)+bytes.readUInt32LE(16),-32));
      const records=language==='khmer'?source.words:source.englishWords;
      const normalize=word=>language==='english'?word.toLowerCase():word;
      assert.equal(records.length,language==='khmer'?stats.words:stats.englishWords);
      assert.equal(metadata.words.length,records.length);
      assert.deepEqual(new Set(metadata.words.map(row=>normalize(row.word))),new Set(records.map(row=>normalize(row.khmer??row.text))));
      assert.equal(new Set(metadata.words.map(row=>row.id)).size,metadata.words.length);
      assert.deepEqual(Object.keys(metadata).sort(),['format','language','snapshotId','translations','words']);
      assert.deepEqual(metadata.translations,[]);
      const tags=new Set('noun verb adj adv prep pron conj det intj num particle classifier aux article'.split(' '));
      for(const word of metadata.words){
        assert.deepEqual(Object.keys(word).sort(),['commonnessLevel','context','id','meanings','partsOfSpeech','sources','translations','word']);
        assert.equal(word.context,null);
        assert.deepEqual(word.meanings,[]);assert.deepEqual(word.translations,[]);assert.deepEqual(word.sources,[]);
        assert(word.partsOfSpeech.every(tag=>tags.has(tag)));
        assert.equal(new Set(word.partsOfSpeech).size,word.partsOfSpeech.length);
      }
      if(language==='khmer')for(const expected of dictionaryCases){
        assert.equal(records.find(row=>row.khmer===expected.word)?.id,expected.id);
        const actual=metadata.words.find(row=>row.word===expected.word);
        assert.equal(actual?.commonnessLevel,expected.commonnessLevel);
        assert.deepEqual(actual?.partsOfSpeech,expected.partsOfSpeech);
      }
    }
  }finally{core.free();}
});

test('owner-selected words containing អាចម៍ stay absent from the bundled dictionary',()=>{
  const core=WasmEngine.fromAppData(files.khmer,files.english);
  try{
    const source=readCompactSource(core);
    assert(!source.words.some(word=>word.khmer.includes('អាចម៍')));
    assert(!source.mappings.some(mapping=>mapping.khmer.includes('អាចម៍')));
    assert.deepEqual(JSON.parse(core.lookupWordMetadata('khmer','អាចម៍រណារ')),[]);
  }finally{core.free();}
});

test('only ចា៎ remains and the existing jah input uses the selected spelling',()=>{
  const core=WasmEngine.fromAppData(files.khmer,files.english);
  try{
    const source=readCompactSource(core);
    for(const spelling of ['ចាស','ចា៎ស']){
      assert(!source.words.some(w=>w.khmer===spelling));
      assert(!source.mappings.some(m=>m.khmer===spelling));
      assert.deepEqual(JSON.parse(core.lookupWordMetadata('khmer',spelling)),[]);
    }
    assert.equal(source.words.filter(w=>w.khmer==='ចា៎').length,1);
    assert.deepEqual(JSON.parse(core.convert('jah')).candidates,[{mappingId:13201,khmer:'ចា៎',rank:0}]);
    const entry=JSON.parse(core.lookupWordMetadata('khmer','ចា៎'));
    assert.equal(entry.length,1);assert.equal(entry[0].commonnessLevel,1);
    assert(JSON.parse(core.suggest('ចា៎')).candidates.some(c=>c.khmer==='ចា៎'));
    assert(source.words.some(w=>w.khmer==='ចាស់'));
  }finally{core.free();}
});

test('Wasm shares the native series-specific vowel selection and leaves other combinations alone',()=>{
  const rows=[...vowelChoice.cases,...vowelChoice.unrelated];
  const core=new WasmEngine(JSON.stringify({schemaVersion:2,mappings:[],words:rows.map((row,i)=>({
    id:98000+i,khmer:row.word,rank:i,status:'approved'
  }))}));
  try{
    for(const row of rows)for(const field of ['yes','no'])for(const input of row[field]){
      const result=JSON.parse(core.suggestWithContext(input,'{}','{}'));
      assert.equal(result.exactCandidates.some(c=>c.khmer===row.word),field==='yes',`${input}: ${row.word}`);
    }
  }finally{core.free();}
});

test('Wasm shares the local silent-coda vowel rule and preserves its boundaries',()=>{
  const rows=[...silentCodaSeries.cases,...silentCodaSeries.unrelated];
  const core=new WasmEngine(JSON.stringify({schemaVersion:2,mappings:[],words:rows.map((row,i)=>({
    id:97000+i,khmer:row.word,rank:i,status:'approved'
  }))}));
  try{
    const page=JSON.parse(core.browseWords(JSON.stringify({query:'',limit:100})));
    for(const row of rows){
      assert.deepEqual(page.words.find(entry=>entry.khmer===row.word)?.soundMapInputs??[],row.inputs,row.word);
      for(const input of row.inputs){
        const result=JSON.parse(core.suggestWithContext(input,'{}','{}'));
        assert(result.exactCandidates.some(candidate=>candidate.khmer===row.word),`${input}: ${row.word}`);
      }
      for(const input of row.removed??[]){
        const result=JSON.parse(core.suggestWithContext(input,'{}','{}'));
        assert(!result.exactCandidates.some(candidate=>candidate.khmer===row.word),`${input}: ${row.word}`);
      }
    }
  }finally{core.free();}
  const bundled=WasmEngine.fromAppData(files.khmer,files.english);
  try{
    const result=JSON.parse(bundled.suggestWithContext('tusna','{}','{}'));
    assert.equal(result.exactCandidates[0]?.khmer,'ទស្សនា');
    assert.equal(result.exactCandidates[0]?.mappingId,10342);
    assert.equal(result.exactCandidates[0]?.rank,0);
    assert(!JSON.parse(bundled.suggestWithContext('tosnea','{}','{}')).exactCandidates
      .some(candidate=>candidate.khmer==='ទស្សនា'));
    for(const input of ['tusnak','tuhnak']){
      const marked=JSON.parse(bundled.suggestWithContext(input,'{}','{}'));
      assert.equal(marked.exactCandidates[0]?.khmer,'ទស្សនៈ');
      assert.equal(marked.exactCandidates[0]?.mappingId,14577);
    }
    assert.equal(JSON.parse(bundled.lookupWordMetadata('khmer','ទស្សន')).length,1);
  }finally{bundled.free();}
});

test('Wasm accepts p and ph for final ផ and ភ at word ends and internal syllable boundaries',()=>{
  const proof=JSON.parse(read('test/fixtures/additive-word-reuse-ph-p.json'));
  // Artificial spellings exercise both final letters without adding dictionary words.
  const words=['កាផ','កាភ','កាផកា','កាភកា','លាភ'];
  const core=new WasmEngine(JSON.stringify({schemaVersion:2,mappings:[],words:words.map((word,i)=>({
    id:96000+i,khmer:word,rank:i,status:'approved'
  }))}));
  try{
    for(const [input,word] of [...proof.compatibility,...proof.phP].filter(([,word])=>words.includes(word))){
      const result=JSON.parse(core.suggestWithContext(input,'{}','{}'));
      assert(result.exactCandidates.some(candidate=>candidate.khmer===word),`${input}: ${word}`);
    }
  }finally{core.free();}
});

test('Wasm uses the shared សាស្ត្រ part in unlisted compounds and preserves other spellings',()=>{
  const proof=JSON.parse(read('test/fixtures/checked-science-ending.json'));
  const core=new WasmEngine(JSON.stringify({schemaVersion:2,mappings:[],words:proof.words.map((word,i)=>({
    id:98100+i,khmer:word,rank:i,status:'approved'
  }))}));
  try{
    for(const group of ['soundYes','compatibility','yes','no'])for(const [input,word] of proof[group]){
      const result=JSON.parse(core.suggestWithContext(input,'{}','{}'));
      assert.equal(result.exactCandidates.some(candidate=>candidate.khmer===word),group!=='no',`${input}: ${word}`);
    }
  }finally{core.free();}
  const bundled=WasmEngine.fromAppData(files.khmer,files.english);
  try{
    const rows=JSON.parse(bundled.browseWords(JSON.stringify({query:'សាស្ត្រ',limit:100}))).words;
    for(const row of rows.filter(row=>row.khmer.endsWith('សាស្ត្រ'))){
      assert(row.soundMapInputs.length,row.khmer);
      assert(row.soundMapInputs.every(input=>input.endsWith('sah')||input.endsWith('sas')),row.khmer);
    }
  }finally{bundled.free();}
});

test('the bundled t inputs distinguish តោះ from ទោះ by the selected vowel series',()=>{
  const core=WasmEngine.fromAppData(files.khmer,files.english);
  try{
    for(const [input,word] of Object.entries(vowelChoice.intendedFirstChoices)){
      const result=JSON.parse(core.suggestWithContext(input,'{}','{}'));
      assert.equal(result.exactCandidates[0].khmer,word,input);
      const other=word==='តោះ'?'ទោះ':'តោះ';
      assert(!result.exactCandidates.some(candidate=>candidate.khmer===other),`${input}: opposite series`);
    }
  }finally{core.free();}
});

test('dictionary cases use the current Sound map and preserve other crowded suggestions',()=>{
  const core=WasmEngine.fromAppData(files.khmer,files.english);
  const source=readCompactSource(core);
  source.words=source.words.filter(word=>!dictionaryCases.some(row=>row.id===word.id));
  const original=new WasmEngine(JSON.stringify(source));
  const metadata=Object.fromEntries(Object.entries(files).map(([language,bytes])=>{
    const file=JSON.parse(bytes.subarray(24+bytes.readUInt32LE(12)+bytes.readUInt32LE(16),-32));
    if(language==='khmer')file.words=file.words.filter(word=>!caseWords.has(word.word));
    return [language,JSON.stringify(file)];
  }));
  original.loadWordMetadata(metadata.khmer,metadata.english);
  // Compare the admission cohort with the same owner pronunciation settings.
  const pronunciations=JSON.parse(core.appDataInfo()).provenance.pronunciations?.rules??[];
  original.setPronunciations(JSON.stringify(pronunciations.filter(rule=>!caseWords.has(rule.word))));
  try{
    for(const admission of dictionaryCases){
      const row=JSON.parse(core.browseWords(JSON.stringify({query:admission.word,limit:10}))).words.find(w=>w.khmer===admission.word);
      assert.deepEqual(row.romanizations,[]);
      const inputs=admission.inputs;
      assert.deepEqual(row.soundMapInputs,inputs);
      assert(inputs.length>0);
      for(const input of [admission.word,...inputs]){
        const result=JSON.parse(core.suggestWithContext(input,'{}','{"previousWords":[]}'));
        assert(result.candidates.some(c=>c.khmer===admission.word),input);
        if(input!==admission.word)assert(result.exactCandidates.some(c=>c.khmer===admission.word),`${input}: complete word`);
      }
    }
    for(const input of ['t','to','tu','toh','tuh','tos','tusna','ត','តោ','តំណាង','tumnang','mi','me','mae','ot','otth','ottom','tr','preah','ព្រះ','មិត្ត']){
      for(const context of [null,{previousWords:[]},{previousWords:['យើង']}]){
        const query=c=>JSON.parse(context?c.suggestWithContext(input,'{}',JSON.stringify(context)):c.suggest(input));
        const before=query(original),after=query(core);
        assert.deepEqual(after.candidates.filter(c=>!caseWords.has(c.khmer)),before.candidates,input);
        if(before.exactCandidates){
          assert.deepEqual(after.exactCandidates.filter(c=>!caseWords.has(c.khmer)),before.exactCandidates);
          if(vowelChoice.intendedFirstChoices[input]){
            assert.equal(after.exactCandidates[0].khmer,vowelChoice.intendedFirstChoices[input]);
          }else{
            assert.deepEqual(after.exactCandidates[0],before.exactCandidates[0],`${input}: existing exact choice`);
          }
        }
      }
    }
    for(const word of ['ចាស','ចា៎ស','ទឹកដោះគោ','ស្រេកទឹក','ឈឺក្បាល','ដើរលេង']){
      assert.deepEqual(JSON.parse(core.lookupWordMetadata('khmer',word)),[]);
      assert(!source.words.some(entry=>entry.khmer===word));
    }
  }finally{core.free();original.free();}
});


test('repeated words keep Khmer completion without automatic doubled Latin inputs',()=>{
  const core=WasmEngine.fromAppData(files.khmer,files.english);
  try{
    for(const [word,input] of [['នីមួយៗ','nimurynimury'],['សឹមៗ','sermserm']]){
      assert.equal(JSON.parse(core.lookupWordMetadata('khmer',word)).length,1);
      assert(JSON.parse(core.suggest(word)).candidates.some(candidate=>candidate.khmer===word));
      assert(!JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(candidate=>candidate.khmer===word));
    }
  }finally{core.free();}
});

test('ke ranks common គេ before the saved rare spelling without removing the alias',()=>{
  const core=WasmEngine.fromAppData(files.khmer,files.english);
  try{
    const result=JSON.parse(core.suggestWithContext('ke','{}','{}'));
    assert.equal(result.candidates[0].khmer,'គេ');
    assert.equal(result.exactCandidates[0].khmer,'គេ');
    assert.equal(result.exactCandidates[0].mappingId,7979);
    const saved=result.candidates.find(candidate=>candidate.mappingId===13256);
    assert.equal(saved.khmer,'កេរ្តិ៍');assert.equal(saved.rank,0);assert.equal(saved.tapOnly,true);
    assert.deepEqual(JSON.parse(core.convert('ke')).candidates,[{mappingId:13256,khmer:'កេរ្តិ៍',rank:0}]);
    assert.equal(JSON.parse(core.suggestWithFeatures('ke','{"soundMap":false}')).candidates[0].khmer,'កេរ្តិ៍');
  }finally{core.free();}
});

test('សិស្ស reads through the current ឹ vowel table in both bundled words',()=>{
  const core=WasmEngine.fromAppData(files.khmer,files.english);
  const reference=new WasmEngine(JSON.stringify({schemaVersion:2,mappings:[],words:
    ['សឹស្ស','កូនសឹស្ស'].map((khmer,i)=>({id:96000+i,khmer,rank:0,status:'approved'}))}));
  const inputs=(engine,word)=>JSON.parse(engine.browseWords(JSON.stringify({query:word,limit:100}))).words.find(r=>r.khmer===word).soundMapInputs;
  try{
    for(const [word,pronounced] of [['សិស្ស','សឹស្ស'],['កូនសិស្ស','កូនសឹស្ស']]){
      const expected=inputs(reference,pronounced);
      assert.deepEqual(inputs(core,word),expected);
      for(const input of expected)assert(JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(c=>c.khmer===word));
    }
  }finally{core.free();reference.free();}
});


test('nh adds an exact ខ្ញុំ reading while keeping its mapped inputs',()=>{
  const core=WasmEngine.fromAppData(files.khmer,files.english);
  try{
    const row=JSON.parse(core.browseWords(JSON.stringify({query:'ខ្ញុំ',limit:10}))).words.find(r=>r.khmer==='ខ្ញុំ');
    assert.deepEqual(row.soundMapInputs,['khnhom','khnyom','nh']);
    for(const input of row.soundMapInputs)assert.equal(JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates[0]?.khmer,'ខ្ញុំ');
  }finally{core.free();}
});

test('ar/or alternatives and the standalone សកម្ម override preserve explicit personal spellings',()=>{
  const core=WasmEngine.fromAppData(files.khmer,files.english);
  const personal=WasmEngine.fromPersonalWords(JSON.stringify([
    {word:'សកម្ម',spellings:['sorkam','sarkam','myactive']},
    {word:'ខា',spellings:['kar','sakkam']}
  ]));
  const before=[Buffer.from(core.exportBinary()),Buffer.from(personal.exportBinary())];
  try{
    for(const [input,word] of [['kor','ក'],['kar','ក'],['mok','មក'],['sakkam','សកម្ម'],
      ['sorkampheap','សកម្មភាព'],['sarkampheap','សកម្មភាព']]){
      const result=JSON.parse(core.suggestWithContext(input,'{}','{}'));
      assert(result.exactCandidates.some(candidate=>candidate.khmer===word),`${input}: ${word}`);
    }
    for(const input of ['sorkam','sarkam'])assert(!JSON.parse(core.suggestWithContext(input,'{}','{}'))
      .exactCandidates.some(candidate=>candidate.khmer==='សកម្ម'),input);
    assert(!JSON.parse(core.suggestWithContext('sakkampheap','{}','{}'))
      .exactCandidates.some(candidate=>candidate.khmer==='សកម្មភាព'));
    for(const soundMap of [true,false])for(const [input,word] of [
      ['sorkam','សកម្ម'],['sarkam','សកម្ម'],['myactive','សកម្ម'],['kar','ខា'],['sakkam','ខា']]){
      const result=JSON.parse(core.suggestWithExtension(personal,input,JSON.stringify({soundMap}),'{}'));
      assert(result.exactCandidates.some(candidate=>candidate.khmer===word),`${input}: ${word}`);
      assert(JSON.parse(core.convertWithExtension(personal,input)).candidates.some(candidate=>candidate.khmer===word));
    }
    assert.deepEqual(Buffer.from(core.exportBinary()),before[0]);
    assert.deepEqual(Buffer.from(personal.exportBinary()),before[1]);
  }finally{core.free();personal.free();}
});

test('Wasm and native fixtures share local letter-pair vowels, marks and explicit-spelling protections',()=>{
  const proof=JSON.parse(read('test/fixtures/local-vowel-pairs.json'));
  const rows=[...proof.cases,...proof.unchanged];
  const core=new WasmEngine(JSON.stringify({schemaVersion:2,mappings:[],words:rows.map((row,i)=>({
    id:98500+i,khmer:row.word,rank:i,status:'approved'
  }))}));
  const personal=WasmEngine.fromPersonalWords(JSON.stringify([
    {word:'សប្បាយ',spellings:['sabay','sorbbay']},
    {word:'លទ្ធិ',spellings:['lerthi','lotthi']},
    {word:'វគ្គ',spellings:['veak','vokko']}
  ]));
  const bytes=[Buffer.from(core.exportBinary()),Buffer.from(personal.exportBinary())];
  try{
    for(const [i,row]of rows.entries()){
      const page=JSON.parse(core.browseWords(JSON.stringify({query:row.word,limit:100})));
      const entry=page.words.find(word=>word.khmer===row.word);
      if(row.prefixes)assert(entry.soundMapInputs.length&&entry.soundMapInputs.every(input=>row.prefixes.some(prefix=>input.startsWith(prefix))),row.word);
      else assert.deepEqual(entry.soundMapInputs,row.inputs,row.word);
      for(const input of row.yes??row.inputs){
        const hit=JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.find(word=>word.khmer===row.word);
        assert(hit,`${input}: ${row.word}`);assert.equal(hit.mappingId,98500+i);assert.equal(hit.rank,i);
      }
      for(const input of row.no??[])assert(!JSON.parse(core.suggestWithContext(input,'{}','{}'))
        .exactCandidates.some(word=>word.khmer===row.word),`${input}: ${row.word}`);
    }
    for(const soundMap of [true,false])for(const [input,word]of [
      ['sabay','សប្បាយ'],['sorbbay','សប្បាយ'],['lerthi','លទ្ធិ'],['lotthi','លទ្ធិ'],['veak','វគ្គ'],['vokko','វគ្គ']]){
      assert(JSON.parse(core.suggestWithExtension(personal,input,JSON.stringify({soundMap}),'{}'))
        .exactCandidates.some(candidate=>candidate.khmer===word),`${input}: ${word}`);
    }
    assert.deepEqual(Buffer.from(core.exportBinary()),bytes[0]);
    assert.deepEqual(Buffer.from(personal.exportBinary()),bytes[1]);
  }finally{core.free();personal.free();}
});
