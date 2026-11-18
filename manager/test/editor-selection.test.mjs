import * as i18n from '../i18n.js';
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import * as suggestionView from '../suggestion-view.js';
import {suggestionGesture} from '../suggestion-gesture.js';
import {suggestionWord} from '../word-list.js';
import * as composer from '../composer.js';
import {createEngine} from '../../dist/public/node.js';

// Run the page's actual event handlers with DOM event targets. Browser gestures
// are checked separately; these tests protect native events from interception.
function editorPage(engine={suggest:()=>({candidates:[{mappingId:1,khmer:'កូន'},{mappingId:2,khmer:'សៀវភៅ'}],exactCandidates:[{mappingId:1,khmer:'កូន'}]})},localStorage){
  const document={activeElement:null};
  const timers=new Map();let timerId=0;
  const schedule=fn=>{const id=++timerId;timers.set(id,fn);return id;},cancel=id=>timers.delete(id);
  class Element{
    constructor(){this.isConnected=true;this._value='';this.writes=0;this.ranges=0;this.selectionStart=0;this.selectionEnd=0;this.selectionDirection='none';this.children=[];this.listeners={};this.dataset={};this.attributes={};this.classList={toggle(){}};}
    get value(){return this._value;}
    set value(value){this._value=value;this.writes++;}
    addEventListener(type,fn){(this.listeners[type]??=[]).push(fn);}
    emit(type,props={}){const event={type,prevented:false,preventDefault(){this.prevented=true;},...props};for(const fn of this.listeners[type]??[])fn(event);return event;}
    append(...nodes){this.children.push(...nodes);}
    replaceChildren(...nodes){this.children=nodes;}
    setAttribute(name,value){this.attributes[name]=value;} focus(){document.activeElement=this;}
    showModal(){this.open=true;} close(){this.open=false;}
    setSelectionRange(start,end,direction='none'){this.ranges++;this.selectionStart=start;this.selectionEnd=end;this.selectionDirection=direction;}
  }
  const nodes=new Map(),$=selector=>{if(!nodes.has(selector))nodes.set(selector,new Element());return nodes.get(selector);};
  const $$=selector=>selector==='#suggestions .candidate'?$('#suggestions').children.filter(node=>node.className==='candidate'):[];
  const el=(tag,text,className)=>Object.assign(new Element(),{tag,textContent:text,className});
  const context=vm.createContext({...i18n,...composer,...suggestionView,suggestionWord,
    suggestionGesture:(button,actions)=>suggestionGesture(button,{...actions,schedule,cancel}),
    dictionaryWords:{},busy:false,damagedWords:null,readingFor:(candidate,result)=>suggestionView.suggestionReading(candidate,result??{},'khmer'),engine,document,$,$$,el,localStorage,settings:{language:'khmer'},announce:message=>{throw new Error(message);},language(){},openWord(row){$('#word-dialog').row=row;$('#word-dialog').showModal();},removeWord(){}});
  const source=readFileSync(new URL('../app.js',import.meta.url),'utf8');
  vm.runInContext(source.slice(source.indexOf('const editor=history();'),source.indexOf('let dictionaryWords=')),context);
  vm.runInContext(source.slice(source.indexOf('function showWordDetails('),source.indexOf('async function loadWordList(')),context);
  return {$,run:code=>vm.runInContext(code,context),hold(){for(const [id,fn] of timers){timers.delete(id);fn();}}};
}

test('suggestion instructions follow the interface language while words keep their own language and behavior',()=>{
  try{
    for(const locale of ['en','km'])for(const language of ['khmer','english']){
      i18n.setUiLanguage(locale);
      const text=language==='khmer'?'សៀវភៅ':'books';
      const page=editorPage({suggest:()=>({normalizedInput:'books',candidates:[language==='khmer'?{khmer:text,completion:'sievphov'}:{text,completion:'books'}],exactCandidates:[]})});
      page.run(`settings.language=${JSON.stringify(language)}`);
      const input=page.$('#typing-input');input.value='books';input.emit('input');
      const button=page.$('#suggestions').children[0],word=button.children.find(node=>node.className==='candidate-word');
      assert.equal(button.lang,locale);assert.equal(word.lang,language==='khmer'?'km':'en');assert.equal(word.textContent,text);
      assert(button.attributes['aria-label'].startsWith(i18n.t('Insert {word}, {states}',{word:text,states:''}).split(text)[0]));
      if(language==='khmer')assert.equal(button.children.find(node=>node.className==='candidate-reading').lang,'en');
      button.onclick();assert.equal(page.$('#message').value,text);
      input.emit('keydown',{key:'z',ctrlKey:true});assert.equal(page.$('#message').value,'');assert.equal(input.value,'books');
    }
  }finally{i18n.setUiLanguage('en');}
});

