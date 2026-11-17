import * as i18n from '../i18n.js';
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import * as rules from '../sound-rules.js';
import {pronunciationSection} from '../pronunciations.js';

// Exercise the actual page handlers without opening a browser or changing data.
function soundPage(map,{catalog=[]}={}){
  const document={activeElement:null};
  class Element{
    constructor(tag){this.tagName=tag.toUpperCase();this.children=[];this.attributes={};this.className='';this.listeners={};this.scrollTop=0;this._text='';this.value='';}
    get textContent(){return this._text+this.children.map(node=>node.textContent).join('');}
    set textContent(value){this._text=value;this.replaceChildren();}
    get isConnected(){return this.root||this.parentNode?.isConnected||false;}
    append(...nodes){for(const node of nodes){node.remove();node.parentNode=this;this.children.push(node);}}
    remove(){if(this.parentNode){const siblings=this.parentNode.children;siblings.splice(siblings.indexOf(this),1);this.parentNode=null;}}
    replaceChildren(...nodes){for(const child of this.children)child.parentNode=null;this.children=[];this.append(...nodes);}
    replaceWith(node){const parent=this.parentNode,index=parent.children.indexOf(this);node.remove();parent.children[index]=node;node.parentNode=parent;this.parentNode=null;}
    setAttribute(name,value){this.attributes[name]=value;}
    getAttribute(name){return this.attributes[name]??null;}
    matches(selector){return selector.startsWith('.')?this.className.split(' ').includes(selector.slice(1)):this.tagName===selector.toUpperCase();}
    querySelectorAll(selector){return this.children.flatMap(child=>[...(child.matches(selector)?[child]:[]),...child.querySelectorAll(selector)]);}
    querySelector(selector){return this.querySelectorAll(selector)[0];}
    closest(selector){return this.matches(selector)?this:this.parentNode?.closest(selector);}
    addEventListener(type,fn){(this.listeners[type]??=[]).push(fn);}
    focus(){document.activeElement=this;}
    showModal(){this.open=true;this.shows=(this.shows??0)+1;}
    close(){this.open=false;for(const fn of this.listeners.close??[])fn();}
  }
  document.createElement=tag=>new Element(tag);
  const nodes=new Map(),$=selector=>{
    if(!nodes.has(selector)){const node=new Element('div');node.root=true;node.value=selector==='#sound-group'?'all':'';nodes.set(selector,node);}
    return nodes.get(selector);
  };
  const el=(tag,text,className)=>{const node=document.createElement(tag);if(text!==undefined)node.textContent=text;if(className)node.className=className;return node;};
  const context=vm.createContext({...i18n,...rules,renderRuleExamples:examples=>rules.renderRuleExamples(examples,document),pronunciationSection,engine:null,document,$,el,map,dictionaryWords:{khmer:catalog},dictionaryError:''});
  const source=readFileSync(new URL('../app.js',import.meta.url),'utf8');
  vm.runInContext(source.slice(source.indexOf('const groups='),source.indexOf('async function start()')),context);
  const run=code=>vm.runInContext(code,context);
  run('renderMap()');
  return {$,run,document};
}
const fixture=()=>({
  consonants:[
    {khmer:'ផ',series:'a',onsetKeys:['ph','p'],codaFallbackKeys:['ph','p']},
    {khmer:'ភ',series:'o',onsetKeys:['ph','p'],codaFallbackKeys:['ph','p']},
    {khmer:'ក',series:'a',onsetKeys:['k']},
    {khmer:'ស',series:'a',onsetKeys:['s'],codaFallbackKeys:['h','s']},
  ],
  dependentVowels:[{khmer:'ិ',aSeriesKeys:['i','e'],oSeriesKeys:['i','e']}],additionalVowelForms:[],independentVowels:[],
  runtime:{wordReadings:[{kind:'scoped',khmer:'អ្នក',keys:['neak'],words:['អ្នក','អ្នកនេសាទ'],conditions:{enabled:false,targetSeries:'a'}}]},
});
test('the Pronunciation link sits immediately above Rules and stays available when filters show no rules',()=>{
  const page=soundPage(fixture()),cards=page.$('#sound-cards'),link=page.$('#pronunciation-open');
  assert.equal(cards.children[cards.children.indexOf(link)+1],cards.querySelector('.sound-rules'));
  page.$('#sound-group').value='rules';page.run('renderMap()');
  assert.equal(cards.children[0],link);
  page.$('#sound-search').value='no match';page.run('renderMap()');
  assert.equal(cards.querySelector('.sound-rules'),undefined);
  assert.equal(cards.children[0],link);
});
test('the nh row renders under Shorthands rather than Word-part readings',()=>{
  const map=JSON.parse(readFileSync(new URL('../../data/khmer-sound-map.json',import.meta.url)));
  const page=soundPage(map),cards=page.$('#sound-cards');
  const families=cards.querySelectorAll('.sound-family');
  const shorthand=families.find(node=>node.getAttribute('data-sound-detail')==='family/shorthands');
  const parts=families.find(node=>node.getAttribute('data-sound-detail')==='family/parts');
  assert(shorthand.textContent.includes('ខ្ញុំ'));assert(shorthand.textContent.includes('nh'));
  assert(!parts.textContent.includes('ខ្ញុំ'));
  const row=shorthand.querySelectorAll('button').find(button=>button.textContent.includes('ខ្ញុំ'));
  assert(row);row.onclick();assert.equal(page.$('#rule-details').open,true);
  assert(page.$('#rule-detail-content').textContent.includes('nh'));
});

