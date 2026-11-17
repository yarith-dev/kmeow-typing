import {readFile,readdir,rm,mkdtemp,rename,unlink,open} from 'node:fs/promises';
import {createHash,randomUUID} from 'node:crypto';
import {join,basename,dirname} from 'node:path';
import {WasmEngine} from '../wasm/khmer_engine.js';
import {createEngine} from '../dist/public/node.js';
import {readCompactSource} from '../scripts/read-compact-source.mjs';
import {loadDictionaryWords} from './dictionary.mjs';

const paths=['compiled-data/khmer.kmd','compiled-data/english.kmd','dictionary-manifest.json'];
const transactionFile='.dictionary-manager-transaction.json';
const sha=value=>createHash('sha256').update(value).digest('hex');
export class DictionaryError extends Error{constructor(message,status=400){super(message);this.status=status;}}
function check(condition,message,status){if(!condition)throw new DictionaryError(message,status);}
function metadata(bytes){return JSON.parse(bytes.subarray(24+bytes.readUInt32LE(12)+bytes.readUInt32LE(16),-32));}
async function exists(path){try{await readFile(path);return true;}catch(error){if(error.code==='ENOENT')return false;throw error;}}
async function durable(path,bytes){const file=await open(path,'w',0o600);try{await file.writeFile(bytes);await file.sync();}finally{await file.close();}}
async function syncDirectory(path){const directory=await open(path,'r');try{await directory.sync();}finally{await directory.close();}}
const preferenceKey=word=>JSON.stringify([word.language??'khmer',word.word]);
function mergePreference(words,entry){
  const key=preferenceKey(entry),old=words.find(word=>preferenceKey(word)===key);
  if(old){old.rank=entry.rank??old.rank;old.spellings=[...new Set([...(old.spellings??[]),...(entry.spellings??[])])];}
  else words.push({word:entry.word,language:entry.language??'khmer',rank:entry.rank??0,spellings:[...new Set(entry.spellings??[])]});
}
const lockBusy=()=>new DictionaryError('Another dictionary save is in progress. Retry shortly.',423);
function processAlive(pid){
  if(!Number.isSafeInteger(pid)||pid<1||pid>0x7fffffff)throw lockBusy();
  try{process.kill(pid,0);return true;}catch(error){if(error.code==='ESRCH')return false;if(error.code==='EPERM')return true;throw error;}
}
async function lockClaims(root){
  const claims=[];
  for(const entry of await readdir(root,{withFileTypes:true})){
    const match=/^\.dictionary-manager\.lock\.(\d+)\.([a-f0-9-]{36})(\.tmp)?$/.exec(entry.name);
    if(!entry.isFile()||!match)continue;
    const file=join(root,entry.name),pid=Number(match[1]);
    // Each attempt owns a unique pathname, so reclaiming a dead process's
    // file cannot unlink a newer writer's claim.
    if(!processAlive(pid)){await rm(file,{force:true});continue;}
    if(match[3])continue;
    let claim;try{claim=JSON.parse(await readFile(file,'utf8'));}catch(error){if(error.code==='ENOENT')continue;throw lockBusy();}
    if(claim.pid!==pid||typeof claim.choosing!=='boolean'||!Number.isSafeInteger(claim.ticket)
      ||(claim.choosing?claim.ticket!==0:claim.ticket<1))throw lockBusy();
    claims.push({...claim,name:entry.name});
  }
  return claims;
}
export async function acquireDictionaryLock(root){
  // Only older manager versions use this shared filename. New claims never
  // reuse it; checking/removing a dead legacy owner is safe across contenders.
  const legacy=join(root,'.dictionary-manager.lock');
  let owner;try{owner=JSON.parse(await readFile(legacy,'utf8'));}catch(error){if(error.code!=='ENOENT')throw lockBusy();}
  if(owner){if(processAlive(owner.pid))throw lockBusy();await rm(legacy,{force:true});}
  const name=`.dictionary-manager.lock.${process.pid}.${randomUUID()}`,file=join(root,name),temporary=file+'.tmp';
  const publish=async claim=>{await durable(temporary,JSON.stringify({pid:process.pid,...claim}));await rename(temporary,file);};
  try{
    // Bakery tickets: publish intent before choosing a number. A contender
    // that still sees another process choosing retries instead of overtaking it.
    await publish({choosing:true,ticket:0});
    const ticket=Math.max(0,...(await lockClaims(root)).map(claim=>claim.ticket))+1;
    if(!Number.isSafeInteger(ticket))throw lockBusy();
    await publish({choosing:false,ticket});
    for(const claim of await lockClaims(root))if(claim.name!==name&&
      (claim.choosing||claim.ticket<ticket||(claim.ticket===ticket&&claim.name<name)))throw lockBusy();
    return ()=>rm(file,{force:true});
  }catch(error){await rm(temporary,{force:true});await rm(file,{force:true});throw error;}
}

