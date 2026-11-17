import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {WasmEngine,initSync} from '../wasm/khmer_engine.js';
const read=path=>readFileSync(new URL('../'+path,import.meta.url));
initSync({module:read('wasm/khmer_engine_bg.wasm')});

test('removed default-equivalent overrides use remaining readings and retain their dictionary words',()=>{
  const {examples}=JSON.parse(read('test/fixtures/removed-default-word-rules.json'));
  const parts=JSON.parse(read('data/khmer-sound-map.json')).runtime.wordParts;
  const core=WasmEngine.fromAppData(read('compiled-data/khmer.kmd'),read('compiled-data/english.kmd'));
  try{
    // This fixture describes the ordinary map after the 35 overrides were
    // removed; owner pronunciation settings are a separate configurable layer.
    core.setPronunciations('[]');
    for(const {part,word,input,retiredInput} of examples){
      assert(!parts.some(rule=>rule.khmer===part),part);
      const result=JSON.parse(core.suggestWithContext(input,'{}','{}'));
      assert(result.exactCandidates.some(candidate=>candidate.khmer===word),`${input} → ${word}`);
      assert(JSON.parse(core.suggestWithContext(word,'{}','{}')).candidates.some(candidate=>candidate.khmer===word),word);
      if(retiredInput)assert(!JSON.parse(core.suggestWithContext(retiredInput,'{}','{}'))
        .exactCandidates.some(candidate=>candidate.khmer===word),`${retiredInput} → ${word}`);
    }
  }finally{core.free();}
});
