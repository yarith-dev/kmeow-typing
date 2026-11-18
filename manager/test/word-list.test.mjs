import test from 'node:test';
import assert from 'node:assert/strict';
import {wordList,wordPage,wordPageLinks,wordLetters,wordsForLetter,romanizedKeys,wordsWithoutRomanized,suggestionWord} from '../word-list.js';
import {loadDictionaryWords} from '../dictionary.mjs';
import {readFileSync} from 'node:fs';

test('the catalog includes every shipped identity and POS without changing either dictionary',async()=>{
  const before=Object.fromEntries(['khmer','english'].map(lang=>[lang,readFileSync(new URL(`../../compiled-data/${lang}.kmd`,import.meta.url))]));
  const catalog=await loadDictionaryWords();
  for(const [language,bytes] of Object.entries(before)){
    const metadata=JSON.parse(bytes.subarray(24+bytes.readUInt32LE(12)+bytes.readUInt32LE(16),-32));
    assert.deepEqual(catalog[language].map(({id,word,partsOfSpeech})=>({id,word,partsOfSpeech})),metadata.words.map(({id,word,partsOfSpeech})=>({id,word,partsOfSpeech})));
    assert.deepEqual(readFileSync(new URL(`../../compiled-data/${language}.kmd`,import.meta.url)),bytes);
    assert.deepEqual(wordList(catalog,[],language).flatMap(row=>row.ids).sort(),catalog[language].map(row=>row.id).sort());
    const rows=wordList(catalog,[],language),letters=wordLetters(rows,language);
    assert.deepEqual(letters[0],{letter:'',count:rows.length});
    const groups=letters.filter(({letter,count})=>letter&&count).map(({letter,count})=>{
      const group=wordsForLetter(rows,language,letter);assert.equal(group.length,count);return group;
    });
    assert.deepEqual(groups.flat().flatMap(row=>row.ids).sort(),rows.flatMap(row=>row.ids).sort());
    for(const group of groups)assert.deepEqual(group,rows.filter(row=>group.includes(row)));
  }
  const book=wordList(catalog,[],'khmer','sievphov').find(row=>row.word==='សៀវភៅ');
  assert(book);assert(romanizedKeys(book).includes('sievphov'));
  assert(catalog.khmer.every(row=>Array.isArray(row.romanizations)&&Array.isArray(row.soundMapInputs)));

});

test('letter filters include independent Khmer vowels, English case variants and other initials without mutating rows',()=>{
  const khmer=[{word:'ខ្មែរ'},{word:'ឥឡូវ'},{word:'កូន'},{word:'123'},{word:'ឯង'}],english=[{word:'Book'},{word:'book'},{word:'apple'},{word:'Éclair'},{word:'🐈'}];
  const before=JSON.stringify({khmer,english});
  assert.equal(wordsForLetter(khmer,'khmer'),khmer);
  assert.deepEqual(wordsForLetter(khmer,'khmer','ឥ'),[khmer[1]]);
  assert.deepEqual(wordsForLetter(khmer,'khmer','ឯ'),[khmer[4]]);
  assert.deepEqual(wordsForLetter(khmer,'khmer','other'),[khmer[3]]);
  assert.deepEqual(wordsForLetter(english,'english','B'),english.slice(0,2));
  assert.deepEqual(wordsForLetter(english,'english','other'),english.slice(3));
  assert.equal(wordLetters(english,'english').find(row=>row.letter==='B').count,2);
  assert.equal(wordLetters(english,'english').find(row=>row.letter==='X').count,0);
  assert.equal(wordLetters(english,'english').at(-1).letter,'other');
  assert.equal(new Set(wordLetters(khmer,'khmer').map(row=>row.letter)).size,wordLetters(khmer,'khmer').length);
  assert.equal(JSON.stringify({khmer,english}),before);
});

test('missing Romanized means no saved, generated or personal readings, with unknown truncated readings kept out',()=>{
  const catalog={khmer:[
    {id:1,word:'ក',partsOfSpeech:[]},
    {id:2,word:'ខ',partsOfSpeech:[],romanizations:['kh']},
    {id:3,word:'គ',partsOfSpeech:[],soundMapInputs:['k']},
    {id:4,word:'ឃ',partsOfSpeech:[]},
    {id:5,word:'ង',partsOfSpeech:[],soundMapInputsTruncated:true},
  ],english:[{id:6,word:'book',partsOfSpeech:[]}]};
  const personal=[{word:'ឃ',spellings:['kh']}],before=JSON.stringify({catalog,personal});
  const rows=wordList(catalog,personal,'khmer');
  assert.deepEqual(wordsWithoutRomanized(rows).map(row=>row.word),['ក']);
  assert.deepEqual(wordsWithoutRomanized(wordList(catalog,personal,'english')),[]);
  assert.deepEqual(wordsWithoutRomanized([]),[]);
  assert.equal(JSON.stringify({catalog,personal}),before);
});

