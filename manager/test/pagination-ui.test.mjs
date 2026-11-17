import * as i18n from '../i18n.js';
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import * as words from '../word-list.js';

function page(){
  const document={activeElement:null},scrolls=[];
  class Element{
    constructor(){this.value='';this.children=[];this.dataset={};this.attributes={};}
    append(...children){this.children.push(...children);}
    replaceChildren(...children){this.children=children;}
    setAttribute(name,value){this.attributes[name]=value;}
    focus(){document.activeElement=this;}
    scrollIntoView(options){scrolls.push(options);}
  }
  const nodes=new Map(),$=selector=>{if(!nodes.has(selector))nodes.set(selector,new Element());return nodes.get(selector);};
  const buttons=['khmer','english'].map(language=>Object.assign(new Element(),{dataset:{wordLanguage:language}}));
  const catalog={khmer:Array.from({length:4561},(_,i)=>({id:i,word:`ក${String(i).padStart(4,'0')}`,partsOfSpeech:[],romanizations:[`r${i}`]})),english:Array.from({length:51},(_,i)=>({id:10000+i,word:`word${i}`,partsOfSpeech:[]}))};
  const original=JSON.stringify(catalog);
  document.createElementNS=()=>new Element();
  const context=vm.createContext({...i18n,...words,$,$$:()=>buttons,el:(tag,text,className)=>Object.assign(new Element(),{textContent:text,className}),document,
    dictionaryWords:catalog,dictionaryError:'',wordLanguage:'khmer',wordLetter:'',missingRomanized:false,wordPageIndex:0,busy:false,damagedWords:null,legacyWords:[],
    engine:{getWords:()=>[],suggest(){throw new Error('Paging must not call the typing engine.');}},map:null});
  const source=readFileSync(new URL('../app.js',import.meta.url),'utf8');
  vm.runInContext(source.slice(source.indexOf('function renderWordList('),source.indexOf('const groups=')),context);
  const run=code=>vm.runInContext(code,context);run('renderWordList()');
  return {$,buttons,scrolls,document,run,catalog,original};
}
const link=(current,number)=>current.$('#words-pages').children.find(node=>node.textContent===String(number));

test('numbered page shortcuts show exact ranges and bring new rows into view',()=>{
  const current=page();
  assert.equal(current.$('#words-range').textContent,'1–25 of 4,561 words');
  assert.equal(current.$('#words-prev').disabled,true);assert.equal(current.$('#words-next').disabled,false);
  assert.equal(link(current,1).attributes['aria-current'],'page');
  link(current,183).onclick();
  assert.equal(current.run('wordPageIndex'),182);
  assert.equal(current.$('#personal-rows').children.length,11);
  assert.equal(current.$('#words-range').textContent,'4,551–4,561 of 4,561 words');
  assert.equal(current.$('#words-next').disabled,true);
  assert.equal(current.document.activeElement,current.$('#word-results'));assert.equal(current.scrolls.length,1);
  current.$('#words-prev').onclick();assert.equal(current.run('wordPageIndex'),181);
  current.$('#words-next').onclick();assert.equal(current.run('wordPageIndex'),182);
  link(current,1).onclick();assert.equal(current.run('wordPageIndex'),0);
  const count=current.scrolls.length;link(current,1).onclick();assert.equal(current.scrolls.length,count);
  assert.equal(JSON.stringify(current.catalog),current.original);
});

test('jumping accepts whole in-range pages and rejects blank or invalid input',()=>{
  const current=page(),field=current.$('#words-page-input');let prevented=false;
  field.value='84';current.$('#words-jump').onsubmit({preventDefault(){prevented=true;}});
  assert(prevented);assert.equal(current.run('wordPageIndex'),83);
  assert.equal(link(current,84).attributes['aria-current'],'page');
  assert.equal(current.$('#words-range').textContent,'2,076–2,100 of 4,561 words');
  const before=current.scrolls.length;
  for(const invalid of ['', '0', '-1', '184', '1.5', 'text']){
    field.value=invalid;current.$('#words-jump').onsubmit({preventDefault(){}});
    assert.equal(current.run('wordPageIndex'),83,invalid);assert.equal(current.scrolls.length,before);
  }
  field.value='183';current.$('#words-jump').onsubmit({preventDefault(){}});
  assert.equal(current.run('wordPageIndex'),182);
});

test('search and language changes reset paging and clamp the jump bounds',()=>{
  const current=page();link(current,183).onclick();
  current.$('#personal-search').value='ក0001';current.$('#personal-search').oninput();
  assert.equal(current.run('wordPageIndex'),0);assert.equal(current.$('#words-page-input').max,'1');
  assert.equal(current.$('#words-range').textContent,'1–1 of 1 word');
  assert.equal(current.$('#words-page-input').disabled,true);assert.equal(current.$('#words-jump-go').disabled,true);
  current.$('#personal-search').value='no match';current.$('#personal-search').oninput();
  assert.equal(current.$('#words-range').textContent,'0 words');
  current.$('#personal-search').value='';current.$('#personal-search').oninput();
  link(current,183).onclick();current.buttons[1].onclick();
  assert.equal(current.run('wordLanguage'),'english');assert.equal(current.run('wordPageIndex'),0);
  assert.equal(current.$('#words-page-input').max,'3');assert.equal(current.$('#words-range').textContent,'1–25 of 51 words');
  assert.equal(JSON.stringify(current.catalog),current.original);
});

