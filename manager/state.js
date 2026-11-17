// Kept only to recover/import additions created by the previous browser version.
export const WORDS_KEY='kmeow-manager-personal-v1';
export function parseImportSpec(text){
  const value=JSON.parse(text);
  const words=Array.isArray(value)?value:['kmeow-personal-v1','kmeow-dictionary-words-v1'].includes(value?.format)?value.words:undefined;
  if(!Array.isArray(words))throw new Error('Choose a dictionary-word or previous personal-word JSON export.');
  if(!words.length||words.length>20000)throw new Error('Import between 1 and 20,000 words at once.');
  return {words,preservePersonal:Array.isArray(value)||value.format==='kmeow-personal-v1',preferences:value?.format==='kmeow-dictionary-words-v1'?value.sharedWords:undefined};
}
export function parseImport(text){return parseImportSpec(text).words;}
export function wordExport(catalog){
  const words=['khmer','english'].flatMap(language=>{
    const levels=new Map();
    for(const row of catalog[language]){
      if(!levels.has(row.word))levels.set(row.word,new Set());
      levels.get(row.word).add(row.commonnessLevel);
    }
    return catalog[language].map(row=>({word:row.word,language,rank:row.rank,spellings:row.romanizations??[],
      ...(row.soundMapEnabled!==undefined?{soundMapEnabled:row.soundMapEnabled}:{}),
      ...(row.components!==undefined?{components:row.components}:{}),
      ...(row.commonnessLevel!==undefined&&levels.get(row.word).size===1?{commonnessLevel:row.commonnessLevel}:{})}));
  });
  return JSON.stringify({format:'kmeow-dictionary-words-v1',...(catalog.sharedWords?.length?{sharedWords:catalog.sharedWords}:{}),words},null,2)+'\n';
}
export async function dictionaryRequest(fetcher,url,options){
  const response=await fetcher(url,options);
  if(!response.ok){let detail;try{detail=(await response.json()).error;}catch{}throw new Error(detail??`Dictionary request failed (${response.status}).`);}
  return response;
}
// Fetch both files against one revision. A concurrent save is retried as a
// whole; the live editor is replaced only after the complete pair validates.
export async function loadSharedDictionary({fetcher=fetch,createEngine,words=[],catalog}){
  for(let attempt=0;attempt<3;attempt++){
    const current=catalog??await (await dictionaryRequest(fetcher,'/api/words')).json();
    try{
      const files=Object.fromEntries(await Promise.all(['khmer','english'].map(async language=>{
        const response=await fetcher(`/compiled-data/${language}.kmd?revision=${encodeURIComponent(current.revision)}`);
        if(response.status===409)throw Object.assign(new Error('Dictionary changed while loading.'),{retry:true});
        if(!response.ok)throw new Error(`Could not load the ${language} dictionary (${response.status}).`);
        return [language,new Uint8Array(await response.arrayBuffer())];
      })));
      const engine=await createEngine({dictionary:files,words});
      return {engine,catalog:current};
    }catch(error){if(!error.retry||attempt===2)throw error;catalog=undefined;}
  }
}
export async function saveSharedDictionary({fetcher=fetch,catalog,request}){
  return (await dictionaryRequest(fetcher,'/api/words',{method:'POST',headers:{'Content-Type':'application/json','X-Kmeow-Token':catalog.token,'If-Match':catalog.revision},body:JSON.stringify(request)})).json();
}