test('Shift+arrows and selection shortcuts stay native while plain arrows choose suggestions',()=>{
  const page=editorPage(),input=page.$('#typing-input');
  input.value='s';input.emit('input');
  for(const key of ['ArrowUp','ArrowDown','ArrowLeft','ArrowRight']){
    assert.equal(input.emit('keydown',{key,shiftKey:true}).prevented,false,key);
    assert.equal(page.run('active'),0);
  }
  for(const key of ['a','c','x','v'])assert.equal(input.emit('keydown',{key,ctrlKey:true}).prevented,false);
  assert.equal(input.emit('keydown',{key:'ArrowDown'}).prevented,true);assert.equal(page.run('active'),1);
});

test('native input and selection do not rewrite editor values or collapse backward ranges',()=>{
  const page=editorPage(),message=page.$('#message'),input=page.$('#typing-input');
  message.focus();message.value='hello world';message.setSelectionRange(2,8,'backward');
  const writes=message.writes,ranges=message.ranges;
  message.emit('input');message.emit('select');message.emit('keyup',{key:'ArrowLeft',shiftKey:true});
  assert.equal(message.writes,writes);assert.equal(message.ranges,ranges);assert.equal(message.selectionDirection,'backward');
  assert.equal(page.run('editor.current.start'),2);assert.equal(page.run('editor.current.end'),8);
  input.value='s';const inputWrites=input.writes;input.emit('input');
  assert.equal(input.writes,inputWrites);assert.equal(message.ranges,ranges);
});

test('blur preserves the selected replacement range without detaching the clicked suggestion',()=>{
  const page=editorPage(),message=page.$('#message'),input=page.$('#typing-input');
  message.focus();message.value='hello world';message.setSelectionRange(11,11);message.emit('input');
  input.value='s';input.emit('input');const candidate=page.$('#suggestions').children[0];
  message.setSelectionRange(6,11,'backward');message.emit('blur');
  assert.equal(page.$('#suggestions').children[0],candidate);
  candidate.onclick();assert.equal(message.value,'hello កូន');
  input.emit('keydown',{key:'z',ctrlKey:true});
  assert.equal(message.value,'hello world');assert.equal(message.selectionStart,6);assert.equal(message.selectionEnd,11);
});


test('Enter uses current English context after the message cursor changes on blur',async()=>{
  const core=await createEngine();
  try{
    const page=editorPage(core),message=page.$('#message'),input=page.$('#typing-input');
    page.run("settings.language='english';settings.correction=true;");
    message.value='She  us go';message.setSelectionRange(4,4);message.focus();message.emit('input');
    input.value='lets';input.emit('input');assert.equal(page.run('result.candidates[0].text'),'lets');
    message.setSelectionRange(0,0);message.emit('blur');
    const fresh=core.suggest('lets',{language:'english',correction:true,before:'',after:'She  us go'});
    assert.equal(fresh.candidates[0].text,"let's");
    input.emit('keydown',{key:'Enter'});
    assert.equal(message.value,"let'sShe  us go");
    input.emit('keydown',{key:'z',ctrlKey:true});assert.equal(message.value,'She  us go');assert.equal(input.value,'lets');
  }finally{core.dispose();}
});

