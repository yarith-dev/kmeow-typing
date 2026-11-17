import * as i18n from '../i18n.js';
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {wordList,wordPage,wordPageLinks,wordLetters,wordsForLetter,romanizedKeys,wordsWithoutRomanized} from '../word-list.js';

function page({legacy=[],failSave=false,failStorage=false}={}){
  const requests=[],bases=[],stored=[],engines=[];
  class Element{
    constructor(){this.value='';this.children=[];this.dataset={};this.checked=false;}
    append(...children){this.children.push(...children);}replaceChildren(...children){this.children=children;}setAttribute(){}
    reset(){}showModal(){this.open=true;}close(){this.open=false;}focus(){}
  }
  const nodes=new Map(),$=key=>{if(!nodes.has(key))nodes.set(key,new Element());return nodes.get(key);};
  const catalog={revision:'original',token:'local',khmer:[{id:1,word:'សៀវភៅ',partsOfSpeech:['noun'],rank:4,soundMapEnabled:true,commonnessLevel:2,romanizations:['sievphov'],soundMapInputs:['seavphov']}],english:[]};
  function engine(){const current={words:[],getWords(){return this.words;},addWords(words){this.words=words;},dispose(){this.disposed=true;}};engines.push(current);return current;}
  const old=engine();old.words=legacy;
  const context=vm.createContext({...i18n,$,el:(tag,text)=>Object.assign(new Element(),{textContent:text}),$$:()=>[],engine:old,
    dictionaryWords:catalog,dictionaryError:'',wordLanguage:'khmer',wordLetter:'',missingRomanized:false,wordPageIndex:0,busy:false,damagedWords:null,legacyWords:legacy,editRow:null,editCatalog:null,dictionaryLoads:Promise.resolve(),map:null,
    wordList,wordPage,wordPageLinks,wordLetters,wordsForLetter,romanizedKeys,wordsWithoutRomanized,suggest(){},refreshExampleEditor(){},renderPronunciations(){},announce(){},errorMessage:error=>error.message,
    createEngine(){},loadSharedDictionary:async()=>({engine:engine(),catalog}),saveSharedDictionary:async({request,catalog:base})=>{requests.push(request);bases.push(base);if(failSave)throw new Error('Save rejected');return catalog;},
    localStorage:{setItem(key,value){if(failStorage)throw new Error('Storage unavailable');stored.push(value);}},WORDS_KEY:'legacy',
    confirm:()=>true,parseImport:JSON.parse,wordExport(){},URL:{},Blob:class{},setTimeout(){},document:{createElementNS:()=>new Element()}});
  const source=readFileSync(new URL('../app.js',import.meta.url),'utf8');
  vm.runInContext(source.slice(source.indexOf('function renderWordList('),source.indexOf('const groups=')),context);
  return {$,requests,bases,stored,engines,old,run:code=>vm.runInContext(code,context)};
}

test('component errors leave word details usable and compositions skip single-word analysis',()=>{
  const current=page();let calls=0;
  current.old.analyzeComponents=()=>{calls++;throw new Error('Analysis failed');};
  current.run("showWordDetails(wordList(dictionaryWords,legacyWords,'khmer')[0]);");
  assert.equal(calls,1);assert.equal(current.$('#word-details').open,true);
  const fields=current.$('#detail-content').children[0].children.map(node=>node.textContent);
  assert(fields.includes('Component analysis is unavailable.'));
  assert.equal(current.$('#detail-actions').children.length,1);
  current.run("showWordDetails({...wordList(dictionaryWords,legacyWords,'khmer')[0],word:'កា ខា'});");
  assert.equal(calls,1);assert.equal(current.$('#word-details').open,true);
});

