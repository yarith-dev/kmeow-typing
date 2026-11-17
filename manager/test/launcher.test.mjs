import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,copyFile,writeFile,readFile,rm} from 'node:fs/promises';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

async function fixture(t,{built=false,buildFails=false}={}){
  const root=await mkdtemp(join(tmpdir(),'kmeow-launch-'));t.after(()=>rm(root,{recursive:true,force:true}));
  for(const directory of ['manager','scripts'])await mkdir(join(root,directory));
  await copyFile(new URL('../start.mjs',import.meta.url),join(root,'manager/start.mjs'));
  const {scripts}=JSON.parse(await readFile(new URL('../../package.json',import.meta.url),'utf8'));
  await writeFile(join(root,'package.json'),JSON.stringify({type:'module',scripts}));
  await writeFile(join(root,'manager/server.mjs'),`export function startManager(){console.log('MANAGER_STARTED');}`);
  await writeFile(join(root,'scripts/build-public.mjs'),buildFails?`throw Error('BUILD_SHOULD_NOT_RUN');`:
    `import fs from 'node:fs';fs.mkdirSync('dist/public',{recursive:true});fs.writeFileSync('dist/public/node.js','');console.log('API_BUILT');`);
  if(built){await mkdir(join(root,'dist/public'),{recursive:true});await writeFile(join(root,'dist/public/node.js'),'');}
  return root;
}
const launch=root=>promisify(execFile)(process.platform==='win32'?'npm.cmd':'npm',['run','manage'],{cwd:root,timeout:15000,shell:process.platform==='win32'});

test('the packaged manager starts with prebuilt files and no development dependencies',async t=>{
  const root=await fixture(t,{built:true,buildFails:true}),result=await launch(root);
  assert.match(result.stdout,/MANAGER_STARTED/);assert.doesNotMatch(result.stdout,/API_BUILT/);
});

test('a source checkout builds its missing API before starting the manager',async t=>{
  const root=await fixture(t),result=await launch(root);
  assert.match(result.stdout,/API_BUILT[\s\S]*MANAGER_STARTED/);
  const again=await launch(root);assert.match(again.stdout,/MANAGER_STARTED/);assert.doesNotMatch(again.stdout,/API_BUILT/);
});

test('a failed source build stops startup before the server runs',async t=>{
  const root=await fixture(t,{buildFails:true});
  await assert.rejects(launch(root),error=>{assert.match(error.stderr,/BUILD_SHOULD_NOT_RUN/);assert.doesNotMatch(error.stdout,/MANAGER_STARTED/);return true;});
});
