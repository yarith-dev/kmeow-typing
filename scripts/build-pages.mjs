import {cp,mkdir,readFile,writeFile,readdir,rm} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {dictionaryStore} from '../manager/dictionary-store.mjs';
const root=resolve(import.meta.dirname,'..'),out=join(root,'.build/pages');
const base='/kmeow-typing/';
await rm(out,{recursive:true,force:true});await mkdir(out,{recursive:true});
for(const name of ['dist','wasm','compiled-data','data','licenses'])await cp(join(root,name),join(out,name),{recursive:true});
await mkdir(join(out,'manager'));
for(const name of await readdir(join(root,'manager'))){
 if(/\.(js|css)$/.test(name)||name==='fonts')await cp(join(root,'manager',name),join(out,'manager',name),{recursive:true});
}
for(const name of ['LICENSE','DICTIONARY-LICENSE.md','NOTICES.txt','dictionary-manifest.json'])await cp(join(root,name),join(out,name));
const snapshot=await dictionaryStore(root).snapshot();
await writeFile(join(out,'catalog.json'),JSON.stringify({...snapshot.catalog,sharedWords:snapshot.info.provenance.sharedWords?.words??[],pronunciations:snapshot.info.provenance.pronunciations?.rules??[],revision:snapshot.revision}));
let html=await readFile(join(root,'manager/index.html'),'utf8');
html=html.replaceAll('"/manager/',`"${base}manager/`);
await writeFile(join(out,'index.html'),html);
for(const name of await readdir(join(out,'manager'))){
 if(!/\.(js|css)$/.test(name))continue;
 const file=join(out,'manager',name);let text=await readFile(file,'utf8');
 text=text.replaceAll("'/dist/",`'${base}dist/`).replaceAll("'/data/",`'${base}data/`).replaceAll("'/manager/",`'${base}manager/`);
 if(name==='state.js'){
  text=text.replace("'/api/words'",`'${base}catalog.json'`).replace('`/compiled-data/', '`'+base+'compiled-data/');
  const start=text.indexOf('export async function saveSharedDictionary(');
  text=text.slice(0,start)+`export async function saveSharedDictionary(){throw new Error('Dictionary editing is available in the local manager.');}\n`;
 }
 if(name==='app.js')text=text.replace('function openWord(row){','function openWord(row){ showWordDetails(row); return;');
 if(name==='style.css')text+='\n#add-word,#import,#migrate-words,#detail-actions,#word-dialog,#pronunciation-open,#pronunciation form,#pronunciation button,#words td:last-child,#words th:last-child{display:none!important}\n';
 await writeFile(file,text);
}
await writeFile(join(out,'.nojekyll'),'');
console.log(`Built ${out}: ${snapshot.catalog.khmer.length} Khmer and ${snapshot.catalog.english.length} English records.`);