test('Space rechecks the current exact ordering when blur supplies a different Khmer context',async()=>{
  const dictionary={schemaVersion:2,mappings:[{id:1,romanized:'bat',khmer:'បាទ',rank:0,status:'approved'},{id:2,romanized:'bat',khmer:'បត្រ',rank:20,status:'approved'}],words:[{id:2,khmer:'បត្រ',rank:20,status:'approved'},{id:3,khmer:'សញ្ញា',rank:0,status:'approved'},{id:4,khmer:'សញ្ញាបត្រ',rank:0,status:'approved'}],structures:[{wordId:4,componentIds:[3,2]}]};
  const core=await createEngine({dictionary});
  try{
    const page=editorPage(core),message=page.$('#message'),input=page.$('#typing-input');
    page.run("editor.push(replace(editor.current,'សញ្ញា',{mappingId:3}));syncEditor(true);");
    input.value='bat';input.emit('input');assert.equal(page.run('result.exactCandidates[0].khmer'),'បត្រ');
    message.setSelectionRange(0,0);message.emit('blur');
    const fresh=core.suggest('bat',{previousWords:[]});assert.equal(fresh.exactCandidates[0].khmer,'បាទ');
    input.emit('keydown',{key:' '});assert.equal(message.value,'បាទសញ្ញា');
    input.emit('keydown',{key:'z',ctrlKey:true});assert.equal(message.value,'សញ្ញា');assert.equal(input.value,'bat');
  }finally{core.dispose();}
});

test('fresh context still protects explicit-selection and disabled correction flags on Space',()=>{
  for(const flag of ['tapOnly','manualOnly','correction']){
    const word={mappingId:1,khmer:'កូន'};
    const core={suggest:(input,options)=>({candidates:[options.before?word:{...word,[flag]:flag==='correction'?'abcde':true}],exactCandidates:!options.before&&flag==='correction'?[]:[word]})};
    const page=editorPage(core),message=page.$('#message'),input=page.$('#typing-input');
    page.run("settings.language='english';");
    message.value='prefix';message.setSelectionRange(6,6);message.focus();message.emit('input');
    input.value='abc';input.emit('input');message.setSelectionRange(0,0);message.emit('blur');
    input.emit('keydown',{key:' '});
    assert.equal(message.value,'abc prefix',flag);
  }
});

test('focus and arrow navigation refresh stale results without overriding a current explicit choice',async()=>{
  const core=await createEngine();
  try{
    const page=editorPage(core),message=page.$('#message'),input=page.$('#typing-input');
    page.run("settings.language='english';settings.correction=true;");
    message.value='She ';message.setSelectionRange(4,4);message.focus();message.emit('input');
    input.value='lets';input.emit('input');message.setSelectionRange(0,0);message.emit('blur');
    input.focus();input.emit('focus');assert.equal(page.run('result.candidates[0].text'),"let's");
    input.emit('keydown',{key:'ArrowDown'});assert.equal(page.run('active'),1);
    input.emit('keydown',{key:'Enter'});assert.equal(message.value,'letsShe ');
  }finally{core.dispose();}
});

test('undo restores forward and backward selections after click, Enter and Space replacement',()=>{
  for(const direction of ['forward','backward'])for(const method of ['click','Enter',' ']){
    const page=editorPage(),message=page.$('#message'),input=page.$('#typing-input');
    message.value='hello world';message.setSelectionRange(6,11);message.focus();message.emit('input');
    // Changing only direction must be recorded without adding an undo step.
    message.setSelectionRange(6,11,direction);message.emit('select');
    input.value='s';input.emit('input');
    if(method==='click')page.$('#suggestions').children[0].onclick();else input.emit('keydown',{key:method});
    assert.equal(message.value,'hello កូន');assert.equal(message.selectionDirection,'none');
    input.emit('keydown',{key:'z',ctrlKey:true});
    assert.equal(message.value,'hello world');assert.equal(message.selectionStart,6);assert.equal(message.selectionEnd,11);assert.equal(message.selectionDirection,direction);
    input.emit('keydown',{key:'z',ctrlKey:true,shiftKey:true});
    assert.equal(message.value,'hello កូន');assert.equal(message.selectionStart,message.selectionEnd);assert.equal(message.selectionDirection,'none');
  }
});


test('suggestion buttons show a corner status dot and Romanized spelling instead of status subtext',()=>{
  const candidate=Object.freeze({mappingId:1,khmer:'ហ្វ្រាំង',completion:'frang'});
  const page=editorPage({suggest:()=>({candidates:[candidate]})}),input=page.$('#typing-input');
  input.value='f';input.emit('input');
  const button=page.$('#suggestions').children[0];
  assert.equal(button.children[0].textContent,'ហ្វ្រាំង');
  assert.equal(button.children[1].className,'candidate-statuses');
  assert.equal(button.children[1].children[0].className,'status-dot status-completion');
  assert.equal(button.children[2].className,'candidate-reading');
  assert.equal(button.children[2].textContent,'frang');
  button.onclick();assert.equal(page.$('#message').value,'ហ្វ្រាំង');
});


