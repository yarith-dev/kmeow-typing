import * as i18n from '../i18n.js';
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {createEngine} from '../../dist/public/node.js';
import {exampleWords,exampleTopics,exampleParagraph,examplePage,initialExample,customExampleTopics,loadExample,saveExample,addExample,convertExample,trackExampleInput,exampleChanged,exampleWordAt,exampleDisplayParts,exampleSuggestions,chooseExampleWord,EXAMPLES_KEY} from '../examples.js';
import {suggestionReading} from '../suggestion-view.js';
import {wordList,wordPage,wordPageLinks,wordLetters,wordsForLetter,romanizedKeys,wordsWithoutRomanized} from '../word-list.js';

test('all original paragraphs use existing words and accepted typing inputs',async()=>{
  const engine=await createEngine();
  try{
    const words=engine.getWords(),before=engine.suggest('sievphov');
    const used=new Set(exampleTopics.flatMap(topic=>topic.sentences.flat()).filter(word=>word!=='?'));
    assert.equal(new Set(exampleTopics.map(topic=>topic.id)).size,exampleTopics.length);
    for(const topic of exampleTopics){
      const paragraph=exampleParagraph(topic);
      assert(paragraph.romanized&&!paragraph.romanized.includes('undefined'));
      assert(/[\u1780-\u17ff]/.test(paragraph.khmer));
      assert.equal(convertExample(engine,initialExample(topic),paragraph.romanized).khmer,paragraph.khmer);
    }
    for(const word of used){
      const input=exampleWords[word];assert(input,word);
      assert(engine.suggest(input,{language:'khmer',completion:false}).candidates.some(candidate=>candidate.khmer===word),`${input} → ${word}`);
    }
    assert.deepEqual(engine.getWords(),words);assert.deepEqual(engine.suggest('sievphov'),before);
  }finally{engine.dispose();}
});

function examplesUI(hash='#words',{core=null,storage={getItem:()=>null,setItem(){}}}={}){
  class Element{
    constructor(){this.children=[];this.attributes={};this.dataset={};this._value='';this._text='';this.writes=0;this.listeners={};this.scrolls=[];this.selectionStart=0;this.selectionEnd=0;this.selectionDirection='none';}
    get value(){return this._value;}set value(value){this._value=value;this.writes++;this.selectionStart=this.selectionEnd=value.length;}
    get textContent(){return this._text+this.children.map(child=>child.textContent).join('');}
    set textContent(value){this._text=value??'';this.children=[];}
    setSelectionRange(start,end,direction='none'){this.selectionStart=start;this.selectionEnd=end;this.selectionDirection=direction;}
    addEventListener(name,listener){(this.listeners[name]??=[]).push(listener);}
    emit(name,properties={}){const event={type:name,prevented:false,preventDefault(){this.prevented=true;},...properties};for(const listener of this.listeners[name]??[])listener(event);return event;}
    focus(){document.activeElement=this;}
    scrollIntoView(options){this.scrolls.push(options);}
    setAttribute(name,value){this.attributes[name]=value;}
    removeAttribute(name){delete this.attributes[name];}
    append(...children){this.children.push(...children);}
    replaceChildren(...children){this._text='';this.children=children;}
  }
  const nodes=new Map(),$=key=>{if(!nodes.has(key))nodes.set(key,new Element());return nodes.get(key);};
  const pages=['typing','words','soundmap','examples','pronunciation'].map(id=>Object.assign($('#'+id),{id}));
  const tabs=['typing','words','soundmap'].map(id=>{const tab=new Element();tab.dataset.page=id;return tab;});
  const windowListeners={};
  const location={hash},document={},window={scrollY:0,scrollTo({top}){this.scrollY=top;},history:{replaceState(_state,_title,hash){location.hash=hash;}},addEventListener(name,listener){windowListeners[name]=listener;}};
  const context=vm.createContext({...i18n,Error,$,el:(tag,text,className)=>Object.assign(new Element(),{tag,textContent:text,className}),
    $$:selector=>selector==='.page'?pages:tabs,location,document,window,examplePage,exampleTopics,initialExample,customExampleTopics,loadExample,saveExample,addExample,convertExample,trackExampleInput,exampleChanged,exampleWordAt,exampleDisplayParts,exampleSuggestions,chooseExampleWord,suggestionReading,
    wordList,wordPage,wordPageLinks,wordLetters,wordsForLetter,romanizedKeys,wordsWithoutRomanized,engine:core,localStorage:storage,errorMessage:error=>error.message,busy:false,damagedWords:null,legacyWords:[]});
  const source=readFileSync(new URL('../app.js',import.meta.url),'utf8');
  const layoutStart=source.indexOf('// Only presentation follows the viewport;');
  vm.runInContext(source.slice(source.indexOf("let wordLanguage='khmer'"),layoutStart),context);
  vm.runInContext(source.slice(source.indexOf('let dictionaryWords='),source.indexOf('function showWordDetails(')),context);
  return {$,pages,tabs,location,document,window,windowListeners,run:code=>vm.runInContext(code,context)};
}

test('Pronunciation opens as a Sound map subpage and Back restores the entry point',()=>{
  const page=examplesUI('#soundmap');
  page.$('#sound-search').value='silent endings';
  page.$('#sound-group').value='rules';
  page.window.scrollY=640;
  page.$('#pronunciation-open').onclick();
  page.location.hash='#soundmap/pronunciation';
  page.windowListeners.hashchange({type:'hashchange',oldURL:'http://localhost/#soundmap'});
  assert.deepEqual(page.pages.filter(node=>!node.hidden).map(node=>node.id),['pronunciation']);
  assert.equal(page.tabs.find(node=>node.dataset.page==='soundmap').attributes['aria-current'],'page');
  assert.equal(page.document.title,'Pronunciation · Kmeow Engine');
  assert.equal(page.document.activeElement,page.$('#pronunciation-title'));
  assert.equal(page.window.scrollY,0);
  page.$('#pronunciation-search').value='met';
  page.location.hash='#soundmap';
  page.windowListeners.hashchange({type:'hashchange',oldURL:'http://localhost/#soundmap/pronunciation'});
  assert.deepEqual(page.pages.filter(node=>!node.hidden).map(node=>node.id),['soundmap']);
  assert.equal(page.document.activeElement,page.$('#pronunciation-open'));
  assert.equal(page.window.scrollY,640);
  assert.equal(page.$('#sound-search').value,'silent endings');
  assert.equal(page.$('#sound-group').value,'rules');
  assert.equal(page.$('#pronunciation-search').value,'met');
  const direct=examplesUI('#soundmap/pronunciation');
  assert.deepEqual(direct.pages.filter(node=>!node.hidden).map(node=>node.id),['pronunciation']);
});

test('Example opens topics, each topic shows Romanized above Khmer, and Back preserves Word list',()=>{
  const page=examplesUI();
  page.$('#personal-search').value='my search';page.run('renderWordList();wordPageIndex=3;');
  assert.equal(page.$('#examples-open').hidden,false);page.$('#examples-open').onclick();
  assert.equal(page.location.hash,'words/examples');
  // Browsers prefix an assigned fragment with #.
  page.location.hash='#'+page.location.hash;page.run('route();');
  assert.equal(page.$('#examples').hidden,false);assert.equal(page.$('#examples-back').href,'#words');
  assert.equal(page.$('#example-topics').children.length,exampleTopics.length);
  assert.equal(page.tabs.find(tab=>tab.dataset.page==='words').attributes['aria-current'],'page');
  for(const link of page.$('#example-topics').children){
    page.location.hash=link.href;page.run('route();');
    const topic=examplePage(page.location.hash,'khmer').topic,paragraph=exampleParagraph(topic);
    assert.equal(page.$('#examples-title').textContent,`${exampleTopics.indexOf(topic)+1}. ${topic.title}`);
    assert.equal(page.$('#example-romanized').value,paragraph.romanized);
    assert.equal(page.$('#example-romanized-view').textContent,paragraph.romanized);
    assert.equal(page.$('#example-romanized').hidden,true);assert.equal(page.$('#example-romanized').disabled,true);
    assert.equal(page.$('#example-romanized-view').hidden,false);assert.equal(page.$('#example-actions').hidden,true);
    assert.equal(page.$('#example-edit').hidden,false);
    assert.equal(page.$('#example-khmer').textContent,paragraph.khmer);
    assert.equal(page.$('#example-topics').hidden,true);assert.equal(page.$('#example-reading').hidden,false);
    assert.equal(page.$('#examples-back').href,'#words/examples');
  }
  page.location.hash=page.$('#examples-back').href;page.run('route();');
  assert.equal(page.$('#example-topics').hidden,false);
  assert.equal(page.$('#example-edit').hidden,true);
  page.location.hash=page.$('#examples-back').href;page.run('route();');
  assert.equal(page.$('#words').hidden,false);assert.equal(page.$('#personal-search').value,'my search');assert.equal(page.run('wordPageIndex'),3);
  const html=readFileSync(new URL('../index.html',import.meta.url),'utf8');
  assert(html.indexOf('id="example-romanized"')<html.indexOf('id="example-khmer"'));
});