test('combined rows merge additions without hiding dictionary identities or mutating inputs',()=>{
  const catalog={khmer:[{id:'1',word:'សៀវភៅ',partsOfSpeech:['noun']}],english:[{id:'2',word:'book',partsOfSpeech:['noun']},{id:'3',word:'book',partsOfSpeech:['verb']}]};
  const personal=[{word:'សៀវភៅ',spellings:['mybook']},{word:'kmeow',language:'english',spellings:[]}];
  const snapshot=JSON.stringify({catalog,personal});
  const khmer=wordList(catalog,personal,'khmer');assert.equal(khmer.length,1);assert(khmer[0].bundled);assert.deepEqual(khmer[0].personalIndices,[0]);assert.deepEqual(khmer[0].spellings,['mybook']);
  assert.equal(wordList(catalog,personal,'khmer','MYBOOK').length,1);
  const english=wordList(catalog,personal,'english');assert.equal(english.length,2);assert.deepEqual(english[0].ids,['2','3']);assert.deepEqual(english[0].partsOfSpeech,['noun','verb']);assert.deepEqual(english[1].personalIndices,[1]);assert.equal(english[1].bundled,false);
  const removed=wordList(catalog,[],'khmer');assert.equal(removed.length,1);assert.deepEqual(removed[0].personalIndices,[]);
  assert.equal(JSON.stringify({catalog,personal}),snapshot);
});

test('pagination covers all rows and clamps after search or deletion',()=>{
  const rows=Array.from({length:61},(_,i)=>({word:String(i)}));
  const combined=[0,1,2].flatMap(page=>wordPage(rows,page).rows);assert.deepEqual(combined,rows);
  assert.equal(wordPage(rows,90).page,2);assert.equal(wordPage([],90).page,0);assert.equal(wordPage([],0).pages,1);
  assert.equal(wordPage(rows,2).from,51);assert.equal(wordPage(rows,2).to,61);
  assert.equal(wordPage([],0).from,0);assert.equal(wordPage([],0).to,0);
});

test('page links keep the first, last and current pages visible with bounded gaps',()=>{
  assert.deepEqual(wordPageLinks(0,183),[0,1,2,3,null,182]);
  assert.deepEqual(wordPageLinks(91,183),[0,null,90,91,92,null,182]);
  assert.deepEqual(wordPageLinks(182,183),[0,null,179,180,181,182]);
  for(const total of [...Array.from({length:20},(_,i)=>i+1),183,283])for(let current=0;current<total;current++){
    const links=wordPageLinks(current,total),numbers=links.filter(number=>number!==null);
    assert.equal(numbers[0],0);assert.equal(numbers.at(-1),total-1);assert(numbers.includes(current));
    assert.equal(new Set(numbers).size,numbers.length);assert(links.length<=7);
    for(let index=1;index<links.length;index++){
      if(links[index]===null){assert(links[index+1]-links[index-1]>2);continue;}
      if(links[index-1]!==null)assert.equal(links[index]-links[index-1],1);
    }
  }
});

test('compact readings combine personal, saved and generated keys without duplicates',()=>{
  const row={spellings:['custom','saved'],romanizations:['saved'],soundMapInputs:['generated','saved']};
  assert.deepEqual(romanizedKeys(row),['custom','saved','generated']);
});

test('suggestion management resolves the exact identity, its additions and English casing',()=>{
  const bundled={khmer:[{id:1,word:'សៀវភៅ',partsOfSpeech:['noun'],romanizations:['sievphov']}],english:[{id:2,word:'book',partsOfSpeech:['noun']}]};
  const personal=[{word:'សៀវភៅ',spellings:['mybook'],rank:3},{word:'Kmeow',language:'english'}];
  const before=JSON.stringify({bundled,personal});
  const row=suggestionWord(bundled,personal,'khmer',{khmer:'សៀវភៅ'});
  assert.equal(row.bundled,true);assert.deepEqual(row.ids,[1]);assert.deepEqual(row.personalIndices,[0]);assert.deepEqual(row.spellings,['mybook']);
  assert.equal(suggestionWord(bundled,personal,'english',{text:'Book'}).word,'book');
  assert.deepEqual(suggestionWord(bundled,personal,'english',{text:'kmeow'}).personalIndices,[1]);
  const generated=suggestionWord(bundled,personal,'english',{text:'books',mappingId:0});
  assert.equal(generated.word,'books');assert.equal(generated.bundled,false);assert.deepEqual(generated.personalIndices,[]);
  assert.equal(JSON.stringify({bundled,personal}),before);
});

test('excluded inputs remain stored but do not appear as available readings in compact rows',()=>{
  const catalog={khmer:[{word:'កូន',id:1,partsOfSpeech:[],romanizations:['kon','kid'],soundMapInputs:['kon','kun'],excludedInputs:['kon']}],english:[]};
  const before=structuredClone(catalog),row=wordList(catalog,[{word:'កូន',spellings:['kon','mykid']}],'khmer')[0];
  assert.deepEqual(row.excludedInputs,['kon']);
  assert.deepEqual(romanizedKeys(row),['mykid','kid','kun']);
  assert(row.romanizations.includes('kon'));assert.deepEqual(catalog,before);
});
