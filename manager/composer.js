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
export function spaceChoice(result,active){
  const selected=result?.candidates[active];
  const matches=(a,b)=>a.mappingId===b.mappingId&&(a.khmer??a.text)===(b.khmer??b.text);
  const exact=result?.exactCandidates??[];
  const candidate=exact.find(c=>selected&&!selected.completion&&matches(c,selected))??exact[0];
  if(!candidate||candidate.tapOnly||candidate.manualOnly||candidate.completion)return;
  if(result.candidates.some(c=>matches(c,candidate)&&(c.tapOnly||c.manualOnly)))return;
  return candidate;
}
export function literalSpace(input){return /^[\u1780-\u17ff]+$/.test(input)?input:input+' ';}
export function history(initial=emptyDraft()){
  let current=initial;const past=[],future=[];
  return {get current(){return current;},select(start,end,direction='none'){current={...current,start,end,direction};},push(next){
    // Browsers may emit a final input after compositionend. Selection changes
    // and unchanged content do not create an undo step or discard redo.
    if(next.text===current.text&&next.input===current.input&&JSON.stringify(next.spans)===JSON.stringify(current.spans)){current=next;return;}
    past.push(current);if(past.length>100)past.shift();future.length=0;current=next;
  },undo(){if(past.length){future.push(current);current=past.pop();}return current;},redo(){if(future.length){past.push(current);current=future.pop();}return current;},get canUndo(){return past.length>0;},get canRedo(){return future.length>0;}};
}
