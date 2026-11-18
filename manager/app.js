import { pronunciationSection } from './pronunciations.js';
import { attachRomanizedConverter } from './converter.js';
import { attachFieldHelp } from './field-help.js';
import { t, uiMessage, posLabel, setUiText, translatePage, setUiLanguage, getUiLanguage, readUiLanguage, UI_LANGUAGE_KEY } from './i18n.js';
import { createEngine } from '/dist/public/browser.js';
import { history, replace, editMessage, suggestDraft, spaceChoice, spaceText } from './composer.js';
import { suggestionStatuses, suggestionStatus, suggestionReading } from './suggestion-view.js';
import { suggestionGesture } from './suggestion-gesture.js';
import { wordList, wordPage, wordPageLinks, wordLetters, wordsForLetter, romanizedKeys, wordsWithoutRomanized, suggestionWord } from './word-list.js';
import { WORDS_KEY, parseImport, parseImportSpec, wordExport, loadSharedDictionary, saveSharedDictionary } from './state.js';
import { soundRules, ruleFamilies, renderRuleValue, renderRuleExamples } from './sound-rules.js';
import { exampleTopics, examplePage, initialExample, customExampleTopics, loadExample, saveExample, addExample, convertExample, trackExampleInput, exampleChanged, exampleWordAt, exampleSuggestions, chooseExampleWord, renderParagraph, paragraphSuggestion } from './examples.js';
const $ = selector => document.querySelector(selector);
const $$ = selector => [...document.querySelectorAll(selector)];
const el = (tag, text, className) => { const node = document.createElement(tag); if (text !== undefined) node.textContent = text; if (className) node.className = className; return node; };
attachFieldHelp($('#word-dialog'));
let engine, map, editRow=null, editCatalog=null, editInitial=null, busy=false, damagedWords=null, legacyWords=[],sentenceConverter;
let typingMode='typing';
const settings = { language: 'khmer', conversion: true, soundMap: true, completion: true, correction: false };
let wordLanguage='khmer',wordLetter='',missingRomanized=false;
const exampleDrafts=new Map();let exampleComposing=false,exampleTarget=null;
let customTopics=[],customTopicsError='';
let exampleListScroll=0;
let soundMapReturnScroll;
window.addEventListener('scroll',()=>{
  if(location.hash==='#words/examples'&&!$('#example-topics').hidden)exampleListScroll=window.scrollY;
},{passive:true});
const titles = {typing:'Typing',soundmap:'Sound map',words:'Word list'};
function announce(message = '',values={}) { setUiText($('#status'),message,values); }
function errorMessage(error) { return error instanceof Error ? error.message : String(error); }
function exampleTitle(topic){return topic?.custom&&!topic.isNew?topic.title:t(topic?.title??'Examples');}
function route(event) {
  const requested=location.hash.slice(1)==='personal'?'words':location.hash.slice(1);
  const pronunciation=requested==='soundmap/pronunciation';
  const examplePath=requested==='words/examples'||requested.startsWith('words/examples/');
  if(examplePath&&wordLanguage==='khmer'){
    try{customTopics=customExampleTopics(localStorage);customTopicsError='';}catch(error){customTopicsError=errorMessage(error);}
  }
  const example=examplePage(location.hash,wordLanguage,customTopics);
  if(example?.topic?.id!==exampleTarget?.topicId)exampleTarget=null;
  if(examplePath&&!example)window.history.replaceState(null,'','#words');
  const page=example?'examples':examplePath?'words':pronunciation?'pronunciation':Object.hasOwn(titles,requested)?requested:'typing';
  const activePage=example?'words':pronunciation?'soundmap':page;
  const title=example?exampleTitle(example.topic):t(pronunciation?'Read-As':titles[page]);
  $$('.page').forEach(node => { node.hidden = node.id !== page; });
  $$('[data-page]').forEach(node => { if (node.dataset.page === activePage) node.setAttribute('aria-current','page'); else node.removeAttribute('aria-current'); });
  const parent=example?t('Word list'):pronunciation?t('Sound map'):'';
  $('#breadcrumb').textContent = '/ ' + (parent?`${parent} / ${title}`:title);
  document.title = `${title} · Kmeow Engine`;
  if(example){
    renderExamples(example.topic);
    if(event?.type==='hashchange'){
      $(example.topic?.isNew?'#example-title':'#examples-title').focus({preventScroll:true});
      window.scrollTo({top:example.topic?0:exampleListScroll,behavior:'instant'});
    }
  }
  if(event?.type==='hashchange'){
    if(pronunciation){
      $('#pronunciation-title').focus({preventScroll:true});
      window.scrollTo({top:0,behavior:'instant'});
    }else if(page==='words'&&event.oldURL?.endsWith('#soundmap/pronunciation')){
      $('#word-results').focus({preventScroll:true});
    }else if(page==='soundmap'&&event.oldURL?.endsWith('#soundmap/pronunciation')){
      const link=$('#pronunciation-open');
      link.focus({preventScroll:true});
      if(soundMapReturnScroll===undefined)link.scrollIntoView({block:'nearest'});
      else window.scrollTo({top:soundMapReturnScroll,behavior:'instant'});
    }
  }
}
window.addEventListener('hashchange', route); route();
$('#pronunciation-open').onclick=()=>{soundMapReturnScroll=window.scrollY;};
function renderExamples(topic){
  const topicIndex=topic&&!topic.isNew?[...exampleTopics,...customTopics].findIndex(item=>item.id===topic.id):-1;
  $('#examples-title').textContent=topicIndex>=0?`${topicIndex+1}. ${exampleTitle(topic)}`:exampleTitle(topic);
  $('#examples-back').href=topic?'#words/examples':'#words';
  const orderedTopics=[...exampleTopics,...customTopics];
  $('#example-navigation').hidden=topicIndex<0;
  for(const [id,offset] of [['example-previous',-1],['example-next',1]]){
    const target=topicIndex>=0?orderedTopics[topicIndex+offset]:null,button=$('#'+id);
    button.disabled=!target;button.onclick=()=>{if(target)location.hash=`#words/examples/${target.id}`;};
  }
  $('#example-edit').hidden=!topic;
  $('#example-add').hidden=!!topic;$('#example-add').disabled=!engine||!!customTopicsError;
  $('#example-list-error').textContent=t(customTopicsError);$('#example-list-error').hidden=!!topic;
  $('#example-title-field').hidden=!topic?.isNew;
  const list=$('#example-topics'),reading=$('#example-reading');list.hidden=!!topic;reading.hidden=!topic;
  if(topic){
    if(!exampleDrafts.has(topic.id)){
      let saved=initialExample(topic),error='';
      if(!topic.isNew)try{saved=loadExample(topic,localStorage);}catch(problem){error=errorMessage(problem);}
      exampleDrafts.set(topic.id,{saved,draft:saved,error,status:'',editing:!!topic.isNew,title:''});
    }
    const entry=exampleDrafts.get(topic.id);
    const text=entry.inputDraft?.romanized??entry.draft.romanized;
    if(!exampleComposing&&$('#example-romanized').value!==text){$('#example-romanized').value=text;entry.inputHistory=null;}
    if(topic.isNew&&$('#example-title').value!==entry.title)$('#example-title').value=entry.title;
    refreshExampleEditor();
  }else{
    list.replaceChildren();
    for(const [index,item] of [...exampleTopics,...customTopics].entries()){
      const link=el('a',undefined,'example-topic');link.href=`#words/examples/${item.id}`;
      const arrow=el('span','›');arrow.setAttribute('aria-hidden','true');link.append(el('span',`${index+1}.`,'example-number'),el('span',exampleTitle(item),'example-topic-title'),arrow);list.append(link);
    }
  }
}
$('#examples-open').onclick=()=>{if(wordLanguage==='khmer')location.hash='words/examples';};
$('#example-add').onclick=()=>{
  if(!engine||wordLanguage!=='khmer'||customTopicsError)return;
  exampleComposing=false;location.hash='#words/examples/new';
};
function currentExample(){return examplePage(location.hash,wordLanguage,customTopics)?.topic;}
function refreshExampleEditor(){
  $('#example-add').disabled=!engine||!!customTopicsError;
  const topic=currentExample(),entry=topic&&exampleDrafts.get(topic.id);if(!entry)return;
  const dirty=exampleChanged(entry.draft,entry.saved);
  const ready=topic.isNew?!!entry.title.trim()&&!!entry.draft.romanized.trim():dirty;
  $('#example-edit').hidden=entry.editing;$('#example-edit').disabled=!engine;
  renderExampleWords(entry.draft,topic.id);$('#example-romanized-view').hidden=entry.editing;
  $('#example-romanized').hidden=!entry.editing;$('#example-romanized').disabled=!engine||!entry.editing;
  $('#example-actions').hidden=!entry.editing&&!dirty;
  $('#example-save').disabled=!engine||!ready||exampleComposing||$('#example-romanized').value!==entry.draft.romanized;
  $('#example-cancel').disabled=!entry.editing&&!dirty;
  $('#example-error').textContent=t(entry.error);
  $('#example-status').textContent=t(entry.status);
  $('#example-unconverted').textContent=entry.draft.unknown?.length?t('Unrecognized words stay as typed.'):'';
  renderExampleSuggestions();
}
function renderExampleWords(draft,topicId){
  for(const [id,language]of [['#example-romanized-view','romanized'],['#example-khmer','khmer']]){
    renderParagraph({element:$(id),draft,language,identity:topicId,engine,el,
      selected:exampleTarget?.topicId===topicId?exampleTarget:null,
      disabled:exampleComposing||$('#example-romanized').value!==draft.romanized,suggestionsId:'example-suggestions',
      onSelect:token=>{
        if(exampleComposing||currentExample()?.id!==topicId||exampleDrafts.get(topicId)?.draft!==draft||$('#example-romanized').value!==draft.romanized)return;
        exampleTarget={topicId,...token};refreshExampleEditor();$('#example-suggestion-panel').scrollIntoView({block:'nearest'});
      }});
  }
}
function renderExampleSuggestions(){
  const panel=$('#example-suggestion-panel'),list=$('#example-suggestions');list.replaceChildren();
  const topic=currentExample(),entry=topic&&exampleDrafts.get(topic.id);
  const unsynced=entry&&$('#example-romanized').value!==entry.draft.romanized;
  if(unsynced)exampleTarget=null;
  panel.hidden=!engine||exampleComposing||!entry||unsynced||exampleTarget?.topicId!==topic.id;
  if(panel.hidden)return;
  try{
    const selection=exampleSuggestions(engine,entry.draft,exampleTarget.start,exampleTarget.end);
    if(!selection||selection.target.input!==exampleTarget.input){panel.hidden=true;exampleTarget=null;return;}
    $('#example-suggestion-title').textContent=t('Suggestions for {input}',{input:selection.target.input});
    const draft=entry.draft;
    for(const candidate of selection.result.candidates){
      const button=paragraphSuggestion(el,candidate,selection.result,selection.target);
      button.onclick=()=>{
        const active=currentExample(),current=active&&exampleDrafts.get(active.id);
        if(exampleComposing||active?.id!==topic.id||current?.draft!==draft||$('#example-romanized').value!==draft.romanized||exampleTarget?.topicId!==topic.id
          ||exampleTarget.start!==selection.target.start||exampleTarget.end!==selection.target.end||exampleTarget.input!==selection.target.input){renderExampleSuggestions();return;}
        try{
          const next=chooseExampleWord(engine,draft,selection.target,candidate),input=$('#example-romanized');
          current.draft=next;current.inputDraft=null;current.beforeInput=null;current.compositionBefore=null;current.error='';current.status='';
          if(input.value!==next.romanized){
            input.value=next.romanized;current.inputHistory=null;
            if(current.editing){const end=selection.target.end+next.romanized.length-draft.romanized.length;input.setSelectionRange(end,end);}
          }else if(current.inputHistory)rememberExampleHistory(current,next);
          exampleTarget=null;refreshExampleEditor();
          if(current.editing)input.focus({preventScroll:true});
          else $($('#example-save').disabled?'#example-edit':'#example-save').focus();
        }catch(error){current.error=errorMessage(error);refreshExampleEditor();}
      };
      list.append(button);
    }
    if(!selection.result.candidates.length)list.append(el('p',t('No suggestions for this input.'),'empty'));
  }catch(error){list.append(el('p',t(errorMessage(error)),'error'));}
}
function exampleCursorSuggestions(){
  const topic=currentExample(),entry=topic&&exampleDrafts.get(topic.id),input=$('#example-romanized');
  if(!entry?.editing||exampleComposing||input.value!==entry.draft.romanized)return;
  const target=exampleWordAt(entry.draft,input.selectionStart,input.selectionEnd);
  exampleTarget=target?{topicId:topic.id,...target}:null;refreshExampleEditor();
}
$('#example-suggestions-close').onclick=()=>{
  exampleTarget=null;refreshExampleEditor();
  const topic=currentExample(),entry=topic&&exampleDrafts.get(topic.id);
  if(entry)$(entry.editing?$('#example-save').disabled?'#example-cancel':'#example-save':'#example-edit').focus({preventScroll:true});
};
$('#example-title').addEventListener('input',()=>{
  const topic=currentExample(),entry=topic?.isNew&&exampleDrafts.get(topic.id);if(!entry)return;
  entry.title=$('#example-title').value;refreshExampleEditor();
});
// The browser owns text undo/grouping; retain the matching choice metadata only.
function rememberExampleHistory(entry,value){
  const history=entry.inputHistory??={entries:[],position:-1,size:0};
  const current=history.entries[history.position],snapshot={value,size:JSON.stringify(value).length};
  if(current?.value.romanized===value.romanized)history.size-=current.size;
  else{
    for(const discarded of history.entries.splice(history.position+1))history.size-=discarded.size;
    history.position++;
  }
  history.entries[history.position]=snapshot;history.size+=snapshot.size;
  // Keep the starting paragraph and active state, with bounded recent snapshots.
  while(history.entries.length>2&&(history.entries.length>256||history.size>2*1024*1024)){
    const at=history.position>1?1:history.entries.length-1;
    history.size-=history.entries.splice(at,1)[0].size;
    if(at<history.position)history.position--;
  }
}
function restoreExampleHistory(entry,text,inputType){
  const history=entry.inputHistory,step=inputType==='historyUndo'?-1:inputType==='historyRedo'?1:0;
  if(!history||!step)return null;
  for(let position=history.position+step;position>=0&&position<history.entries.length;position+=step){
    if(history.entries[position].value.romanized===text){history.position=position;return history.entries[position].value;}
  }
  return null;
}
function updateExample(event){
  const topic=currentExample(),entry=topic&&exampleDrafts.get(topic.id);if(!entry?.editing||!engine||exampleComposing)return;
  entry.status='';entry.error='';
  const input=$('#example-romanized'),before=entry.compositionBefore??entry.beforeInput;
  entry.beforeInput=null;entry.compositionBefore=null;
  if(!entry.inputHistory)rememberExampleHistory(entry,entry.inputDraft??entry.draft);
  entry.inputDraft=restoreExampleHistory(entry,input.value,event?.inputType||before?.inputType)
    ??trackExampleInput(entry.inputDraft??entry.draft,input.value,before,input.selectionStart,input.selectionEnd);
  try{entry.draft=convertExample(engine,entry.inputDraft,input.value);entry.inputDraft=null;}
  catch(error){entry.error=errorMessage(error);}
  rememberExampleHistory(entry,entry.inputDraft??entry.draft);
  refreshExampleEditor();exampleCursorSuggestions();
}
$('#example-edit').onclick=()=>{
  const topic=currentExample(),entry=topic&&exampleDrafts.get(topic.id);if(!entry||!engine)return;
  entry.editing=true;entry.status='';refreshExampleEditor();$('#example-romanized').focus();
};
$('#example-romanized').addEventListener('input',updateExample);
function rememberExampleInput(inputType){
  const topic=currentExample(),entry=topic&&exampleDrafts.get(topic.id),input=$('#example-romanized');
  if(!entry?.editing)return null;
  return {text:input.value,start:input.selectionStart,end:input.selectionEnd,inputType};
}
$('#example-romanized').addEventListener('beforeinput',event=>{
  if(exampleComposing)return;
  const topic=currentExample(),entry=topic&&exampleDrafts.get(topic.id);
  if(entry)entry.beforeInput=rememberExampleInput(event.inputType);
});
for(const event of ['click','keyup','select','selectionchange','focus'])$('#example-romanized').addEventListener(event,exampleCursorSuggestions);
$('#example-romanized').addEventListener('compositionstart',()=>{
  const topic=currentExample(),entry=topic&&exampleDrafts.get(topic.id);
  if(entry){entry.compositionBefore=rememberExampleInput('insertCompositionText');entry.beforeInput=null;}
  exampleComposing=true;exampleTarget=null;refreshExampleEditor();
});
$('#example-romanized').addEventListener('compositionend',()=>{exampleComposing=false;updateExample();});
$('#example-save').onclick=()=>{
  const topic=currentExample(),entry=topic&&exampleDrafts.get(topic.id);if(!entry||(!entry.editing&&!exampleChanged(entry.draft,entry.saved))||!engine||exampleComposing)return;
  if(entry.editing){updateExample();if(entry.error)return;}else entry.error='';
  try{
    if(topic.isNew){
      const created=addExample(entry.title,entry.draft,localStorage);
      customTopics=[...customTopics,created.topic];
      exampleDrafts.set(created.topic.id,{...entry,saved:created.paragraph,editing:false,status:'Saved on this device.'});
      exampleDrafts.delete('new');exampleTarget=null;location.hash=`#words/examples/${created.topic.id}`;return;
    }
    entry.saved=saveExample(topic,entry.draft,localStorage);entry.editing=false;entry.status='Saved on this device.';exampleTarget=null;
  }
  catch(error){entry.error=uiMessage('Could not save this example: {error}',{error:uiMessage(errorMessage(error))});}
  refreshExampleEditor();
  if(!entry.editing)$('#example-edit').focus();
};
$('#example-cancel').onclick=()=>{
  const topic=currentExample(),entry=topic&&exampleDrafts.get(topic.id);if(!entry)return;
  exampleTarget=null;
  entry.inputDraft=null;entry.beforeInput=null;entry.compositionBefore=null;
  if(topic.isNew){exampleDrafts.delete('new');exampleComposing=false;location.hash='#words/examples';return;}
  entry.draft=entry.saved;if(entry.inputHistory)rememberExampleHistory(entry,entry.saved);
  entry.editing=false;entry.error='';entry.status='Changes canceled.';exampleComposing=false;renderExamples(topic);$('#example-edit').focus();
};
// Only presentation follows the viewport; the same controls and editor remain mounted.
const mobileLayout=window.matchMedia('(max-width: 760px)');
const typingOptions=$('#typing-options');
function syncLayout(){
  const filter=$('#missing-romanized'),slot=$(mobileLayout.matches?'#word-filter-mobile':'#word-filter-column');
  if(filter.parentElement!==slot){const focused=document.activeElement===filter;slot.append(filter);if(focused)filter.focus({preventScroll:true});}
}
mobileLayout.addEventListener('change',syncLayout);syncLayout();

