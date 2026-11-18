import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import * as i18n from '../i18n.js';
import {messages,ruleGuides,familyGuides} from '../khmer-ui.js';
import {soundRules,ruleFamilies} from '../sound-rules.js';
import {createEngine} from '../../dist/public/node.js';

test('interface language is saved separately and handles unavailable or invalid storage',()=>{
  assert.equal(i18n.readUiLanguage({getItem:()=>null},'km-KH'),'km');
  assert.equal(i18n.readUiLanguage({getItem:()=>null},'fr'),'en');
  assert.equal(i18n.readUiLanguage({getItem:()=> 'en'},'km'),'en');
  assert.equal(i18n.readUiLanguage({getItem:()=> 'khmer'},'km'),'km');
  assert.equal(i18n.readUiLanguage({getItem(){throw new Error();}},'km'),'km');
  assert.notEqual(i18n.UI_LANGUAGE_KEY,'kmeow-manager-personal-v1');
});

test('a blocked localStorage getter does not prevent interface startup',()=>{
  const context=vm.createContext({navigator:{language:'km-KH'},readUiLanguage:i18n.readUiLanguage,applied:null});
  Object.defineProperty(context,'localStorage',{get(){throw new Error('Storage blocked');}});
  context.applyUiLanguage=language=>{context.applied=language;};
  const source=readFileSync(new URL('../app.js',import.meta.url),'utf8');
  vm.runInContext(source.slice(source.indexOf('let initialUiLanguage;'),source.lastIndexOf('start();')),context);
  assert.equal(context.applied,'km');
});

test('translations preserve template parameters and never translate interpolated word data',()=>{
  try{
    for(const [key,value] of Object.entries(messages)){
      const variables=text=>[...new Set([...text.matchAll(/\{(\w+)\}/g)].map(match=>match[1]))].sort();
      assert.deepEqual(variables(value),variables(key),key);assert(/[\u1780-\u17ff]/.test(value),key);
    }
    i18n.setUiLanguage('km');
    assert.equal(i18n.t('Word list'),'បញ្ជីពាក្យ');
    const label=i18n.t('Insert {word}, {states}',{word:'Copy {count}',states:i18n.t('Match')});
    assert(label.includes('Copy {count}'));assert(!label.includes(messages.Copy));
    assert.equal(i18n.t('Unknown technical diagnostic'),'Unknown technical diagnostic');
  }finally{i18n.setUiLanguage('en');}
});

test('import failures translate both the message and its cause after repeated language switches',async()=>{
  const attributes={},status={textContent:'',getAttribute:key=>attributes[key]??null,setAttribute:(key,value)=>{attributes[key]=value;}};
  const field={},document={querySelectorAll:selector=>selector==='[data-i18n]'?[status]:[]};
  const context=vm.createContext({...i18n,$:()=>field,announce:(key,values)=>i18n.setUiText(status,key,values),errorMessage:error=>error.message});
  const source=readFileSync(new URL('../app.js',import.meta.url),'utf8');
  vm.runInContext(source.slice(source.indexOf("$('#import-file').onchange="),source.indexOf('const groups=')),context);
  try{
    for(const initial of ['en','km']){
      i18n.setUiLanguage(initial);
      await field.onchange({target:{value:'file',files:[{size:2*1024*1024+1}]}});
      for(const locale of ['km','en','km','en']){
        i18n.setUiLanguage(locale);i18n.translatePage(document);
        assert.equal(status.textContent,i18n.t('Import failed: {error}',{error:i18n.t('Choose a JSON file smaller than 2 MB.')}));
      }
    }
    const message=i18n.uiMessage('Import failed: {error}',{error:i18n.uiMessage('Saved {word} to the dictionary.',{word:'Copy {count}'})});
    for(const locale of ['km','en']){
      i18n.setUiLanguage(locale);assert(i18n.t(message).includes('Copy {count}'));
    }
  }finally{i18n.setUiLanguage('en');}
});

test('static interface labels and accessibility attributes have Khmer translations',()=>{
  const html=readFileSync(new URL('../index.html',import.meta.url),'utf8');
  for(const match of html.matchAll(/data-i18n(?:-aria-label|-placeholder|-title)?="([^"]+)"/g)){
    const key=match[1].replaceAll('&quot;','"').replaceAll('&#x27;',"'").replaceAll('&amp;','&');
    assert(Object.hasOwn(messages,key),key);
  }
  assert.equal((html.match(/<button type="button" data-ui-language/g)||[]).length,2);
});