const letterButton=(current,letter)=>current.$('#word-letters').children.find(node=>node.dataset.letter===letter);
test('letter navigation combines with search and paging, and All keeps every word reachable',()=>{
  const current=page();
  current.catalog.khmer.push({id:9000,word:'ខ0000',partsOfSpeech:[],romanizations:['r0']});
  current.run('renderWordList();');
  assert.equal(letterButton(current,'ខ').disabled,false);assert.equal(letterButton(current,'ឍ').disabled,true);
  link(current,183).onclick();letterButton(current,'ខ').onclick();
  assert.equal(current.run('wordLetter'),'ខ');assert.equal(current.run('wordPageIndex'),0);
  assert.equal(current.$('#personal-rows').children[0].children[0].textContent,'ខ0000');
  assert.equal(current.$('#words-range').textContent,'1–1 of 1 word');
  assert.equal(letterButton(current,'ខ').attributes['aria-pressed'],'true');
  assert.equal(current.document.activeElement,current.$('#word-results'));
  current.$('#personal-search').value='r0';current.$('#personal-search').oninput();
  assert.equal(current.run('wordLetter'),'ខ');assert.equal(current.$('#personal-rows').children.length,1);
  letterButton(current,'ក').onclick();assert.equal(current.$('#personal-rows').children.length,1);
  letterButton(current,'').onclick();assert.equal(current.$('#personal-rows').children.length,2);
  current.$('#personal-search').value='';current.$('#personal-search').oninput();
  assert.equal(current.$('#words-range').textContent,'1–25 of 4,562 words');
  letterButton(current,'ក').onclick();link(current,183).onclick();
  assert.equal(current.$('#personal-rows').children.length,11);
  current.buttons[1].onclick();assert.equal(current.run('wordLetter'),'');assert.equal(current.run('wordPageIndex'),0);
  assert(letterButton(current,'W'));assert(!letterButton(current,'ក'));assert.equal(letterButton(current,'X').disabled,true);
});

test('interface switching preserves a selected letter and translates its controls',()=>{
  const current=page();letterButton(current,'ក').onclick();link(current,183).onclick();
  try{
    for(const locale of ['km','en']){
      i18n.setUiLanguage(locale);current.run('renderWordList();');
      assert.equal(current.run('wordLetter'),'ក');assert.equal(current.run('wordPageIndex'),182);
      assert.equal(letterButton(current,'').children[0].textContent,i18n.t('All'));
      assert.equal(letterButton(current,'ក').attributes['aria-label'],i18n.t('Words starting with {letter}',{letter:'ក'}));
      assert.equal(letterButton(current,'ក').children[0].lang,'km');
    }
    assert.equal(JSON.stringify(current.catalog),current.original);
  }finally{i18n.setUiLanguage('en');}
});

test('No Romanized spelling combines with letters, search and paging without changing data or typing',()=>{
  const current=page();
  current.catalog.khmer.push({id:9000,word:'កទទេ',partsOfSpeech:[]},{id:9001,word:'ខទទេ',partsOfSpeech:[]});
  const before=JSON.stringify(current.catalog);current.run('renderWordList();');link(current,183).onclick();
  const filter=current.$('#missing-romanized');filter.onclick();
  assert.equal(current.run('missingRomanized'),true);assert.equal(current.run('wordPageIndex'),0);
  assert.equal(filter.attributes['aria-pressed'],'true');assert.equal(filter.hidden,false);
  assert.equal(current.$('#words-range').textContent,'1–2 of 2 words');
  assert.equal(current.$('#words-page-input').max,'1');assert.equal(current.$('#words-next').disabled,true);
  assert.equal(letterButton(current,'ក').disabled,false);assert.equal(letterButton(current,'ខ').disabled,false);
  assert.equal(letterButton(current,'គ').disabled,true);
  letterButton(current,'ក').onclick();assert.equal(current.$('#personal-rows').children.length,1);
  assert.equal(current.$('#personal-rows').children[0].children[0].textContent,'កទទេ');
  current.$('#personal-search').value='ខ';current.$('#personal-search').oninput();
  assert.equal(current.$('#words-range').textContent,'0 words');
  current.$('#personal-search').value='ទទេ';current.$('#personal-search').oninput();
  assert.equal(current.$('#personal-rows').children.length,1);
  letterButton(current,'').onclick();assert.equal(current.$('#personal-rows').children.length,2);
  try{
    i18n.setUiLanguage('km');current.run('renderWordList();');
    assert.equal(current.run('missingRomanized'),true);assert.equal(current.$('#personal-rows').children.length,2);
  }finally{i18n.setUiLanguage('en');}
  filter.onclick();assert.equal(current.run('missingRomanized'),false);
  current.$('#personal-search').value='';current.$('#personal-search').oninput();
  assert.equal(current.$('#words-range').textContent,'1–25 of 4,563 words');
  filter.onclick();current.buttons[1].onclick();
  assert.equal(filter.hidden,true);assert.equal(current.run('missingRomanized'),false);
  assert.equal(current.$('#words-range').textContent,'1–25 of 51 words');
  current.buttons[0].onclick();assert.equal(filter.hidden,false);
  assert.equal(current.$('#words-range').textContent,'1–25 of 4,563 words');
  assert.equal(JSON.stringify(current.catalog),before);
});