test('Edit word saves only selected fields without touching the draft or browser storage',async()=>{
  const current=page();
  current.run("openWord(wordList(dictionaryWords,[],'khmer')[0]);");
  assert.equal(current.$('#word-text').readOnly,true);assert.equal(current.$('#word-language').disabled,true);
  assert.equal(current.$('#word-rank').value,4);assert.equal(current.$('#word-spellings').value,'sievphov');
  current.$('#word-spellings').value='sievphov, sharedbook';
  current.run("wordLetter='ក';missingRomanized=true;");
  current.run("dictionaryWords={...dictionaryWords,revision:'changed-since-form-opened'};");
  await current.$('#word-form').onsubmit({preventDefault(){}});
  assert.equal(current.requests.length,1);assert.equal(current.requests[0].original.word,'សៀវភៅ');
  assert.equal(current.requests[0].word.rank,undefined);assert.deepEqual(Array.from(current.requests[0].word.spellings),['sievphov','sharedbook']);
  assert.equal(current.bases[0].revision,'original');
  assert.equal(current.old.disposed,true);assert.deepEqual(current.stored,[]);assert.equal(current.$('#word-dialog').open,false);
  assert.equal(current.run('wordLetter'),'');assert.equal(current.$('#personal-rows').children.length,1);
  assert.equal(current.run('missingRomanized'),false);
});

test('a failed shared save retains the old live engine and previous browser words',async()=>{
  const legacy=[{word:'សៀវភៅ',spellings:['mybook'],rank:0}],current=page({legacy,failSave:true});
  current.run("openWord(wordList(dictionaryWords,legacyWords,'khmer')[0]);");
  current.$('#word-spellings').value='mybook, sievphov, sharedbook';
  await current.$('#word-form').onsubmit({preventDefault(){}});
  assert.equal(current.old.disposed,undefined);assert.deepEqual(current.old.words,legacy);assert.deepEqual(current.stored,[]);
  assert.equal(current.$('#word-dialog').open,true);assert.match(current.$('#form-error').textContent,/Save rejected/);
});

test('browser words are retired only after an explicit successful shared import',async()=>{
  const legacy=[{word:'សៀវភៅ',spellings:['mybook'],rank:0}],current=page({legacy});
  await current.$('#migrate-words').onclick();
  assert.equal(current.requests[0].type,'import');assert.equal(current.requests[0].reviewed,true);assert.equal(current.requests[0].preservePersonal,true);
  assert.deepEqual(current.stored,['[]']);assert.equal(current.run('legacyWords.length'),0);assert.equal(current.old.disposed,true);
  const failed=page({legacy,failStorage:true});await failed.$('#migrate-words').onclick();
  assert.equal(failed.old.disposed,undefined);assert.deepEqual(failed.old.words,legacy);assert.equal(failed.engines.at(-1).disposed,true);
});


test('word management loads existing Sound map policies and only submits selected policy and level changes',async()=>{
  const current=page();
  current.run("dictionaryWords.khmer[0].soundMapEnabled=false;openWord(wordList(dictionaryWords,[],'khmer')[0]);");
  assert.equal(current.$('#word-sound-map').checked,false);
  assert.equal(current.$('#word-sound-map-field').hidden,false);
  assert.equal(current.$('#word-level').value,'2');
  await current.$('#word-form').onsubmit({preventDefault(){}});
  assert.equal(current.requests.length,0);
  current.run("openWord(wordList(dictionaryWords,[],'khmer')[0]);");
  current.$('#word-sound-map').checked=true;current.$('#word-level').value='4';
  await current.$('#word-form').onsubmit({preventDefault(){}});
  assert.equal(current.requests[0].word.soundMapEnabled,true);
  assert.equal(current.requests[0].word.commonnessLevel,4);
  assert.equal(current.requests[0].original.word,'សៀវភៅ');
  current.run("openWord({word:'book',language:'english',bundled:true,personalIndices:[],romanizations:[],spellings:[],rank:0,commonnessLevel:2});");
  assert.equal(current.$('#word-sound-map-field').hidden,true);
  assert.equal(current.$('#word-sound-map').disabled,true);
  await current.$('#word-form').onsubmit({preventDefault(){}});
  assert.equal(current.requests.length,1);
});