test('retranslating marked UI nodes leaves editor values and unmarked words intact',()=>{
  const node=(text,attributes={})=>({textContent:text,attributes:{...attributes},value:'do not change',getAttribute(key){return this.attributes[key]??null;},setAttribute(key,value){this.attributes[key]=value;}});
  const label=node('Word list',{'data-i18n':''}),field=node('',{'placeholder':'Your message grows here. You can also type or paste directly…','data-i18n-placeholder':'Your message grows here. You can also type or paste directly…'}),word=node('Copy');
  const nodes=[label,field,word],doc={querySelectorAll:selector=>nodes.filter(item=>item.getAttribute(selector.slice(1,-1))!==null)};
  try{
    i18n.setUiLanguage('km');i18n.translatePage(doc);
    assert.equal(label.textContent,'បញ្ជីពាក្យ');assert.equal(word.textContent,'Copy');assert.equal(field.value,'do not change');
    assert(/[\u1780-\u17ff]/.test(field.attributes.placeholder));
    i18n.setUiLanguage('en');i18n.translatePage(doc);assert.equal(label.textContent,'Word list');
  }finally{i18n.setUiLanguage('en');}
});

test('the live language switch preserves the typing language, draft, selection and filters',()=>{
  const nodes=new Map(),$=key=>{if(!nodes.has(key))nodes.set(key,{value:'',selectionStart:2,selectionEnd:5});return nodes.get(key);};
  const controls=Array.from({length:2},()=>({caption:{},querySelector(){return this.caption;},setAttribute(key,value){this[key]=value;}})),settings={language:'english',completion:true,correction:false},saved=[],mapRenders=[];
  const document={documentElement:{lang:'en',dataset:{theme:'dark'}},querySelectorAll:()=>[]};
  const draft={text:'hello កូន',input:'books',start:2,end:5,direction:'backward'},original=JSON.stringify(draft);
  $('#typing-input').value='books';$('#message').value=draft.text;$('#personal-search').value='existing filter';
  const context=vm.createContext({...i18n,$,$$:()=>controls,settings,document,localStorage:{setItem:(key,value)=>saved.push([key,value])},route(){},appearance(){},renderWordList(){},suggest(){},renderPronunciations(){},renderMap(options){mapRenders.push(options.preserveDetails);},engine:{},map:{},draft,sentenceConverter:null});
  const source=readFileSync(new URL('../app.js',import.meta.url),'utf8');
  vm.runInContext(source.slice(source.indexOf('function applyUiLanguage('),source.indexOf('function readingFor(')),context);
  try{
    i18n.setUiLanguage('en');controls[0].onclick();
    assert.equal(document.documentElement.lang,'km');assert(controls.every(control=>control.caption.textContent==='English'&&control.caption.lang==='en'));
    assert.equal(settings.language,'english');assert.equal($('#typing-input').value,'books');
    assert.equal($('#typing-input').placeholder,'សាកវាយ books');assert.equal($('#personal-search').value,'existing filter');
    assert.equal($('#message').value,draft.text);assert.equal($('#message').selectionEnd,5);assert.equal(JSON.stringify(draft),original);
    assert.deepEqual(saved,[[i18n.UI_LANGUAGE_KEY,'km']]);
    assert.deepEqual(mapRenders,[true]);
    controls[1].onclick();
    assert.equal(document.documentElement.lang,'en');
    assert(controls.every(control=>control.caption.textContent==='ខ្មែរ'&&control.caption.lang==='km'));
    assert.deepEqual(saved,[[i18n.UI_LANGUAGE_KEY,'km'],[i18n.UI_LANGUAGE_KEY,'en']]);
  }finally{i18n.setUiLanguage('en');}
});

test('Khmer rule explanations and searches retain every original configuration and word',()=>{
  const runtime=JSON.parse(readFileSync(new URL('../../data/khmer-sound-map.json',import.meta.url))).runtime;
  const snapshot=JSON.stringify(runtime),english=soundRules(runtime),families=ruleFamilies(runtime);
  for(const rule of english)assert(ruleGuides[rule.id],rule.id);
  for(const family of families)assert(familyGuides[family.id],family.id);
  try{
    i18n.setUiLanguage('km');const khmer=soundRules(runtime);
    assert.deepEqual(khmer.map(rule=>rule.id),english.map(rule=>rule.id));
    for(const rule of khmer){assert.equal(rule.value,runtime[rule.id]);assert(/[\u1780-\u17ff]/.test(rule.title));}
    assert(ruleFamilies(runtime,'ស្រៈ').some(family=>family.id==='vowels'));
    assert(soundRules(runtime,'target series').length>0);
    assert.equal(JSON.stringify(runtime),snapshot);
  }finally{i18n.setUiLanguage('en');}
});

test('interface language never changes Khmer or English candidate results and flags',async()=>{
  const engine=await createEngine();
  const cases=[['sievphov','khmer'],['angkor','khmer'],['tos','khmer'],['សៀវ','khmer'],['books','english'],['lets','english'],['c','english']];
  try{
    const before=cases.map(([input,language])=>engine.suggest(input,{language}));
    for(const locale of ['km','en','km']){
      i18n.setUiLanguage(locale);
      assert.deepEqual(cases.map(([input,language])=>engine.suggest(input,{language})),before);
    }
  }finally{engine.dispose();i18n.setUiLanguage('en');}
});