async function restoreTransaction(root,directory){
    check(basename(directory)===directory&&directory.startsWith('.dictionary-write-'),'Invalid transaction directory.',500);
    const originals=await Promise.all(paths.map(path=>readFile(join(root,directory,path.replaceAll('/','__')+'.before'))));
    for(const [index,path] of paths.entries()){
      const temporary=join(root,path+'.recovering');
      await durable(temporary,originals[index]);
      await rename(temporary,join(root,path));
      await syncDirectory(dirname(join(root,path)));
    }
    await unlink(join(root,transactionFile));await syncDirectory(root);
    await rm(join(root,directory),{recursive:true});await syncDirectory(root);
  }

// Call only while holding acquireDictionaryLock(root).
export async function recoverDictionaryTransaction(root){
  const journal=join(root,transactionFile);
  if(await exists(journal))await restoreTransaction(root,JSON.parse(await readFile(journal,'utf8')).directory);
  // A crash before publication or after commit can leave an unreferenced
  // staging directory. No completed save retains a previous generation.
  for(const entry of await readdir(root,{withFileTypes:true}))
    if(entry.isDirectory()&&entry.name.startsWith('.dictionary-write-'))await rm(join(root,entry.name),{recursive:true});
  await rm(journal+'.tmp',{force:true});
}

// Call only while holding acquireDictionaryLock(root). Temporary rollback
// files protect this one transaction and are discarded after success/failure.
export async function commitDictionaryFiles(root,updates,{commitRename=rename}={}){
  check(updates.length===paths.length,'Invalid dictionary file set.',500);
  const directory=await mkdtemp(join(root,'.dictionary-write-')),journal=join(root,transactionFile);
  try{
    const originals=await Promise.all(paths.map(path=>readFile(join(root,path))));
    for(const [index,path] of paths.entries()){
      await durable(join(directory,path.replaceAll('/','__')+'.before'),originals[index]);
      await durable(join(directory,path.replaceAll('/','__')+'.after'),updates[index]);
    }
    await syncDirectory(directory);
    await durable(journal+'.tmp',JSON.stringify({directory:basename(directory)}));
    await rename(journal+'.tmp',journal);await syncDirectory(root);
    for(const path of paths){await commitRename(join(directory,path.replaceAll('/','__')+'.after'),join(root,path));await syncDirectory(dirname(join(root,path)));}
    await unlink(journal);await syncDirectory(root);
  }catch(error){
    if(await exists(journal))await restoreTransaction(root,basename(directory));
    throw error;
  }finally{
    if(!await exists(journal)){
      await rm(directory,{recursive:true,force:true});await rm(journal+'.tmp',{force:true});await syncDirectory(root);
    }
  }
}