test('rules use one dialog and character details appear only for distinct final keys',()=>{
  const map=fixture(),before=JSON.stringify(map),page=soundPage(map),cards=page.$('#sound-cards');
  const details=cards.querySelectorAll('details').filter(node=>node.querySelector('summary')?.textContent==='Character details');
  assert.deepEqual(details.map(node=>node.getAttribute('data-sound-detail')),['character/consonants/ស']);
  assert.deepEqual(details[0].querySelectorAll('.key').map(node=>node.textContent),['h','s']);
  for(const letter of ['ផ','ភ']){
    const card=cards.querySelectorAll('.sound-card').find(node=>node.querySelector('.glyph')?.textContent===letter);
    assert.deepEqual(card.querySelectorAll('.key').map(node=>node.textContent),['ph','p']);
    assert.equal(card.querySelector('details'),undefined);
  }
  const rule=cards.querySelectorAll('button').find(node=>node.getAttribute('data-rule-trigger')==='rule/wordReadings');assert(rule);
  rule.onclick();const dialog=page.$('#rule-details');assert.equal(dialog.open,true);
  const item=page.$('#rule-detail-content').querySelector('button');assert(item);item.onclick();
  assert(page.$('#rule-detail-content').textContent.includes('អ្នកនេសាទ'));
  assert.equal(dialog.shows,1);assert.equal(page.$('#rule-detail-content').querySelectorAll('details').length,0);
  page.$('#rule-detail-content').querySelector('button').onclick();
  assert(page.$('#rule-detail-content').textContent.includes('No'));
  page.$('#rule-detail-back').onclick();page.$('#rule-detail-back').onclick();
  assert.equal(page.$('#rule-detail-back').hidden,true);
  page.$('#rule-detail-close').onclick();assert.equal(dialog.open,false);assert.equal(page.document.activeElement,rule);
  assert.equal(JSON.stringify(map),before);
});

test('all rule fields remain reachable without nested disclosures or changing their scopes',()=>{
  const map=JSON.parse(readFileSync(new URL('../../data/khmer-sound-map.json',import.meta.url))),before=JSON.stringify(map);
  const page=soundPage(map),cards=page.$('#sound-cards');
  const list=cards.querySelector('.sound-rules');assert.equal(list.tagName,'SECTION');
  assert(list.querySelectorAll('details').every(node=>node.className==='sound-family'));
  assert.equal(list.querySelectorAll('.sound-family').length,rules.ruleFamilies(map.runtime).length);
  const text=[];let views=0;
  function visit(depth=0){
    assert(depth<20);views++;
    const content=page.$('#rule-detail-content');text.push(content.textContent);
    assert.equal(content.querySelectorAll('details').length,0);
    for(const child of content.querySelectorAll('button')){child.onclick();visit(depth+1);page.$('#rule-detail-back').onclick();}
  }
  for(const button of list.querySelectorAll('button')){button.onclick();visit();page.$('#rule-detail-close').onclick();}
  const rendered=text.join(' ');
  function leaves(value){return value&&typeof value==='object'?Object.values(value).flatMap(leaves):[value];}
  for(const value of leaves(map.runtime)){
    const expected=value===null?'Not set':value===true?'Yes':value===false?'No':String(value);
    assert(rendered.includes(expected),expected);
  }
  assert(views>Object.keys(map.runtime).length);
  page.$('#sound-search').value='ពេទ្យ';page.run('renderMap()');
  assert.equal(cards.querySelector('.sound-family').open,true);
  assert(cards.querySelectorAll('button').length>0);
  assert.equal(JSON.stringify(map),before);
});

