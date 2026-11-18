import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {attachRomanizedConverter} from '../converter.js';
import {createEngine} from '../../dist/public/node.js';
import {setUiLanguage} from '../i18n.js';

function page(core){
  const calls=[],managed=[];
  const words={knhom:['ខ្ញុំ'],neak:['អ្នក','នាក់'],tov:['ទៅ'],na:['ណា'],te:['ទេ']};
  let engine=core??{suggest(input,options){
    calls.push({input,options});const candidates=(words[input.toLowerCase()]??[]).map((khmer,index)=>({khmer,mappingId:index+1}));
    if(options.completion&&input==='knh')candidates.push({khmer:'ខ្ញុំ',mappingId:1,completion:'knhom'});
    return {normalizedInput:input.toLowerCase(),candidates,exactCandidates:candidates.filter(row=>!row.completion)};
  }};
  const document={activeElement:null,defaultView:{}};
  class Element{
    constructor(){this._value='';this.children=[];this.dataset={};this.attributes={};this.listeners={};this.selectionStart=0;this.selectionEnd=0;this.selectionDirection='none';this.writes=0;this.ranges=0;this.isConnected=true;}
    get value(){return this._value;}set value(value){this._value=value;this.writes++;this.selectionStart=this.selectionEnd=value.length;}
    append(...children){this.children.push(...children);}replaceChildren(...children){this.children=children;}
    setAttribute(key,value){this.attributes[key]=value;}
    addEventListener(type,listener){(this.listeners[type]??=[]).push(listener);}
    emit(type,properties={}){const event={type,currentTarget:this,prevented:false,preventDefault(){this.prevented=true;},...properties};for(const listener of this.listeners[type]??[])listener(event);return event;}
    focus(){if(document.activeElement!==this){document.activeElement=this;this.emit('focus');}}
    setSelectionRange(start,end,direction='none'){this.ranges++;this.selectionStart=start;this.selectionEnd=end;this.selectionDirection=direction;}
    select(){this.setSelectionRange(0,this.value.length);}
  }
  const nodes=new Map(),$=id=>{if(!nodes.has(id))nodes.set(id,new Element());return nodes.get(id);};
  document.querySelector=selector=>$(selector.replace('#converter-',''));document.createElement=()=>new Element();
  const view=attachRomanizedConverter({document,getEngine:()=>engine,onManage:word=>managed.push(word)});view.setActive(true);
  const source=$('input');
  function edit(text,{start=source.selectionStart,end=source.selectionEnd,caret=text.length,type='insertText'}={}){
    source.setSelectionRange(start,end);source.emit('beforeinput',{inputType:type});source.value=text;source.setSelectionRange(caret,caret);source.emit('input',{inputType:type});
  }
  const pick=word=>$('suggestions').children.find(button=>button.children[0]?.textContent===word);
  return {$,view,source,output:$('output'),calls,managed,document,edit,pick,swap:value=>{engine=value;}};
}

test('sentences use the shared engine and every caret inside a word queries its entire input',()=>{
  const current=page();current.edit('knhom neak.\ntov x');
  assert.equal(current.output.value,'ខ្ញុំអ្នក។\nទៅ x');
  for(let caret=0;caret<=5;caret++){
    current.source.setSelectionRange(caret,caret);const writes=current.source.writes,ranges=current.source.ranges;
    current.source.emit('select');assert.equal(current.calls.at(-1).input,'knhom');
    assert.equal(current.source.writes,writes);assert.equal(current.source.ranges,ranges);
  }
  for(const key of [' ','Enter','ArrowLeft','ArrowRight'])assert.equal(current.source.emit('keydown',{key}).prevented,false);
  current.source.setSelectionRange(1,9,'backward');current.source.emit('select');
  assert.equal(current.$('suggestion-panel').hidden,true);assert.equal(current.source.selectionDirection,'backward');
});

