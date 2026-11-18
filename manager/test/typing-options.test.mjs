import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';

function dropdown(){
  let reads=0;
  class Target{
    constructor(parent=null){this.parent=parent;this.listeners={};this.attributes={};this.style={};}
    addEventListener(type,fn){(this.listeners[type]??=[]).push(fn);}
    emit(type,properties={}){const event={type,target:this,prevented:false,preventDefault(){this.prevented=true;},...properties};for(const fn of this.listeners[type]??[])fn(event);return event;}
    contains(target){return target===this||!!target?.parent&&this.contains(target.parent);}
    setAttribute(name,value){this.attributes[name]=value;}
    focus(){document.activeElement=this;}
    getBoundingClientRect(){reads++;return this===button?{left:700,right:900,top:130,bottom:170,width:200,height:40}:{width:340,height:350};}
  }
  const document=new Target(),window=Object.assign(new Target(),{innerWidth:1280,innerHeight:800});
  const root=new Target(),button=new Target(root),menu=Object.assign(new Target(root),{hidden:true,scrollHeight:350});
  const paragraph=new Target(menu),checkbox=Object.assign(new Target(menu),{checked:true}),outside=new Target();
  const $=selector=>selector==='#typing-options-toggle'?button:selector==='#typing-options-menu'?menu:null;
  const context=vm.createContext({$,typingOptions:root,window,document});
  const source=readFileSync(new URL('../app.js',import.meta.url),'utf8');
  vm.runInContext(source.slice(source.indexOf('const optionsButton='),source.indexOf('function language(')),context);
  return {root,button,menu,paragraph,checkbox,outside,document,window,reads:()=>reads};
}

test('drag selection never hides or repositions its text while the pointer is active',()=>{
  const page=dropdown();page.button.emit('click');assert.equal(page.menu.hidden,false);
  const reads=page.reads();
  page.document.emit('pointerdown',{target:page.paragraph});
  page.root.emit('focusout',{relatedTarget:page.outside});
  page.window.emit('scroll');page.window.emit('resize');
  assert.equal(page.menu.hidden,false);assert.equal(page.button.attributes['aria-expanded'],'true');
  assert.equal(page.reads(),reads);assert.equal(page.checkbox.checked,true);
  page.document.emit('pointerup',{target:page.outside});
  assert.equal(page.menu.hidden,false);assert(page.reads()>reads);
  page.root.emit('focusout',{relatedTarget:page.outside});assert.equal(page.menu.hidden,true);
});

test('outside clicks, Tab navigation and Escape dismiss options without intercepting text gestures',()=>{
  const page=dropdown();page.button.emit('click');
  assert.equal(page.document.emit('pointerdown',{target:page.paragraph}).prevented,false);
  assert.equal(page.menu.hidden,false);page.document.emit('pointercancel');
  assert.equal(page.document.emit('keydown',{key:'Tab',target:page.checkbox}).prevented,false);
  page.root.emit('focusout',{relatedTarget:page.checkbox});assert.equal(page.menu.hidden,false);
  page.root.emit('focusout',{relatedTarget:page.outside});assert.equal(page.menu.hidden,true);
  page.button.emit('click');assert.equal(page.document.emit('keydown',{key:'Escape',target:page.outside}).prevented,true);
  assert.equal(page.menu.hidden,true);assert.equal(page.document.activeElement,page.button);
  assert.equal(page.button.attributes['aria-expanded'],'false');
  page.button.emit('click');page.document.emit('pointerdown',{target:page.outside});
  assert.equal(page.menu.hidden,true);assert.equal(page.checkbox.checked,true);
});

test('repeated selection gestures leave the dropdown usable and route changes close it',()=>{
  const page=dropdown();
  for(let i=0;i<10;i++){
    page.button.emit('click');assert.equal(page.menu.hidden,false);
    page.document.emit('pointerdown',{target:page.paragraph});page.root.emit('focusout',{relatedTarget:page.outside});
    page.document.emit('pointerup',{target:page.paragraph});assert.equal(page.menu.hidden,false);
    page.button.emit('click');assert.equal(page.menu.hidden,true);
  }
  page.button.emit('click');page.window.emit('hashchange');assert.equal(page.menu.hidden,true);
});