test('English hides Example and prevents old example links from changing the selected language',()=>{
  const page=examplesUI();page.run("wordLanguage='english';renderWordList();");
  assert.equal(page.$('#examples-open').hidden,true);page.$('#examples-open').onclick();assert.equal(page.location.hash,'#words');
  for(const hash of ['#words/examples','#words/examples/food']){
    page.location.hash=hash;page.run('route();');
    assert.equal(page.location.hash,'#words');assert.equal(page.$('#examples').hidden,true);
    assert.equal(page.$('#words').hidden,false);assert.equal(page.run('wordLanguage'),'english');
  }
  page.run("wordLanguage='khmer';renderWordList();");assert.equal(page.$('#examples-open').hidden,false);
  page.location.hash='#words/examples/not-a-topic';page.run('route();');assert.equal(page.$('#example-topics').hidden,false);
});

test('live conversion updates changed words while preserving unchanged choices, punctuation and unknown input',async()=>{
  const core=await createEngine();
  try{
    const topic=exampleTopics.find(topic=>topic.id==='family'),original=initialExample(topic),before=core.suggest('sievphov');
    const changed=convertExample(core,original,'sievphov. '+original.romanized);
    assert.equal(changed.khmer,'សៀវភៅ។ '+original.khmer);
    assert(changed.khmer.includes('បួននាក់')); // Keep នាក់ instead of the homographic អ្នក.
    assert.deepEqual(convertExample(core,changed,original.romanized).khmer,original.khmer);
    const literal=convertExample(core,initialExample(exampleTopics[0]),'sievphov 3.5\nzzzzzzzzzzzzzz!');
    assert.equal(literal.khmer,'សៀវភៅ 3.5\nzzzzzzzzzzzzzz!');assert.deepEqual(literal.unknown,['zzzzzzzzzzzzzz']);
    assert.equal(convertExample(core,original,'').khmer,'');
    assert.throws(()=>convertExample(core,original,'a'.repeat(20001)),/20,000/);
    assert.deepEqual(core.suggest('sievphov'),before);
  }finally{core.dispose();}
});

function memoryStorage(){
  const values=new Map();return {values,getItem:key=>values.get(key)??null,setItem:(key,value)=>values.set(key,value)};
}
test('Save persists the live output per topic, and Cancel restores the last saved pair',async()=>{
  const core=await createEngine(),storage=memoryStorage();
  try{
    const page=examplesUI('#words/examples/food',{core,storage}),input=page.$('#example-romanized');
    const original=input.value;
    assert.equal(input.disabled,true);assert.equal(input.hidden,true);assert.equal(page.$('#example-actions').hidden,true);
    page.$('#example-edit').onclick();
    assert.equal(input.hidden,false);assert.equal(page.$('#example-romanized-view').hidden,true);assert.equal(page.$('#example-edit').hidden,true);
    assert.equal(page.$('#example-actions').hidden,false);assert.equal(page.$('#example-cancel').disabled,false);
    page.$('#example-cancel').onclick();assert.equal(input.hidden,true);assert.equal(page.$('#example-edit').hidden,false);
    page.$('#example-edit').onclick();
    assert.equal(input.disabled,false);assert.equal(page.$('#example-save').disabled,true);
    input.value='sievphov.';input.emit('input');
    assert.equal(page.$('#example-khmer').textContent,'សៀវភៅ។');assert.equal(page.$('#example-save').disabled,false);
    page.$('#example-cancel').onclick();assert.equal(input.value,original);assert.equal(storage.getItem(EXAMPLES_KEY),null);
    assert.equal(input.hidden,true);assert.equal(page.$('#example-actions').hidden,true);
    page.$('#example-edit').onclick();
    input.value='sievphov.';input.emit('input');page.$('#example-save').onclick();
    assert.equal(page.$('#example-save').disabled,true);assert.equal(page.$('#example-status').textContent,'Saved on this device.');
    assert.equal(input.hidden,true);assert.equal(page.$('#example-romanized-view').textContent,'sievphov.');assert.equal(page.$('#example-actions').hidden,true);
    page.$('#example-edit').onclick();
    input.value='khnhom';input.emit('input');page.$('#example-cancel').onclick();
    assert.equal(input.value,'sievphov.');assert.equal(page.$('#example-khmer').textContent,'សៀវភៅ។');
    const reopened=examplesUI('#words/examples/food',{core,storage});
    assert.equal(reopened.$('#example-romanized').hidden,true);assert.equal(reopened.$('#example-romanized-view').textContent,'sievphov.');
    assert.equal(reopened.$('#example-romanized').value,'sievphov.');assert.equal(reopened.$('#example-khmer').textContent,'សៀវភៅ។');
    reopened.location.hash='#words/examples/family';reopened.run('route();');
    assert.equal(reopened.$('#example-romanized').value,exampleParagraph(exampleTopics.find(topic=>topic.id==='family')).romanized);
  }finally{core.dispose();}
});

test('typing preserves native selections and navigation keeps unsaved drafts until canceled',async()=>{
  const core=await createEngine(),storage=memoryStorage();
  try{
    const page=examplesUI('#words/examples/food',{core,storage}),input=page.$('#example-romanized');
    page.$('#example-edit').onclick();
    input.value='sievphov';input.selectionStart=1;input.selectionEnd=4;input.selectionDirection='backward';const writes=input.writes;
    input.emit('input');assert.equal(input.writes,writes);assert.equal(input.selectionDirection,'backward');assert.equal(input.selectionStart,1);assert.equal(input.selectionEnd,4);
    page.location.hash='#words/examples';page.run('route();');page.location.hash='#words/examples/food';page.run('route();');
    assert.equal(input.value,'sievphov');assert.equal(page.$('#example-khmer').textContent,'សៀវភៅ');assert.equal(storage.getItem(EXAMPLES_KEY),null);
    assert.equal(input.hidden,false);
    page.$('#example-cancel').onclick();assert.equal(input.value,exampleParagraph(exampleTopics.find(topic=>topic.id==='food')).romanized);
  }finally{core.dispose();}
});

test('IME preview waits for composition completion and storage failure keeps the draft and previous save',async()=>{
  const core=await createEngine(),storage=memoryStorage();
  try{
    const page=examplesUI('#words/examples/food',{core,storage}),input=page.$('#example-romanized'),original=page.$('#example-khmer').textContent;
    page.$('#example-edit').onclick();
    input.emit('compositionstart');input.value='sievphov';input.emit('input');
    assert.equal(page.$('#example-khmer').textContent,original);assert.equal(page.$('#example-save').disabled,true);
    input.emit('compositionend');assert.equal(page.$('#example-khmer').textContent,'សៀវភៅ');page.$('#example-save').onclick();
    const saved=storage.getItem(EXAMPLES_KEY);storage.setItem=()=>{throw new Error('Storage full');};
    page.$('#example-edit').onclick();
    input.value='khnhom';input.emit('input');page.$('#example-save').onclick();
    assert.match(page.$('#example-error').textContent,/Storage full/);assert.equal(input.value,'khnhom');assert.equal(storage.getItem(EXAMPLES_KEY),saved);
    assert.equal(input.hidden,false);assert.equal(page.$('#example-actions').hidden,false);assert.equal(page.$('#example-edit').hidden,true);
    page.$('#example-cancel').onclick();assert.equal(input.value,'sievphov');assert.equal(page.$('#example-khmer').textContent,'សៀវភៅ');
  }finally{core.dispose();}
});