test('a suggestion displays completion, correction and explicit-selection dots together',()=>{
  const candidate=Object.freeze({mappingId:1,khmer:'សៀវភៅ',completion:'sievphov',correction:'sievphov',tapOnly:true});
  const page=editorPage({suggest:()=>({candidates:[candidate]})}),input=page.$('#typing-input');
  input.value='sievpho';input.emit('input');
  const button=page.$('#suggestions').children[0],dots=button.children[1];
  assert.equal(button.dataset.statusCount,'3');
  assert.deepEqual(dots.children.map(dot=>dot.className),['status-dot status-completion','status-dot status-correction','status-dot status-explicit']);
  assert.equal(button.title,'Completion · Correction · Select explicitly');
  assert.equal(button.children[2].textContent,'sievphov');
  button.onclick();assert.equal(page.$('#message').value,'សៀវភៅ');
});

test('English suggestions omit repeated subtext, keep Match and still insert and undo normally',async()=>{
  const core=await createEngine();
  try{
    const page=editorPage(core),input=page.$('#typing-input');
    page.run("settings.language='english';");input.value='books';input.emit('input');
    const button=page.$('#suggestions').children[0];
    assert.equal(button.children[1].children[0].className,'status-dot status-match');
    assert.equal(button.title,'Match');assert.equal(button.children.length,2);
    assert.equal(button.lang,'en');
    assert.equal(button.attributes['aria-label'],'Insert books, Match');
    button.onclick();assert.equal(page.$('#message').value,'books');
    input.emit('keydown',{key:'z',ctrlKey:true});assert.equal(page.$('#message').value,'');assert.equal(input.value,'books');
  }finally{core.dispose();}
});

test('holding a suggestion opens its management dialog without changing the editor, results or undo',async()=>{
  const core=await createEngine({words:[{word:'សៀវភៅ',spellings:['mybook'],rank:5}]});
  try{
    const page=editorPage(core),input=page.$('#typing-input'),message=page.$('#message');
    page.run("dictionaryWords={khmer:[{id:1,word:'សៀវភៅ',partsOfSpeech:['noun'],romanizations:['sievphov'],soundMapInputs:[]}]};");
    message.value='hello world';message.setSelectionRange(6,11,'backward');message.emit('input');
    input.value='mybook';input.emit('input');
    const before=page.run('JSON.stringify({draft:editor.current,result,undo:editor.canUndo,redo:editor.canRedo})');
    const button=page.$('#suggestions').children[0];
    button.emit('pointerdown',{pointerId:1,button:0,clientX:20,clientY:20});page.hold();
    assert.equal(page.$('#word-dialog').open,true);
    const row=page.$('#word-dialog').row;
    assert.equal(row.word,'សៀវភៅ');assert(row.bundled);assert(row.personalIndices.length);
    assert(row.spellings.includes('mybook'));assert(row.romanizations.includes('sievphov'));
    assert.notEqual(page.$('#word-details').open,true);
    page.$('#word-dialog').close();
    button.emit('pointerup',{pointerId:1});button.onclick({preventDefault(){}});
    assert.equal(page.run('JSON.stringify({draft:editor.current,result,undo:editor.canUndo,redo:editor.canRedo})'),before);
    assert.equal(message.value,'hello world');assert.equal(input.value,'mybook');
    button.emit('pointerdown',{pointerId:2,button:0,clientX:20,clientY:20});button.emit('pointerup',{pointerId:2});button.onclick();
    assert.equal(message.value,'hello សៀវភៅ');
    input.emit('keydown',{key:'z',ctrlKey:true});
    assert.equal(message.value,'hello world');assert.equal(input.value,'mybook');assert.equal(message.selectionDirection,'backward');
  }finally{core.dispose();}
});