test('exact choices preserve the Romanized sentence and survive edits to repeated occurrences',()=>{
  const current=page();current.edit('neak neak');current.source.setSelectionRange(7,7);current.source.emit('click');
  current.pick('នាក់').onclick();assert.equal(current.source.value,'neak neak');assert.equal(current.output.value,'អ្នកនាក់');
  current.edit('neak',{start:0,end:5,caret:0,type:'deleteContentForward'});
  assert.equal(current.output.value,'នាក់');
  current.source.emit('beforeinput',{inputType:'historyUndo'});assert.equal(current.source.value,'neak neak');assert.equal(current.output.value,'អ្នកនាក់');
  current.source.emit('beforeinput',{inputType:'historyUndo'});assert.equal(current.output.value,'អ្នកអ្នក');
  current.source.emit('beforeinput',{inputType:'historyRedo'});assert.equal(current.output.value,'អ្នកនាក់');
});

test('selecting a converted word offers its source alternatives and changes only that word',()=>{
  const current=page();current.edit('knhom neak');
  current.source.setSelectionRange(1,1);current.source.emit('select');
  current.output.setSelectionRange(6,6);current.output.emit('click');
  assert.equal(current.calls.at(-1).input,'neak');current.pick('នាក់').onclick();
  assert.equal(current.source.value,'knhom neak');assert.equal(current.output.value,'ខ្ញុំនាក់');
  assert.equal(current.source.selectionStart,10);
});

test('a completion edits one source word and undo restores source, output and choices',()=>{
  const current=page();current.edit('knh neak',{caret:2});
  assert.equal(current.output.value,'knh អ្នក');current.pick('ខ្ញុំ').onclick();
  assert.equal(current.source.value,'knhom neak');assert.equal(current.output.value,'ខ្ញុំអ្នក');
  assert.equal(current.source.emit('keydown',{key:'z',ctrlKey:true}).prevented,true);
  assert.equal(current.source.value,'knh neak');assert.equal(current.output.value,'knh អ្នក');
  current.source.emit('keydown',{key:'z',metaKey:true,shiftKey:true});assert.equal(current.source.value,'knhom neak');
  assert.equal(current.source.emit('beforeinput',{inputType:'historyUndo'}).prevented,true);
  assert.equal(current.source.value,'knh neak');
});

test('composition waits for completion and canceled composition preserves explicit choices',()=>{
  const current=page();current.edit('neak');current.pick('នាក់').onclick();
  current.source.setSelectionRange(4,4);const calls=current.calls.length;
  current.source.emit('compositionstart');current.source.value='neakx';current.source.emit('input',{isComposing:true});
  assert.equal(current.calls.length,calls);assert.equal(current.output.value,'នាក់');assert.equal(current.$('suggestion-panel').hidden,true);
  current.source.value='neak';current.source.setSelectionRange(4,4);current.source.emit('compositionend');
  assert.equal(current.output.value,'នាក់');current.source.emit('beforeinput',{inputType:'historyUndo'});assert.equal(current.output.value,'អ្នក');
});

test('stale cursor and engine results cannot apply an old suggestion',()=>{
  const current=page();current.edit('neak tov',{caret:2});const stale=current.pick('នាក់');
  current.source.setSelectionRange(7,7);stale.onclick();assert.equal(current.output.value,'អ្នកទៅ');
  current.source.setSelectionRange(2,2);current.source.emit('select');const old=current.pick('នាក់');
  current.swap({suggest:()=>({candidates:[],exactCandidates:[]})});old.onclick();assert.equal(current.output.value,'អ្នកទៅ');
});

