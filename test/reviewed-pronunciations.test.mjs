import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {WasmEngine,initSync} from '../wasm/khmer_engine.js';
import {readCompactSource} from '../scripts/read-compact-source.mjs';
const read=file=>readFileSync(new URL('../'+file,import.meta.url));
initSync({module:read('wasm/khmer_engine_bg.wasm')});
const fixture=JSON.parse(read('test/fixtures/reviewed-pronunciations.json'));
function page(core){let after;const words=[];do{const p=JSON.parse(core.browseWords(JSON.stringify({query:'',limit:100,after})));words.push(...p.words);after=p.nextAfter;}while(after);return words;}

test('reviewed Khmer respellings generate exact original-word suggestions without losing words or saved aliases',()=>{
  const core=WasmEngine.fromAppData(read('compiled-data/khmer.kmd'),read('compiled-data/english.kmd'));
  try{
    core.setPronunciations('[]');const source=readCompactSource(core),before=page(core);
    core.setPronunciations(JSON.stringify(fixture.rules));
    assert.deepEqual(readCompactSource(core),source);
    const after=page(core),byWord=new Map(after.map(w=>[w.khmer,w]));
    for(const word of before){
      const next=byWord.get(word.khmer);assert(next,word.khmer);
      assert.deepEqual(next.romanizations,word.romanizations,word.khmer);
      if(word.soundMapInputs?.length)assert(next.soundMapInputs?.length,`readings lost: ${word.khmer}`);
    }
    for(const [input,word] of [...fixture.examples,...fixture.compoundExamples]){
      assert(JSON.parse(core.suggestWithContext(input,'{}','{}')).exactCandidates.some(c=>c.khmer===word),`${input} → ${word}`);
    }
    for(const word of fixture.normalDefaults)assert(!fixture.rules.some(r=>r.word===word),word);
    for(const word of ['book','books','children','wrote'])assert(JSON.parse(core.suggestWithContext(word,'{"mode":"english"}','{}')).candidates.length,word);
  }finally{core.free();}
});
