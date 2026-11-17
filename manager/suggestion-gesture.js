// Keep native scrolling and short taps. Only a stationary hold opens management.
export function suggestionGesture(button,{insert,manage,schedule=setTimeout,cancel=clearTimeout}){
  let timer,pointer,origin,blocked=false,opened=false;
  function stop(){if(timer!==undefined)cancel(timer);timer=undefined;pointer=undefined;}
  function open(){
    stop();blocked=true;
    if(!opened&&button.isConnected){opened=true;manage();}
  }
  button.addEventListener('pointerdown',event=>{
    stop();
    if(event.isPrimary===false){blocked=true;return;}
    blocked=false;opened=false;
    if(event.button!==0)return;
    pointer=event.pointerId;origin={x:event.clientX,y:event.clientY};
    timer=schedule(()=>{if(pointer!==undefined)open();},500);
  });
  button.addEventListener('pointermove',event=>{
    if(event.pointerId===pointer&&Math.hypot(event.clientX-origin.x,event.clientY-origin.y)>10){blocked=true;stop();}
  });
  button.addEventListener('pointerup',event=>{if(event.pointerId===pointer)stop();});
  for(const type of ['pointercancel','pointerleave','lostpointercapture'])button.addEventListener(type,event=>{
    if(event.pointerId===pointer){blocked=true;stop();}
  });
  button.addEventListener('contextmenu',event=>{event.preventDefault();open();});
  button.addEventListener('keydown',event=>{
    if(event.key==='ContextMenu'||(event.key==='F10'&&event.shiftKey)){
      event.preventDefault();opened=false;open();
    }else if(event.key==='Enter'||event.key===' '){stop();blocked=false;opened=false;}
  });
  button.onclick=event=>{
    stop();
    if(blocked){event?.preventDefault();return;}
    insert();
  };
  // The caller disposes handlers' timers before replacing the suggestion list.
  return stop;
}