function appearance(theme) {
  document.documentElement.dataset.theme = theme;
  $$('[data-appearance]').forEach(button=>{
    button.querySelector('[data-appearance-label]').textContent = t(theme === 'dark' ? 'Light mode' : 'Dark mode');
    button.setAttribute('aria-label', t(theme === 'dark' ? 'Switch to light appearance' : 'Switch to dark appearance'));
  });
}
try { appearance(localStorage.getItem('kmeow-manager-theme') || 'light'); } catch { appearance('light'); }
let appearanceTimer;
function toggleAppearance(){
  const root=document.documentElement,theme=root.dataset.theme==='dark'?'light':'dark';
  root.classList.add('theme-changing');
  clearTimeout(appearanceTimer);
  appearance(theme);
  appearanceTimer=setTimeout(()=>root.classList.remove('theme-changing'),260);
  try { localStorage.setItem('kmeow-manager-theme',theme); } catch {}
}
$$('[data-appearance]').forEach(button=>{button.onclick=toggleAppearance;});
for (const [key,label] of Object.entries({conversion:'Romanized conversion',soundMap:'Sound map readings',completion:'Word completion',correction:'English corrections'})) {
  const row = el('label',undefined,'setting-row'), input = el('input'); input.type='checkbox'; input.checked=settings[key]; input.dataset.setting=key;
  input.onchange=()=>{settings[key]=input.checked; suggest();}; const text=el('span',label);text.setAttribute('data-i18n',label);row.append(text,input); $('#settings').append(row);
}
const optionsButton=$('#typing-options-toggle'),optionsMenu=$('#typing-options-menu');
let optionsPointerActive=false;
function setTypingOptionsOpen(open,restoreFocus=false){
  optionsMenu.hidden=!open;optionsButton.setAttribute('aria-expanded',String(open));
  if(open)positionTypingOptions();
  else if(restoreFocus)optionsButton.focus({preventScroll:true});
}
function positionTypingOptions(){
  if(optionsMenu.hidden||optionsPointerActive)return;
  const view=window.visualViewport,gap=8,edge=12;
  const left=(view?.offsetLeft??0)+edge,top=(view?.offsetTop??0)+edge;
  const right=left+(view?.width??window.innerWidth)-edge*2,bottom=top+(view?.height??window.innerHeight)-edge*2;
  const anchor=optionsButton.getBoundingClientRect();
  if(anchor.bottom<top||anchor.top>bottom){setTypingOptionsOpen(false);return;}
  optionsMenu.style.maxWidth=`${right-left}px`;
  const below=bottom-anchor.bottom-gap,above=anchor.top-gap-top;
  const down=below>=Math.min(optionsMenu.scrollHeight,360)||below>=above;
  optionsMenu.style.maxHeight=`${Math.max(0,down?below:above)}px`;
  const box=optionsMenu.getBoundingClientRect();
  optionsMenu.style.left=`${Math.max(left,Math.min(anchor.right-box.width,right-box.width))}px`;
  optionsMenu.style.top=`${down?anchor.bottom+gap:anchor.top-gap-box.height}px`;
}
optionsButton.addEventListener('click',()=>setTypingOptionsOpen(optionsMenu.hidden));
document.addEventListener('keydown',event=>{
  if(event.key==='Escape'&&!optionsMenu.hidden){event.preventDefault();setTypingOptionsOpen(false,true);}
});
typingOptions.addEventListener('focusout',event=>{
  if(!optionsPointerActive&&event.relatedTarget&&!typingOptions.contains(event.relatedTarget))setTypingOptionsOpen(false);
});
document.addEventListener('pointerdown',event=>{
  optionsPointerActive=typingOptions.contains(event.target);
  if(!optionsPointerActive)setTypingOptionsOpen(false);
},true);
function finishOptionsPointer(){optionsPointerActive=false;positionTypingOptions();}
document.addEventListener('pointerup',finishOptionsPointer,true);
document.addEventListener('pointercancel',finishOptionsPointer,true);
window.addEventListener('blur',()=>{optionsPointerActive=false;});
window.addEventListener('resize',positionTypingOptions);
window.addEventListener('scroll',positionTypingOptions,true);
window.visualViewport?.addEventListener('resize',positionTypingOptions);
window.addEventListener('hashchange',()=>setTypingOptionsOpen(false));
function language(value) {
  settings.language=value;
  $$('[data-language]').forEach(node=>node.setAttribute('aria-pressed',String(node.dataset.language===value)));
  $('#typing-input').placeholder=t(value==='khmer'?'Try sievphov':'Try books');
  $$('[data-setting]').forEach(node=>{node.disabled=(node.dataset.setting==='correction' ? value!=='english' : ['conversion','soundMap'].includes(node.dataset.setting) && value!=='khmer');});
  syncTypingMode();suggest();
}
function syncTypingMode(){
  const active=settings.language==='khmer'&&typingMode==='converter';
  $('#typing-mode').hidden=settings.language!=='khmer';
  $('#typing-pad').hidden=active;$('#romanized-converter').hidden=!active;$('#typing-options').hidden=active;
  if(active)setTypingOptionsOpen(false);
  $('#typing-layout').classList.toggle('converter-mode',active);
  for(const mode of ['typing','converter'])$('#mode-'+mode).setAttribute('aria-pressed',String(typingMode===mode));
  sentenceConverter?.setActive(active);
}
for(const mode of ['typing','converter'])$('#mode-'+mode).onclick=()=>{typingMode=mode;syncTypingMode();};
$$('[data-language]').forEach(node=>{node.onclick=()=>language(node.dataset.language);});
for(const status of suggestionStatuses){
  const item=el('span',undefined,'legend-item'),dot=el('span',undefined,`status-dot status-${status.kind}`);
  dot.setAttribute('aria-hidden','true');const label=el('span',status.label);label.setAttribute('data-i18n',status.label);item.append(dot,label);$('#suggestion-legend').append(item);
}
function applyUiLanguage(value,persist=true){
  const selected=setUiLanguage(value);document.documentElement.lang=selected;
  $$('[data-ui-language]').forEach(control=>{
    const next=selected==='en'?'km':'en',label=next==='km'?'ខ្មែរ':'English';
    const caption=control.querySelector('[data-ui-language-label]');caption.textContent=label;caption.lang=next;control.setAttribute('aria-label',`${t('Interface language')}: ${label}`);
  });
  translatePage(document);route();appearance(document.documentElement.dataset.theme||'light');
  $('#typing-input').placeholder=t(settings.language==='khmer'?'Try sievphov':'Try books');
  renderWordList();if(engine)suggest();if(map)renderMap({preserveDetails:true});
  renderPronunciations();
  sentenceConverter?.refresh();
  if(persist)try{localStorage.setItem(UI_LANGUAGE_KEY,selected);}catch{}
}
$$('[data-ui-language]').forEach(control=>{control.onclick=()=>applyUiLanguage(getUiLanguage()==='en'?'km':'en');});
function readingFor(candidate,currentResult){
  const row=dictionaryWords?.khmer.find(word=>word.word===candidate.khmer);
  return suggestionReading(candidate,currentResult,settings.language,[...(row?.romanizations??[]),...(row?.soundMapInputs??[])],engine?.getWords()??[]);
}
const editor=history();let result,resultDraft,active=0,composing=false;
let suggestionGestures=[];
const input=$('#typing-input'),message=$('#message');
const acceptance=$('#suggestion-acceptance');
try{acceptance.value=localStorage.getItem('kmeow-manager-acceptance-v1')==='manual'?'manual':'auto';}catch{acceptance.value='auto';}
acceptance.addEventListener('change',()=>{
  try{localStorage.setItem('kmeow-manager-acceptance-v1',acceptance.value==='manual'?'manual':'auto');}catch{}
});
function clearSuggestions(){for(const dispose of suggestionGestures)dispose();suggestionGestures=[];$('#suggestions').replaceChildren();}
function syncEditor(restoreSelection=false){
  const draft=editor.current;
  // Native editing already updated the field. Avoid rewriting values or ranges
  // during input/IME events, which can disrupt selection handles and direction.
  if(input.value!==draft.input)input.value=draft.input;
  const messageChanged=message.value!==draft.text;
  if(messageChanged)message.value=draft.text;
  const direction=draft.direction??'none';
  if((messageChanged||restoreSelection)&&(message.selectionStart!==draft.start||message.selectionEnd!==draft.end||message.selectionDirection!==direction))message.setSelectionRange(draft.start,draft.end,direction);
  $('#message-count').textContent=`${draft.text.length.toLocaleString()} / 20,000`;
  suggest();
}
function insert(text,choice){
  if(!engine||composing||!text)return;
  try{const next=replace(editor.current,text,choice);editor.push(next,{accepted:!!choice});active=0;syncEditor(true);input.focus();}
  catch(error){announce(errorMessage(error));}
}
function choose(index){const candidate=result?.candidates[index];if(candidate)insert(candidate.khmer??candidate.text,candidate);}
function currentSuggestions(){
  // A final touch selection can arrive on blur without repainting its click
  // target. Keyboard acceptance must still use the latest cursor context.
  if(!result||resultDraft!==editor.current){active=0;suggest();}
  return result;
}
function highlight(){
  $$('#suggestions .candidate').forEach((button,index)=>{button.classList.toggle('highlighted',index===active);button.setAttribute('aria-pressed',String(index===active));});
}
input.addEventListener('compositionstart',()=>{composing=true;result=undefined;clearSuggestions();});
input.addEventListener('compositionend',()=>{composing=false;editor.push({...editor.current,input:input.value});syncEditor();});
input.addEventListener('input',()=>{if(composing)return;editor.push({...editor.current,input:input.value});active=0;syncEditor();});
input.addEventListener('focus',currentSuggestions);
input.addEventListener('keydown',event=>{
  if(composing||event.isComposing||event.keyCode===229)return;
  if((event.metaKey||event.ctrlKey)&&event.key.toLowerCase()==='z'){event.preventDefault();event.shiftKey?editor.redo():editor.undo();syncEditor(true);return;}
  if(event.altKey||event.metaKey||event.ctrlKey){if(event.key==='Enter'&&(event.metaKey||event.ctrlKey)){event.preventDefault();insert(editor.current.input);}return;}
  if(!event.shiftKey&&(event.key==='ArrowDown'||event.key==='ArrowUp')){const current=currentSuggestions();if(current?.candidates.length){event.preventDefault();active=(active+(event.key==='ArrowDown'?1:-1)+current.candidates.length)%current.candidates.length;highlight();}}
  else if(event.key==='Enter'){event.preventDefault();if(!event.repeat){currentSuggestions();choose(active);}}
  else if(event.key===' '&&!event.shiftKey){
    event.preventDefault();if(event.repeat)return;
    const current=currentSuggestions();
    const candidate=editor.restoredAcceptance?undefined:spaceChoice(current,active,{acceptance:acceptance.value,...settings});
    insert(spaceText(editor.current.input,candidate,settings.language),candidate);
  }
});
function rememberSelection(event){
  if(composing)return;
  if(editor.current.start===message.selectionStart&&editor.current.end===message.selectionEnd&&editor.current.direction===message.selectionDirection)return;
  editor.select(message.selectionStart,message.selectionEnd,message.selectionDirection);
  // Blurring to a suggestion must not replace that button before its click.
  if(event.type!=='blur'&&document.activeElement===message)suggest();
}
message.addEventListener('select',rememberSelection);message.addEventListener('click',rememberSelection);message.addEventListener('keyup',rememberSelection);message.addEventListener('blur',rememberSelection);
message.addEventListener('compositionstart',()=>{composing=true;result=undefined;clearSuggestions();});
message.addEventListener('compositionend',()=>{composing=false;editor.push(editMessage(editor.current,message.value,message.selectionStart,message.selectionEnd,message.selectionDirection));syncEditor();});
message.addEventListener('input',event=>{if(event.isComposing||composing)return;editor.push(editMessage(editor.current,message.value,message.selectionStart,message.selectionEnd,message.selectionDirection));syncEditor();});
message.addEventListener('keydown',event=>{if(!event.isComposing&&(event.metaKey||event.ctrlKey)&&event.key.toLowerCase()==='z'){event.preventDefault();event.shiftKey?editor.redo():editor.undo();syncEditor(true);}});
function suggest() {
  if(!engine||composing)return;
  const target=$('#suggestions');clearSuggestions();$('#result-count').textContent='';result=undefined;resultDraft=undefined;
  const draft=editor.current;
  try {
    result=suggestDraft(engine,draft,settings);
    resultDraft=draft;
    active=Math.min(active,Math.max(0,result.candidates.length-1));
    $('#result-count').textContent=t('{count} results',{count:result.candidates.length});
    result.candidates.forEach((item,index)=>{
      const text=item.khmer??item.text,button=el('button',undefined,'candidate'),status=suggestionStatus(item,result);
      const word=el('span',text,'candidate-word'),dots=el('span',undefined,'candidate-statuses'),reading=settings.language==='khmer'?el('small',readingFor(item,result)||'—','candidate-reading'):null;
      dots.setAttribute('aria-hidden','true');
      for(const state of status.states){const dot=el('span',undefined,`status-dot status-${state.kind}`);dot.title=t(state.label);dots.append(dot);}
      button.dataset.statusCount=String(status.states.length);
      const description=status.states.map(state=>t(state.label)).join(' · ');button.title=description;button.append(word,dots);
      if(reading){reading.lang='en';button.append(reading);}
      button.lang=getUiLanguage();word.lang=settings.language==='khmer'?'km':'en';
      button.setAttribute('aria-label',t(reading&&reading.textContent!=='—'?'Insert {word}, {reading}, {states}':'Insert {word}, {states}',{word:text,reading:reading?.textContent,states:description}));
      button.setAttribute('aria-keyshortcuts','Shift+F10');
      button.addEventListener('mousedown',event=>event.preventDefault());
      const suggestionLanguage=settings.language;
      suggestionGestures.push(suggestionGesture(button,{manage:()=>{
        openWord(suggestionWord(dictionaryWords??{},engine.getWords(),suggestionLanguage,item));
      },insert:()=>{
        const current=editor.current;
        if(draft.text!==current.text||draft.input!==current.input){suggest();return;}
        if(draft.start!==current.start||draft.end!==current.end){
          // A touch/blur may report the final selection after rendering. Accept
          // this exact choice only if it remains valid at the new cursor.
          try{
            const fresh=suggestDraft(engine,current,settings);
            const candidate=fresh.candidates.find(hit=>JSON.stringify(hit)===JSON.stringify(item));
            if(candidate)insert(candidate.khmer??candidate.text,candidate);else suggest();
          }catch(error){announce(errorMessage(error));}
          return;
        }
        insert(text,item);
      }}));
      target.append(button);
    });
    highlight();
    if(!result.candidates.length)target.append(el('p',t(draft.input?'No suggestions for this input.':'Type a word to see suggestions.'),'empty'));
  }catch(error){target.append(el('p',t(errorMessage(error)),'error'));}
}
let dictionaryWords=null,dictionaryError='',wordPageIndex=0,dictionaryLoads=Promise.resolve();
function renderWordList() {
  $('#examples-open').hidden=wordLanguage!=='khmer';
  $('#missing-romanized').hidden=wordLanguage!=='khmer';
  $('#missing-romanized').setAttribute('aria-pressed',String(wordLanguage==='khmer'&&missingRomanized));
  const personal=engine?.getWords()??[],query=$('#personal-search').value;
  $('#missing-romanized').disabled=!dictionaryWords&&!personal.length;
  const searched=wordList(dictionaryWords??{},personal,wordLanguage,query);
  const matches=wordLanguage==='khmer'&&missingRomanized?wordsWithoutRomanized(searched):searched,rows=wordsForLetter(matches,wordLanguage,wordLetter);
  const page=wordPage(rows,wordPageIndex);wordPageIndex=page.page;
  const letters=$('#word-letters'),letterScroll=letters.dataset.language===wordLanguage?(letters.scrollLeft??0):0;
  letters.dataset.language=wordLanguage;letters.replaceChildren();letters.hidden=!dictionaryWords&&!personal.length;
  for(const {letter,count} of wordLetters(matches,wordLanguage)){
    const label=letter?letter==='other'?t('Other'):letter:t('All');
    const description=t(letter?letter==='other'?'Words with other initials':'Words starting with {letter}':'All letters',{letter});
    const button=el('button',undefined,'word-letter'),text=el('span',label);button.type='button';
    if(letter&&letter!=='other')text.lang=wordLanguage==='khmer'?'km':'en';button.append(text);
    button.dataset.letter=letter;button.disabled=Boolean(letter)&&count===0;
    button.setAttribute('aria-pressed',String(letter===wordLetter));button.setAttribute('aria-label',description);button.setAttribute('aria-controls','word-results');
    button.title=`${description} · ${t('{count} words',{count:count.toLocaleString()})}`;
    button.onclick=()=>goToWordLetter(letter);letters.append(button);
  }
  letters.scrollLeft=letterScroll;
  $('#word-count').textContent=dictionaryError?t('Dictionary unavailable'):dictionaryWords?t('{count} {language} words',{count:rows.length.toLocaleString(),language:t(wordLanguage==='khmer'?'Khmer':'English')}):t('Loading dictionary…');
  $('#reload-words').hidden=false;$('#reload-words').disabled=busy;$('#reload-words').textContent=t(dictionaryError?'Retry':'Refresh');
  $('#legacy-notice').hidden=!legacyWords.length&&damagedWords===null;$('#migrate-words').disabled=busy||!engine||!dictionaryWords||damagedWords!==null;
  const target=$('#personal-rows');target.replaceChildren();const compact=$('#compact-words');compact.replaceChildren();
  page.rows.forEach(row=>{
    const item=el('button',undefined,'compact-word');
    item.type='button';item.setAttribute('aria-haspopup','dialog');item.setAttribute('aria-label',t('Details for {word}',{word:row.word}));
    const word=el('span',row.word,'compact-word-text');word.lang=row.language==='khmer'?'km':'en';
    const keys=romanizedKeys(row);const reading=el('span',keys.join(', ')||(t(row.language==='khmer'?'No Romanized spelling':'English')),'compact-word-reading');
    const arrow=el('span','›','compact-word-arrow');arrow.setAttribute('aria-hidden','true');
    item.append(word,reading,arrow);item.onclick=()=>showWordDetails(row);compact.append(item);
    const tr=el('tr');tr.setAttribute('role','row');const cell=el('td',row.word);cell.lang=wordLanguage==='khmer'?'km':'en';
    const added=row.personalIndices.length>0;
    tr.append(cell,el('td',t(row.bundled?(added?'Dictionary + browser words':'Dictionary'):'Browser word')),el('td',keys.join(', ')||'—'),el('td',posLabel(row.partsOfSpeech)||'—'));
    const actions=el('div',undefined,'actions');
    const edit=el('button',undefined,'word-edit');edit.type='button';edit.title=t('Edit {word}',{word:row.word});edit.setAttribute('aria-label',t('Edit {word}',{word:row.word}));
    const icon=document.createElementNS('http://www.w3.org/2000/svg','svg'),path=document.createElementNS('http://www.w3.org/2000/svg','path');
    icon.setAttribute('viewBox','0 0 24 24');icon.setAttribute('aria-hidden','true');icon.setAttribute('focusable','false');
    path.setAttribute('d','m16 3 5 5-13 13H3v-5L16 3ZM14 5l5 5');icon.append(path);edit.append(icon);
    edit.disabled=busy||!engine||!dictionaryWords;edit.onclick=()=>openWord(row);actions.append(edit);
    const actionCell=el('td');actionCell.append(actions);tr.append(actionCell);
    [...tr.children].forEach((cell,index)=>{cell.setAttribute('role','cell');cell.dataset.label=t(['Word','Source','Romanized spellings','Part of speech','Actions'][index]);});target.append(tr);
  });
  $('#personal-empty').hidden=page.rows.length>0;
  $('#personal-empty h2').textContent=t(!dictionaryWords&&!dictionaryError?'Loading words…':'No matching words.');
  $('#personal-empty p').textContent=t(dictionaryError?'Your additions are still available. Retry to load the dictionary.':dictionaryWords?missingRomanized?'Try changing the search or filters.':wordLetter?'Try another search or choose All letters.':'Try another search or add a word.':'');
  $('#words-page').textContent=t('Page {page} of {pages}',{page:page.page+1,pages:page.pages});
  $('#words-prev').disabled=page.page===0;$('#words-next').disabled=page.page+1>=page.pages;
  $('#words-range').textContent=page.total?t(page.total===1?'{from}–{to} of {total} word':'{from}–{to} of {total} words',{from:page.from.toLocaleString(),to:page.to.toLocaleString(),total:page.total.toLocaleString()}):t('0 words');
  const numbers=$('#words-pages');numbers.replaceChildren();
  for(const number of wordPageLinks(page.page,page.pages)){
    if(number===null){const gap=el('span','…','page-ellipsis');gap.setAttribute('aria-hidden','true');numbers.append(gap);continue;}
    const button=el('button',String(number+1),'page-number');button.type='button';button.setAttribute('aria-label',t('Page {page}',{page:number+1}));button.setAttribute('aria-controls','word-results');
    if(number===page.page)button.setAttribute('aria-current','page');
    button.onclick=()=>goToWordPage(number);numbers.append(button);
  }
  $('#words-page-input').value=String(page.page+1);$('#words-page-input').max=String(page.pages);
  $('#words-page-total').textContent=t('of {pages}',{pages:page.pages});
  $('#words-page-input').disabled=page.pages===1;$('#words-jump-go').disabled=page.pages===1;
  for(const id of ['add-word','import','export']) $('#'+id).disabled=busy||!engine||!dictionaryWords;
}
function goToWordPage(index){
  if(index===wordPageIndex)return;
  wordPageIndex=index;renderWordList();focusWordResults();
}
function goToWordLetter(letter){
  if(letter===wordLetter)return;
  wordLetter=letter;wordPageIndex=0;renderWordList();focusWordResults();
}
function focusWordResults(){
  const results=$('#word-results');results.focus({preventScroll:true});results.scrollIntoView({block:'start'});
}
function showWordDetails(row){
  const dialog=$('#word-details'),content=$('#detail-content'),actions=$('#detail-actions');
  $('#detail-word').textContent=row.word;$('#detail-word').lang=row.language==='khmer'?'km':'en';content.replaceChildren();actions.replaceChildren();
  const list=el('dl',undefined,'word-detail-list');
  const field=(label,value)=>{list.append(el('dt',t(label)),el('dd',value));};
  field('Source',t(row.bundled?(row.personalIndices.length?'Dictionary + browser words':'Dictionary'):row.personalIndices.length?'Browser word':'Typing suggestion'));
  field('Part of speech',posLabel(row.partsOfSpeech)||t('Not set'));
  field('Word level',t(row.commonnessLevel?['','1 · Super common','2 · Very common','3 · Common','4 · Uncommon','5 · Academic'][row.commonnessLevel]:'Unrated'));
  if(row.language==='khmer'){
    field('Use default Sound map',t(row.soundMapEnabled===false?'Off · saved spellings only':'On'));
    field('Saved Romanized spellings',row.romanizations.join(', ')||t('None'));
    field('Shorthand',(row.shorthands??[]).join(', ')||t('None'));
    field('Excluded Romanized inputs',(row.excludedInputs??[]).join(', ')||t('None'));
    field('Sound map spellings',row.soundMapInputs.join(', ')||t('None'));
    if(row.soundMapInputsTruncated)field('More spellings',t('This word has additional generated readings. Try them in Typing.'));
    if(row.spellings.length)field('Previous browser spellings',row.spellings.join(', '));
    // A typing suggestion can be a multi-word composition rather than one
    // dictionary headword; component validation must not break its dialog.
    let analysis;
    if(engine&&!row.word.includes(' ')){
      try{analysis=engine.analyzeComponents(row.word);}
      catch{field('Components',t('Component analysis is unavailable.'));}
    }
    if(analysis&&analysis.status!=='unknown'){
      const text=parts=>parts.map(part=>part.wordId===undefined?`[${part.text}]`:part.text).join(' + ');
      field(analysis.source==='saved'?'Saved components':'Possible components',
        (analysis.status==='ambiguous'?analysis.alternatives.map(text).join(' / '):analysis.parts.length?text(analysis.parts):t('Standalone word'))+(analysis.moreAlternatives?' …':''));
      if(analysis.status==='partial'||analysis.alternatives.some(parts=>parts.some(part=>part.wordId===undefined)))field('Unresolved text',t('Parts in brackets are unresolved.'));
    }
  }
  content.append(list);
  field('Rank',String(row.rank??1));
  const edit=el('button',t(row.bundled?'Edit word':'Add word'),'primary');edit.disabled=busy||!engine||!dictionaryWords;
  edit.onclick=()=>{dialog.close();openWord(row);};actions.append(edit);
  dialog.showModal();
}
$('#close-details').onclick=()=>$('#word-details').close();
function componentFields(){
  const connected=!$('#word-components-section').hidden&&$('#component-mode').value==='connected';
  $('#component-parts-field').hidden=!connected;$('#component-parts').disabled=!connected;
}
$('#component-mode').onchange=()=>{
  if($('#component-mode').value==='connected'&&!$('#component-parts').value.trim()){
    try{
      const analysis=engine.analyzeComponents(editRow.word);
      const alternatives=analysis.status==='ambiguous'?analysis.alternatives:[analysis.parts];
      $('#component-parts').value=alternatives.filter(parts=>parts.length>=2&&parts.every(part=>part.wordId!==undefined))
        .map(parts=>parts.map(part=>part.text).join(' + ')).join('\n');
    }catch{}
  }
  componentFields();
};