test('conversion errors retain native input, block stale choices and recover without dictionary edits',()=>{
  const engine={suggest(input){const candidates=input==='neak'?[{khmer:'អ្នក'}]:input==='tov'?[{khmer:'ទៅ'}]:[];return {candidates,exactCandidates:candidates};}};
  const current=page(engine);current.edit('neak');const stale=current.pick('អ្នក');
  current.swap({suggest(){throw new Error('Conversion unavailable');}});current.edit('neak tov');
  assert.equal(current.source.value,'neak tov');assert.equal(current.output.value,'');
  assert.equal(current.$('error').textContent,'Conversion unavailable');stale.onclick();assert.equal(current.source.value,'neak tov');
  current.swap(engine);current.view.refresh();assert.equal(current.output.value,'អ្នកទៅ');assert.equal(current.$('error').textContent,'');
  current.source.emit('beforeinput',{inputType:'historyUndo'});assert.equal(current.source.value,'neak');assert.equal(current.output.value,'អ្នក');
});

test('mode and interface changes preserve the draft and management uses the chosen words',()=>{
  const current=page();current.edit('neak');current.pick('នាក់').onclick();
  current.view.setActive(false);assert.equal(current.source.disabled,true);current.view.setActive(true);
  try{
    setUiLanguage('km');current.view.refresh();assert.equal(current.output.value,'នាក់');assert.equal(current.source.value,'neak');
    current.pick('នាក់').emit('contextmenu');assert.equal(current.managed[0].khmer,'នាក់');
  }finally{setUiLanguage('en');}
});

test('Khmer-only mode selection keeps independent English typing settings and both drafts',()=>{
  const nodes=new Map(),$=id=>{if(!nodes.has(id))nodes.set(id,{hidden:false,value:'',attributes:{},classList:{toggle(){}},setAttribute(key,value){this.attributes[key]=value;}});return nodes.get(id);};
  const states=[],settings={language:'khmer',conversion:false,soundMap:false,completion:false,correction:true};
  $('#typing-input').value='books';$('#message').value='existing draft';$('#converter-input').value='knhom neak';
  const context=vm.createContext({$,settings,typingMode:'typing',setTypingOptionsOpen(){},sentenceConverter:{setActive:active=>states.push(active)}});
  const source=readFileSync(new URL('../app.js',import.meta.url),'utf8');
  const start=source.indexOf('function syncTypingMode(');
  vm.runInContext(source.slice(start,source.indexOf("$$('[data-language]')",start)),context);
  $('#mode-converter').onclick();assert.equal($('#typing-pad').hidden,true);assert.equal($('#typing-options').hidden,true);
  settings.language='english';vm.runInContext('syncTypingMode()',context);assert.equal($('#typing-pad').hidden,false);assert.equal($('#typing-mode').hidden,true);
  settings.language='khmer';vm.runInContext('syncTypingMode()',context);assert.equal($('#romanized-converter').hidden,false);
  $('#mode-typing').onclick();assert.equal($('#typing-pad').hidden,false);assert.equal($('#typing-options').hidden,false);
  assert.deepEqual(states,[true,false,true,false]);assert.equal($('#typing-input').value,'books');assert.equal($('#message').value,'existing draft');assert.equal($('#converter-input').value,'knhom neak');
  assert.deepEqual(settings,{language:'khmer',conversion:false,soundMap:false,completion:false,correction:true});
});

test('real engine conversion and alternative selection leave dictionary settings and typing results intact',async()=>{
  const engine=await createEngine();
  try{
    const probes=['khnhom','neak','tov','books','angkor'];const before=probes.map(input=>engine.suggest(input)),words=engine.getWords(),settings=engine.settings;
    const current=page(engine);current.edit('khnhom neak');assert.equal(current.output.value.startsWith('ខ្ញុំ'),true);
    current.source.setSelectionRange(9,9);current.source.emit('select');
    const alternative=current.$('suggestions').children.find(button=>button.children[0]?.textContent==='នាក់');assert(alternative);alternative.onclick();
    assert.equal(current.source.value,'khnhom neak');assert.equal(current.output.value,'ខ្ញុំនាក់');
    assert.deepEqual(probes.map(input=>engine.suggest(input)),before);assert.deepEqual(engine.getWords(),words);assert.deepEqual(engine.settings,settings);
  }finally{engine.dispose();}
});
