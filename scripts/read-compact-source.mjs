import assert from 'node:assert/strict';
import {WasmEngine} from '../wasm/khmer_engine.js';

// Build/test utility for the current release format. Rust validates the input
// and recompiles the recovered source; never use a lossy browse/search export.
export function readCompactSource(core) {
  const bytes=Buffer.from(core.exportBinary());
  assert.equal(bytes.subarray(0,8).toString(),'KMEOWC04');
  let start=32;
  const sections=Array.from({length:5},(_,i)=>{
    const length=bytes.readUInt32LE(12+i*4), section=bytes.subarray(start,start+length);
    start+=length;return section;
  });
  function rows(data,kind) {
    const count=data.readUInt32LE(4), entries=16+Math.ceil(count/16)*12;
    let pos=entries,key=Buffer.alloc(0),id=0;
    function integer(){let n=0n,shift=0n,b;do{b=data[pos++];n|=BigInt(b&127)<<shift;shift+=7n;}while(b>=128);return n;}
    return Array.from({length:count},(_,i)=>{
      if(i%16===0){pos=entries+data.readUInt32LE(16+Math.floor(i/16)*12);key=Buffer.alloc(0);id=0;}
      const prefix=Number(integer()),length=Number(integer());
      key=Buffer.concat([key.subarray(0,prefix),data.subarray(pos,pos+length)]);pos+=length;
      const delta=integer();id+=Number((delta>>1n)^-(delta&1n));
      const rank=Number(integer()),target=integer(),source=kind===0?Number(integer()):0;
      return {key:key.toString('utf8'),id,rank,target,source};
    });
  }
  const pool=sections[3],count=pool.readUInt32LE(0),strings=4+(count+1)*4;
  const outputs=Array.from({length:count},(_,i)=>pool.subarray(strings+pool.readUInt32LE(4+i*4),strings+pool.readUInt32LE(8+i*4)).toString('utf8'));
  const romanized=rows(sections[0],0),khmer=rows(sections[1],1),english=rows(sections[2],2);
  const dataset={schemaVersion:bytes.readUInt32LE(8),
    mappings:romanized.filter(row=>!row.source).map(row=>({id:row.id,romanized:row.key,khmer:outputs[Number(row.target)],rank:row.rank,status:'approved'})),
    words:khmer.map(row=>({id:row.id,khmer:row.key,rank:row.rank,status:'approved'})),
    englishWords:english.map(row=>({id:row.id,text:[...row.key].map((char,i)=>row.target&(1n<<BigInt(i))?char.toUpperCase():char).join(''),rank:row.rank,status:'approved'})),
    soundMapDisabledWords:khmer.filter(row=>row.target!==0n).map(row=>row.key),structures:[]};
  const context=sections[4],ids=new Map(khmer.map(row=>[row.key,row.id]));
  if(context.length){
    const records=4+(context.readUInt32LE(0)+1)*4;
    function visit(node,path){
      const from=records+context.readUInt32LE(4+node*4),to=records+context.readUInt32LE(8+node*4),parent=context.readUInt32LE(from);
      if(parent!==0xffffffff)dataset.structures.push({wordId:ids.get(outputs[parent]),componentIds:path.map(index=>ids.get(outputs[index]))});
      for(let at=from+4;at<to;at+=8)visit(context.readUInt32LE(at+4),[...path,context.readUInt32LE(at)]);
    }
    visit(0,[]);
  }
  const rebuilt=new WasmEngine(JSON.stringify(dataset));
  try{assert.deepEqual(Buffer.from(rebuilt.exportBinary()),bytes,'Recovered source must recompile byte-for-byte before editing');}
  finally{rebuilt.free();}
  return dataset;
}