// The shipped pair is authoritative. Lossless extraction is checked against its
// exact compact bytes before any edit; ordinary reads never rebuild the files.
export function dictionaryStore(root,{commitRename=rename}={}){
  let queue=Promise.resolve(),cached;
  function serial(work){const next=queue.then(work);queue=next.catch(()=>{});return next;}
  const acquire=()=>acquireDictionaryLock(root);
  async function readLocked(){
    await recoverDictionaryTransaction(root);
    const bytes=await Promise.all(paths.map(path=>readFile(join(root,path))));
    const revision=sha(Buffer.concat(bytes));
    if(cached?.revision===revision)return cached;
    const files={khmer:bytes[0],english:bytes[1]},manifest=JSON.parse(bytes[2]);
    const validated=await createEngine({dictionary:files});validated.dispose();
    const core=WasmEngine.fromAppData(files.khmer,files.english);
    try{
      const source=readCompactSource(core),info=JSON.parse(core.appDataInfo());
      check(manifest.khmer===sha(files.khmer)&&manifest.english===sha(files.english)&&manifest.pairId===info.pairId,'Dictionary files do not match their manifest.',500);
      const catalog=await loadDictionaryWords(files,source);
      for(const language of ['khmer','english']){
        const ranks=new Map((language==='khmer'?source.words:source.englishWords).map(word=>[word.khmer??word.text,word.rank]));
        for(const row of catalog[language])row.rank=ranks.get(row.word)??0;
      }
      cached={revision,files,manifest,source,metadata:{khmer:metadata(files.khmer),english:metadata(files.english)},info,catalog,bytes};
      return cached;
    }finally{core.free();}
  }
  async function read(){
    const release=await acquire();
    try{return await readLocked();}finally{await release();}
  }
  async function save(request,revision){
    const release=await acquire();
    try{
      // Recover a previous interrupted commit while holding the exclusive lock.
      const before=await readLocked();
      check(revision===before.revision,'The dictionary changed. Refresh it and reopen the word before saving again.',409);
      check(request&&['save','import','pronunciations'].includes(request.type),'Unknown dictionary operation.');
      const entries=request.type==='pronunciations'?[]:request.type==='save'?[request.word]:request.words;
      check(Array.isArray(entries)&&(request.type==='pronunciations'||entries.length>0)&&entries.length<=20000,'Save between 1 and 20,000 words.');
      const pronunciationRules=request.type==='pronunciations'?request.rules:before.info.provenance.pronunciations?.rules??[];
      check(Array.isArray(pronunciationRules)&&pronunciationRules.length<=1000,'Use at most 1,000 pronunciation rules.');
      const source=structuredClone(before.source),meta=structuredClone(before.metadata);
      const preferences=structuredClone(before.info.provenance.sharedWords?.words??[]);
      check(request.preferences===undefined||(Array.isArray(request.preferences)&&request.preferences.length<=10000),'Invalid shared word preferences.');
      const componentChoices=structuredClone(before.info.provenance.wordComponents?.words??[]);
      const ratingChanges=new Map(),pendingComponents=new Map();
      let nextId=Math.max(0,...source.words.map(word=>word.id),...source.englishWords.map(word=>word.id),...source.mappings.map(word=>word.id))+1;
      for(const entry of entries){
        check(entry&&typeof entry==='object'&&!Array.isArray(entry),'Invalid word.');
        check(Object.keys(entry).every(key=>['word','language','spellings','rank','soundMapEnabled','commonnessLevel','components'].includes(key)),'Unexpected word fields.');
        const language=entry.language??'khmer',word=entry.word;
        check(['khmer','english'].includes(language),'Choose Khmer or English.');
        check(typeof word==='string'&&word.length>0&&word.length<=240&&word===word.trim(),'Enter a word of at most 240 characters.');
        check(entry.rank===undefined||(Number.isSafeInteger(entry.rank)&&entry.rank>=0&&entry.rank<=1000000),'Rank must be an integer from 0 to 1000000.');
        check(entry.soundMapEnabled===undefined||typeof entry.soundMapEnabled==='boolean','Sound map setting must be a boolean.');
        check(language==='khmer'||entry.soundMapEnabled===undefined,'Sound map settings apply only to Khmer words.');
        check(entry.commonnessLevel===undefined||entry.commonnessLevel===null||(Number.isInteger(entry.commonnessLevel)&&entry.commonnessLevel>=1&&entry.commonnessLevel<=5),'Word level must be from 1 to 5 or unrated.');
        check(entry.spellings===undefined||(Array.isArray(entry.spellings)&&entry.spellings.length<=1000&&entry.spellings.every(key=>typeof key==='string'&&key.length<=512&&/^[a-z]+(?:[ '\-][a-z]+)*$/i.test(key))),'Use Romanized letters, spaces, apostrophes or hyphens for spellings.');
        check(language==='khmer'||!entry.spellings?.length,'English words do not use Romanized spellings.');
        const records=language==='khmer'?source.words:source.englishWords,key=language==='khmer'?'khmer':'text';
        let typing=records.find(row=>row[key]===word);
        if(!typing&&language==='english'&&(request.type==='import'||request.preservePersonal===true))typing=records.find(row=>row.text.toLowerCase()===word.toLowerCase());
        const existed=!!typing;
        // Existing identities are stable: renaming/deleting words is deliberately
        // outside this editor so saved changes cannot silently drop vocabulary.
        if(request.type==='save'&&request.original)check(request.original.language===language&&request.original.word===word&&typing,'Existing words keep their text and language.');
        if(!typing){
          check(request.reviewed===true,'Review new words for dictionary suitability before saving.');
          check(language!=='english'||!records.some(row=>row.text.toLowerCase()===word.toLowerCase()),'That English word already exists with different capitalization.');
          typing={id:nextId++,[key]:word,rank:entry.rank??1,status:'approved'};records.push(typing);
          if(language==='english')source.schemaVersion=3;
          meta[language].words.push({id:sha(`kmeow:${language}:${word}`),word,context:null,commonnessLevel:null,partsOfSpeech:[],meanings:[],translations:[],sources:[]});
        }
        const rankChanged=entry.rank!==undefined&&entry.rank!==typing.rank&&!(request.preservePersonal===true&&existed)&&typing[key]===word;
        if(rankChanged)typing.rank=entry.rank;
        if(entry.commonnessLevel!==undefined){
          ratingChanges.set(JSON.stringify([language,typing[key]]),entry.commonnessLevel);
          for(const row of meta[language].words)if(row.word===typing[key])row.commonnessLevel=entry.commonnessLevel;
        }
        if(entry.components!==undefined)pendingComponents.set(JSON.stringify([language,word]),{language,word,value:entry.components});
        if(language==='khmer'&&entry.soundMapEnabled!==undefined){
          const disabled=new Set(source.soundMapDisabledWords??[]);
          if(entry.soundMapEnabled)disabled.delete(word);else disabled.add(word);
          source.soundMapDisabledWords=[...disabled].sort();
        }
        if(language==='khmer'){
          const previous=source.mappings.filter(mapping=>mapping.khmer===word);
          const wanted=new Set((entry.spellings??previous.map(mapping=>mapping.romanized)).map(key=>key.toLowerCase()));
          if(request.type==='import')for(const mapping of previous)wanted.add(mapping.romanized);
          source.mappings=source.mappings.filter(mapping=>mapping.khmer!==word||wanted.has(mapping.romanized));
          for(const spelling of wanted){
            const existing=source.mappings.filter(mapping=>mapping.khmer===word&&mapping.romanized===spelling);
            if(existing.length){if(rankChanged)for(const mapping of existing)mapping.rank=typing.rank;}
            else source.mappings.push({id:nextId++,romanized:spelling,khmer:word,rank:request.preservePersonal===true?(entry.rank??0):typing.rank,status:'approved'});
          }
        }
        if(request.type==='save')for(const preferred of preferences){
          if((preferred.language??'khmer')!==language||(language==='english'?preferred.word.toLowerCase()!==word.toLowerCase():preferred.word!==word))continue;
          if(entry.rank!==undefined)preferred.rank=entry.rank;
          if(language==='khmer'&&entry.spellings!==undefined)preferred.spellings=preferred.spellings.filter(key=>entry.spellings.includes(key));
        }
        if(request.preservePersonal===true)mergePreference(preferences,entry);
      }
      // Imports may list a parent before its component words. Validate the
      // final explicit choices against the complete staged vocabulary.
      const known=new Set(source.words.map(row=>row.khmer));
      for(const {language,word,value} of pendingComponents.values()){
        check(language==='khmer'&&!word.includes(' '),'Component editing requires one existing Khmer word.');
        check(value===null||(Array.isArray(value)&&value.length<=64),'Use at most 64 component alternatives.');
        const seen=new Set(),chars=[...word];
        for(const parts of value??[]){
          check(Array.isArray(parts)&&parts.length>=2&&parts.length<=64&&parts.every(part=>typeof part==='string'&&known.has(part)&&!part.includes(' ')),'Choose 2–64 existing Khmer words for each component list.');
          check(parts.join('')===word,'Components must join exactly to the word.');
          check(!seen.has(JSON.stringify(parts)),'Each component alternative must be unique.');seen.add(JSON.stringify(parts));
          let end=0;
          for(const part of parts){
            end+=[...part].length;
            check(end===chars.length||(/[\u1780-\u17b3\u17e0-\u17e9]/u.test(chars[end])&&chars[end-1]!=='្'),'Components must not split attached vowels, marks or subscripts.');
          }
        }
        const current=before.catalog.khmer.find(row=>row.word===word)?.components??null;
        if(JSON.stringify(value)!==JSON.stringify(current)){
          const index=componentChoices.findIndex(row=>row.word===word);
          if(index>=0)componentChoices.splice(index,1);
          if(value!==null)componentChoices.push({word,components:value});
        }
      }
      for(const preferred of request.preferences??[])mergePreference(preferences,preferred);
      const previousPreferences=before.info.provenance.sharedWords?.words??[];
      if(JSON.stringify(pronunciationRules)===JSON.stringify(before.info.provenance.pronunciations?.rules??[])&&JSON.stringify(source)===JSON.stringify(before.source)&&JSON.stringify(preferences)===JSON.stringify(previousPreferences)&&JSON.stringify(meta)===JSON.stringify(before.metadata)&&JSON.stringify(componentChoices)===JSON.stringify(before.info.provenance.wordComponents?.words??[]))return before;
      const core=new WasmEngine(JSON.stringify(source));
      let packed,manifest;
      try{
        try{core.setPronunciations(JSON.stringify(pronunciationRules));}catch(error){throw new DictionaryError(String(error));}
        const compactHash=sha(core.exportBinary()),snapshot=sha(JSON.stringify({compactHash,words:[meta.khmer.words,meta.english.words]}));
        const json={};for(const language of ['khmer','english']){meta[language].snapshotId=snapshot;json[language]=JSON.stringify(meta[language]);}
        const provenance=structuredClone(before.info.provenance);
        if(pronunciationRules.length)provenance.pronunciations={format:'kmeow-pronunciations-v1',rules:pronunciationRules};
        else delete provenance.pronunciations;
        if(componentChoices.length)provenance.wordComponents={format:'kmeow-word-components-v1',words:componentChoices};
        else delete provenance.wordComponents;
        if(preferences.length)provenance.sharedWords={format:'kmeow-shared-words-v1',words:preferences};
        Object.assign(provenance.compactManifest,{artifactHash:compactHash,datasetHash:sha(JSON.stringify(source)),...JSON.parse(core.stats())});
        Object.assign(provenance.metadataManifest,{snapshotId:snapshot,compactArtifactHash:compactHash});
        for(const language of ['khmer','english'])Object.assign(provenance.metadataManifest.files[language],{bytes:Buffer.byteLength(json[language]),sha256:sha(json[language]),words:meta[language].words.length});
        packed=Object.fromEntries(['khmer','english'].map(language=>[language,Buffer.from(core.compileAppData(json.khmer,json.english,JSON.stringify(provenance),language))]));
        const verified=WasmEngine.fromAppData(packed.khmer,packed.english);
        try{
          const recovered=readCompactSource(verified);
          check(recovered.words.length===source.words.length&&recovered.englishWords.length===source.englishWords.length,'Dictionary vocabulary verification failed.',500);
          for(const field of ['words','englishWords']){
            const identities=new Map(recovered[field].map(word=>[word.id,word.khmer??word.text]));
            for(const word of before.source[field])check(identities.get(word.id)===(word.khmer??word.text),'An existing word identity changed.',500);
          }
          for(const language of ['khmer','english']){
            const identities=new Map(meta[language].words.map(word=>[word.id,JSON.stringify(word)]));
            for(const old of before.metadata[language].words){
              const key=JSON.stringify([language,old.word]),expected=ratingChanges.has(key)?{...old,commonnessLevel:ratingChanges.get(key)}:old;
              check(identities.get(old.id)===JSON.stringify(expected),'An existing identity or unselected rating changed.',500);
            }
          }
          const info=JSON.parse(verified.appDataInfo());manifest={...before.manifest,pairId:info.pairId,datasetHash:info.datasetHash,snapshotId:info.snapshotId,khmer:sha(packed.khmer),english:sha(packed.english)};
        }finally{verified.free();}
      }finally{core.free();}
      // Detect external CLI writes as well as other browser saves.
      const current=await Promise.all(paths.map(path=>readFile(join(root,path))));
      check(sha(Buffer.concat(current))===revision,'The dictionary changed during validation. Refresh and retry.',409);
      await commitDictionaryFiles(root,[packed.khmer,packed.english,Buffer.from(JSON.stringify(manifest,null,2)+'\n')],{commitRename});cached=undefined;
      return await readLocked();
    }finally{await release();}
  }
  return {snapshot:()=>serial(read),save:(request,revision)=>serial(()=>save(request,revision))};
}