test('generated English suggestions offer to add the exact form rather than edit its base',async()=>{
  const core=await createEngine();
  try{
    const page=editorPage(core),input=page.$('#typing-input');
    page.run("settings.language='english';dictionaryWords={english:[{id:1,word:'book',partsOfSpeech:['noun']}]};");
    input.value='books';input.emit('input');const button=page.$('#suggestions').children[0];
    assert.equal(button.emit('contextmenu').prevented,true);
    assert.equal(page.$('#word-dialog').open,true);
    assert.equal(page.$('#word-dialog').row.word,'books');
    assert.equal(page.$('#word-dialog').row.bundled,false);
    assert.notEqual(page.$('#word-details').open,true);
    assert.equal(page.$('#message').value,'');assert.equal(input.value,'books');
  }finally{core.dispose();}
});

test('alternate consonant counts insert by click, Enter or Space and undo restores the typed variant',async()=>{
  const dictionary={schemaVersion:2,mappings:[],words:[
    {id:99100,khmer:'លទ្ធិ',rank:0,status:'approved'},
    {id:99101,khmer:'វគ្គីករណ៍',rank:0,status:'approved'}]};
  const core=await createEngine({dictionary});
  try{
    for(const [typed,word,preferred]of [['lertthi','លទ្ធិ','lerthi'],['vaekikor','វគ្គីករណ៍','vaekkikor']]){
      for(const method of ['click','Enter',' ']){
        const page=editorPage(core),message=page.$('#message'),input=page.$('#typing-input');
        message.value='hello world';message.setSelectionRange(6,11,'backward');message.emit('input');
        input.value=typed;input.emit('input');
        const button=page.$('#suggestions').children.find(button=>button.children[0].textContent===word);
        assert(button,typed);assert.equal(button.children[2].textContent,preferred);
        if(method==='click')button.onclick();else input.emit('keydown',{key:method});
        assert.equal(message.value,`hello ${word}`,`${typed}: ${method}`);
        input.emit('keydown',{key:'z',ctrlKey:true});
        assert.equal(message.value,'hello world');assert.equal(input.value,typed);
        assert.equal(message.selectionStart,6);assert.equal(message.selectionEnd,11);assert.equal(message.selectionDirection,'backward');
        input.emit('keydown',{key:'z',ctrlKey:true,shiftKey:true});assert.equal(message.value,`hello ${word}`);
      }
    }
  }finally{core.dispose();}
});

test('Auto completes outeaho on Space at the current selection, with exact undo and redo',async()=>{
  const core=await createEngine();
  try{
    const before=core.suggest('outeaho',{previousWords:[]});
    const page=editorPage(core),input=page.$('#typing-input'),message=page.$('#message');
    message.value='hello world!';message.setSelectionRange(6,11,'backward');message.emit('input');
    input.value='outeaho';input.emit('input');
    assert.equal(page.$('#suggestion-acceptance').value,'auto');
    assert.equal(input.emit('keydown',{key:' '}).prevented,true);
    assert.equal(message.value,'hello ឧទាហរណ៍!');assert.equal(input.value,'');
    input.emit('keydown',{key:'z',ctrlKey:true});
    assert.equal(message.value,'hello world!');assert.equal(input.value,'outeaho');
    assert.equal(message.selectionStart,6);assert.equal(message.selectionEnd,11);assert.equal(message.selectionDirection,'backward');
    input.emit('keydown',{key:'z',ctrlKey:true,shiftKey:true});assert.equal(message.value,'hello ឧទាហរណ៍!');
    input.emit('keydown',{key:'z',ctrlKey:true});input.emit('keydown',{key:' '});
    assert.equal(message.value,'hello outeaho !');assert.equal(input.value,'');
    assert.deepEqual(core.suggest('outeaho',{previousWords:[]}),before);
  }finally{core.dispose();}
});

