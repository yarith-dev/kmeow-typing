// Editor state only. Suggestions and ranking remain in the shared engine.
export const MAX_TEXT = 20000;
export const emptyDraft = () => ({text:'',input:'',start:0,end:0,direction:'none',spans:[]});
export function replace(draft,text,choice){
  if(draft.text.length-(draft.end-draft.start)+text.length>MAX_TEXT)throw new Error('The message is full (20,000 characters). Copy it before starting another.');
  const delta=text.length-(draft.end-draft.start);
  const spans=draft.spans.flatMap(span=>draft.end<=span.start?[{...span,start:span.start+delta,end:span.end+delta}]:draft.start>=span.end?[span]:[]);
  if(choice&&/^[\u1780-\u17ff]+$/.test(text))spans.push({start:draft.start,end:draft.start+text.length,word:text});
  spans.sort((a,b)=>a.start-b.start);
  return {...draft,text:draft.text.slice(0,draft.start)+text+draft.text.slice(draft.end),input:'',start:draft.start+text.length,end:draft.start+text.length,direction:'none',spans};
}
export function editMessage(draft,text,start,end,direction='none'){
  if(text.length>MAX_TEXT)throw new Error('The message is full (20,000 characters).');
  if(text===draft.text)return {...draft,start,end,direction};
  // Find the actual edited range. Preserve and shift intact choices outside it,
  // but never turn pasted or directly typed text into a committed choice.
  let from=0,oldEnd=draft.text.length,newEnd=text.length;
  while(from<oldEnd&&from<newEnd&&draft.text[from]===text[from])from++;
  while(oldEnd>from&&newEnd>from&&draft.text[oldEnd-1]===text[newEnd-1]){oldEnd--;newEnd--;}
  const updated=replace({...draft,start:from,end:oldEnd},text.slice(from,newEnd));
  return {...updated,input:draft.input,start,end,direction};
}
export function contextFor(draft,language){
  // Rust counts Unicode scalar values, not UTF-16 code units. Keep the text
  // nearest the cursor without splitting emoji or other supplementary letters.
  if(language==='english')return {before:[...draft.text.slice(0,draft.start)].slice(-512).join(''),after:[...draft.text.slice(draft.end)].slice(0,128).join('')};
  let cursor=draft.start;const words=[];
  for(let i=draft.spans.length-1;i>=0&&words.length<63;i--){const span=draft.spans[i];if(span.start>=cursor)continue;if(span.end!==cursor||draft.text.slice(span.start,span.end)!==span.word)break;words.unshift(span.word);cursor=span.start;}
  return {previousWords:words};
}
export function suggestDraft(engine,draft,settings){
  // Empty input can still produce contextual next-word suggestions.
  return engine.suggest(draft.input,{...settings,...contextFor(draft,settings.language)});
}
export function spaceChoice(result,active,{acceptance='auto',language='khmer',correction=false}={}){
  if(acceptance!=='auto'||!result||result.kind==='next-word'||result.input===''||result.normalizedInput==='')return;
  const candidates=result.candidates??[],selected=candidates[active];
  const eligible=candidate=>candidate&&!candidate.tapOnly&&!candidate.manualOnly;
  const matches=(a,b)=>a.mappingId===b.mappingId&&(a.khmer??a.text)===(b.khmer??b.text);
  const exact=result.exactCandidates??[];
  if(language==='english'){
    const input=result.input??result.normalizedInput??'';
    const permitsCase=input.length>1&&input===input.toUpperCase()||!/[A-Z]/.test(input.slice(1));
    if(!permitsCase)return;
  }
  // Arrow navigation chooses among completions only when there is no exact
  // match. A protected highlighted choice must not accept a different word.
  if(active>0&&!exact.length){
    if(!eligible(selected)||selected.correction&&!(language==='english'&&(correction||selected.wordAlternative)))return;
    return selected;
  }
  if(language==='english'){
    // Mobile applies an eligible contextual English replacement before
    // ordinary completion. Khmer typo recovery always requires a choice.
    const first=candidates[0];
    if(eligible(first)&&(first.wordAlternative||correction&&first.correction))return first;
    if(candidates.some(candidate=>candidate.manualOnly))return;
  }
  const candidate=exact.find(c=>selected&&!selected.completion&&matches(c,selected))??exact[0];
  if(candidate){
    if(!eligible(candidate)||candidates.some(c=>matches(c,candidate)&&!eligible(c)))return;
    return candidate;
  }
  // Keep the engine's order. A rare first completion never silently falls
  // through to a different word farther down the list.
  const completion=candidates.find(candidate=>!candidate.correction&&!candidate.manualOnly);
  return eligible(completion)?completion:undefined;
}
export function literalSpace(input){return input+' ';}
export function spaceText(input,candidate,language){
  if(!candidate)return literalSpace(input);
  let text=candidate.khmer??candidate.text;
  if(language!=='english')return text;
  if(input.length>1&&input===input.toUpperCase())text=text.toUpperCase();
  else if(/^[A-Z]/.test(input))text=text[0].toUpperCase()+text.slice(1);
  return text+' ';
}
export function history(initial=emptyDraft()){
  let current=initial,accepted=false,restoredAcceptance=false;const past=[],future=[];
  return {get current(){return current;},get restoredAcceptance(){return restoredAcceptance;},select(start,end,direction='none'){
    if(start!==current.start||end!==current.end||direction!==current.direction)restoredAcceptance=false;
    current={...current,start,end,direction};
  },push(next,{accepted:choice=false}={}){
    // Browsers may emit a final input after compositionend. Selection changes
    // and unchanged content do not create an undo step or discard redo.
    if(next.text===current.text&&next.input===current.input&&JSON.stringify(next.spans)===JSON.stringify(current.spans)){current=next;return;}
    past.push({draft:current,accepted});if(past.length>100)past.shift();future.length=0;current=next;accepted=choice;restoredAcceptance=false;
  },undo(){if(past.length){future.push({draft:current,accepted});restoredAcceptance=accepted;({draft:current,accepted}=past.pop());}return current;},redo(){if(future.length){past.push({draft:current,accepted});({draft:current,accepted}=future.pop());restoredAcceptance=false;}return current;},get canUndo(){return past.length>0;},get canRedo(){return future.length>0;}};
}
