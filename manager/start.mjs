import {existsSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const root=new URL('../',import.meta.url);
// Release archives already contain the runnable API. A fresh source checkout
// builds it once using the development tools installed by npm ci.
if(!existsSync(new URL('dist/public/node.js',root))){
  const result=spawnSync(process.execPath,[fileURLToPath(new URL('scripts/build-public.mjs',root))],
    {cwd:fileURLToPath(root),stdio:'inherit'});
  if(result.error)throw result.error;
  if(result.status!==0)process.exit(result.status??1);
}
const {startManager}=await import('./server.mjs');
startManager();