test('failed example saves follow the interface language without rewriting the unsaved paragraph',()=>{
  const storage={getItem:()=>null,setItem(){throw new Error('This example could not be saved.');}};
  try{
    for(const initial of ['en','km']){
      i18n.setUiLanguage(initial);
      const page=examplesUI('#words/examples/food',{core:{suggest:()=>({candidates:[]})},storage}),input=page.$('#example-romanized');
      page.$('#example-edit').onclick();input.value='unchanged unknown paragraph';input.emit('input');page.$('#example-save').onclick();
      input.selectionStart=2;input.selectionEnd=6;input.selectionDirection='backward';const writes=input.writes;
      for(const locale of ['km','en','km','en']){
        i18n.setUiLanguage(locale);page.run('route();');
        assert.equal(page.$('#example-error').textContent,i18n.t('Could not save this example: {error}',{error:i18n.t('This example could not be saved.')}));
        assert.equal(input.value,'unchanged unknown paragraph');assert.equal(input.writes,writes);
        assert.equal(input.selectionStart,2);assert.equal(input.selectionEnd,6);assert.equal(input.selectionDirection,'backward');
        assert.equal(input.hidden,false);assert.equal(page.$('#example-save').disabled,false);
      }
    }
  }finally{i18n.setUiLanguage('en');}
});

test('example saves merge topics and preserve unreadable storage instead of overwriting it',()=>{
  const storage=memoryStorage(),[first,second]=exampleTopics;
  saveExample(first,initialExample(first),storage);saveExample(second,initialExample(second),storage);
  assert.equal(loadExample(first,storage).khmer,exampleParagraph(first).khmer);
  assert.equal(loadExample(second,storage).khmer,exampleParagraph(second).khmer);
  storage.values.set(EXAMPLES_KEY,'unreadable');
  assert.throws(()=>loadExample(first,storage));assert.throws(()=>saveExample(first,initialExample(first),storage));
  assert.equal(storage.getItem(EXAMPLES_KEY),'unreadable');
});

