// Presentation only. Preserve the engine's candidate objects, order and flags.
export const suggestionStatuses = [
  {kind:'match',label:'Match'},
  {kind:'completion',label:'Completion'},
  {kind:'correction',label:'Correction'},
  {kind:'explicit',label:'Select explicitly'},
];
export function suggestionStatus(candidate,result){
  const kinds=new Set();
  // The engine also supplies completion as the lookup spelling for exact
  // English/direct-Khmer hits. An unchanged spelling is a match, not an expansion.
  const input=(result.normalizedInput??'').toLowerCase();
  if(candidate.completion&&candidate.completion.toLowerCase()!==input)kinds.add('completion');
  if(candidate.correction)kinds.add('correction');
  if(!kinds.size)kinds.add('match');
  if(candidate.tapOnly||candidate.manualOnly)kinds.add('explicit');
  const states=suggestionStatuses.filter(status=>kinds.has(status.kind));
  return {states,description:states.map(status=>status.label).join(' · ')};
}
const latin = value => typeof value==='string' && /[a-z]/i.test(value) && /^[a-z '\-]+$/i.test(value);
export function suggestionReading(candidate,result,language,readings=[],personal=[]){
  if(language==='english')return candidate.text??candidate.completion??'';
  // The engine's completed/corrected input is the most precise reading. A
  // direct Khmer or next-word query instead uses its existing dictionary keys.
  const choices=[candidate.correction,candidate.preferredInput,candidate.completion,result.normalizedInput,
    ...readings,...personal.filter(word=>word.word===candidate.khmer&&(word.language??'khmer')==='khmer').flatMap(word=>word.spellings??[])];
  return choices.find(latin)??'';
}
