import {convertExample,trackExampleInput,exampleChanged,exampleWordAt,exampleSuggestions,chooseExampleWord,renderParagraph,paragraphSuggestion,paragraphWordAt} from './examples.js';
import {suggestionStatuses} from './suggestion-view.js';
import {suggestionGesture} from './suggestion-gesture.js';
import {t} from './i18n.js';

// Sentence editing and explicit choices only; conversion and ranking stay in the engine.
export function attachRomanizedConverter({document,getEngine,onManage}){
  const $=id=>document.querySelector('#converter-'+id),source=$('input'),output=$('output');
  let draft={romanized:'',khmer:'',spans:[]},error='',active=false,composing=false,beforeInput=null;
  let target=null,cursorField=source,gestures=[],position=0;
  const snapshots=[{draft,error,start:0,end:0,direction:'none'}];
  const node=(tag,text,className)=>{const item=document.createElement(tag);if(text!==undefined)item.textContent=text;if(className)item.className=className;return item;};
  const snapshot=()=>({draft,error,start:source.selectionStart,end:source.selectionEnd,direction:source.selectionDirection});
  function rememberSelection(){Object.assign(snapshots[position],{start:source.selectionStart,end:source.selectionEnd,direction:source.selectionDirection});}
  function remember(){
    const current=snapshots[position];
    if(!exampleChanged(draft,current.draft)&&error===current.error){rememberSelection();return;}
    snapshots.splice(position+1);snapshots.push(snapshot());position++;
    while(snapshots.length>2&&(snapshots.length>100||snapshots.reduce((size,item)=>size+JSON.stringify(item).length,0)>2*1024*1024)){
      const index=position>1?1:snapshots.length-1;snapshots.splice(index,1);if(index<position)position--;
    }
  }
  function clearSuggestions(){for(const dispose of gestures)dispose();gestures=[];$('suggestions').replaceChildren();}
  function wordAtCursor(){
    if(composing||error||source.value!==draft.romanized)return null;
    return paragraphWordAt(draft,cursorField===source?'romanized':'khmer',cursorField.selectionStart,cursorField.selectionEnd);
  }
  const sameWord=(left,right)=>left&&right&&left.start===right.start&&left.end===right.end&&left.input===right.input;
  function renderSuggestions(){
    clearSuggestions();$('suggestion-panel').hidden=!active||!getEngine()||composing||!!error||!target;
    if($('suggestion-panel').hidden)return;
    try{
      const engine=getEngine(),value=draft,selection=exampleSuggestions(engine,value,target.start,target.end);
      if(!selection||!sameWord(selection.target,target)){$('suggestion-panel').hidden=true;return;}
      $('suggestion-title').textContent=t('Suggestions for {input}',{input:target.input});
      for(const candidate of selection.result.candidates){
        const button=paragraphSuggestion(node,candidate,selection.result,target,{statuses:true});
        const current=()=>active&&!composing&&!error&&getEngine()===engine&&draft===value&&source.value===value.romanized&&sameWord(wordAtCursor(),selection.target);
        gestures.push(suggestionGesture(button,{manage:()=>{if(current())onManage?.(candidate);},insert:()=>{
          if(!current()){selectWord();return;}
          try{
            rememberSelection();const next=chooseExampleWord(engine,value,selection.target,candidate);
            draft=next;error='';
            if(source.value!==next.romanized||cursorField===output){
              if(source.value!==next.romanized)source.value=next.romanized;
              const end=selection.target.end+next.romanized.length-value.romanized.length;source.setSelectionRange(end,end);
            }
            cursorField=source;target=exampleWordAt(draft,selection.target.start);remember();render();source.focus({preventScroll:true});
          }catch(problem){error=problem.message??String(problem);render();}
        }}));
        $('suggestions').append(button);
      }
      if(!selection.result.candidates.length)$('suggestions').append(node('p',t('No suggestions for this input.'),'empty'));
    }catch(problem){$('suggestions').append(node('p',t(problem.message??String(problem)),'error'));}
  }
  function render(){
    source.disabled=output.disabled=!active||!getEngine();
    renderParagraph({element:output,draft:error?{...draft,khmer:''}:draft,language:'khmer',textField:true});
    $('count').textContent=`${source.value.length.toLocaleString()} / 20,000`;
    $('error').textContent=t(error);
    $('legend').replaceChildren();
    for(const state of suggestionStatuses){
      const item=node('span',undefined,'legend-item'),dot=node('span',undefined,`status-dot status-${state.kind}`);
      dot.setAttribute('aria-hidden','true');item.append(dot,node('span',t(state.label)));$('legend').append(item);
    }
    renderSuggestions();
  }
  function selectWord(event){
    if(!active)return;
    if(event)cursorField=event.currentTarget??source;
    if(cursorField===source)rememberSelection();
    target=wordAtCursor();renderSuggestions();
  }
  function update(event){
    if(composing||event?.isComposing||!getEngine())return;
    draft=trackExampleInput(draft,source.value,beforeInput,source.selectionStart,source.selectionEnd);beforeInput=null;
    try{draft=convertExample(getEngine(),draft,source.value);error='';}
    catch(problem){error=problem.message??String(problem);}
    cursorField=source;target=wordAtCursor();remember();render();
  }
  function moveHistory(step){
    if(composing||position+step<0||position+step>=snapshots.length)return;
    rememberSelection();position+=step;
    const restored=snapshots[position];draft=restored.draft;error=restored.error;source.value=draft.romanized;
    source.setSelectionRange(restored.start,restored.end,restored.direction);cursorField=source;target=wordAtCursor();beforeInput=null;render();source.focus({preventScroll:true});
  }
  source.addEventListener('beforeinput',event=>{
    if(composing)return;
    if(event.inputType==='historyUndo'||event.inputType==='historyRedo'){event.preventDefault();moveHistory(event.inputType==='historyUndo'?-1:1);return;}
    rememberSelection();beforeInput={text:source.value,start:source.selectionStart,end:source.selectionEnd,inputType:event.inputType};
  });
  source.addEventListener('input',update);
  source.addEventListener('keydown',event=>{
    if(!composing&&!event.isComposing&&(event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='z'){event.preventDefault();moveHistory(event.shiftKey?1:-1);}
  });
  source.addEventListener('compositionstart',()=>{rememberSelection();beforeInput={text:source.value,start:source.selectionStart,end:source.selectionEnd,inputType:'insertCompositionText'};composing=true;target=null;render();});
  source.addEventListener('compositionend',()=>{composing=false;update();});
  for(const field of [source,output])for(const event of ['click','keyup','select','selectionchange','focus'])field.addEventListener(event,selectWord);
  return {setActive(value){active=value;render();},refresh(){
    if(getEngine()&&!composing){try{draft=convertExample(getEngine(),draft,source.value);error='';}catch(problem){error=problem.message??String(problem);}}
    if(draft.romanized===snapshots[position].draft.romanized)Object.assign(snapshots[position],{draft,error});
    render();
  }};
}
