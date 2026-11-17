import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,readdir,rm} from 'node:fs/promises';
import {execFile,spawn} from 'node:child_process';
import {once} from 'node:events';
import {promisify} from 'node:util';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {acquireDictionaryLock} from '../dictionary-store.mjs';

const workerSource=`
import fs from 'node:fs';
import {mkdir,rmdir,writeFile} from 'node:fs/promises';
import {setTimeout as delay} from 'node:timers/promises';
import {acquireDictionaryLock} from ${JSON.stringify(new URL('../dictionary-store.mjs',import.meta.url).href)};
const [root,mode]=process.argv.slice(2);
function until(test){const end=Date.now()+5000;while(!test()){if(Date.now()>end)throw Error('Barrier timeout');Atomics.wait(new Int32Array(new SharedArrayBuffer(4)),0,0,2);}}
if(mode==='legacyA'||mode==='legacyB'){
  const nativeKill=process.kill;
  process.kill=(pid,signal)=>{
    if(pid===2147483647){
      if(mode==='legacyB'){fs.writeFileSync(root+'/B-saw-stale','');until(()=>fs.existsSync(root+'/A-entered'));}
      else until(()=>fs.existsSync(root+'/B-saw-stale'));
    }
    return nativeKill(pid,signal);
  };
  try{
    const release=await acquireDictionaryLock(root);
    try{
      if(mode==='legacyA'){fs.writeFileSync(root+'/A-entered','');until(()=>fs.existsSync(root+'/B-finished'));}
      else if(fs.existsSync(root+'/A-entered'))fs.writeFileSync(root+'/overlap','');
      console.log(JSON.stringify({acquired:true}));
    }finally{await release();}
  }catch(error){if(error.status!==423)throw error;console.log(JSON.stringify({busy:true}));}
  finally{if(mode==='legacyB')fs.writeFileSync(root+'/B-finished','');}
}else if(mode==='hold'){
  await acquireDictionaryLock(root);console.log('held');setInterval(()=>{},1000);
}else{
  for(let count=0;count<8;count++){
    const deadline=Date.now()+10000;let release;
    while(!release){
      try{release=await acquireDictionaryLock(root);}
      catch(error){if(error.status!==423||Date.now()>deadline)throw error;await delay(2+Math.random()*8);}
    }
    try{
      try{await mkdir(root+'/critical');}catch(error){await writeFile(root+'/overlap','');throw error;}
      try{await delay(3);}finally{await rmdir(root+'/critical');}
    }finally{await release();}
  }
  console.log(JSON.stringify({completed:8}));
}
`;
async function fixture(t){
  const directory=await mkdtemp(join(tmpdir(),'kmeow-lock-'));t.after(()=>rm(directory,{recursive:true,force:true}));
  const root=join(directory,'dictionary'),worker=join(directory,'worker.mjs');
  await mkdir(root);await writeFile(worker,workerSource);
  const run=mode=>promisify(execFile)(process.execPath,[worker,root,mode],{timeout:20000});
  return {root,worker,run};
}
async function noClaims(root){assert(!(await readdir(root)).some(name=>name.startsWith('.dictionary-manager.lock')));}

test('a live dictionary owner blocks another claim and release removes its files',async t=>{
  const {root}=await fixture(t),release=await acquireDictionaryLock(root);
  try{await assert.rejects(acquireDictionaryLock(root),error=>error.status===423);}
  finally{await release();}
  await noClaims(root);
});

test('a delayed stale-lock cleaner cannot delete a newly acquired writer claim',async t=>{
  const {root,run}=await fixture(t);
  await writeFile(join(root,'.dictionary-manager.lock'),JSON.stringify({pid:2147483647}));
  const [first,second]=await Promise.all([run('legacyA'),run('legacyB')]);
  assert.deepEqual(JSON.parse(first.stdout),{acquired:true});assert.deepEqual(JSON.parse(second.stdout),{busy:true});
  assert(!(await readdir(root)).includes('overlap'));await noClaims(root);
});

test('a process crash releases the dictionary to a later writer without retaining claims',async t=>{
  const {root,worker}=await fixture(t);
  const child=spawn(process.execPath,[worker,root,'hold'],{stdio:['ignore','pipe','pipe']});
  t.after(()=>{if(child.exitCode===null&&child.signalCode===null)child.kill('SIGKILL');});
  const [output]=await once(child.stdout,'data');assert.match(String(output),/held/);
  const exited=once(child,'exit');child.kill('SIGKILL');await exited;
  const release=await acquireDictionaryLock(root);await release();await noClaims(root);
});

test('competing processes serialize repeated dictionary critical sections',async t=>{
  const {root,run}=await fixture(t);
  const results=await Promise.all(Array.from({length:4},()=>run('contend')));
  for(const result of results)assert.deepEqual(JSON.parse(result.stdout),{completed:8});
  assert(!(await readdir(root)).includes('overlap'));await noClaims(root);
});
