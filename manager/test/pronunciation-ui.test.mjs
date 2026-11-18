import test from 'node:test';
import assert from 'node:assert/strict';
import {pronunciationSection} from '../pronunciations.js';
function editor(){
  class Element{
    constructor(tag){this.tag=tag;this.children=[];this.value='';this.attributes={};}
    append(...children){this.children.push(...children);}replaceChildren(...children){this.children=children;}
    setAttribute(name,value){this.attributes[name]=value;}focus(){}reset(){}
  }
  const document={createElement:tag=>new Element(tag),createElementNS:(_namespace,tag)=>new Element(tag)},saved=[];
  const stock=[{word:'កម្ម',readAs:'កាំ',originalSpellingOnly:false}];
  let overrides=[{word:'មិត្ត',readAs:'មិត'}];
  const view=pronunciationSection({document,preview:value=>[...new Set((Array.isArray(value)?value:[value])
    .map(spelling=>({'កាំ':'kam','ដេល':'del','ដែល':'dael'}[spelling]??'met')))],onSave:async next=>{
    saved.push(structuredClone(next));overrides=next;view.update(overrides,'',true,stock);
  }});
  view.update(overrides,'',true,stock);
  const list=view.root.children.find(node=>node.className==='pronunciation-list');
  const form=view.root.children.find(node=>node.tag==='form');
  return {view,saved,list,form,stock,overrides};
}
test('stock Read as rows edit and reset without copying boundary policy into the saved settings',async()=>{
  const {list,form,saved}=editor();
  assert.equal(list.children.length,2);
  assert.equal(list.children[0].children[1].attributes['aria-label'],'Edit កម្ម');
  assert.equal(list.children[0].children[2].attributes['aria-label'],'Reset កម្ម');
  list.children[0].children[1].onclick();
  form.children[1].children[1].value='មិត';
  await form.onsubmit({preventDefault(){}});
  assert.deepEqual(saved[0],[{word:'មិត្ត',readAs:'មិត'},{word:'កម្ម',readAs:'មិត'}]);
  const changed=list.children.find(row=>row.children[0].children[0].textContent==='កម្ម → មិត');
  await changed.children[2].onclick();
  assert.deepEqual(saved[1],[{word:'មិត្ត',readAs:'មិត'}]);
  assert.equal(list.children[0].children[0].children[0].textContent,'កម្ម → កាំ');
});
test('search matches the word, its respelling and its Romanized preview without changing the edit draft',()=>{
  const {view,stock,overrides,list,form,saved}=editor();
  list.children[0].children[1].onclick();
  const input=form.children[1].children[1];
  input.value='កា';
  for(const query of ['មិត្ត','មិត','met']){
    view.update(overrides,query,true,stock);
    assert.equal(list.children.length,1);
    assert.equal(list.children[0].children[0].children[0].textContent,'មិត្ត → មិត');
    assert.equal(input.value,'កា');
  }
  view.update(overrides,'no match',true,stock);
  assert.equal(list.children[0].textContent,'No matching pronunciations.');
  assert.deepEqual(saved,[]);
});
test('removing a custom row preserves stock rules and duplicate additions are rejected',async()=>{
  const {list,form,saved}=editor();
  form.children[0].children[1].value='កម្ម';form.children[1].children[1].value='មិត';
  await form.onsubmit({preventDefault(){}});assert.equal(saved.length,0);
  assert.equal(list.children[1].children[2].attributes['aria-label'],'Remove មិត្ត');
  await list.children[1].children[2].onclick();
  assert.deepEqual(saved,[[]]);assert.equal(list.children.length,1);
  assert.equal(list.children[0].children[0].children[0].textContent,'កម្ម → កាំ');
});

test('an open Read as draft cannot overwrite a newer version of the same rule', async () => {
  const {view, stock, list, form, saved} = editor();
  list.children[1].children[1].onclick();
  const alternate = form.children[1].children[1];
  alternate.value = 'កា';
  view.update([{word:'មិត្ត', readAs:'កូន'}], '', true, stock);
  await form.onsubmit({preventDefault(){}});
  assert.deepEqual(saved, []);
  assert.equal(alternate.value, 'កា');
  assert.match(view.root.children.at(-1).textContent, /dictionary changed/i);
  list.children[1].children[1].onclick();
  alternate.value = 'កា';
  await form.onsubmit({preventDefault(){}});
  assert.deepEqual(saved, [[{word:'មិត្ត', readAs:'កា'}]]);
});

test('a general pronunciation and its migrated compound scopes share one editable row',async()=>{
  const {view,stock,overrides,list,form,saved}=editor();
  const grouped=[...stock,{...stock[0],cases:[{kind:'scoped',words:['កម្មករ']}]}];
  view.update(overrides,'',true,grouped);
  assert.equal(list.children.length,2);
  list.children[0].children[1].onclick();
  form.children[1].children[1].value='មិត';
  await form.onsubmit({preventDefault(){}});
  assert.deepEqual(saved[0],[{word:'មិត្ត',readAs:'មិត'},{word:'កម្ម',readAs:'មិត'}]);
});

test('multiple Read as spellings preview together, save without duplicates and reopen',async()=>{
  const {view,list,form,saved,stock}=editor();
  form.children[0].children[1].value='ដែល';
  const alternate=form.children[1].children[1];
  alternate.value='ដេល, ដែល, ដេល';alternate.oninput();
  assert.equal(form.children.find(node=>node.tag==='output').textContent,'del / dael');
  await form.onsubmit({preventDefault(){}});
  assert.deepEqual(saved[0],[{word:'មិត្ត',readAs:'មិត'},{word:'ដែល',readAs:['ដេល','ដែល']}]);
  const row=list.children.find(node=>node.children[0]?.children[0]?.textContent==='ដែល → ដេល / ដែល');
  assert(row);row.children[1].onclick();assert.equal(alternate.value,'ដេល, ដែល');
  view.update(structuredClone(saved[0]),'dael',true,stock);
  assert.equal(list.children.length,1);
  await form.onsubmit({preventDefault(){}});
  assert.deepEqual(saved[1],saved[0]);
});

test('changed arrays invalidate an open draft even when the source array is mutated',async()=>{
  const {view,list,form,saved,stock}=editor();
  const rows=[{word:'ដែល',readAs:['ដេល','ដែល']}];
  view.update(rows,'',true,stock);
  list.children[1].children[1].onclick();
  rows[0].readAs[1]='មិត';view.update(rows,'',true,stock);
  await form.onsubmit({preventDefault(){}});
  assert.deepEqual(saved,[]);
  assert.match(view.root.children.at(-1).textContent,/dictionary changed/i);
});
