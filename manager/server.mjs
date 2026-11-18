import { dictionaryStore, DictionaryError } from './dictionary-store.mjs';
import { randomBytes, timingSafeEqual } from 'node:crypto';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve, sep } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
const types = { '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.json': 'application/json', '.wasm': 'application/wasm', '.kmd': 'application/octet-stream', '.woff2': 'font/woff2' };
export function managerServer({dictionaryRoot=root,store=dictionaryStore(dictionaryRoot)}={}) {
  const token=randomBytes(32).toString('hex');
  const catalog=snapshot=>({...snapshot.catalog,sharedWords:snapshot.info.provenance.sharedWords?.words??[],pronunciations:snapshot.info.provenance.pronunciations?.rules??[],revision:snapshot.revision,token});
  return createServer(async (req, res) => {
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('Cache-Control', 'no-store');
    res.setHeader('Content-Security-Policy', "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; font-src 'self'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'");
    try {
      const url=new URL(req.url,'http://localhost'),pathname=decodeURIComponent(url.pathname);
      const host=new URL(`http://${req.headers.host}`).hostname;
      if(!['127.0.0.1','localhost','[::1]'].includes(host)){res.writeHead(403);res.end('Use the local manager address.');return;}
      if(pathname==='/api/words'&&req.method==='POST'){
        const supplied=Buffer.from(req.headers['x-kmeow-token']??'');
        if(supplied.length!==token.length||!timingSafeEqual(supplied,Buffer.from(token))
          ||req.headers['sec-fetch-site']==='cross-site'
          ||(req.headers.origin&&req.headers.origin!==`http://${req.headers.host}`))throw new DictionaryError('Dictionary saves must come from this local manager.',403);
        if(req.headers['content-type']?.split(';')[0]!=='application/json')throw new DictionaryError('Send a JSON dictionary edit.',415);
        let size=0;const chunks=[];
        for await(const chunk of req){size+=chunk.length;if(size>2*1024*1024)throw new DictionaryError('Import a file smaller than 2 MB.',413);chunks.push(chunk);}
        let request;try{request=JSON.parse(Buffer.concat(chunks).toString('utf8'));}catch{throw new DictionaryError('Invalid JSON.');}
        const snapshot=await store.save(request,req.headers['if-match']);
        res.setHeader('Content-Type','application/json');res.writeHead(200);res.end(JSON.stringify(catalog(snapshot)));return;
      }
      if(!['GET','HEAD'].includes(req.method)){res.writeHead(405);res.end();return;}
      if (pathname === '/api/words') {
        const body=JSON.stringify(catalog(await store.snapshot()));
        res.setHeader('Content-Type','application/json');res.writeHead(200);res.end(req.method==='HEAD'?undefined:body);
        return;
      }
      if(['/compiled-data/khmer.kmd','/compiled-data/english.kmd'].includes(pathname)){
        const snapshot=await store.snapshot();
        if(url.searchParams.has('revision')&&url.searchParams.get('revision')!==snapshot.revision)throw new DictionaryError('Dictionary changed. Refresh and retry.',409);
        res.setHeader('Content-Type','application/octet-stream');res.writeHead(200);res.end(req.method==='HEAD'?undefined:snapshot.files[pathname.includes('khmer')?'khmer':'english']);return;
      }
      const relative = pathname === '/' ? 'manager/index.html' : pathname.slice(1);
      const allowed = ['manager/index.html', 'manager/app.js', 'manager/converter.js', 'manager/i18n.js', 'manager/khmer-ui.js', 'manager/examples.js', 'manager/sound-rules.js', 'manager/pronunciations.js', 'manager/field-help.js', 'manager/suggestion-view.js', 'manager/suggestion-gesture.js', 'manager/word-list.js', 'manager/composer.js', 'manager/state.js', 'manager/style.css', 'manager/mobile.css',
        'manager/fonts/noto-sans-khmer-khmer-400-normal.woff2', 'data/khmer-sound-map.json',
        'compiled-data/khmer.kmd', 'compiled-data/english.kmd', 'wasm/khmer_engine.js', 'wasm/khmer_engine_bg.wasm'];
      const file = resolve(root, relative);
      const isModule = relative.startsWith('dist/') && relative.endsWith('.js');
      if ((!allowed.includes(relative) && !isModule) || !file.startsWith(root + (root.endsWith(sep) ? '' : sep)) || relative.split('/').includes('..')) {
        res.writeHead(404); res.end('Not found'); return;
      }
      const body = await readFile(file);
      const extension = file.slice(file.lastIndexOf('.'));
      res.setHeader('Content-Type', types[extension] ?? 'application/octet-stream');
      res.writeHead(200); res.end(req.method === 'HEAD' ? undefined : body);
    } catch(error) {
      if(error.code==='ENOENT'){res.writeHead(404);res.end('Not found');return;}
      res.setHeader('Content-Type','application/json');res.writeHead(error.status??500);res.end(JSON.stringify({error:error.message??'Could not read or save the dictionary.'}));
    }
  });
}
export function startManager() {
  const port = Number(process.env.PORT || 3001);
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('PORT must be between 1 and 65535.');
  const server = managerServer();
  server.on('error', error => { console.error(error.message); process.exitCode = 1; });
  for(const signal of ['SIGTERM','SIGINT'])process.once(signal,()=>server.close(()=>{process.exitCode=0;}));
  server.listen(port, '127.0.0.1', () => console.log(`Kmeow Engine Manager: http://127.0.0.1:${port}`));
  return server;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) startManager();