test('Auto Space accepts the arrow-highlighted English or Khmer completion with reversible selection replacement',async()=>{
  const dictionary={schemaVersion:3,
    mappings:[{id:11,romanized:'kon',khmer:'កូន',rank:0,status:'approved'},{id:12,romanized:'konsis',khmer:'កូនសិស្ស',rank:1,status:'approved'}],
    words:[{id:1,khmer:'កូន',rank:0,status:'approved'},{id:2,khmer:'កូនសិស្ស',rank:1,status:'approved'}],
    englishWords:[{id:3,text:'book',rank:0,status:'approved'},{id:4,text:'boot',rank:1,status:'approved'}]};
  const core=await createEngine({dictionary});
  try{
    const before=['bo','ko'].map(input=>core.suggest(input));
    for(const [language,typed,word]of [['english','bo','boot'],['khmer','ko','កូនសិស្ស']]){
      const page=editorPage(core),input=page.$('#typing-input'),message=page.$('#message');
      page.run(`settings.language=${JSON.stringify(language)};settings.soundMap=false;`);
      message.value='hello old';message.setSelectionRange(6,9,'backward');message.emit('input');
      input.value=typed;input.emit('input');input.emit('keydown',{key:'ArrowDown'});
      assert.equal(page.run('result.candidates[active].text??result.candidates[active].khmer'),word);
      input.emit('keydown',{key:' '});const expected='hello '+word+(language==='english'?' ':'');
      assert.equal(message.value,expected);
      input.emit('keydown',{key:'z',ctrlKey:true});
      assert.equal(message.value,'hello old');assert.equal(input.value,typed);
      assert.equal(message.selectionStart,6);assert.equal(message.selectionEnd,9);assert.equal(message.selectionDirection,'backward');
      input.emit('keydown',{key:'z',ctrlKey:true,shiftKey:true});assert.equal(message.value,expected);
    }
    assert.deepEqual(['bo','ko'].map(input=>core.suggest(input)),before);
  }finally{core.dispose();}
});

test('Manual preserves the first and repeated spaces after Khmer with undo and redo',async()=>{
  const core=await createEngine({dictionary:{schemaVersion:2,mappings:[],words:[{id:1,khmer:'កូន',rank:0,status:'approved'}]}});
  try{
    const page=editorPage(core),input=page.$('#typing-input'),message=page.$('#message');
    page.$('#suggestion-acceptance').value='manual';input.value='កូន';input.emit('input');
    input.emit('keydown',{key:' '});assert.equal(message.value,'កូន ');assert.equal(input.value,'');
    input.emit('keydown',{key:' '});assert.equal(message.value,'កូន  ');
    input.emit('keydown',{key:'z',ctrlKey:true});assert.equal(message.value,'កូន ');
    input.emit('keydown',{key:'z',ctrlKey:true});assert.equal(message.value,'');assert.equal(input.value,'កូន');
    input.emit('keydown',{key:'z',ctrlKey:true,shiftKey:true});assert.equal(message.value,'កូន ');
    input.emit('keydown',{key:'z',ctrlKey:true,shiftKey:true});assert.equal(message.value,'កូន  ');
  }finally{core.dispose();}
});

test('a protected highlighted completion never falls back to a different automatic word',()=>{
  for(const flag of ['tapOnly','manualOnly','correction']){
    const core={suggest:input=>({input,candidates:[{khmer:'កូន',completion:'kon'},
      {khmer:'កូនសិស្ស',completion:'konsis',[flag]:flag==='correction'?'konsis':true}],exactCandidates:[]})};
    const page=editorPage(core),input=page.$('#typing-input');
    input.value='ko';input.emit('input');input.emit('keydown',{key:'ArrowDown'});input.emit('keydown',{key:' '});
    assert.equal(page.$('#message').value,'ko ',flag);
  }
});

test('Manual keeps exact matches and completions literal on Space but still accepts explicit choices',async()=>{
  const core=await createEngine();
  try{
    for(const typed of ['outeaho','outeahor']){
      const page=editorPage(core),input=page.$('#typing-input');
      page.$('#suggestion-acceptance').value='manual';input.value=typed;input.emit('input');
      input.emit('keydown',{key:' '});assert.equal(page.$('#message').value,typed+' ');
      input.emit('keydown',{key:'z',ctrlKey:true});assert.equal(input.value,typed);
      input.emit('keydown',{key:'Enter'});assert.equal(page.$('#message').value,'ឧទាហរណ៍');
    }
  }finally{core.dispose();}
});

test('Auto keeps rare completions, Khmer typo recovery, Shift+Space and IME input explicit',()=>{
  for(const candidate of [{khmer:'កូន',completion:'kon',tapOnly:true},{khmer:'កូន',completion:'kon',manualOnly:true},{khmer:'កូន',completion:'kon',correction:'kon'}]){
    const page=editorPage({suggest:input=>({input,candidates:[candidate],exactCandidates:[]})}),input=page.$('#typing-input');
    input.value='ko';input.emit('input');
    assert.equal(input.emit('keydown',{key:' ',shiftKey:true}).prevented,false);
    assert.equal(input.emit('keydown',{key:' ',isComposing:true}).prevented,false);
    input.emit('keydown',{key:' ',repeat:true});assert.equal(page.$('#message').value,'');
    input.emit('keydown',{key:' '});assert.equal(page.$('#message').value,'ko ');
  }
});

