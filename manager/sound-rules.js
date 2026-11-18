import {t,ruleGuide,familyGuide,fieldLabel} from './i18n.js';
// Read the same configuration as the engine; new rule groups remain visible
// even before they receive a more specific explanatory label here.
const guides={
  shorthands:['Shorthands','Short inputs suggest a complete word with normal typing acceptance. They are never reused as word-part readings.',['ខ្ញុំ']],
  defaultComponentParts:['Components using the default sound map','These names establish complete component boundaries. Their spellings come from the normal sound map; no custom Romanized reading is stored.',['ក្រដាស','ខុស']],
  wordRuleReuse:['Using word readings in compounds','Alternative readings can be reused in complete, unambiguous word components. Shorthands follow normal typing acceptance and stay with the complete word. Saved aliases and ambiguous splits are not reused; existing full-word readings remain available.',['កិច្ចការ','ខកចិត្ត']],
  componentReadings:['Complete component readings','Selected keys for complete word parts inside supported compounds. Other parts retain their own reading rules.',['សាស្ត្រ','វិទ្យាសាស្ត្រ']],
  nasalBoundaries:['Splitting at nasal clusters','One set of matchers handles these consonant-pair patterns. Each case keeps its own initial letters, cluster and final-letter restrictions.',['កន្ត្រៃ','កញ្ចក់']],
  inherent:["When no vowel is written","A consonant can carry its own vowel. The consonant series decides which keys to use.",["មតិ","សមុទ្រ"]],
  localVowelPairs:['Unwritten vowels from letter pairs','The first two letters, their subscript and syllable structure select a, o, ae or er. Written vowels, shortening marks and shared overrides retain their own rules.',['បណ្ណាល័យ','វគ្គីករណ៍','លទ្ធិ','សប្បាយ']],
  consonantCountTolerance:['Single or double consonants','Generated readings accept one or two of the same consonant: lerthi / lertthi and vaekkikor / vaekikor. Suggestions show the preferred spelling. Vowels and saved spellings stay literal.',['លទ្ធិ','វគ្គីករណ៍']],
  shortInherent:["Short vowels with ់","With an unwritten vowel, the shortening mark uses o in both A-series and O-series. Separately configured ending choices still apply.",["ចង់","គង់","រលត់"]],
  closedInherent:["Unwritten vowels before final letters","When a consonant closes the syllable, use the closed-syllable vowel keys.",["កង","ទង"]],
  endingAlternatives:["Final ech / ich can use ex / ix","At the end of a word or a complete, unambiguous component, ech also accepts ex and ich accepts ix: តិច → tech / tex, tich / tix; កិច្ចការ → kechka / kexka. Other ch endings and initial ch stay unchanged. Saved aliases and shorthands remain literal.",["តិច","ភ្លេច","កិច្ចការ"]],
  codas:["Consonants at the end of a syllable","A final consonant can sound different from the same letter at the beginning. Final ប accepts p or b, including inside compounds: brap / brab for ប្រាប់ and reapjom / reabjom for រៀបចំ.",["ប្រាប់","រៀបចំ","កាត","ទឹក"]],
  terminalReadingUnits:["Reading ទស្សន៍ at the end","Read the ending as tuh or tus, including in the listed longer words. The marked ន៍ is silent.",["ទស្សន៍","ទូរទស្សន៍"]],
  silentCodaClusters:["Closing ស្ស and ក្ខ","These clusters close the syllable without adding another syllable for the subscript. Some preceding vowels have a special reading.",["សិស្ស","ទស្សនា","ទុក្ខ"]],
  terminalSilentCodaClusters:["Keeping the final sound, dropping its subscript","At the supported word endings, keep the main final consonant sound and leave its subscript silent.",["ព្យាង្គ","សម្ព័ន្ធ","បុណ្យ","គម្ពីរ"]],
  terminalShortCodaClusters:["Short readings for ទ្ធ endings","The supported endings can use a short t reading. The existing longer reading is also kept.",["សិទ្ធ","សុទ្ធ","រុទ្ធ"]],
  terminalSilentGroups:["Silent រ្យ after ា","In the supported endings, រ្យ adds no keys after the vowel.",["អស្ចារ្យ","អាចារ្យ"]],
  pairVowelReadings:["Two vowel choices before ម្លាំង","The specified consonant-pair shape allows more than one reading for its first vowel.",["កម្លាំង"]],
  optionalInitialOnsets:["Alternative p at the beginning","ប can also use p before the listed subscripts. The usual b reading remains available.",["ប្រាំ","ប្តី"]],
  terminalInherentBoundaryCodas:["Starting another syllable after a final consonant","Some final consonants close the current syllable while the following consonant starts another.",["ទឹកកក","ឆ្លើយតប"]],
  pairBeforeSubscript:["Splitting before a subscript","In supported shapes, the consonant before the subscript ends one syllable and the subscript starts the next.",["កម្លាំង","ចម្រៀង"]],
  terminalCancellation:["Letters silenced by ៍","Leave out the marked final consonant, together with its supported attached vowel. Other parts of the word keep their readings.",["ប្រយោជន៍","ព្រឹត្តិការណ៍"]],
  simpleMarkedRhymes:["Reading ័យ and ័រ together","Read the vowel and mark as one ending. Supported ័យ forms use ey; supported ័រ forms use oa.",["ជ័យ","វ័យ","ជ័រ"]],
  exactInputPriorities:["Priority for complete spellings","These settings can prefer a specific complete input. No extra overrides are currently configured in this group.",["អ្នក","សៀវភៅ"]],
  wordFinalSilentConsonants:["Silent រ after a written vowel","At a supported word end, រ after a written vowel does not add another sound.",["ការ","អាហារ"]],
  finalClusters:["Reading ត្វ at the end","The supported final cluster contributes its closing t sound instead of another full syllable.",["សត្វ"]],
  silentWordEndings:["Silent endings in selected words","Some words omit their final written letter or sign when read. This applies only to the listed words.",["ភូមិ","សមុទ្រ"]],
  repeatedInitialPresyllable:["Repeated letters with a short first syllable","In these selected words, read the first repeated consonant separately before continuing with the word.",["ពពក","បបរ"]],
  shortInherentBeforeCoda:["Short vowel choices before final ក","For the supported O-series ending, u, ua and uo are alternative typing keys.",["កន្ទក់"]],
  glottalOnset:["Typing subscript អ with an apostrophe","An apostrophe marks the break before subscript អ. The pattern ្អូ accepts 'o and 'ou: ប្អូន → p'on / p'oun. You may omit apostrophes when looking for suggestions: poun suggests ប្អូន as a completion, after existing matches such as ពូន. Explicit apostrophe spellings stay exact. This also applies after a nasal and in Read-As.",["ប្អូន","ចង្អូរ","ផ្អែម","ក្អក"]],
  scopedOnsetClusters:["ហ្វ reads f in កាហ្វេ","Use f for the selected ហ្វ cluster in កាហ្វេ. This does not change every spelling containing ហ្វ.",["កាហ្វេ"]],
  vowelsBeforeCoda:["ាំ changes before final ង","Before the specified final ង, the vowel uses a in A-series and ea in O-series.",["ខាង","ទាំង"]],
  closedVowelAlternatives:['◌ួ before a closing consonant','When a consonant closes the syllable, ◌ួ accepts ur or u: មួយ → mury / muy and ជួយ → jury / juy. Open syllables and silent final រ keep their existing readings: តួ → tur and ជួរ → jur. ◌ូ is unchanged.',['មួយ','ជួយ','ខ្លួន']],
  explicitBantoc:["How ់ changes a written ា","The shortened reading depends on the consonant series and the final letter.",["កាត់","គាត់","ទាក់"]],
  crossSyllableGovernance:["An earlier consonant changes a later vowel","In supported two-syllable words, the earlier A-series consonant makes a later consonant use A-series vowel keys. Listed exceptions keep their own series.",["សាលា","កុលាប"]],
  reviewedWordFinalCodas:["Special final keys in selected words","The listed words can use a shorter final-consonant reading while keeping any explicitly retained alternatives.",["ពុធ","មេឃ"]],
  onsetClusterGovernance:["Which consonant controls a cluster vowel","When consonants share a vowel, the cluster determines whether its A- or O-series reading applies.",["ខ្មែរ","ស្រី"]],
  nasalPairGovernance:["A vowel after a nasal consonant pair","The supported nasal-pair shapes let an earlier consonant determine the later vowel reading.",["កន្លះ","តម្លៃ"]],
  riyaGovernance:["Reading រិយា after an A-series syllable","In the supported shape, រិយា takes the A-series vowel reading.",["កិរិយា","ចរិយា"]],
  openLaaGovernance:["Reading លា as la","In the supported open syllable, an earlier A-series consonant makes លា read la.",["សាលា"]],
  localWrittenClusterGovernance:["Cluster readings inside longer words","The listed written clusters keep their selected A-series vowel reading when they occur at a supported syllable start.",["ក្លាហាន","ខ្មៅដៃ"]],
  initialSamaaGovernance:["Reading the សមា beginning","In the supported initial សមា shape, the មា vowel uses its A-series reading.",["សមាគម","សមាជិក"]],
  writtenNhBoundary:["Splitting ញ្ច after a written vowel","The ញ closes the first syllable; ច begins the next and keeps its attached vowel.",["ចិញ្ចឹម","ចិញ្ចៀន"]],
  terminalCombinedVowels:["Combined signs at the word end","These vowel-and-mark combinations have their own keys when they form a supported complete word ending.",["គន្លឹះ","គ្រឹះ"]],
  unwrittenNasalRBoundary:["A nasal before a រ cluster","Supply the first unwritten vowel, close its syllable with the nasal and then read the រ cluster.",["កន្ត្រក","កញ្ច្រង់"]],
  writtenVelarRBoundary:["Splitting ង្ក្រ and ង្គ្រ","The ង closes the first syllable while the following cluster starts the next.",["ចង្ក្រាន","ចង្ក្រិម"]],
  writtenNasalGlottalBoundary:["A nasal before subscript អ","Close the first syllable with the nasal, then use an apostrophe before the following អ sound.",["ចង្អុល","ចង្អៀត"]],
  weakRSecondVowelGovernance:["Vowels after a short រ-cluster syllable","In the listed shapes, a short first syllable determines which vowel keys the following syllable uses.",["ក្រមា","ស្រមោល"]],
  explicitGlottalBantoc:["Shortened ា after subscript អ","The supported អ syllable keeps its a key when the final consonant carries ់.",["ផ្អាក","ក្អាត់","ចង្អាប់"]],
  terminalPlainASign:["Existing final ៈ spellings","Keep the existing a / ak alternatives at supported A-series endings.",["ខណៈ","លក្ខណៈ"]],
  terminalWrittenSameOSign:["Existing O-series final ៈ spellings","Keep the existing eak readings of supported two-syllable words.",["ជីវៈ","ធុរៈ"]],
  shortInherentSign:["Short vowel and closure: ៈ","Use ak in A-series and eak in O-series, including inside words. Resolve the syllable's series and cluster first: ស្រៈ → srak; ឋានៈ → thanak. In Read-As, គៈណៈ → keaknak. Existing keanak remains accepted. Irregular syllable splits use Read-As. The enabled flag controls ordinary dictionary paths; explicit Read-As stays available. Independent vowels can start the next syllable: គៈឥ → keakei / keakey. This is separate from ះ.",["ស្រៈ","ឋានៈ","គណៈ"]],
  terminalNasalCompoundBoundary:["Keeping a nasal boundary in compounds","Read the supported compound boundary without merging its two syllables.",["ទូបង្កក","កាត់បន្ថយ"]],
  independentCoengY:["ឲ្យ and ឱ្យ read oy / aoy","Read the independent vowel together with subscript យ as oy or aoy.",["ឲ្យ","ឱ្យ"]],
  compactOrdinaryABoundaryPriority:["Priority for compact readings around អ","A small priority adjustment favors a supported compact compound reading without changing its spelling.",["ទឹកអប់","កៅអី"]],
  checkedAamNgBoundary:["Special boundaries in វាំងនន and បាំងភក់","Keep the selected first-syllable vowel when splitting these two words.",["វាំងនន","បាំងភក់"]],
  savedInputMatchPriorities:["Priority for a selected saved spelling","The listed saved input gets its own priority adjustment; other spellings keep their usual order.",["ប៉ុស្តិ៍"]],
  initialCarrierDirectVowel:["Initial អ before a consonant with a written vowel","Use ak for bare initial អ when the next consonant carries a written vowel directly.",["អធិដ្ឋាន","អតិថិជន"]],
  initialCarrierNasal:["Initial អ before nasal clusters","In the selected initial cluster shapes, the first unwritten vowel uses o. Later letters keep their own keys.",["អង្ករ","អង្កត់"]],
};
const englishLabel=name=>name.replace(/([a-z0-9])([A-Z])/g,'$1 $2').replace(/^./,c=>c.toUpperCase());
export const ruleLabel=name=>fieldLabel(name,englishLabel(name));
function searchable(value){
  if(Array.isArray(value))return value.map(searchable).join(' ');
  if(value&&typeof value==='object')return Object.entries(value).map(([key,item])=>`${key} ${englishLabel(key)} ${ruleLabel(key)} ${searchable(item)}`).join(' ');
  return String(value??'');
}
// Some configured shapes have no admitted word in this dictionary. Keep
// those demonstrations separate from real, selectable dictionary examples.
const patternExamples={nasalBoundaries:[{word:'កន្ទ្រាញ',inputs:['kontreanh','kontrianh'],bundled:false}]};
const preferredInputs={savedInputMatchPriorities:['poh'],compactOrdinaryABoundaryPriority:['teukop']};
function scopedWords(value){
  if(Array.isArray(value))return value.flatMap(scopedWords);
  if(!value||typeof value!=='object')return [];
  return [...(value.words??[]),...(typeof value.word==='string'?[value.word]:[]),
    ...Object.values(value).filter(item=>item&&typeof item==='object').flatMap(scopedWords)];
}
function partWords(part){return [...new Set([part.khmer,...scopedWords(part)])];}
function entryMatches(entry,term){
  return searchable([entry?.word,entry?.soundMapInputs,entry?.romanizations]).toLowerCase().includes(term);
}
function ruleExamples(id,value,byWord,term='',filtered=false){
  let words=filtered?value.flatMap(partWords):[...(guides[id]?.[2]??[]),...scopedWords(value)];
  words=[...new Set(words)];
  if(term)words.sort((a,b)=>Number(entryMatches(byWord.get(b),term))-Number(entryMatches(byWord.get(a),term)));
  const examples=[];
  for(const word of words){
    const entry=byWord.get(word);if(!entry)continue;
    const generated=entry.soundMapInputs??[],saved=entry.romanizations??[];
    const preferred=(preferredInputs[id]??[]).filter(input=>[...generated,...saved].includes(input));
    const inputs=[...new Set([...preferred,...(generated.length?generated:saved).slice().sort((a,b)=>a.length-b.length||a.localeCompare(b))])]
      .sort((a,b)=>Number(b.toLowerCase().includes(term))-Number(a.toLowerCase().includes(term))).slice(0,2);
    if(inputs.length)examples.push({word,inputs,bundled:true});
    if(examples.length===2)break;
  }
  if(patternExamples[id])examples.push(...patternExamples[id]);
  return examples;
}
export function soundRules(runtime,query='',catalog=[]){
  const term=query.trim().toLowerCase(),byWord=new Map(catalog.map(row=>[row.word,row]));
  return Object.entries(runtime??{}).flatMap(([id,original])=>{
    if(id==='pronunciationParts'||id==='wordParts')return [];
    const [englishTitle,englishDescription]=guides[id]??[englishLabel(id),'Supported letters, conditions and choices for this rule.'];
    const [title,description]=ruleGuide(id,ruleLabel(id)===englishLabel(id)?englishTitle:ruleLabel(id),t(englishDescription));
    let value=original,filtered=false;
    if(id==='shorthands'&&term&&!`${id} ${englishTitle} ${englishDescription} ${title} ${description}`.toLowerCase().includes(term)){
      value=original.filter(part=>searchable(part).toLowerCase().includes(term)
        ||partWords(part).some(word=>entryMatches(byWord.get(word),term)));
      if(!value.length)return [];
      filtered=true;
    }
    const examples=ruleExamples(id,value,byWord,term,filtered);
    const rule={id,title,description,value,examples};
    return filtered||!term||searchable([rule,englishTitle,englishDescription]).toLowerCase().includes(term)?[rule]:[];
  });
}
export const ruleFamilyDefinitions=[
  {id:'vowels',title:'Vowels',description:'Written and unwritten vowels, short forms and combined vowel sounds.',groups:['inherent','shortInherent','closedInherent','localVowelPairs','pairVowelReadings','shortInherentBeforeCoda','vowelsBeforeCoda','closedVowelAlternatives','terminalCombinedVowels','independentCoengY','initialCarrierDirectVowel','initialCarrierNasal']},
  {id:'consonants',title:'Consonants & endings',description:'Initial consonants, final sounds and alternative consonant keys.',groups:['codas','endingAlternatives','consonantCountTolerance','optionalInitialOnsets','finalClusters','reviewedWordFinalCodas','glottalOnset','scopedOnsetClusters']},
  {id:'syllables',title:'Syllable splitting',description:'Where one syllable ends and the next begins.',groups:['terminalInherentBoundaryCodas','pairBeforeSubscript','nasalBoundaries','writtenNhBoundary','unwrittenNasalRBoundary','writtenVelarRBoundary','writtenNasalGlottalBoundary','terminalNasalCompoundBoundary','checkedAamNgBoundary','wordRuleReuse','defaultComponentParts']},
  {id:'series',title:'A/O changes',description:'Which consonant determines the vowel series.',groups:['crossSyllableGovernance','onsetClusterGovernance','nasalPairGovernance','riyaGovernance','openLaaGovernance','localWrittenClusterGovernance','initialSamaaGovernance','weakRSecondVowelGovernance']},
  {id:'silence',title:'Silent letters & marks',description:'Silent letters, subscripts, shortening and other spelling marks.',groups:['terminalReadingUnits','silentCodaClusters','terminalSilentCodaClusters','terminalShortCodaClusters','terminalSilentGroups','terminalCancellation','wordFinalSilentConsonants','silentWordEndings','simpleMarkedRhymes','explicitBantoc','explicitGlottalBantoc','terminalPlainASign','terminalWrittenSameOSign','shortInherentSign']},
  {id:'shorthands',title:'Shorthands',description:'Short inputs suggesting a complete word.',groups:['shorthands']},
  {id:'repetition',title:'Repetition',description:'Repeated initial letters that form a short first syllable.',groups:['repeatedInitialPresyllable']},
  {id:'ranking',title:'Ranking',description:'How existing readings are ordered without changing their spelling.',groups:['exactInputPriorities','compactOrdinaryABoundaryPriority','savedInputMatchPriorities']},
];
export function ruleFamilies(runtime,query='',catalog=[]){
  const all=soundRules(runtime,'',catalog),term=query.trim().toLowerCase();
  const matches=soundRules(runtime,query,catalog);
  const known=new Set(ruleFamilyDefinitions.flatMap(family=>family.groups));
  return ruleFamilyDefinitions.map(family=>{
    const translated=familyGuide(family);
    return {...translated,rules:(!term||`${family.title} ${family.description} ${translated.title} ${translated.description}`.toLowerCase().includes(term)?all:matches).filter(rule=>
      family.groups.includes(rule.id)||(family.id==='syllables'&&!known.has(rule.id)))};
  }).filter(family=>family.rules.length);
}
export function renderRuleExamples(examples,doc=document){
  const list=doc.createElement('div');list.className='rule-examples';
  for(const example of examples){
    const row=doc.createElement('div');row.className='rule-example';
    const word=doc.createElement('span');word.className='example-khmer';word.lang='km';word.textContent=example.word;
    const arrow=doc.createElement('span');arrow.className='muted';arrow.textContent='→';arrow.setAttribute('aria-hidden','true');
    const keys=doc.createElement('span');keys.className='example-inputs';keys.lang='en';keys.textContent=example.inputs.join(' / ');
    row.append(word,arrow,keys);list.append(row);
    if(!example.bundled){const note=doc.createElement('p');note.className='helper';note.textContent=t('Pattern example · not in the bundled word list');list.append(note);}
  }
  return list;
}

