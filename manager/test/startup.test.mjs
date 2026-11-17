import * as i18n from '../i18n.js';
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';

async function startup({stored=null,loaded=true}={}){
  const nodes=new Map(),$=id=>{
    if(!nodes.has(id))nodes.set(id,{disabled:true,textContent:'',append(){}});
    return nodes.get(id);
  };
  let synced=false;
  const context=vm.createContext({...i18n,navigator:{language:'en'},applyUiLanguage(){},$,WORDS_KEY:'legacy',localStorage:{getItem:()=>stored},legacyWords:[],damagedWords:null,map:null,
    language(){},parseImport:JSON.parse,announce:(message='')=>{$('#status').textContent=message;},
    loadWordList:async()=>{if(!loaded)$('#status').textContent='Dictionary unavailable';return loaded;},
    syncEditor(){synced=true;},fetch:async()=>({ok:true,json:async()=>({})}),renderMap(){},renderPronunciations(){},el(){},errorMessage:error=>error.message});
  const source=readFileSync(new URL('../app.js',import.meta.url),'utf8');
  await vm.runInContext(source.slice(source.indexOf('async function start(){')),context);
  return {$,synced};
}

test('successful startup enables typing without a global loading notice',async()=>{
  for(const stored of [null,'[]','[{"word":"សៀវភៅ","spellings":["mybook"]}]']){
    const page=await startup({stored});
    assert.equal(page.$('#status').textContent,'');assert.equal(page.synced,true);
    for(const id of ['typing-input','message'])assert.equal(page.$('#'+id).disabled,false);
  }
});

test('startup failures keep the error visible and controls disabled',async()=>{
  const page=await startup({loaded:false});
  assert.equal(page.$('#status').textContent,'Dictionary unavailable');assert.equal(page.synced,false);
  assert.equal(page.$('#typing-input').disabled,true);assert.equal(page.$('#message').disabled,true);
});

test('a working engine keeps the recovery warning for unreadable previous browser words',async()=>{
  const page=await startup({stored:'unreadable'});
  assert.match(page.$('#status').textContent,/Export a recovery copy/);
  assert.equal(page.$('#typing-input').disabled,false);assert.equal(page.synced,true);
});