test('English Auto accepts completions and enabled corrections with one trailing space',async()=>{
  const core=await createEngine();
  try{
    for(const [typed,correction,expected]of [['bo',false,'both '],['Bo',false,'Both '],['BO',false,'BOTH '],['im',true,"I'm "],['iM',true,'iM '],['adn',true,'and '],['adn',false,'adn ']]){
      const page=editorPage(core),input=page.$('#typing-input');
      page.run(`settings.language='english';settings.correction=${correction};`);
      input.value=typed;input.emit('input');input.emit('keydown',{key:' '});
      assert.equal(page.$('#message').value,expected,typed);
      input.emit('keydown',{key:'z',ctrlKey:true});assert.equal(page.$('#message').value,'');assert.equal(input.value,typed);
    }
  }finally{core.dispose();}
});

test('Auto/Manual is remembered locally and storage failures leave Auto usable',()=>{
  const stored=new Map(),storage={getItem:key=>stored.get(key),setItem:(key,value)=>stored.set(key,value)};
  const page=editorPage(undefined,storage),select=page.$('#suggestion-acceptance');
  assert.equal(select.value,'auto');select.value='manual';select.emit('change');
  assert.equal(editorPage(undefined,storage).$('#suggestion-acceptance').value,'manual');
  const blocked={getItem(){throw new Error('Blocked');},setItem(){throw new Error('Blocked');}};
  const fallback=editorPage(undefined,blocked);assert.equal(fallback.$('#suggestion-acceptance').value,'auto');
  assert.doesNotThrow(()=>fallback.$('#suggestion-acceptance').emit('change'));
  assert.equal(editorPage(undefined,{getItem:()=> 'invalid'}).$('#suggestion-acceptance').value,'auto');
});

test('a changed query cancels an in-progress hold',()=>{
  const page=editorPage(),input=page.$('#typing-input');
  input.value='s';input.emit('input');
  page.$('#suggestions').children[0].emit('pointerdown',{pointerId:1,button:0,clientX:20,clientY:20});
  input.value='sa';input.emit('input');page.hold();
  assert.equal(page.$('#word-details').open,undefined);assert.equal(page.$('#message').value,'');
});

test('exact shorthands accept Space, Enter and clicks after Read as edits, with undo and redo',async()=>{
  const core=await createEngine({dictionary:{schemaVersion:2,mappings:[],words:['ខ្ញុំ','ទៀត'].map((khmer,index)=>({id:99600+index,khmer,rank:0,status:'approved'}))}});
  try{
    core.setShorthands([{word:'ទៀត',inputs:['tt']}]);
    core.setPronunciations([{word:'ខ្ញុំ',readAs:'មិត'}]);
    for(const [typed,word]of [['nh','ខ្ញុំ'],['tt','ទៀត'],['met','ខ្ញុំ']]){
      for(const method of ['click','Enter',' ']){
        const page=editorPage(core),message=page.$('#message'),input=page.$('#typing-input');
        message.value='hello world';message.setSelectionRange(6,11,'backward');message.emit('input');
        input.value=typed;input.emit('input');
        const hit=page.run('result.candidates').find(hit=>hit.khmer===word);assert(hit,typed);assert(!hit.tapOnly);
        const button=page.$('#suggestions').children.find(button=>button.children[0].textContent===word);assert(button,typed);
        if(method==='click')button.onclick();else input.emit('keydown',{key:method});
        assert.equal(message.value,`hello ${word}`,`${typed}: ${method}`);
        input.emit('keydown',{key:'z',ctrlKey:true});
        assert.equal(message.value,'hello world');assert.equal(input.value,typed);
        assert.equal(message.selectionStart,6);assert.equal(message.selectionEnd,11);assert.equal(message.selectionDirection,'backward');
        input.emit('keydown',{key:'z',ctrlKey:true,shiftKey:true});assert.equal(message.value,`hello ${word}`);
      }
    }
  }finally{core.dispose();}
});
