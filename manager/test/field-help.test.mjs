import test from 'node:test';
import assert from 'node:assert/strict';
import {attachFieldHelp} from '../field-help.js';

function fixture({popover=true}={}){
  class Element{
    constructor(rect={left:60,top:70,right:88,bottom:98,width:28,height:28}){
      this.events={};this.attributes={};this.style={};this.rect=rect;this.hidden=true;
    }
    addEventListener(type,handler){(this.events[type]??=[]).push(handler);}
    emit(type,event={}){
      event.target??=this;event.preventDefault=()=>{event.prevented=true;};event.stopPropagation=()=>{event.stopped=true;};
      for(const handler of this.events[type]??[])handler(event);return event;
    }
    setAttribute(key,value){this.attributes[key]=value;}
    getAttribute(key){return this.attributes[key]??null;}
    removeAttribute(key){delete this.attributes[key];}
    contains(node){return node===this;}
    matches(){return this.popoverOpen??false;}
    getBoundingClientRect(){return this.rect;}
  }
  const document=new Element(),window=new Element(),dialog=new Element({left:20,top:20,right:380,bottom:600,width:360,height:580});
  window.innerWidth=400;window.innerHeight=640;
  let nextTimer=0;const timers=new Map();
  window.setTimeout=callback=>{timers.set(++nextTimer,callback);return nextTimer;};window.clearTimeout=id=>timers.delete(id);
  document.defaultView=window;dialog.ownerDocument=document;dialog.open=true;
  const buttons=[new Element(),new Element()],tooltips=buttons.map(()=>new Element({width:300,height:100}));
  buttons.forEach((button,index)=>{button.setAttribute('aria-controls','help-'+index);button.setAttribute('aria-expanded','false');});
  if(popover)for(const tooltip of tooltips){tooltip.showPopover=()=>{tooltip.popoverOpen=true;};tooltip.hidePopover=()=>{tooltip.popoverOpen=false;};}
  document.getElementById=id=>tooltips[Number(id.slice(-1))];dialog.querySelectorAll=()=>buttons;
  document.activeElement=null;
  attachFieldHelp(dialog);
  const focus=button=>{document.activeElement=button;button.emit('focus');};
  const blur=button=>{document.activeElement=null;button.emit('blur',{relatedTarget:null});};
  const tick=()=>{for(const [id,fn]of timers){timers.delete(id);fn();}};
  return {document,window,dialog,buttons,tooltips,focus,blur,tick};
}

test('hover and keyboard focus reveal help without moving focus, and pointer travel keeps it readable',()=>{
  const {document,buttons:[button],tooltips:[tooltip],focus,blur,tick}=fixture();
  const field={value:'frang'};document.activeElement=field;
  button.emit('pointerenter',{pointerType:'mouse'});
  assert.equal(tooltip.hidden,false);assert.equal(document.activeElement,field);
  assert.equal(button.getAttribute('aria-expanded'),'true');
  button.emit('pointerleave');tooltip.emit('pointerenter');tick();assert.equal(tooltip.hidden,false);
  tooltip.emit('pointerleave');tick();assert.equal(tooltip.hidden,true);
  focus(button);button.emit('pointerleave');tick();assert.equal(tooltip.hidden,false);
  blur(button);assert.equal(tooltip.hidden,true);assert.equal(field.value,'frang');
});

test('tap pins one tooltip, a second tap closes it, and tapping elsewhere dismisses help',()=>{
  const {document,buttons:[first,second],tooltips:[one,two],focus,tick}=fixture();
  first.emit('pointerenter',{pointerType:'touch'});assert.equal(one.hidden,true);
  focus(first);first.emit('click');first.emit('pointerleave');tick();assert.equal(one.hidden,false);
  first.emit('click');assert.equal(one.hidden,true);
  first.emit('click');assert.equal(one.hidden,false);
  document.emit('pointerdown',{target:second});focus(second);second.emit('click');
  assert.equal(one.hidden,true);assert.equal(two.hidden,false);
  document.emit('pointerdown',{target:two});assert.equal(two.hidden,false);
  document.emit('pointerdown',{target:{value:'unchanged'}});assert.equal(two.hidden,true);
});

test('Escape dismisses help before the dialog, and dialog close or form reset clears pinned help',()=>{
  const {document,dialog,buttons:[button],tooltips:[tooltip]}=fixture();
  button.emit('click');const escape=document.emit('keydown',{key:'Escape'});
  assert.equal(escape.prevented,true);assert.equal(escape.stopped,true);
  assert.equal(tooltip.hidden,true);assert.equal(dialog.open,true);
  assert.equal(document.emit('keydown',{key:'Escape'}).prevented,undefined);
  button.emit('click');dialog.emit('close');assert.equal(tooltip.hidden,true);assert.equal(tooltip.popoverOpen,false);
  button.emit('click');dialog.emit('reset');assert.equal(tooltip.hidden,true);assert.equal(button.getAttribute('aria-expanded'),'false');
});

test('help fits inside the visible dialog, follows scrolling, and works without the Popover API',()=>{
  const {window,buttons:[button],tooltips:[tooltip]}=fixture({popover:false});
  button.rect={left:330,top:550,right:358,bottom:578,width:28,height:28};
  button.emit('click');assert.equal(tooltip.hidden,false);
  assert.equal(tooltip.style.left,'72px');assert.equal(tooltip.style.top,'442px');
  button.rect={left:60,top:100,right:88,bottom:128,width:28,height:28};
  window.emit('scroll');assert.equal(tooltip.style.top,'136px');
  button.rect={left:60,top:-50,right:88,bottom:-22,width:28,height:28};
  window.emit('scroll');assert.equal(tooltip.hidden,true);
});