async function loadWordList(catalog,words){
  const pending=dictionaryLoads.then(async()=>{
  words??=legacyWords;dictionaryError='';
  try{
    const loaded=await loadSharedDictionary({createEngine,catalog});
    try{if(words.length)loaded.engine.addWords(words);}
    catch(error){
      if(engine){loaded.engine.dispose();throw error;}
      damagedWords=JSON.stringify(words);words=[];announce('Previous browser words could not be loaded. Export their recovery copy before importing a corrected file.');
    }
    if(words.length!==legacyWords.length&&damagedWords===null){
      try{localStorage.setItem(WORDS_KEY,JSON.stringify(words));}
      catch(error){loaded.engine.dispose();throw new Error('Dictionary saved, but the previous browser copy could not be updated. Export its recovery copy before retrying.');}
    }
    const previous=engine;engine=loaded.engine;legacyWords=words;dictionaryWords=loaded.catalog;previous?.dispose();
    renderWordList();suggest();refreshExampleEditor();renderPronunciations();sentenceConverter?.refresh();
    if(map)renderMap({preserveDetails:true});
    return true;
  }catch(error){dictionaryError=errorMessage(error);announce(dictionaryError);renderWordList();return false;}
  });dictionaryLoads=pending;return pending;
}
$('#reload-words').onclick=()=>loadWordList();
$('#personal-search').oninput=()=>{wordPageIndex=0;renderWordList();};
$('#missing-romanized').onclick=()=>{if(wordLanguage!=='khmer')return;missingRomanized=!missingRomanized;wordPageIndex=0;renderWordList();focusWordResults();};
$$('[data-word-language]').forEach(button=>{button.onclick=()=>{wordLanguage=button.dataset.wordLanguage;wordLetter='';missingRomanized=false;wordPageIndex=0;$$('[data-word-language]').forEach(b=>b.setAttribute('aria-pressed',String(b===button)));renderWordList();};});
$('#words-prev').onclick=()=>goToWordPage(wordPageIndex-1);$('#words-next').onclick=()=>goToWordPage(wordPageIndex+1);
$('#words-jump').onsubmit=event=>{
  event.preventDefault();
  const field=$('#words-page-input'),page=Number(field.value);
  if(Number.isInteger(page)&&page>=1&&page<=Number(field.max))goToWordPage(page-1);
};
function spellingField(){
  const english=$('#word-language').value==='english';
  $('#spelling-field').hidden=english;$('#word-spellings').disabled=english;
  $('#shorthand-field').hidden=english;$('#word-shorthands').disabled=english;
  $('#excluded-inputs-field').hidden=english;$('#word-excluded-inputs').disabled=english;
  $('#word-sound-map-field').hidden=english;$('#word-sound-map').disabled=english;
  $('#word-generated-field').hidden=english||!editRow?.bundled;
  $('#word-components-section').hidden=english||!editRow?.bundled||editRow.word.includes(' ');
  $('#component-mode').disabled=$('#word-components-section').hidden;componentFields();
}
$('#word-language').onchange=spellingField;
function markExcludedReadings(){
  const excluded=new Set($('#word-excluded-inputs').value.split(',').map(input=>input.trim().toLowerCase()));
  for(const chip of $('#word-generated-inputs').children){
    const blocked=excluded.has(chip.textContent);
    chip.className=blocked?'key excluded-reading':'key';
    chip.title=blocked?t('Excluded Romanized inputs'):'';
    if(blocked)chip.setAttribute('aria-label',t('Excluded input {input}',{input:chip.textContent}));
    else chip.removeAttribute('aria-label');
  }
}
$('#word-excluded-inputs').oninput=markExcludedReadings;
function renderWordReadings(append=false){
  const list=$('#word-generated-inputs'),more=$('#word-generated-more'),status=$('#word-generated-status');
  if(!append){
    list.replaceChildren();
    list.scrollTop=0;
    more.dataset.after='';
    more.hidden=true;
  }
  status.textContent='';
  if($('#word-generated-field').hidden)return;
  try{
    const page=engine.soundMapInputs(editRow.word,more.dataset.after);
    for(const input of page.inputs){
      const chip=el('span',input,'key');
      chip.lang='en';list.append(chip);
    }
    if(!list.children.length)list.textContent=t('None');
    more.dataset.after=page.nextAfter??'';
    more.hidden=!page.nextAfter;
    setUiText(more,'Show more');
    markExcludedReadings();
  }catch(error){
    status.textContent=t(errorMessage(error));
    more.hidden=false;
    setUiText(more,'Retry');
  }
}
$('#word-generated-more').onclick=()=>renderWordReadings(Boolean($('#word-generated-more').dataset.after));
function openWord(row){
  editRow=row??null;editCatalog=dictionaryWords;$('#word-form').reset();
  const legacy=row?.personalIndices.length?legacyWords[row.personalIndices[0]]:null;
  $('#dialog-title').textContent=t(row?.bundled?'Edit word':'Add word');
  $('#word-text').value=row?.word??'';$('#word-language').value=row?.language??wordLanguage;
  $('#word-text').readOnly=!!row?.bundled;$('#word-language').disabled=!!row?.bundled;
  $('#word-spellings').value=[...new Set([...(row?.romanizations??[]),...(row?.spellings??[])])].join(', ');
  $('#word-shorthands').value=(row?.shorthands??[]).join(', ');
  $('#word-excluded-inputs').value=(row?.excludedInputs??[]).join(', ');
  $('#word-rank').value=legacy?.rank??row?.rank??1;
  $('#word-level').value=String(row?.commonnessLevel??'');
  $('#word-sound-map').checked=row?.soundMapEnabled!==false;
  $('#component-mode').value=row?.components==null?'default':row.components.length?'connected':'standalone';
  $('#component-parts').value=(row?.components??[]).map(parts=>parts.join(' + ')).join('\n');
  editInitial={rank:Number($('#word-rank').value),spellings:$('#word-spellings').value.split(',').map(value=>value.trim()).filter(Boolean),components:row?.components??null,excludedInputs:row?.excludedInputs??[],shorthands:row?.shorthands??[]};
  $('#review-field').hidden=!!row?.bundled;$('#word-reviewed').required=!row?.bundled;
  $('#form-error').textContent='';spellingField();renderWordReadings();$('#word-dialog').showModal();$('#word-text').focus();
}
$('#add-word').onclick=()=>openWord();
for(const id of ['close-dialog','cancel-dialog']) $('#'+id).onclick=()=>$('#word-dialog').close();
async function persist(request,base=dictionaryWords){
  if(busy)throw new Error('Please wait for the current save to finish.');
  busy=true;renderWordList();$('#save-word').disabled=true;
  try{
    const catalog=await saveSharedDictionary({catalog:base,request});
    const changed=request.type==='import'?request.words:request.preservePersonal?[request.word]:[];
    const words=legacyWords.filter(word=>!changed.some(saved=>(saved.language??'khmer')===(word.language??'khmer')&&saved.word===word.word));
    if(!await loadWordList(catalog,words))throw new Error('Dictionary saved, but this page could not reload it. Press Refresh to load the saved files.');
  }finally{busy=false;renderWordList();$('#save-word').disabled=false;}
}
$('#word-form').onsubmit=async event=>{
  event.preventDefault();$('#form-error').textContent='';
  if(busy)return;
  const word={word:$('#word-text').value.trim(),language:$('#word-language').value};
  const rank=Number($('#word-rank').value),spellings=word.language==='english'?[]:$('#word-spellings').value.split(',').map(value=>value.trim()).filter(Boolean);
  if(String($('#word-rank').value).trim()===''||!Number.isSafeInteger(rank)||rank< -1000000||rank>1000000){
    $('#form-error').textContent=t('Rank must be an integer from -1000000 to 1000000.');$('#word-rank').focus();return;
  }
  const rankChanged=rank!==editInitial.rank,spellingsChanged=JSON.stringify(spellings)!==JSON.stringify(editInitial.spellings);
  if(!editRow?.bundled||rankChanged)word.rank=rank;
  if(!editRow?.bundled||spellingsChanged)word.spellings=spellings;
  const level=$('#word-level').value?Number($('#word-level').value):null;
  if(level!==(editRow?.commonnessLevel??null))word.commonnessLevel=level;
  const shorthands=[...new Set($('#word-shorthands').value.split(',').map(value=>value.trim().toLowerCase()).filter(Boolean))].sort();
  if(word.language==='khmer'&&JSON.stringify(shorthands)!==JSON.stringify([...editInitial.shorthands].sort()))word.shorthands=shorthands;
  const excludedInputs=[...new Set($('#word-excluded-inputs').value.split(',').map(value=>value.trim().toLowerCase()).filter(Boolean))].sort();
  if(word.language==='khmer'&&JSON.stringify(excludedInputs)!==JSON.stringify([...editInitial.excludedInputs].sort()))word.excludedInputs=excludedInputs;
  if(word.language==='khmer'&&$('#word-sound-map').checked!==(editRow?.soundMapEnabled!==false))word.soundMapEnabled=$('#word-sound-map').checked;
  try{
    if(!$('#word-components-section').hidden){
      const mode=$('#component-mode').value;
      const components=mode==='default'?null:mode==='standalone'?[]:$('#component-parts').value.split('\n').map(line=>line.trim()).filter(Boolean).map(line=>line.split('+').map(part=>part.trim()));
      if(mode==='connected'&&!components.length)throw new Error('Enter at least one complete component list.');
      if(JSON.stringify(components)!==JSON.stringify(editInitial.components))word.components=components;
    }
    const preservePersonal=!!editRow?.personalIndices.length&&(rankChanged||spellingsChanged||!editRow.bundled);
    if(preservePersonal&&word.rank===undefined)word.rank=rank;
    if(!editRow?.bundled||Object.keys(word).length>2)await persist({type:'save',word,original:editRow?.bundled?{word:editRow.word,language:editRow.language}:null,reviewed:$('#word-reviewed').checked,preservePersonal},editCatalog);
    wordLanguage=word.language;wordLetter='';missingRomanized=false;wordPageIndex=0;$('#personal-search').value=word.word;
    $$('[data-word-language]').forEach(button=>button.setAttribute('aria-pressed',String(button.dataset.wordLanguage===wordLanguage)));
    renderWordList();$('#word-dialog').close();announce('Saved {word} to the dictionary.',{word:word.word});
  }catch(error){$('#form-error').textContent=t(errorMessage(error));}
};

