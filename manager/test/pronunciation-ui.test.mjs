import test from 'node:test';
import assert from 'node:assert/strict';
import {pronunciationSection} from '../pronunciations.js';
function editor(){
  class Element{
    constructor(tag){this.tag=tag;this.children=[];this.value='';this.attributes={};}
    append(...children){this.children.push(...children);}replaceChildren(...children){this.children=children;}
    setAttribute(name,value){this.attributes[name]=value;}focus(){}reset(){}
  }
  const document={createElement:tag=>new Element(tag)},saved=[];
  const stock=[{word:'កម្ម',readAs:'កាំ',originalSpellingOnly:false}];
  let overrides=[{word:'មិត្ត',readAs:'មិត'}];
  const view=pronunciationSection({document,preview:value=>[value==='កាំ'?'kam':'met'],onSave:async next=>{
    saved.push(structuredClone(next));overrides=next;view.update(overrides,'',true,stock);
  }});
  view.update(overrides,'',true,stock);
  const list=view.root.children.find(node=>node.className==='pronunciation-list');
  const form=view.root.children.find(node=>node.tag==='form');
  return {view,saved,list,form,stock,overrides};
}
test('stock Read as rows edit and reset without copying boundary policy into the saved settings',async()=>{
  const {list,form,saved}=editor();
  assert.equal(list.children.length,2);assert.equal(list.children[0].children[2].textContent,'Reset');
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
  await list.children[1].children[2].onclick();
  assert.deepEqual(saved,[[]]);assert.equal(list.children.length,1);
  assert.equal(list.children[0].children[0].children[0].textContent,'កម្ម → កាំ');
});
