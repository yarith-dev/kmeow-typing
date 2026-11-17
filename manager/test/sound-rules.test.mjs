import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {soundRules,ruleFamilies,ruleFamilyDefinitions,renderRuleValue,renderRuleExamples} from '../sound-rules.js';
import {loadDictionaryWords} from '../dictionary.mjs';
import {WasmEngine} from '../../wasm/khmer_engine.js';
import {setUiLanguage} from '../i18n.js';

const runtime=JSON.parse(readFileSync(new URL('../../data/khmer-sound-map.json',import.meta.url))).runtime;
test('each rule guide has exactly one source definition',()=>{
  for(const [file,declaration]of [['sound-rules.js','const guides={'],['khmer-ui.js','export const ruleGuides={']]){
    const source=readFileSync(new URL('../'+file,import.meta.url),'utf8');
    const start=source.indexOf(declaration),block=source.slice(start,source.indexOf('\n};',start));
    const names=[...block.matchAll(/^\s{2}(?:([\w$]+)|['"]([^'"]+)['"]):/gm)].map(match=>match[1]??match[2]);
    assert(names.length>0);
    assert.deepEqual(names.filter((name,index)=>names.indexOf(name)!==index),[],file+': rule guides must not silently overwrite earlier definitions');
  }
});

test('the reuse guide retains alternatives, shorthand protections and its intended examples in both languages',()=>{
  const config={wordRuleReuse:{enabled:true,rankPriority:-3000000}};
  const catalog=[{word:'កិច្ចការ',soundMapInputs:['kechka','kichka']},{word:'ខកចិត្ត',soundMapInputs:['khokchet']}];
  const before=JSON.stringify({config,catalog});
  try{
    setUiLanguage('en');const english=soundRules(config,'',catalog)[0];
    assert.equal(english.title,'Using word readings in compounds');
    assert(english.description.includes('Alternative readings'));
    assert(english.description.includes('Shorthands require explicit selection'));
    assert(english.description.includes('Saved aliases and ambiguous splits are not reused'));
    assert(english.description.includes('existing full-word readings remain available'));
    assert.deepEqual(english.examples.map(row=>row.word),['កិច្ចការ','ខកចិត្ត']);
    setUiLanguage('km');const khmer=soundRules(config,'',catalog)[0];
    assert.notEqual(khmer.title,english.title);assert(/[\u1780-\u17ff]/.test(khmer.title));
    assert(khmer.description.includes('ជ្រើសរើសដោយផ្ទាល់'));
    assert(khmer.description.includes('របៀបវាយដែលបានរក្សាទុក'));
    assert.deepEqual(khmer.examples,english.examples);assert.equal(khmer.value,config.wordRuleReuse);
    assert.equal(JSON.stringify({config,catalog}),before);
  }finally{setUiLanguage('en');}
});

test('every configured rule group is visible and nested conditions are searchable',()=>{
  const snapshot=JSON.stringify(runtime),rules=soundRules(runtime);
  assert.deepEqual(rules.map(rule=>rule.id),Object.keys(runtime));
  for(const rule of rules){assert.equal(rule.value,runtime[rule.id]);assert(rule.title&&rule.description);}
  assert(soundRules(runtime,'ទស្សន៍').some(rule=>rule.id==='terminalReadingUnits'));
  assert(soundRules(runtime,'silent endings').some(rule=>rule.id==='silentWordEndings'));
  assert(soundRules(runtime,'TARGET SERIES').some(rule=>rule.id==='crossSyllableGovernance'||rule.id==='riyaGovernance'));
  assert.deepEqual(soundRules(runtime,'no-such-rule-xyz'),[]);
  assert.equal(soundRules({...runtime,futureRule:{keys:['new-key']}},'new-key').at(-1).id,'futureRule');
  assert.equal(JSON.stringify(runtime),snapshot);
});

const document={createElement(tag){return {tag,children:[],textContent:'',setAttribute(){},append(...children){this.children.push(...children);}};}};
const allText=node=>[node.textContent,...node.children.map(allText)].join(' ');
test('expanded rules keep full nested scope and use text nodes for values',()=>{
  const value={khmer:'ទស្សន៍',words:['ទស្សន៍','ទូរទស្សន៍'],enabled:false,keys:[],conditions:{targetSeries:'a',originalSpellingOnly:true},retainedReadings:[['s','a'],['s','o']],unknown:'<script>test</script>'};
  const rendered=renderRuleValue(value,'',document),text=allText(rendered);
  for(const expected of ['ទស្សន៍','ទូរទស្សន៍','No','None configured','A-series','Yes','<script>test</script>'])assert(text.includes(expected),expected);
  assert(allText(renderRuleValue(runtime.wordParts,'',document)).includes('អ្នកនេសាទ'));
  assert.equal(rendered.tag,'dl');
});