function download(text,name){const url=URL.createObjectURL(new Blob([text],{type:'application/json'}));const anchor=el('a');anchor.href=url;anchor.download=name;anchor.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
$('#export').onclick=()=>download(wordExport(dictionaryWords),'kmeow-dictionary-words.json');
$('#export-legacy').onclick=()=>download(damagedWords??JSON.stringify(legacyWords,null,2),'kmeow-browser-words-recovery.json');
async function importSharedWords(words,preservePersonal=false,preferences){
  if(!confirm(t('Import {count} reviewed words into the shared dictionary? Confirm that you have reviewed every new word for suitability. Existing words stay available.',{count:words.length})))return;
  await persist({type:'import',words,reviewed:true,preservePersonal,preferences});announce('Imported {count} words into the dictionary.',{count:words.length});
}
$('#migrate-words').onclick=async()=>{try{await importSharedWords(legacyWords,true);}catch(error){announce(errorMessage(error));}};
$('#import').onclick=()=>$('#import-file').click();
$('#import-file').onchange=async event=>{
  const file=event.target.files[0];event.target.value='';if(!file)return;
  try{if(file.size>2*1024*1024)throw new Error('Choose a JSON file smaller than 2 MB.');const spec=parseImportSpec(await file.text());await importSharedWords(spec.words,spec.preservePersonal,spec.preferences);}
  catch(error){announce('Import failed: {error}',{error:uiMessage(errorMessage(error))});}
};
const groups={consonants:'Consonants',dependentVowels:'Dependent vowels',additionalVowelForms:'Vowel combinations',independentVowels:'Independent vowels'};
let ruleDetailPath=[],ruleDetailTrigger=null;
function renderRuleDetail(){
  const current=ruleDetailPath.at(-1);if(!current)return;
  $('#rule-detail-title').textContent=current.title();
  const context=ruleDetailPath[0].context;
  $('#rule-detail-path').textContent=[...(context?[context()]:[]),...ruleDetailPath.slice(0,-1).map(item=>item.title())].join(' › ');
  $('#rule-detail-back').hidden=ruleDetailPath.length<2;
  const content=$('#rule-detail-content');content.replaceChildren();
  if(current.description)content.append(el('p',current.description(),'helper'));
  if(current.examples?.length)content.append(renderRuleExamples(current.examples));
  if(Object.hasOwn(current,'value'))content.append(renderRuleValue(current.value,current.field??'',document,dictionaryWords?.khmer??[],showRuleDetail));
}
function showRuleDetail(detail,trigger){
  const dialog=$('#rule-details');
  if(!dialog.open){ruleDetailPath=[];ruleDetailTrigger=trigger;}
  else if(ruleDetailPath.length){
    ruleDetailPath.at(-1).navigation={scrollTop:dialog.scrollTop,key:trigger?.getAttribute('data-rule-trigger'),
      index:[...$('#rule-detail-content').querySelectorAll('button')].indexOf(trigger)};
  }
  ruleDetailPath.push({...detail});renderRuleDetail();
  if(!dialog.open)dialog.showModal();
  dialog.scrollTop=0;$('#rule-detail-title').focus({preventScroll:true});
}
$('#rule-detail-back').onclick=()=>{
  if(ruleDetailPath.length<2)return;
  ruleDetailPath.pop();renderRuleDetail();
  const navigation=ruleDetailPath.at(-1).navigation,buttons=[...$('#rule-detail-content').querySelectorAll('button')];
  const indexed=buttons[navigation?.index],target=indexed?.getAttribute('data-rule-trigger')===navigation?.key?indexed:
    buttons.find(button=>button.getAttribute('data-rule-trigger')===navigation?.key);
  $('#rule-details').scrollTop=navigation?.scrollTop??0;(target??$('#rule-detail-title')).focus({preventScroll:true});
};
$('#rule-detail-close').onclick=()=>$('#rule-details').close();
$('#rule-details').addEventListener('close',()=>{
  const key=ruleDetailTrigger?.getAttribute('data-rule-trigger');
  const target=ruleDetailTrigger?.isConnected?ruleDetailTrigger:[...$('#sound-cards').querySelectorAll('button')].find(node=>node.getAttribute('data-rule-trigger')===key);
  ruleDetailPath=[];ruleDetailTrigger=null;(target??$('#sound-search')).focus({preventScroll:true});
});
const keys=value=>(value??[]).map(item=>typeof item==='string'?item:item.key);
function keyRow(values,series){
  const row=el('div',undefined,'keys');
  if(series){
    const badge=el('span',series==='A/O'?undefined:series,`series ${series==='O'?'o':series==='A/O'?'ao':''}`);
    if(series==='A/O'){
      badge.append(el('span','A'),el('span','O'));
      badge.setAttribute('aria-label',t('A and O series, same keys'));badge.title=t('Same keys for A and O series');
    }
    row.append(badge);
  }
  const valuesToShow=keys(values);
  for(const value of valuesToShow)row.append(el('span',value,'key'));
  if(!valuesToShow.length)row.append(el('span',t('Context only'),'muted'));
  return row;
}
const pronunciationEditor=pronunciationSection({document,
  preview:readAs=>engine.previewPronunciation(readAs),
  onSave:async rules=>{
    const catalog=await saveSharedDictionary({catalog:dictionaryWords,request:{type:'pronunciations',rules}});
    if(!await loadWordList(catalog))throw new Error('Dictionary saved, but this page could not reload it. Press Refresh to load the saved files.');
  }
});
$('#pronunciation-editor').append(pronunciationEditor.root);
function renderPronunciations(){
  const query=$('#pronunciation-search').value.trim().toLowerCase();
  pronunciationEditor.update(dictionaryWords?.pronunciations??[],query,Boolean(map&&engine&&dictionaryWords),[...(map?.pronunciationSyllables??[]),...(map?.runtime?.pronunciationParts??[])]);
}
$('#pronunciation-search').oninput=renderPronunciations;
function renderMap({preserveDetails=false}={}){
  if(!map)return;
  const query=$('#sound-search').value.trim().toLowerCase(),group=$('#sound-group').value,target=$('#sound-cards');
  const detailStates=preserveDetails?new Map([...target.querySelectorAll('details')].map(node=>[node.getAttribute('data-sound-detail'),Boolean(node.open)])):null;
  const pronunciationLink=$('#pronunciation-open');
  target.replaceChildren();let hasMappings=false;
  for(const [id,title] of Object.entries(groups)){
    if(group!=='all'&&group!==id)continue;
    const rows=map[id].filter(row=>[row.khmer,...keys(row.keys),...keys(row.onsetKeys),...keys(row.aSeriesKeys),...keys(row.oSeriesKeys)].join(' ').toLowerCase().includes(query));
    if(!rows.length)continue;
    hasMappings=true;
    const section=el('section',undefined,'sound-section');section.append(el('h2',`${t(title)} · ${rows.length}`));const grid=el('div',undefined,'sound-grid');
    for(const row of rows){
      const card=el('article',undefined,'sound-card'),top=el('div',undefined,'sound-top');const glyph=el('span',(id==='dependentVowels'||id==='additionalVowelForms'?'◌':'')+row.khmer,'glyph');glyph.lang='km';top.append(glyph);
      if(row.series)top.append(el('span',t('{series}-series',{series:row.series.toUpperCase()}),`series ${row.series.toLowerCase()==='o'?'o':''}`));card.append(top);
      const mainKeys=row.onsetKeys??row.keys;
      const finalKeys=keys(row.codaFallbackKeys);
      if(row.aSeriesKeys||row.oSeriesKeys){
        const shared=JSON.stringify(keys(row.aSeriesKeys))===JSON.stringify(keys(row.oSeriesKeys));
        if(shared)card.append(keyRow(row.aSeriesKeys,'A/O'));
        else card.append(keyRow(row.aSeriesKeys,'A'),keyRow(row.oSeriesKeys,'O'));
      }else card.append(keyRow(mainKeys));
      if(finalKeys.length&&JSON.stringify(finalKeys)!==JSON.stringify(keys(mainKeys))){
        const details=el('details');
        details.setAttribute('data-sound-detail',`character/${id}/${row.khmer}`);
        details.append(el('summary',t('Character details')),el('p',t('Final keys'),'helper'),keyRow(row.codaFallbackKeys));
        card.append(details);
      }
      grid.append(card);
    }
    section.append(grid);target.append(section);
  }
  target.append(pronunciationLink);
  const families=group==='all'||group==='rules'?ruleFamilies(map.runtime,query,dictionaryWords?.khmer??[]):[];
  if(families.length){
    const section=el('section',undefined,'sound-rules');
    section.append(el('h2',t('Rules · {count} families',{count:families.length})));
    const list=el('div',undefined,'sound-rule-list');
    for(const family of families){
      const familyRow=el('details',undefined,'sound-family');familyRow.open=Boolean(query);
      familyRow.setAttribute('data-sound-detail',`family/${family.id}`);
      familyRow.append(el('summary',family.title));
      for(const rule of family.rules){
        const guide=()=>soundRules(map.runtime).find(item=>item.id===rule.id)??rule;
        if(rule.id==='shorthands'&&Array.isArray(rule.value)){
          familyRow.append(el('p',rule.description,'helper'));
          if(rule.examples.length){
            const examples=el('button',undefined,'rule-link');examples.type='button';examples.setAttribute('aria-haspopup','dialog');examples.setAttribute('data-rule-trigger',`rule/${rule.id}/examples`);
            const arrow=el('span','›','rule-link-arrow');arrow.setAttribute('aria-hidden','true');examples.append(el('span',t('Examples'),'rule-link-title'),arrow);
            examples.onclick=()=>showRuleDetail({title:()=>t('Examples'),context:()=>guide().title,description:()=>guide().description,examples:rule.examples},examples);
            familyRow.append(examples);
          }
          const rows=renderRuleValue(rule.value,'',document,dictionaryWords?.khmer??[],(detail,trigger)=>showRuleDetail({...detail,context:()=>guide().title},trigger));
          rows.className+=' rule-part-list';rows.tabIndex=0;rows.setAttribute('role','region');rows.setAttribute('aria-label',rule.title);familyRow.append(rows);
        }else{
          const row=el('button',undefined,'rule-link');row.type='button';row.setAttribute('aria-haspopup','dialog');row.setAttribute('data-rule-trigger',`rule/${rule.id}`);
          const arrow=el('span','›','rule-link-arrow');arrow.setAttribute('aria-hidden','true');row.append(el('span',rule.title,'rule-link-title'),arrow);
          row.onclick=()=>showRuleDetail({title:()=>guide().title,description:()=>guide().description,value:rule.value,examples:rule.examples},row);
          familyRow.append(row);
        }
      }
      list.append(familyRow);
    }
    section.append(list);target.append(section);
  }
  if(detailStates)for(const node of target.querySelectorAll('details')){
    const key=node.getAttribute('data-sound-detail');if(detailStates.has(key))node.open=detailStates.get(key);
  }
  if($('#rule-details').open)renderRuleDetail();
  if(!hasMappings&&!families.length)target.append(el('p',t('No matching mappings or rules.'),'empty'));
}
$('#sound-search').oninput=renderMap;$('#sound-group').onchange=renderMap;
async function start(){
  sentenceConverter=attachRomanizedConverter({document,getEngine:()=>engine,
    onManage:candidate=>openWord(suggestionWord(dictionaryWords??{},engine.getWords(),'khmer',candidate))});
  language('khmer');
  let stored;
  try{stored=localStorage.getItem(WORDS_KEY);if(stored){const parsed=JSON.parse(stored);legacyWords=Array.isArray(parsed)&&!parsed.length?[]:parseImport(stored);}}
  catch(error){damagedWords=stored??'[]';announce(`Previous browser words could not be read. Export a recovery copy before importing a corrected file.`);}
  if(await loadWordList()){
    for(const id of ['typing-input','message'])$('#'+id).disabled=false;
    syncEditor();
    if(damagedWords===null)announce();
  }
  try{
    const response=await fetch('/data/khmer-sound-map.json');
    if(!response.ok)throw new Error('Sound map unavailable');
    map=await response.json();
    renderMap();renderPronunciations();
  }catch(error){
    for(const target of ['#sound-cards','#pronunciation-editor'])$(target).append(el('p',t(errorMessage(error)),'error'));
  }
}
let initialUiLanguage;
try{initialUiLanguage=readUiLanguage(localStorage,navigator.language);}
catch{initialUiLanguage=String(navigator.language).toLowerCase().startsWith('km')?'km':'en';}
applyUiLanguage(initialUiLanguage,false);
start();