// A filtered view reuses the original configuration objects. Keep disclosure
// identities tied to those objects rather than their translated label or index.
const ruleItemIds=new WeakMap();let nextRuleItemId=0;
function ruleItemKey(item){
  if(!item||typeof item!=='object')return String(item);
  if(!ruleItemIds.has(item))ruleItemIds.set(item,String(++nextRuleItemId));
  return ruleItemIds.get(item);
}
export function renderRuleValue(value,field='',doc=document,catalog=[],openDetail=null){
  const byWord=catalog instanceof Map?catalog:new Map(catalog.map(row=>[row.word,row]));
  const node=(tag,text,className)=>{const element=doc.createElement(tag);if(text!==undefined)element.textContent=text;if(className)element.className=className;return element;};
  const text=item=>item===null?t('Not set'):typeof item==='boolean'?t(item?'Yes':'No'):
    /series$/i.test(field)&&['a','o'].includes(item)?t('{series}-series',{series:item.toUpperCase()}):String(item);
  const link=(title,item,itemField='',examples=[])=>{
    const button=node('button',undefined,'rule-link');button.type='button';button.setAttribute('aria-haspopup','dialog');
    const copy=node('span',undefined,'rule-link-copy');copy.append(node('span',title(),'rule-link-title'));
    if(Array.isArray(item?.keys)&&item.keys.length)copy.append(node('span',item.keys.map(key=>typeof key==='string'?key:key.key).join(' / '),'rule-link-keys'));
    const arrow=node('span','›','rule-link-arrow');arrow.setAttribute('aria-hidden','true');button.append(copy,arrow);
    button.setAttribute('data-rule-trigger',ruleItemKey(item));
    button.onclick=()=>openDetail({title,value:item,field:itemField,examples},button);return button;
  };
  if(!value||typeof value!=='object')return node('span',text(value));
  if(Array.isArray(value)){
    if(!value.length)return node('span',t('None configured'),'muted');
    if(field==='cases'){
      const sections=node('div',undefined,'rule-items');
      for(const [title,description,select] of [
        ['Reusable patterns','Applies wherever the configured pattern matches.',item=>!item.words?.length],
        ['Selected word groups','Applies only to the listed words and positions.',item=>item.words?.length>1],
        ['Word exceptions','Applies only to the single named word.',item=>item.words?.length===1],
      ]){
        const items=value.filter(select);if(!items.length)continue;
        const section=node('section');
        section.append(node('h4',t(title)),node('p',t(description),'helper'),renderRuleValue(items,'',doc,byWord,openDetail));
        sections.append(section);
      }
      return sections;
    }
    if(value.every(item=>item===null||typeof item!=='object')){
      const chips=node('div',undefined,'rule-values');
      for(const item of value)chips.append(node('span',text(item),'key'));
      return chips;
    }
    const list=node('div',undefined,'rule-items');
    value.forEach((item,index)=>{
      if(Array.isArray(item)){
        if(openDetail){list.append(link(()=>t('Pattern {count}',{count:index+1}),item,field));return;}
        const row=node('div',undefined,'rule-sequence');row.append(renderRuleValue(item,field,doc,byWord));list.append(row);return;
      }
      const details=node('details',undefined,'rule-item');
      details.setAttribute('data-rule-detail',ruleItemKey(item));
      const name=item?.khmer??item?.root??item?.ending??item?.prefix??item?.cluster??item?.unit??item?.input??item?.word??item?.coda??item?.base??item?.clusters?.join(' / ')??(item?.first&&item?.second?`${item.first} + ${item.second}`:undefined);
      const kindLabel={syllable:item?.words?.length||item?.positions?.length?'Selected words and positions':'Whole syllable',prefix:'At the beginning',ending:'Short ending',terminalRoot:'At the end',scoped:'Selected words and positions'}[item?.kind];
      details.append(node('summary',name?String(name):kindLabel?t(kindLabel):t('Pattern {count}',{count:index+1})));
      const hasReadings=row=>row?.soundMapInputs?.length||row?.romanizations?.length;
      const entries=[...new Set(scopedWords(item))].map(word=>byWord.get(word)).filter(row=>row?.soundMapInputs?.length).slice(0,2);
      const standalone=byWord.get(item?.khmer);
      if(hasReadings(standalone)&&!entries.includes(standalone))entries.unshift(standalone);
      const examples=entries.map(row=>({word:row.word,inputs:(row.soundMapInputs?.length?row.soundMapInputs:row.romanizations).slice()
          .sort((a,b)=>a.length-b.length||a.localeCompare(b)).slice(0,2),bundled:true}));
      if(openDetail){list.append(link(()=>name?String(name):kindLabel?t(kindLabel):t('Pattern {count}',{count:index+1}),item,field,examples));return;}
      if(examples.length)details.append(renderRuleExamples(examples,doc));
      details.append(renderRuleValue(item,field,doc,byWord));list.append(details);
    });
    return list;
  }
  const fields=node('dl',undefined,'rule-fields');
  for(const [key,item] of Object.entries(value)){
    const label=()=>key==='a'?t('{series}-series',{series:'A'}):key==='o'?t('{series}-series',{series:'O'}):ruleLabel(key);
    const term=node('dt',label()),content=node('dd');
    if(openDetail&&item&&typeof item==='object'&&!Array.isArray(item)){
      term.className='sr-only';content.className='rule-field-wide';content.append(link(label,item,key));
    }else{
      if(openDetail&&Array.isArray(item)&&item.some(value=>value&&typeof value==='object')){term.className='rule-field-wide';content.className='rule-field-wide';}
      content.append(renderRuleValue(item,key,doc,byWord,openDetail));
    }
    fields.append(term,content);
  }
  if(!fields.children.length)return node('span',t('None configured'),'muted');
  return fields;
}