test('every rule has readable examples verified against the same engine',async()=>{
  const {khmer}=await loadDictionaryWords(),rules=soundRules(runtime,'',khmer);
  const core=WasmEngine.fromAppData(readFileSync(new URL('../../compiled-data/khmer.kmd',import.meta.url)),readFileSync(new URL('../../compiled-data/english.kmd',import.meta.url)));
  const patterns=rules.flatMap(rule=>rule.examples).filter(example=>!example.bundled);
  const illustration=new WasmEngine(JSON.stringify({schemaVersion:2,mappings:[],words:patterns.map((row,i)=>({id:91000+i,khmer:row.word,rank:0,status:'approved'}))}));
  try{
    for(const rule of rules){
      assert(rule.examples.length,`${rule.id}: missing word examples`);
      for(const example of rule.examples){
        if(!example.bundled)assert(!khmer.some(row=>row.word===example.word));
        for(const input of example.inputs){
          const result=JSON.parse((example.bundled?core:illustration).suggestWithContext(input,'{}','{}'));
          assert(result.exactCandidates.some(candidate=>candidate.khmer===example.word),`${rule.id}: ${input} → ${example.word}`);
        }
      }
    }
    assert(soundRules(runtime,'kontreanh',khmer).some(rule=>rule.id==='nasalBoundaries'));
    const marked=rules.find(rule=>rule.id==='silentCodaClusters');
    assert(marked.examples.some(example=>example.word==='សិស្ស'&&example.inputs.includes('serh')));
    const rendered=allText(renderRuleExamples(patterns,document));
    assert(rendered.includes('not in the bundled word list'));
    assert(rendered.includes('កន្ទ្រាញ')&&rendered.includes('kontreanh'));
    const scopes=allText(renderRuleValue(runtime.wordParts,'',document,khmer));
    assert(scopes.includes('neak')&&scopes.includes('អ្នក'));
  }finally{core.free();illustration.free();}
});

test('families cover each rule once and keep nested searches visible',()=>{
  const families=ruleFamilies(runtime);
  assert.equal(families.length,ruleFamilyDefinitions.length);
  assert.equal(ruleFamilyDefinitions.length,9);
  const declared=ruleFamilyDefinitions.flatMap(family=>family.groups);
  assert.equal(new Set(declared).size,declared.length);
  assert.deepEqual(families.flatMap(family=>family.rules.map(rule=>rule.id)).sort(),Object.keys(runtime).sort());
  assert.deepEqual(ruleFamilies(runtime,'syllable splitting').map(family=>family.id),['syllables']);
  assert(ruleFamilies(runtime,'ទស្សន៍').some(family=>family.id==='silence'));
  assert.equal(runtime.wordParts.length,new Set(runtime.wordParts.map(part=>part.khmer)).size);
  const heart=runtime.wordParts.find(part=>part.khmer==='ចិត្ត');
  assert(heart.cases.some(item=>item.kind==='ending')&&heart.cases.some(item=>item.kind==='scoped'));
});

test('nh belongs only to the separate Shorthands registry and family',()=>{
  assert(!runtime.wordParts.some(part=>part.khmer==='ខ្ញុំ'||part.readingType==='shorthand'));
  assert(runtime.shorthands.some(part=>part.khmer==='ខ្ញុំ'&&part.keys.includes('nh')));
  try{
    for(const language of ['en','km']){
      setUiLanguage(language);
      const families=ruleFamilies(runtime);
      const shorthand=families.find(family=>family.id==='shorthands');
      assert.equal(shorthand.rules.length,1);assert.equal(shorthand.rules[0].id,'shorthands');
      assert(!families.find(family=>family.id==='parts').rules.some(rule=>rule.id==='shorthands'));
      assert(ruleFamilies(runtime,'ខ្ញុំ').some(family=>family.id==='shorthands'));
      assert(ruleFamilies(runtime,'nh').some(family=>family.id==='shorthands'));
    }
  }finally{setUiLanguage('en');}
});

test('word-part searches narrow entries and examples without changing their full scopes',async()=>{
  const snapshot=JSON.stringify(runtime),{khmer}=await loadDictionaryWords();
  for(const query of ['ធម៌','ព័ន្ធ']){
    const direct=soundRules(runtime,query,khmer).find(rule=>rule.id==='wordParts');
    const family=ruleFamilies(runtime,query,khmer).find(family=>family.id==='parts');
    assert(direct.value.length>0&&direct.value.length<runtime.wordParts.length);
    assert.deepEqual(family.rules.find(rule=>rule.id==='wordParts'),direct);
    for(const part of direct.value){
      assert(JSON.stringify(part).includes(query));
      assert.equal(part,runtime.wordParts.find(original=>original.khmer===part.khmer));
    }
    assert(direct.examples.length>0);
    assert(direct.examples.every(example=>example.word.includes(query)));
    const rendered=allText(renderRuleValue(direct.value,'',document,khmer));
    assert(!rendered.includes('អ្នកនេសាទ'));
  }
  const word=khmer.find(row=>row.word==='ធម៌'),input=word.soundMapInputs[0];
  const latin=soundRules(runtime,input.toUpperCase(),khmer).find(rule=>rule.id==='wordParts');
  assert(latin.value.some(part=>part.khmer==='ធម៌'));
  assert(latin.examples.some(example=>example.inputs.some(key=>key.includes(input))));
  assert.equal(soundRules(runtime,'word-part readings',khmer).find(rule=>rule.id==='wordParts').value,runtime.wordParts);
  assert.equal(JSON.stringify(runtime),snapshot);
});

test('word exceptions are displayed separately while retaining every case',()=>{
  const cases=[
    {kind:'syllable',order:0},
    {kind:'scoped',order:0,words:['ខ្ញុំ'],positions:['standalone']},
    {kind:'ending',order:0,words:['មិត្ត','ខេត្ត']},
  ];
  const snapshot=JSON.stringify(cases),rendered=renderRuleValue(cases,'cases',document);
  assert.deepEqual(rendered.children.map(section=>section.children[0].textContent),
    ['Reusable patterns','Selected word groups','Word exceptions']);
  assert(allText(rendered.children[2]).includes('ខ្ញុំ'));
  assert(!allText(rendered.children[2]).includes('មិត្ត'));
  assert(allText(rendered.children[1]).includes('មិត្ត'));
  assert.equal(JSON.stringify(cases),snapshot);
});