test('Add example previews, saves, reopens and edits a custom paragraph without changing built-ins or typing',async()=>{
  const core=await createEngine(),storage=memoryStorage();
  const files=['khmer','english'].map(language=>readFileSync(new URL(`../../compiled-data/${language}.kmd`,import.meta.url)));
  const builtins=JSON.stringify(exampleTopics),queries=['sievphov','angkor','tos'],before=queries.map(input=>core.suggest(input)),words=core.getWords();
  try{
    const page=examplesUI('#words/examples',{core,storage});
    assert.equal(page.$('#example-add').hidden,false);assert.equal(page.$('#example-add').disabled,false);
    page.$('#example-add').onclick();assert.equal(page.location.hash,'#words/examples/new');page.run('route();');
    const title=page.$('#example-title'),input=page.$('#example-romanized');
    assert.equal(page.$('#example-title-field').hidden,false);assert.equal(input.hidden,false);
    assert.equal(page.$('#example-add').hidden,true);assert.equal(page.$('#example-edit').hidden,true);
    assert.equal(page.$('#example-save').disabled,true);
    title.value='Food';title.emit('input');assert.equal(page.$('#example-save').disabled,true);
    input.value='sievphov.';input.emit('input');assert.equal(page.$('#example-khmer').textContent,'សៀវភៅ។');
    assert.equal(page.$('#example-save').disabled,false);page.$('#example-save').onclick();
    assert.match(page.location.hash,/^#words\/examples\/custom-/);page.run('route();');
    const customHash=page.location.hash;
    assert.equal(page.$('#examples-title').textContent,`${exampleTopics.length+1}. Food`);assert.equal(page.$('#example-title-field').hidden,true);
    assert.equal(input.hidden,true);assert.equal(page.$('#example-romanized-view').textContent,'sievphov.');
    assert.equal(customExampleTopics(storage).length,1);
    const reopened=examplesUI(customHash,{core,storage});
    assert.equal(reopened.$('#example-khmer').textContent,'សៀវភៅ។');assert.equal(reopened.$('#example-romanized-view').textContent,'sievphov.');
    reopened.$('#example-edit').onclick();reopened.$('#example-romanized').value='khnhom';reopened.$('#example-romanized').emit('input');reopened.$('#example-save').onclick();
    assert.equal(loadExample(customExampleTopics(storage)[0],storage).khmer,'ខ្ញុំ');assert.equal(customExampleTopics(storage).length,1);
    const saved=storage.getItem(EXAMPLES_KEY);
    reopened.$('#example-edit').onclick();reopened.$('#example-romanized').value='sievphov';reopened.$('#example-romanized').emit('input');reopened.$('#example-cancel').onclick();
    assert.equal(reopened.$('#example-romanized').value,'khnhom');assert.equal(storage.getItem(EXAMPLES_KEY),saved);
    reopened.location.hash='#words/examples';reopened.run('route();');
    assert.equal(reopened.$('#example-topics').children.length,exampleTopics.length+1);
    assert.equal(reopened.$('#example-topics').children.at(-1).children[1].textContent,'Food');
    i18n.setUiLanguage('km');reopened.run('route();');
    assert.equal(reopened.$('#example-topics').children.at(-1).children[1].textContent,'Food');
    reopened.location.hash=customHash;reopened.run('route();');assert.equal(reopened.$('#examples-title').textContent,`${exampleTopics.length+1}. Food`);
    assert.equal(JSON.stringify(exampleTopics),builtins);assert.deepEqual(core.getWords(),words);
    assert.deepEqual(queries.map(input=>core.suggest(input)),before);
    for(const [index,language]of ['khmer','english'].entries())assert.deepEqual(readFileSync(new URL(`../../compiled-data/${language}.kmd`,import.meta.url)),files[index]);
  }finally{core.dispose();i18n.setUiLanguage('en');}
});

test('a new draft survives navigation and language switches, and Cancel never creates an example',()=>{
  const storage=memoryStorage(),page=examplesUI('#words/examples',{core:{suggest:()=>({candidates:[]})},storage});
  page.$('#example-add').onclick();page.run('route();');
  const title=page.$('#example-title'),input=page.$('#example-romanized');
  title.value='My paragraph';title.emit('input');input.value='words to practice';input.emit('input');
  page.location.hash='#words/examples';page.run('route();');page.$('#example-add').onclick();page.run('route();');
  assert.equal(title.value,'My paragraph');assert.equal(input.value,'words to practice');
  input.selectionStart=1;input.selectionEnd=4;input.selectionDirection='backward';const writes=input.writes;
  try{
    i18n.setUiLanguage('km');page.run('route();');
    assert.equal(title.value,'My paragraph');assert.equal(input.value,'words to practice');assert.equal(input.writes,writes);
    assert.equal(input.selectionDirection,'backward');assert.equal(page.$('#example-save').disabled,false);
    page.$('#example-cancel').onclick();page.run('route();');
    assert.equal(page.location.hash,'#words/examples');assert.equal(storage.getItem(EXAMPLES_KEY),null);
    assert.equal(page.$('#example-topics').children.length,exampleTopics.length);
    page.$('#example-add').onclick();page.run('route();');
    assert.equal(title.value,'');assert.equal(input.value,'');assert.equal(page.$('#example-save').disabled,true);
  }finally{i18n.setUiLanguage('en');}
});

test('a failed new save keeps its title and preview, and IME cannot create a partial example',()=>{
  const storage=memoryStorage(),page=examplesUI('#words/examples',{core:{suggest:()=>({candidates:[]})},storage});
  page.$('#example-add').onclick();page.run('route();');
  const title=page.$('#example-title'),input=page.$('#example-romanized');
  title.value='My text';title.emit('input');input.emit('compositionstart');input.value='my draft';input.emit('input');
  assert.equal(page.$('#example-save').disabled,true);page.$('#example-save').onclick();assert.equal(storage.getItem(EXAMPLES_KEY),null);
  input.emit('compositionend');assert.equal(page.$('#example-save').disabled,false);
  storage.setItem=()=>{throw new Error('Storage full');};page.$('#example-save').onclick();
  assert.equal(page.location.hash,'#words/examples/new');assert.match(page.$('#example-error').textContent,/Storage full/);
  assert.equal(title.value,'My text');assert.equal(input.value,'my draft');assert.equal(page.$('#example-khmer').textContent,'my draft');
  assert.equal(customExampleTopics(storage).length,0);
});

test('custom-example persistence merges old edits and rejects invalid, duplicate, unreadable or oversized saves',()=>{
  const storage=memoryStorage(),first=exampleTopics[0],original=initialExample(first);
  saveExample(first,original,storage);
  const value={romanized:'my text',khmer:'my text',spans:[]};
  const created=addExample('  My title  ',value,storage,'custom-first');
  assert.equal(created.topic.title,'My title');assert.deepEqual(customExampleTopics(storage),[created.topic]);
  assert.deepEqual(loadExample(created.topic,storage),value);assert.deepEqual(loadExample(first,storage),original);
  saveExample(created.topic,{...value,romanized:'updated',khmer:'updated'},storage);
  assert.equal(loadExample(created.topic,storage).romanized,'updated');
  const saved=storage.getItem(EXAMPLES_KEY);
  for(const action of [
    ()=>addExample('',value,storage,'custom-empty'),()=>addExample('x'.repeat(121),value,storage,'custom-long'),
    ()=>addExample('Empty',{romanized:' ',khmer:'',spans:[]},storage,'custom-blank'),
    ()=>addExample('Duplicate',value,storage,'custom-first'),()=>addExample('Reserved',value,storage,first.id),
    ()=>saveExample({id:'custom-missing'},value,storage),
  ]){assert.throws(action);assert.equal(storage.getItem(EXAMPLES_KEY),saved);}
  assert.equal(examplePage('#words/examples/custom-first','khmer',customExampleTopics(storage)).topic.title,'My title');
  assert.equal(examplePage('#words/examples/custom-first','english',customExampleTopics(storage)),null);
  const large=JSON.stringify({format:'kmeow-example-edits-v1',topics:{},extra:'x'.repeat(2*1024*1024-100)});
  storage.values.set(EXAMPLES_KEY,large);
  assert.throws(()=>addExample('Limit',{romanized:'a'.repeat(200),khmer:'a'.repeat(200),spans:[]},storage,'custom-limit'),/too large/);
  assert.equal(storage.getItem(EXAMPLES_KEY),large);
  for(const broken of ['unreadable',JSON.stringify({format:'kmeow-example-edits-v1',topics:{},customTopics:[{id:'food',title:'Invalid'}]})]){
    storage.values.set(EXAMPLES_KEY,broken);assert.throws(()=>customExampleTopics(storage));
    assert.throws(()=>addExample('Preserve',value,storage,'custom-safe'));assert.equal(storage.getItem(EXAMPLES_KEY),broken);
  }
});

test('every caret position uses the complete Romanized word, with intact choices as context',async()=>{
  const core=await createEngine();
  const draft={romanized:'neak knhom!',khmer:'នាក់ knhom!',spans:[{start:0,end:4,input:'neak',word:'នាក់'}]};
  const calls=[],engine={suggest(input,options){calls.push({input,options});return core.suggest(input,options);}};
  try{
    const expected=core.suggest('knhom',{language:'khmer',conversion:true,soundMap:true,completion:true,correction:false,previousWords:['នាក់']});
    for(let caret=5;caret<=10;caret++){
      const selection=exampleSuggestions(engine,draft,caret);
      assert.equal(selection.target.input,'knhom');assert.equal(selection.target.start,5);assert.equal(selection.target.end,10);
      assert.deepEqual(selection.result,expected);assert.equal(calls.at(-1).input,'knhom');
    }
    assert.equal(exampleWordAt(draft,1,8),null);assert.equal(exampleWordAt(draft,-1),null);
    assert.equal(exampleWordAt(draft,11),null);assert.equal(exampleWordAt(draft,NaN),null);
    const boundary={...draft,romanized:'neak. knhom!',spans:draft.spans};
    exampleSuggestions(engine,boundary,8);assert.deepEqual(calls.at(-1).options.previousWords,[]);
    const unicode={romanized:'🐈 knhom',khmer:'🐈 knhom',spans:[]};
    assert.equal(exampleWordAt(unicode,5).input,'knhom');assert.equal(exampleWordAt(unicode,5).start,3);
  }finally{core.dispose();}
});

test('an exact alternative changes only that example choice and survives saving and later edits',async()=>{
  const core=await createEngine(),storage=memoryStorage(),topic=exampleTopics.find(item=>item.id==='family'),original=initialExample(topic);
  const target=original.spans.find(span=>span.word==='នាក់'),before=core.suggest('neak',{language:'khmer'}),personal=core.getWords();
  try{
    const selection=exampleSuggestions(core,original,target.start+2);
    const alternative=selection.result.candidates.find(candidate=>candidate.khmer==='អ្នក');assert(alternative);
    const changed=chooseExampleWord(core,original,selection.target,alternative);
    assert.equal(changed.romanized,original.romanized);assert(exampleChanged(changed,original));
    assert.equal(changed.spans.find(span=>span.start===target.start).word,'អ្នក');
    assert.deepEqual(changed.spans.filter(span=>span.start!==target.start),original.spans.filter(span=>span.start!==target.start));
    assert.equal(exampleDisplayParts(changed,'romanized').map(part=>part.text).join(''),original.romanized);
    assert.equal(exampleDisplayParts(changed,'khmer').map(part=>part.text).join(''),changed.khmer);
    saveExample(topic,changed,storage);const reloaded=loadExample(topic,storage);
    assert.equal(convertExample(core,reloaded,reloaded.romanized).spans.find(span=>span.start===target.start).word,'អ្នក');
    const later=convertExample(core,reloaded,reloaded.romanized+' sievphov.');
    assert.equal(later.spans.find(span=>span.start===target.start).word,'អ្នក');
    assert.deepEqual(core.suggest('neak',{language:'khmer'}),before);assert.deepEqual(core.getWords(),personal);
    assert.equal(original.spans.find(span=>span.start===target.start).word,'នាក់');
  }finally{core.dispose();}
});

test('completions replace the whole token, preserve unknown text and keep multi-word saved spellings intact',()=>{
  const candidate={mappingId:1,khmer:'សួស្តី',rank:0,completion:'som reab'};
  const engine={suggest:input=>({normalizedInput:input,candidates:input==='som'||input==='som reab'?[candidate]:[],exactCandidates:[]})};
  const original={romanized:'som 3.5\nzzzz',khmer:'som 3.5\nzzzz',spans:[]};
  const selection=exampleSuggestions(engine,original,1),changed=chooseExampleWord(engine,original,selection.target,candidate);
  assert.equal(changed.romanized,'som reab 3.5\nzzzz');assert.equal(changed.khmer,'សួស្តី 3.5\nzzzz');
  assert.deepEqual(changed.unknown,['zzzz']);assert.equal(exampleWordAt(changed,6).input,'som reab');
  assert.equal(convertExample(engine,changed,changed.romanized).khmer,changed.khmer);
  assert.throws(()=>chooseExampleWord(engine,original,selection.target,{...candidate,khmer:'អ្នក'}),/changed/);
  assert.throws(()=>chooseExampleWord(engine,original,{...selection.target,start:1},candidate),/changed/);
  assert.equal(original.romanized,'som 3.5\nzzzz');assert.equal(original.spans.length,0);
});

test('a repeated multiword completion preserves the following word choice and saves its shifted range',async()=>{
  const core=await createEngine({words:[{word:'សួស្តី',spellings:['som som']},{word:'សូម',spellings:['som']}]}),storage=memoryStorage();
  const value={romanized:'som som',khmer:'សូមសូម',spans:[
    {start:0,end:3,input:'som',word:'សូម'},{start:4,end:7,input:'som',word:'សូម'}
  ]};
  const created=addExample('Repeated completion',value,storage,'custom-repeat-completion');
  try{
    const page=examplesUI(`#words/examples/${created.topic.id}`,{core,storage});
    page.$('#example-romanized-view').children.find(node=>node.exampleToken).onclick();
    const completion=exampleCandidate(page,'សួស្តី');assert(completion);completion.onclick();
    assert.equal(page.$('#example-romanized').value,'som som som');
    assert.equal(page.$('#example-khmer').textContent,'សួស្តីសូម');
    page.$('#example-save').onclick();const saved=loadExample(created.topic,storage);
    assert.deepEqual(saved.spans,[{start:0,end:7,input:'som som',word:'សួស្តី'},{start:8,end:11,input:'som',word:'សូម'}]);
    assert.equal(convertExample(core,saved,saved.romanized).khmer,'សួស្តីសូម');
    const reopened=examplesUI(`#words/examples/${created.topic.id}`,{core,storage});
    assert.equal(reopened.$('#example-khmer').textContent,'សួស្តីសូម');
  }finally{core.dispose();}
});

test('editing the last token of a multiword choice queries the whole edited word and saves no stale prefix',async()=>{
  const core=await createEngine({words:[{word:'សួស្តី',spellings:['som som']},{word:'សូម',spellings:['som']}]}),storage=memoryStorage(),calls=[];
  const firstWord=core.suggest('som',{language:'khmer',conversion:true,soundMap:true,completion:false,correction:false,previousWords:[]}).exactCandidates[0].khmer;
  const engine={suggest(input,options){calls.push(input);return core.suggest(input,options);}};
  const created=addExample('Multiword edit',{romanized:'som',khmer:'សូម',spans:[{start:0,end:3,input:'som',word:'សូម'}]},storage,'custom-multiword-edit');
  try{
    const page=examplesUI(`#words/examples/${created.topic.id}`,{core:engine,storage}),input=page.$('#example-romanized');
    page.$('#example-romanized-view').children.find(node=>node.exampleToken).onclick();
    exampleCandidate(page,'សួស្តី').onclick();page.$('#example-save').onclick();
    const selected=loadExample(created.topic,storage);assert.equal(selected.romanized,'som som');
    assert.equal(selected.khmer,'សួស្តី');page.$('#example-edit').onclick();
    for(const suffix of ['x',"'x",'-x']){
      // Restore the saved multiword choice before extending its last word.
      page.$('#example-cancel').onclick();page.$('#example-edit').onclick();calls.length=0;
      const text='som som'+suffix;
      nativeExampleEdit(input,7,7,text,text.length,'insertText');
      assert(calls.includes('som'+suffix));assert.equal(page.$('#example-khmer').textContent,firstWord+' som'+suffix);
      for(let caret=4;caret<=text.length;caret++){
        input.setSelectionRange(caret,caret);input.emit('click');
        assert.equal(page.$('#example-suggestion-panel').hidden,false);assert.equal(calls.at(-1),'som'+suffix);
        assert.equal(page.$('#example-suggestion-title').textContent,'Suggestions for som'+suffix);
      }
    }
    page.$('#example-save').onclick();const saved=loadExample(created.topic,storage);
    assert.equal(saved.romanized,'som som-x');assert.equal(saved.khmer,firstWord+' som-x');
    assert.deepEqual(saved.spans,[{start:0,end:3,input:'som',word:firstWord}]);
    const reopened=examplesUI(`#words/examples/${created.topic.id}`,{core:engine,storage});
    assert.equal(reopened.$('#example-khmer').textContent,saved.khmer);
    reopened.$('#example-romanized-view').children.find(node=>node.exampleToken?.input==='som-x').onclick();
    assert.equal(calls.at(-1),'som-x');
    // A separating space or punctuation still leaves the complete choice intact.
    for(const suffix of [' ','.','?']){
      const text=selected.romanized+suffix,tracked=trackExampleInput(selected,text,{text:selected.romanized,start:7,end:7,inputType:'insertText'},8);
      assert.equal(convertExample(core,tracked,text).spans[0].word,'សួស្តី');
    }
  }finally{core.dispose();}
});

const repeatedExample={romanized:'neak neak',khmer:'អ្នកនាក់',spans:[
  {start:0,end:4,input:'neak',word:'អ្នក'},{start:5,end:9,input:'neak',word:'នាក់'}
]};
function nativeExampleEdit(input,start,end,text,caret,inputType){
  input.setSelectionRange(start,end);
  assert.equal(input.emit('beforeinput',{inputType}).prevented,false);
  input.value=text;input.setSelectionRange(caret,caret);
  assert.equal(input.emit('input',{inputType}).prevented,false);
}
test('native edits retain choices on the correct repeated occurrence, including IME and history caret fallback',async()=>{
  const core=await createEngine();
  const cases=[
    {name:'insert repeated prefix',start:0,end:0,text:'neak neak neak',caret:5,type:'insertText',kept:[[5,'អ្នក'],[10,'នាក់']]},
    {name:'paste repeated prefix',start:0,end:0,text:'neak neak neak',caret:5,type:'insertFromPaste',kept:[[5,'អ្នក'],[10,'នាក់']]},
    {name:'cut first occurrence',start:0,end:5,text:'neak',caret:0,type:'deleteByCut',kept:[[0,'នាក់']]},
    {name:'delete backward',start:5,end:5,text:'neak',caret:0,type:'deleteWordBackward',kept:[[0,'នាក់']]},
    {name:'delete forward',start:0,end:0,text:'neak',caret:0,type:'deleteWordForward',kept:[[0,'នាក់']]},
    {name:'replace repeated occurrence',start:0,end:4,text:'neak neak neak',caret:9,type:'insertFromPaste',kept:[[10,'នាក់']]},
    {name:'IME repeated prefix',start:0,end:0,text:'neak neak neak',caret:5,ime:true,kept:[[5,'អ្នក'],[10,'នាក់']]},
    {name:'undo deletion',start:0,end:0,text:'neak neak neak',caret:5,type:'historyUndo',kept:[[5,'អ្នក'],[10,'នាក់']]},
    {name:'redo deletion',start:0,end:0,text:'neak',caret:0,type:'historyRedo',kept:[[0,'នាក់']]}
  ];
  try{
    for(const item of cases){
      const storage=memoryStorage(),created=addExample(item.name,repeatedExample,storage,'custom-repeat-edit');
      const page=examplesUI(`#words/examples/${created.topic.id}`,{core,storage}),input=page.$('#example-romanized');
      page.$('#example-edit').onclick();
      if(item.ime){
        input.setSelectionRange(item.start,item.end);input.emit('compositionstart');
        input.value=item.text;input.setSelectionRange(item.caret,item.caret);input.emit('input');input.emit('compositionend');
      }else nativeExampleEdit(input,item.start,item.end,item.text,item.caret,item.type);
      assert.equal(input.value,item.text,item.name);assert.equal(input.selectionStart,item.caret,item.name);
      page.$('#example-save').onclick();const saved=loadExample(created.topic,storage);
      for(const [start,word]of item.kept)assert.equal(saved.spans.find(span=>span.start===start)?.word,word,item.name);
      const reopened=examplesUI(`#words/examples/${created.topic.id}`,{core,storage});
      assert.equal(reopened.$('#example-khmer').textContent,saved.khmer,item.name);
    }
    const fallback=trackExampleInput(repeatedExample,'neak neak neak',null,5);
    assert.deepEqual(fallback.spans.map(span=>[span.start,span.word]),[[5,'អ្នក'],[10,'នាក់']]);
  }finally{core.dispose();}
});

const exampleCandidate=(page,word)=>page.$('#example-suggestions').children.find(button=>button.children[0]?.textContent===word);
test('tapping Khmer or Romanized words lets a read-only example save and cancel a different exact choice',async()=>{
  const core=await createEngine(),storage=memoryStorage(),topic=exampleTopics.find(item=>item.id==='family');
  const original=initialExample(topic),page=examplesUI('#words/examples/family',{core,storage});
  try{
    const khmer=page.$('#example-khmer').children.find(node=>node.textContent==='នាក់');assert.equal(khmer.tag,'button');
    const input=page.$('#example-romanized'),writes=input.writes;khmer.onclick();
    assert.equal(page.$('#example-suggestion-panel').hidden,false);assert.equal(input.hidden,true);
    assert.equal(page.$('#example-suggestion-panel').scrolls.at(-1).block,'nearest');
    assert.equal(page.$('#example-suggestion-title').textContent,'Suggestions for neak');
    exampleCandidate(page,'អ្នក').onclick();
    assert.equal(input.value,original.romanized);assert.equal(input.writes,writes);assert.equal(page.$('#example-save').disabled,false);
    assert.equal(page.$('#example-actions').hidden,false);assert.equal(page.$('#example-suggestion-panel').hidden,true);
    page.$('#example-save').onclick();assert.equal(page.$('#example-actions').hidden,true);
    const selected=loadExample(topic,storage);assert.equal(selected.spans.find(span=>span.input==='neak').word,'អ្នក');
    const reopened=examplesUI('#words/examples/family',{core,storage}),saved=storage.getItem(EXAMPLES_KEY);
    const romanized=reopened.$('#example-romanized-view').children.find(node=>node.textContent==='neak');romanized.onclick();
    const candidate=exampleCandidate(reopened,'នាក់');assert(candidate);
    i18n.setUiLanguage('km');reopened.run('route();');
    assert.equal(reopened.$('#example-romanized-view').children.find(node=>node.textContent==='neak'),romanized);
    exampleCandidate(reopened,'នាក់').onclick();reopened.$('#example-cancel').onclick();
    assert.equal(reopened.$('#example-khmer').textContent,selected.khmer);assert.equal(storage.getItem(EXAMPLES_KEY),saved);
    assert.equal(reopened.$('#example-romanized').value,original.romanized);
  }finally{core.dispose();i18n.setUiLanguage('en');}
});

test('the editor queries knho|m as knhom and accepts its full-word correction without blocking native Space or selection',async()=>{
  const core=await createEngine(),storage=memoryStorage(),calls=[];
  const created=addExample('Caret',{romanized:'knhom',khmer:'knhom',spans:[]},storage,'custom-caret');
  const engine={getWords:()=>core.getWords(),suggest(input,options){calls.push(input);return core.suggest(input,options);}};
  try{
    const page=examplesUI(`#words/examples/${created.topic.id}`,{core:engine,storage}),input=page.$('#example-romanized');
    page.$('#example-edit').onclick();input.setSelectionRange(4,4);const writes=input.writes;input.emit('click');
    assert.equal(calls.at(-1),'knhom');assert.equal(input.selectionStart,4);assert.equal(input.writes,writes);
    assert.equal(page.$('#example-suggestion-title').textContent,'Suggestions for knhom');
    assert.equal(input.emit('keydown',{key:' '}).prevented,false);
    input.setSelectionRange(1,4,'backward');input.emit('select');assert.equal(calls.at(-1),'knhom');
    assert.equal(input.selectionDirection,'backward');
    const candidate=exampleCandidate(page,'ខ្ញុំ');assert(candidate);
    assert.equal(candidate.emit('mousedown').prevented,true);candidate.onclick();
    assert.equal(input.value,'khnhom');assert.equal(input.selectionStart,6);assert.equal(page.$('#example-khmer').textContent,'ខ្ញុំ');
    assert.equal(page.$('#example-save').disabled,false);page.$('#example-save').onclick();
    assert.equal(loadExample(created.topic,storage).spans[0].word,'ខ្ញុំ');
  }finally{core.dispose();}
});

test('stale picker clicks and composition events never apply a suggestion to a different cursor word',async()=>{
  const core=await createEngine(),storage=memoryStorage();
  const value={romanized:'neak knhom',khmer:'អ្នក knhom',spans:[{start:0,end:4,input:'neak',word:'អ្នក'}]};
  const created=addExample('Stale selection',value,storage,'custom-stale');
  try{
    const page=examplesUI(`#words/examples/${created.topic.id}`,{core,storage}),input=page.$('#example-romanized');
    page.$('#example-edit').onclick();input.setSelectionRange(2,2);input.emit('click');const stale=exampleCandidate(page,'នាក់');assert(stale);
    input.setSelectionRange(8,8);input.emit('keyup',{key:'ArrowLeft'});stale.onclick();
    assert.equal(input.value,value.romanized);assert.equal(page.$('#example-khmer').textContent,value.khmer);
    const pending=exampleCandidate(page,'ខ្ញុំ');input.emit('compositionstart');
    assert.equal(page.$('#example-suggestion-panel').hidden,true);pending.onclick();
    assert.equal(input.value,value.romanized);assert.equal(storage.getItem(EXAMPLES_KEY).includes('custom-stale'),true);
    input.emit('compositionend');assert.equal(input.value,value.romanized);
    page.$('#example-cancel').onclick();assert.equal(page.$('#example-khmer').textContent,value.khmer);
  }finally{core.dispose();}
});

test('canceled composition and unchanged native replacements preserve saved and unsaved choices',async()=>{
  const core=await createEngine(),storage=memoryStorage(),created=addExample('Canceled IME',repeatedExample,storage,'custom-canceled-ime');
  try{
    const page=examplesUI(`#words/examples/${created.topic.id}`,{core,storage}),input=page.$('#example-romanized'),stored=storage.getItem(EXAMPLES_KEY);
    page.$('#example-edit').onclick();input.setSelectionRange(7,7);
    input.emit('compositionstart');input.emit('compositionend',{data:''});
    assert.equal(input.value,repeatedExample.romanized);assert.equal(page.$('#example-khmer').textContent,repeatedExample.khmer);
    assert.equal(page.$('#example-save').disabled,true);assert.equal(storage.getItem(EXAMPLES_KEY),stored);
    nativeExampleEdit(input,5,9,input.value,9,'insertReplacementText');
    assert.equal(page.$('#example-khmer').textContent,repeatedExample.khmer);assert.equal(page.$('#example-save').disabled,true);
    // An unsaved explicit choice survives an IME preview that returns to the same text.
    input.setSelectionRange(2,2);input.emit('click');exampleCandidate(page,'នាក់').onclick();
    assert.equal(page.$('#example-khmer').textContent,'នាក់នាក់');assert.equal(page.$('#example-save').disabled,false);
    input.setSelectionRange(6,8,'backward');input.emit('compositionstart');
    input.value='neak nxxk';input.emit('input',{inputType:'insertCompositionText',isComposing:true});
    input.value=repeatedExample.romanized;input.setSelectionRange(6,8,'backward');const writes=input.writes;
    input.emit('compositionend',{data:''});
    // Some browsers deliver one final input after compositionend.
    input.emit('input',{inputType:'insertCompositionText',isComposing:false});
    assert.equal(input.writes,writes);assert.equal(input.selectionStart,6);assert.equal(input.selectionEnd,8);assert.equal(input.selectionDirection,'backward');
    assert.equal(page.$('#example-khmer').textContent,'នាក់នាក់');assert.equal(page.$('#example-save').disabled,false);
    page.$('#example-save').onclick();const saved=loadExample(created.topic,storage);
    assert.equal(saved.khmer,'នាក់នាក់');assert.deepEqual(saved.spans.map(span=>span.word),['នាក់','នាក់']);
    const reopened=examplesUI(`#words/examples/${created.topic.id}`,{core,storage});
    assert.equal(reopened.$('#example-khmer').textContent,saved.khmer);
    reopened.$('#example-edit').onclick();reopened.$('#example-cancel').onclick();
    assert.equal(reopened.$('#example-khmer').textContent,saved.khmer);
  }finally{core.dispose();}
});

test('native Undo and Redo restore saved and unsaved word choices without rewriting text or selection',async()=>{
  const core=await createEngine(),storage=memoryStorage(),created=addExample('Undo choices',repeatedExample,storage,'custom-undo-choices');
  try{
    const page=examplesUI(`#words/examples/${created.topic.id}`,{core,storage}),input=page.$('#example-romanized');
    page.$('#example-edit').onclick();nativeExampleEdit(input,8,8,'neak neaxk',9,'insertText');
    const changed=page.$('#example-khmer').textContent;assert.notEqual(changed,repeatedExample.khmer);
    nativeExampleEdit(input,9,9,repeatedExample.romanized,8,'historyUndo');
    assert.equal(page.$('#example-khmer').textContent,repeatedExample.khmer);assert.equal(page.$('#example-save').disabled,true);
    nativeExampleEdit(input,8,8,'neak neaxk',9,'historyRedo');assert.equal(page.$('#example-khmer').textContent,changed);
    // Browsers may omit beforeinput for history operations.
    input.value=repeatedExample.romanized;input.setSelectionRange(6,8,'backward');const writes=input.writes;
    assert.equal(input.emit('input',{inputType:'historyUndo'}).prevented,false);
    assert.equal(input.writes,writes);assert.equal(input.selectionStart,6);assert.equal(input.selectionEnd,8);assert.equal(input.selectionDirection,'backward');
    assert.equal(page.$('#example-khmer').textContent,repeatedExample.khmer);
    input.setSelectionRange(2,2);input.emit('click');exampleCandidate(page,'នាក់').onclick();
    assert.equal(page.$('#example-khmer').textContent,'នាក់នាក់');
    nativeExampleEdit(input,1,1,'nxeak neak',2,'insertText');
    nativeExampleEdit(input,2,2,repeatedExample.romanized,1,'historyUndo');
    assert.equal(page.$('#example-khmer').textContent,'នាក់នាក់');assert.equal(page.$('#example-save').disabled,false);
    page.$('#example-save').onclick();const saved=loadExample(created.topic,storage);
    assert.equal(saved.khmer,'នាក់នាក់');assert.deepEqual(saved.spans.map(span=>span.word),['នាក់','នាក់']);
    const reopened=examplesUI(`#words/examples/${created.topic.id}`,{core,storage});
    assert.equal(reopened.$('#example-khmer').textContent,saved.khmer);
    // Saving ends editing without discarding the browser's existing text history.
    page.$('#example-edit').onclick();nativeExampleEdit(input,1,1,'nxeak neak',2,'insertText');page.$('#example-save').onclick();
    page.$('#example-edit').onclick();nativeExampleEdit(input,2,2,repeatedExample.romanized,1,'historyUndo');
    assert.equal(page.$('#example-khmer').textContent,saved.khmer);assert.equal(page.$('#example-save').disabled,false);
    page.$('#example-save').onclick();assert.equal(loadExample(created.topic,storage).khmer,saved.khmer);
  }finally{core.dispose();}
});

test('grouped history restores the nearest matching choices, branches after Undo and stays local to each example',async()=>{
  const core=await createEngine(),storage=memoryStorage(),created=addExample('History groups',repeatedExample,storage,'custom-history-groups');
  try{
    const page=examplesUI(`#words/examples/${created.topic.id}`,{core,storage}),input=page.$('#example-romanized');
    page.$('#example-edit').onclick();
    nativeExampleEdit(input,5,9,'neak',4,'deleteByCut');
    nativeExampleEdit(input,4,4,'neak n',6,'insertText');
    nativeExampleEdit(input,6,6,'neak ne',7,'insertText');
    nativeExampleEdit(input,7,7,'neak neak',9,'insertText');
    // The same Latin paragraph now has a new automatically selected second word.
    const retyped=page.$('#example-khmer').textContent;assert.notEqual(retyped,repeatedExample.khmer);
    nativeExampleEdit(input,9,9,'neak',4,'historyUndo');
    nativeExampleEdit(input,4,4,repeatedExample.romanized,9,'historyUndo');
    assert.equal(page.$('#example-khmer').textContent,repeatedExample.khmer);
    nativeExampleEdit(input,9,9,'neak',4,'historyRedo');
    nativeExampleEdit(input,4,4,repeatedExample.romanized,9,'historyRedo');
    assert.equal(page.$('#example-khmer').textContent,retyped);
    // A new edit after Undo discards the old future, even if its text is reused.
    nativeExampleEdit(input,9,9,'neak',4,'historyUndo');
    nativeExampleEdit(input,4,4,repeatedExample.romanized,9,'historyUndo');
    input.setSelectionRange(2,2);input.emit('click');exampleCandidate(page,'នាក់').onclick();
    nativeExampleEdit(input,8,8,'neak neaxk',9,'insertText');
    nativeExampleEdit(input,9,9,repeatedExample.romanized,8,'historyUndo');
    assert.equal(page.$('#example-khmer').textContent,'នាក់នាក់');
    // Locale changes and returning from the topic list preserve the active history.
    i18n.setUiLanguage('km');page.run('route();');page.location.hash='#words/examples';page.run('route();');
    page.location.hash=`#words/examples/${created.topic.id}`;page.run('route();');
    nativeExampleEdit(input,8,8,'neak neaxk',9,'historyRedo');
    nativeExampleEdit(input,9,9,repeatedExample.romanized,8,'historyUndo');
    assert.equal(page.$('#example-khmer').textContent,'នាក់នាក់');
    page.$('#example-cancel').onclick();assert.equal(page.$('#example-khmer').textContent,repeatedExample.khmer);
    // A different textarea value installed by navigation starts a fresh history.
    page.location.hash='#words/examples/food';page.run('route();');page.location.hash=`#words/examples/${created.topic.id}`;page.run('route();');
    page.$('#example-edit').onclick();nativeExampleEdit(input,8,8,'neak neaxk',9,'insertText');
    nativeExampleEdit(input,9,9,repeatedExample.romanized,8,'historyUndo');
    assert.equal(page.$('#example-khmer').textContent,repeatedExample.khmer);
  }finally{core.dispose();i18n.setUiLanguage('en');}
});

test('Undo restores choices after composed edits and conversion failures, and Redo preserves the failed raw input',async()=>{
  const core=await createEngine(),storage=memoryStorage(),created=addExample('Undo recovery',repeatedExample,storage,'custom-undo-recovery');
  const engine={suggest(input,options){if(input==='badinput')throw new Error('Temporary conversion failure');return core.suggest(input,options);}};
  try{
    const page=examplesUI(`#words/examples/${created.topic.id}`,{core:engine,storage}),input=page.$('#example-romanized');
    page.$('#example-edit').onclick();input.setSelectionRange(8,8);input.emit('compositionstart');
    input.value='neak neaxk';input.setSelectionRange(9,9);input.emit('input',{inputType:'insertCompositionText'});input.emit('compositionend');
    nativeExampleEdit(input,9,9,repeatedExample.romanized,8,'historyUndo');
    assert.equal(page.$('#example-khmer').textContent,repeatedExample.khmer);
    nativeExampleEdit(input,5,9,'neak badinput',13,'insertFromPaste');
    assert.match(page.$('#example-error').textContent,/Temporary conversion failure/);assert.equal(page.$('#example-save').disabled,true);
    nativeExampleEdit(input,13,13,repeatedExample.romanized,9,'historyUndo');
    assert.equal(page.$('#example-error').textContent,'');assert.equal(page.$('#example-khmer').textContent,repeatedExample.khmer);
    nativeExampleEdit(input,9,9,'neak badinput',13,'historyRedo');
    assert.equal(input.value,'neak badinput');assert.match(page.$('#example-error').textContent,/Temporary conversion failure/);
    assert.equal(page.$('#example-save').disabled,true);assert.equal(page.$('#example-suggestion-panel').hidden,true);
    nativeExampleEdit(input,13,13,repeatedExample.romanized,9,'historyUndo');
    page.$('#example-save').onclick();assert.equal(loadExample(created.topic,storage).khmer,repeatedExample.khmer);
  }finally{core.dispose();}
});

test('choice history stays bounded and retains the initial and latest states for grouped native Undo and Redo',()=>{
  const storage=memoryStorage(),created=addExample('Long edit group',repeatedExample,storage,'custom-long-history');
  const page=examplesUI(`#words/examples/${created.topic.id}`,{core:{suggest:()=>({candidates:[]})},storage}),input=page.$('#example-romanized');
  page.$('#example-edit').onclick();
  for(let index=1;index<=320;index++){
    const start=input.value.length;nativeExampleEdit(input,start,start,repeatedExample.romanized+'x'.repeat(index),start+1,'insertText');
  }
  const edited=input.value,preview=page.$('#example-khmer').textContent;
  assert(page.run(`exampleDrafts.get('${created.topic.id}').inputHistory.entries.length`)<=256);
  assert(page.run(`exampleDrafts.get('${created.topic.id}').inputHistory.size`)<=2*1024*1024);
  nativeExampleEdit(input,edited.length,edited.length,repeatedExample.romanized,9,'historyUndo');
  assert.equal(page.$('#example-khmer').textContent,repeatedExample.khmer);assert.equal(page.$('#example-save').disabled,true);
  nativeExampleEdit(input,9,9,edited,edited.length,'historyRedo');assert.equal(page.$('#example-khmer').textContent,preview);
  page.$('#example-cancel').onclick();assert.equal(input.value,repeatedExample.romanized);assert.equal(page.$('#example-khmer').textContent,repeatedExample.khmer);
});

test('conversion failure preserves raw edits and blocks old preview and picker actions until recovery or Cancel',async()=>{
  const core=await createEngine(),storage=memoryStorage();
  const created=addExample('Conversion recovery',repeatedExample,storage,'custom-error-recovery'),saved=storage.getItem(EXAMPLES_KEY);
  const engine={suggest(input,options){if(input==='badinput')throw new Error('Temporary conversion failure');return core.suggest(input,options);}};
  try{
    const page=examplesUI(`#words/examples/${created.topic.id}`,{core:engine,storage}),input=page.$('#example-romanized');
    page.$('#example-edit').onclick();input.setSelectionRange(2,2);input.emit('click');
    const stale=exampleCandidate(page,'នាក់'),preview=page.$('#example-khmer').children.find(node=>node.exampleToken);assert(stale&&preview);
    nativeExampleEdit(input,0,0,'badinput neak neak',9,'insertFromPaste');
    assert.match(page.$('#example-error').textContent,/Temporary conversion failure/);
    assert.equal(page.$('#example-suggestion-panel').hidden,true);assert.equal(page.$('#example-save').disabled,true);
    assert.equal(preview.disabled,true);preview.onclick();stale.onclick();
    assert.equal(input.value,'badinput neak neak');assert.equal(page.$('#example-suggestion-panel').hidden,true);
    page.$('#example-save').onclick();assert.equal(storage.getItem(EXAMPLES_KEY),saved);assert.equal(input.value,'badinput neak neak');
    input.setSelectionRange(1,4,'backward');const writes=input.writes;
    for(const locale of ['km','en']){
      i18n.setUiLanguage(locale);page.run('route();');
      assert.equal(input.value,'badinput neak neak');assert.equal(input.writes,writes);
      assert.equal(input.selectionStart,1);assert.equal(input.selectionEnd,4);assert.equal(input.selectionDirection,'backward');
    }
    page.location.hash='#words/examples';page.run('route();');page.location.hash=`#words/examples/${created.topic.id}`;page.run('route();');
    assert.equal(input.value,'badinput neak neak');assert.equal(preview.disabled,true);
    // Another edit while conversion still fails must retain the raw-text span basis.
    nativeExampleEdit(input,0,0,'neak badinput neak neak',5,'insertFromPaste');
    assert.equal(page.$('#example-save').disabled,true);assert.equal(input.value,'neak badinput neak neak');
    nativeExampleEdit(input,5,14,'neak neak neak',5,'deleteByCut');
    assert.equal(page.$('#example-error').textContent,'');assert.equal(page.$('#example-save').disabled,false);
    assert.equal(page.$('#example-khmer').children.find(node=>node.exampleToken).disabled,false);
    page.$('#example-save').onclick();const recovered=loadExample(created.topic,storage);
    assert.equal(recovered.romanized,'neak neak neak');
    assert.equal(recovered.spans.find(span=>span.start===5).word,'អ្នក');
    assert.equal(recovered.spans.find(span=>span.start===10).word,'នាក់');
    const reopened=examplesUI(`#words/examples/${created.topic.id}`,{core:engine,storage});
    assert.equal(reopened.$('#example-khmer').textContent,recovered.khmer);
    page.$('#example-edit').onclick();nativeExampleEdit(input,0,0,'badinput '+recovered.romanized,9,'insertFromPaste');
    page.$('#example-cancel').onclick();assert.equal(input.value,recovered.romanized);
    assert.equal(page.$('#example-khmer').textContent,recovered.khmer);assert.equal(page.$('#example-error').textContent,'');
    assert.equal(loadExample(created.topic,storage).romanized,recovered.romanized);
  }finally{core.dispose();i18n.setUiLanguage('en');}
});

test('a failed read-only word-choice save retains the choice and can be retried',async()=>{
  const core=await createEngine(),storage=memoryStorage(),page=examplesUI('#words/examples/family',{core,storage});
  try{
    page.$('#example-khmer').children.find(node=>node.textContent==='នាក់').onclick();
    exampleCandidate(page,'អ្នក').onclick();const preview=page.$('#example-khmer').textContent;
    const write=storage.setItem;storage.setItem=()=>{throw new Error('Storage full');};
    page.$('#example-save').onclick();assert.match(page.$('#example-error').textContent,/Storage full/);
    assert.equal(page.$('#example-khmer').textContent,preview);assert.equal(page.$('#example-actions').hidden,false);
    assert.equal(page.$('#example-save').disabled,false);assert.equal(storage.getItem(EXAMPLES_KEY),null);
    storage.setItem=write;page.$('#example-save').onclick();
    assert.equal(page.$('#example-status').textContent,'Saved on this device.');
    assert.equal(page.$('#example-khmer').textContent,preview);assert.equal(page.$('#example-actions').hidden,true);
    assert.equal(loadExample(exampleTopics.find(topic=>topic.id==='family'),storage).khmer,preview);
  }finally{core.dispose();}
});

test('closing suggestions leaves the editor selection intact and does not reopen the picker on focus',async()=>{
  const core=await createEngine(),page=examplesUI('#words/examples/family',{core});
  try{
    page.$('#example-edit').onclick();const input=page.$('#example-romanized'),text=input.value;
    const start=text.indexOf('neak');input.setSelectionRange(start+1,start+3,'backward');input.emit('select');
    assert.equal(page.$('#example-suggestion-panel').hidden,false);
    const writes=input.writes;let refocused=false;
    input.focus=()=>{refocused=true;input.emit('focus');};
    page.$('#example-suggestions-close').onclick();
    assert.equal(page.$('#example-suggestion-panel').hidden,true);assert.equal(refocused,false);
    assert.equal(input.value,text);assert.equal(input.writes,writes);
    assert.equal(input.selectionStart,start+1);assert.equal(input.selectionEnd,start+3);assert.equal(input.selectionDirection,'backward');
    input.emit('click');assert.equal(page.$('#example-suggestion-panel').hidden,false);
  }finally{core.dispose();}
});


test('Back restores the Examples list scroll position without changing it during locale rerenders',()=>{
  const page=examplesUI('#words/examples');
  page.window.scrollY=1250;page.windowListeners.scroll();
  page.location.hash='#words/examples/food';page.run("route({type:'hashchange'});");
  assert.equal(page.window.scrollY,0);
  page.window.scrollY=350;page.windowListeners.scroll();
  page.location.hash='#words/examples';page.run("route({type:'hashchange'});");
  assert.equal(page.window.scrollY,1250);
  page.run('route();');assert.equal(page.window.scrollY,1250);
  page.location.hash='#words/examples/new';page.run("route({type:'hashchange'});");
  assert.equal(page.window.scrollY,0);
  page.location.hash='#words/examples';page.run("route({type:'hashchange'});");
  assert.equal(page.window.scrollY,1250);
});

test('example header arrows follow list order and preserve drafts between examples',()=>{
  const page=examplesUI(`#words/examples/${exampleTopics[0].id}`);
  assert.equal(page.$('#example-navigation').hidden,false);
  assert.equal(page.$('#example-previous').disabled,true);
  assert.equal(page.$('#example-next').disabled,false);
  page.run(`exampleDrafts.get('${exampleTopics[0].id}').status='unsaved marker';`);
  page.$('#example-next').onclick();
  assert.equal(page.location.hash,`#words/examples/${exampleTopics[1].id}`);
  page.run("route({type:'hashchange'});");
  page.$('#example-previous').onclick();page.run("route({type:'hashchange'});");
  assert.equal(page.run(`exampleDrafts.get('${exampleTopics[0].id}').status`),'unsaved marker');
  page.location.hash=`#words/examples/${exampleTopics.at(-1).id}`;page.run('route();');
  assert.equal(page.$('#example-next').disabled,true);
  page.location.hash='#words/examples';page.run('route();');
  assert.equal(page.$('#example-navigation').hidden,true);
});
