import test from 'node:test';
import assert from 'node:assert/strict';
import {suggestionGesture} from '../suggestion-gesture.js';

function gesture(){
  let now=0,id=0,inserts=0,managed=0;
  const jobs=new Map(),listeners=new Map();
  const button={isConnected:true,addEventListener(type,handler){listeners.set(type,handler);}};
  const dispose=suggestionGesture(button,{insert:()=>inserts++,manage:()=>managed++,
    schedule(fn,delay){const key=++id;jobs.set(key,{fn,at:now+delay});return key;},cancel(key){jobs.delete(key);}});
  const emit=(type,props={})=>{
    const event={pointerId:1,button:0,isPrimary:true,clientX:20,clientY:20,prevented:false,preventDefault(){this.prevented=true;},...props};
    (type==='click'?button.onclick:listeners.get(type))?.(event);return event;
  };
  const tick=ms=>{now+=ms;for(const [key,job] of jobs)if(job.at<=now){jobs.delete(key);job.fn();}};
  return {emit,tick,button,dispose,counts:()=>({inserts,managed}),pending:()=>jobs.size};
}

test('a short tap inserts, a hold manages once, and its release never inserts',()=>{
  const page=gesture();
  assert.equal(page.emit('pointerdown').prevented,false);
  page.tick(499);page.emit('pointerup');page.emit('click');
  assert.deepEqual(page.counts(),{inserts:1,managed:0});
  page.emit('pointerdown');page.tick(500);
  assert.deepEqual(page.counts(),{inserts:1,managed:1});
  page.tick(1000);assert.equal(page.emit('contextmenu').prevented,true);
  page.emit('pointercancel');page.emit('pointerup');assert.equal(page.emit('click').prevented,true);
  assert.deepEqual(page.counts(),{inserts:1,managed:1});
  page.emit('pointerdown');page.emit('pointerup');page.emit('click');
  assert.deepEqual(page.counts(),{inserts:2,managed:1});
});

test('scrolling, cancellation and leaving cancel holds without accepting a suggestion',()=>{
  for(const [type,props] of [['pointermove',{clientX:45}],['pointermove',{clientY:45}],['pointercancel',{}],['pointerleave',{}],['lostpointercapture',{}],['pointerdown',{isPrimary:false,pointerId:2}]]){
    const page=gesture();page.emit('pointerdown');page.tick(200);page.emit(type,props);page.tick(500);
    page.emit('pointerup');page.emit('click');
    assert.deepEqual(page.counts(),{inserts:0,managed:0},type);assert.equal(page.pending(),0);
  }
});

test('small finger movement keeps the hold, while unrelated pointers cannot release it',()=>{
  const page=gesture();page.emit('pointerdown');page.emit('pointermove',{clientX:23,clientY:24});
  page.emit('pointerup',{pointerId:2});page.tick(500);
  assert.deepEqual(page.counts(),{inserts:0,managed:1});
});

test('right click and keyboard context menus open management without insertion',()=>{
  const page=gesture();page.emit('pointerdown',{button:2});page.emit('contextmenu');
  page.emit('pointerup',{button:2});page.emit('click');
  assert.deepEqual(page.counts(),{inserts:0,managed:1});
  assert.equal(page.emit('keydown',{key:'F10',shiftKey:true}).prevented,true);
  page.emit('contextmenu');assert.deepEqual(page.counts(),{inserts:0,managed:2});
  page.emit('keydown',{key:'Enter'});page.emit('click');
  assert.deepEqual(page.counts(),{inserts:1,managed:2});
  page.emit('keydown',{key:'ContextMenu'});assert.deepEqual(page.counts(),{inserts:1,managed:3});
});

test('repainting or disconnecting a suggestion prevents a delayed dialog',()=>{
  for(const action of ['dispose','disconnect']){
    const page=gesture();page.emit('pointerdown');
    if(action==='dispose')page.dispose();else page.button.isConnected=false;
    page.tick(500);assert.deepEqual(page.counts(),{inserts:0,managed:0});assert.equal(page.pending(),0);
  }
});