test('language switches preserve open and closed character and family groups',()=>{
  const map=fixture(),before=JSON.stringify(map),page=soundPage(map),cards=page.$('#sound-cards');
  // Preserve explicit closures too, including sections that search opens by default.
  page.$('#sound-group').value='rules';page.run('renderMap();');
  cards.querySelector('.sound-family').open=false;
  i18n.setUiLanguage('km');page.run('renderMap({preserveDetails:true});');
  assert.equal(cards.querySelector('.sound-family').open,false);
  i18n.setUiLanguage('en');page.$('#sound-group').value='all';page.run('renderMap();');
  const details=cards.querySelectorAll('details');
  const expected=new Map(details.map((node,index)=>{node.open=index%2===0;return [node.getAttribute('data-sound-detail'),node.open];}));
  assert.equal(expected.size,details.length);assert(!expected.has(null));
  try{
    for(const locale of ['km','en','km','en']){
      i18n.setUiLanguage(locale);page.run('renderMap({preserveDetails:true});');
      assert.deepEqual(new Map(cards.querySelectorAll('details').map(node=>[node.getAttribute('data-sound-detail'),Boolean(node.open)])),expected);
      assert.equal(JSON.stringify(map),before);
    }
    // A new search still opens its matching families normally.
    page.$('#sound-search').value='អ្នក';page.run('renderMap();');
    assert.equal(cards.querySelector('.sound-family').open,true);
  }finally{i18n.setUiLanguage('en');}
});

test('filtered word-part details keep their path and return focus when switching languages',()=>{
  const map=fixture();map.runtime={wordParts:[
    {khmer:'ក',cases:[{kind:'scoped',words:['កាត']}]},
    {khmer:'ខ',cases:[{kind:'scoped',words:['ពាក្យ']}]},
  ]};
  const before=JSON.stringify(map);
  try{
    i18n.setUiLanguage('km');const page=soundPage(map),cards=page.$('#sound-cards');
    page.$('#sound-search').value='ពាក្យ';page.run('renderMap();');
    const item=cards.querySelectorAll('button').find(node=>node.querySelector('.rule-link-title')?.textContent==='ខ');
    item.onclick();const key=item.getAttribute('data-rule-trigger');
    page.$('#rule-detail-content').querySelector('button').onclick();
    assert.equal(page.run('ruleDetailPath.at(-1).value'),map.runtime.wordParts[1].cases[0]);
    i18n.setUiLanguage('en');page.run('renderMap({preserveDetails:true});');
    const remaining=cards.querySelectorAll('button').find(node=>node.getAttribute('data-rule-trigger')===key);
    assert(remaining);assert.equal(page.$('#rule-details').open,true);assert.equal(page.$('#rule-details').shows,1);
    assert.equal(page.run('ruleDetailPath.at(-1).value'),map.runtime.wordParts[1].cases[0]);
    assert.equal(page.$('#rule-detail-title').textContent,'Selected words and positions');
    page.$('#rule-detail-back').onclick();assert.equal(page.$('#rule-detail-title').textContent,'ខ');
    page.$('#rule-detail-close').onclick();assert.equal(page.document.activeElement,remaining);
    assert.equal(cards.querySelectorAll('button').some(node=>node.querySelector('.rule-link-title')?.textContent==='ក'),false);
    assert.equal(JSON.stringify(map),before);
  }finally{i18n.setUiLanguage('en');}
});

