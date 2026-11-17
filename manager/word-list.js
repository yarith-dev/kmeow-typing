// Combine display rows only; the bundled data and personal engine stay separate.
export function wordList(bundled,personal,language,query=''){
  const rows=new Map();
  for(const entry of bundled[language]??[]){
    let row=rows.get(entry.word);
    if(!row){row={word:entry.word,language,bundled:true,ids:[],partsOfSpeech:[],spellings:[],romanizations:[],soundMapInputs:[],soundMapInputsTruncated:false,personalIndices:[]};rows.set(entry.word,row);}
    if(entry.commonnessLevel!=null)row.commonnessLevel=Math.min(row.commonnessLevel??Infinity,entry.commonnessLevel);
    if(language==='khmer'&&Object.hasOwn(entry,'components'))row.components=entry.components;
    if(language==='khmer')row.soundMapEnabled=(row.soundMapEnabled??true)&&(entry.soundMapEnabled??true);
    row.romanizations=[...new Set([...row.romanizations,...(entry.romanizations??[])])];row.soundMapInputs=[...new Set([...row.soundMapInputs,...(entry.soundMapInputs??[])])];row.soundMapInputsTruncated ||= entry.soundMapInputsTruncated??false;
    row.rank=Math.min(row.rank??Infinity,entry.rank??0);row.ids.push(entry.id);row.partsOfSpeech=[...new Set([...row.partsOfSpeech,...entry.partsOfSpeech])];
  }
  personal.forEach((entry,index)=>{
    if((entry.language??'khmer')!==language)return;
    let row=rows.get(entry.word);
    if(!row){row={word:entry.word,language,bundled:false,ids:[],partsOfSpeech:[],spellings:[],romanizations:[],soundMapInputs:[],soundMapInputsTruncated:false,personalIndices:[]};rows.set(entry.word,row);}
    row.personalIndices.push(index);row.spellings=[...new Set([...row.spellings,...(entry.spellings??[])])];
  });
  const needle=query.trim().toLowerCase();
  return [...rows.values()].filter(row=>[row.word,...row.spellings,...row.romanizations,...row.soundMapInputs].some(value=>value.toLowerCase().includes(needle)))
    .sort((a,b)=>a.word<b.word?-1:a.word>b.word?1:0);
}
export function wordPage(rows,page,size=25){
  const pages=Math.max(1,Math.ceil(rows.length/size));const current=Math.min(Math.max(page,0),pages-1);
  return {rows:rows.slice(current*size,(current+1)*size),page:current,pages,total:rows.length,
    from:rows.length?current*size+1:0,to:Math.min((current+1)*size,rows.length)};
}
export function wordPageLinks(page,pages){
  if(pages<=5)return Array.from({length:pages},(_,index)=>index);
  const start=Math.max(1,Math.min(page-1,pages-4)),numbers=[0,start,start+1,start+2,pages-1],links=[];
  for(const number of numbers){
    const previous=links.at(-1);
    if(number-previous===2)links.push(previous+1);
    else if(number-previous>2)links.push(null);
    links.push(number);
  }
  return links;
}

const alphabets={
  khmer:[...'កខគឃងចឆជឈញដឋឌឍណតថទធនបផពភមយរលវសហឡអឥឦឧឨឩឪឫឬឭឮឯឰឱឲឳ'],
  english:[...'ABCDEFGHIJKLMNOPQRSTUVWXYZ'],
};
function wordInitial(word,language){
  const first=[...word][0]??'',letter=language==='english'?first.toUpperCase():first;
  return alphabets[language]?.includes(letter)?letter:'other';
}
export function wordLetters(rows,language){
  const counts=new Map();
  for(const row of rows){const letter=wordInitial(row.word,language);counts.set(letter,(counts.get(letter)??0)+1);}
  return [{letter:'',count:rows.length},...(alphabets[language]??[]).map(letter=>({letter,count:counts.get(letter)??0})),
    ...(counts.has('other')?[{letter:'other',count:counts.get('other')}]:[])];
}
export function wordsForLetter(rows,language,letter=''){
  return letter?rows.filter(row=>wordInitial(row.word,language)===letter):rows;
}

export function romanizedKeys(row){return [...new Set([...row.spellings,...row.romanizations,...row.soundMapInputs])];}

export function wordsWithoutRomanized(rows){
  return rows.filter(row=>row.language==='khmer'&&!romanizedKeys(row).length&&!row.soundMapInputsTruncated);
}

export function suggestionWord(bundled,personal,language,candidate){
  const word=candidate.khmer??candidate.text;
  const rows=wordList(bundled,personal,language,word);
  // English casing can follow the sentence. Never substitute a base word for a
  // generated form (books is managed as books, independently of book).
  return rows.find(row=>row.word===word)
    ??(language==='english'?rows.find(row=>row.word.toLowerCase()===word.toLowerCase()):undefined)
    ??{word,language,bundled:false,ids:[],partsOfSpeech:[],spellings:[],romanizations:[],soundMapInputs:[],soundMapInputsTruncated:false,personalIndices:[]};
}
