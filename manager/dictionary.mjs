import { readFile } from 'node:fs/promises';
import { WasmEngine } from '../wasm/khmer_engine.js';
import { createEngine } from '../dist/public/node.js';
import { readCompactSource } from '../scripts/read-compact-source.mjs';

// Read only the same shipped pair that powers typing. Validate it through the
// shared loader before exposing a small word-list projection to the manager.
export async function loadDictionaryWords(files,source){
  files??=Object.fromEntries(await Promise.all(['khmer','english'].map(async language=>[language,await readFile(new URL(`../compiled-data/${language}.kmd`,import.meta.url))])));
  const engine=await createEngine({dictionary:files});
  const raw=WasmEngine.fromAppData(files.khmer,files.english);
  const readings=new Map();
  try{
    source??=readCompactSource(raw);
    const disabled=new Set(source.soundMapDisabledWords),components=new Map();
    const names=new Map(source.words.map(row=>[row.id,row.khmer]));
    for(const row of source.structures){
      const word=names.get(row.wordId);
      if(!components.has(word))components.set(word,[]);
      components.get(word).push(row.componentIds.map(id=>names.get(id)));
    }
    for(const row of JSON.parse(raw.appDataInfo()).provenance.wordComponents?.words??[])components.set(row.word,row.components);
    let after;
    do{
      const page=JSON.parse(raw.browseWords(JSON.stringify({query:'',after,limit:100})));
      for(const row of page.words)readings.set(row.khmer,{romanizations:row.romanizations,soundMapInputs:row.soundMapInputs??[],soundMapInputsTruncated:row.soundMapInputsTruncated??false});
      after=page.nextAfter;
    }while(after);
    return Object.fromEntries(Object.entries(files).map(([language,bytes])=>{
    const start=24+bytes.readUInt32LE(12)+bytes.readUInt32LE(16);
    const metadata=JSON.parse(bytes.subarray(start,-32).toString('utf8'));
    return [language,metadata.words.map(({id,word,partsOfSpeech,commonnessLevel})=>({id,word,partsOfSpeech,commonnessLevel:commonnessLevel??null,...(language==='khmer'?{components:components.get(word)??null,soundMapEnabled:!disabled.has(word),...(readings.get(word)??{romanizations:[],soundMapInputs:[],soundMapInputsTruncated:false})}:{})}))];
  }));}finally{raw.free();engine.dispose();}
}