test('components save inside Edit word without migrating personal spellings or unselected word fields',async()=>{
  const legacy=[{word:'កូនសិស្ស',spellings:['mychildpupil'],rank:5}],current=page({legacy});
  current.run("dictionaryWords.khmer[0].word='កូនសិស្ស';dictionaryWords.khmer[0].components=null;openWord(wordList(dictionaryWords,legacyWords,'khmer')[0]);");
  assert.equal(current.$('#word-dialog').open,true);
  assert.equal(current.$('#word-components-section').hidden,false);
  assert.equal(current.$('#component-mode').value,'default');
  current.$('#component-mode').value='connected';current.$('#component-parts').value='កូន + សិស្ស';
  await current.$('#word-form').onsubmit({preventDefault(){}});
  assert.equal(current.requests.length,1);
  assert.deepEqual(Object.keys(current.requests[0].word).sort(),['components','language','word']);
  assert.equal(JSON.stringify(current.requests[0].word.components),JSON.stringify([['កូន','សិស្ស']]));
  assert.equal(current.requests[0].preservePersonal,false);
  assert.equal(current.run('legacyWords.length'),1);assert.deepEqual(current.stored,[]);
  assert.equal(JSON.stringify(current.engines.at(-1).getWords()),JSON.stringify(legacy));
  assert.equal(current.$('#word-dialog').open,false);
  current.run("openWord({...wordList(dictionaryWords,legacyWords,'khmer')[0],components:[['កូន','សិស្ស']]});");
  await current.$('#word-form').onsubmit({preventDefault(){}});
  assert.equal(current.requests.length,1,'unchanged saves leave every field alone');
  current.run("openWord({...wordList(dictionaryWords,legacyWords,'khmer')[0],components:[['ទេស','ធម៌'],['ទេ','ស','ធម៌']]});");
  assert.equal(current.$('#component-parts').value,'ទេស + ធម៌\nទេ + ស + ធម៌');
  current.$('#component-mode').value='connected';current.$('#component-parts').value='';
  await current.$('#word-form').onsubmit({preventDefault(){}});
  assert.match(current.$('#form-error').textContent,/at least one/);assert.equal(current.requests.length,1);
});

test('one Save word request includes component, spelling, level and rank edits together; Cancel saves nothing',async()=>{
  const current=page();current.run("openWord(wordList(dictionaryWords,[],'khmer')[0]);");
  current.$('#component-mode').value='standalone';current.$('#word-spellings').value='sievphov, sharedbook';
  current.$('#word-level').value='4';current.$('#word-rank').value='7';
  await current.$('#word-form').onsubmit({preventDefault(){}});
  assert.equal(current.requests.length,1);assert.equal(current.requests[0].word.rank,7);
  assert.equal(current.requests[0].word.commonnessLevel,4);assert.equal(JSON.stringify(current.requests[0].word.components),'[]');
  assert.equal(current.requests[0].word.spellings.join(','),'sievphov,sharedbook');
  current.run("openWord(wordList(dictionaryWords,[],'khmer')[0]);");
  current.$('#component-mode').value='standalone';current.$('#cancel-dialog').onclick();
  assert.equal(current.requests.length,1);assert.equal(current.$('#word-dialog').open,false);
});


test('an explicit spelling edit preserves the existing personal rank during shared migration',async()=>{
  const current=page({legacy:[{word:'សៀវភៅ',spellings:['mybook'],rank:5}]});
  current.run("openWord(wordList(dictionaryWords,legacyWords,'khmer')[0]);");
  current.$('#word-spellings').value='sievphov, mybook, sharedbook';
  await current.$('#word-form').onsubmit({preventDefault(){}});
  assert.equal(current.requests[0].preservePersonal,true);assert.equal(current.requests[0].word.rank,5);
  assert.equal(current.requests[0].word.spellings.join(','),'sievphov,mybook,sharedbook');
});