test('word-part dialogs retain standalone, scoped and overview examples from existing readings',()=>{
  const map=fixture();map.runtime={wordParts:[
    {khmer:'សាស្ត្រ',keys:['sah','sas'],cases:[{kind:'syllable'}]},
    {khmer:'កម្ម',keys:['kam'],cases:[{kind:'scoped',words:['សកម្ម','អសកម្ម']}]},
  ]};
  const catalog=[
    {word:'សាស្ត្រ',soundMapInputs:['sas','sah'],romanizations:[]},
    {word:'វិទ្យាសាស្ត្រ',soundMapInputs:['vetyeasas','vetyeasah'],romanizations:[]},
    {word:'កម្ម',soundMapInputs:[],romanizations:['kam']},
    {word:'សកម្ម',soundMapInputs:['sakam'],romanizations:[]},
    {word:'អសកម្ម',soundMapInputs:['asakam'],romanizations:[]},
  ];
  const snapshot=JSON.stringify({map,catalog}),page=soundPage(map,{catalog}),cards=page.$('#sound-cards');
  const part=word=>cards.querySelectorAll('button').find(node=>node.querySelector('.rule-link-title')?.textContent===word);
  const examples=()=>page.$('#rule-detail-content').querySelectorAll('.rule-example');
  part('សាស្ត្រ').onclick();
  assert.deepEqual(examples().map(row=>row.querySelector('.example-khmer').textContent),['សាស្ត្រ']);
  assert(examples()[0].querySelector('.example-inputs').textContent.includes('sah'));
  assert(examples()[0].querySelector('.example-inputs').textContent.includes('sas'));
  page.$('#rule-detail-close').onclick();part('កម្ម').onclick();
  assert.deepEqual(examples().map(row=>row.querySelector('.example-khmer').textContent),['កម្ម','សកម្ម','អសកម្ម']);
  assert.equal(examples()[0].querySelector('.example-inputs').textContent,'kam');
  page.$('#rule-detail-close').onclick();
  const overview=cards.querySelectorAll('button').find(node=>node.getAttribute('data-rule-trigger')==='rule/wordParts/examples');assert(overview);
  overview.onclick();
  const expected=rules.soundRules(map.runtime,'',catalog).find(rule=>rule.id==='wordParts').examples;
  assert.deepEqual(examples().map(row=>row.querySelector('.example-khmer').textContent),expected.map(row=>row.word));
  assert.equal(page.$('#rule-detail-content').querySelector('.rule-fields'),undefined);
  assert(!page.$('#rule-detail-content').textContent.includes('None configured'));
  try{
    i18n.setUiLanguage('km');page.run('renderMap({preserveDetails:true});');
    assert.equal(page.$('#rule-detail-title').textContent,i18n.t('Examples'));
    assert.deepEqual(examples().map(row=>row.querySelector('.example-khmer').textContent),expected.map(row=>row.word));
    page.$('#rule-detail-close').onclick();
    assert.equal(page.document.activeElement.getAttribute('data-rule-trigger'),'rule/wordParts/examples');
    assert.equal(JSON.stringify({map,catalog}),snapshot);
  }finally{i18n.setUiLanguage('en');}
});

test('Back restores each parent scroll position and the chosen row across multiple levels and language changes',()=>{
  const map=fixture();map.runtime.wordReadings=Array.from({length:25},(_,index)=>({
    khmer:'ផ្នែក'+index,variants:Array.from({length:20},(_,variant)=>({word:'លំនាំ'+variant})),
    conditions:{enabled:false,targetSeries:'a'},
  }));
  const snapshot=JSON.stringify(map),page=soundPage(map),dialog=page.$('#rule-details');
  const root=page.$('#sound-cards').querySelectorAll('button').find(node=>node.getAttribute('data-rule-trigger')==='rule/wordReadings');
  const buttons=()=>page.$('#rule-detail-content').querySelectorAll('button');
  root.onclick();dialog.scrollTop=420;
  const parent=buttons()[20],parentKey=parent.getAttribute('data-rule-trigger');parent.onclick();
  assert.equal(dialog.scrollTop,0);dialog.scrollTop=228;
  const child=buttons().at(-1),childKey=child.getAttribute('data-rule-trigger');child.onclick();
  assert.equal(dialog.scrollTop,0);
  try{
    i18n.setUiLanguage('km');page.run('renderMap({preserveDetails:true});');
    page.$('#rule-detail-back').onclick();
    assert.equal(dialog.scrollTop,228);
    assert.equal(page.document.activeElement.getAttribute('data-rule-trigger'),childKey);assert(page.document.activeElement.isConnected);
    page.$('#rule-detail-back').onclick();
    assert.equal(dialog.scrollTop,420);
    assert.equal(page.document.activeElement.getAttribute('data-rule-trigger'),parentKey);assert(page.document.activeElement.isConnected);
    dialog.scrollTop=160;const next=buttons()[10],nextKey=next.getAttribute('data-rule-trigger');next.onclick();
    page.$('#rule-detail-back').onclick();assert.equal(dialog.scrollTop,160);
    assert.equal(page.document.activeElement.getAttribute('data-rule-trigger'),nextKey);assert.equal(dialog.shows,1);
    page.$('#rule-detail-close').onclick();
    assert.equal(page.document.activeElement.getAttribute('data-rule-trigger'),'rule/wordReadings');
    assert.equal(JSON.stringify(map),snapshot);
  }finally{i18n.setUiLanguage('en');}
});
