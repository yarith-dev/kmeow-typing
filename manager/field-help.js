// Read-only field help: hover/focus to preview, tap to keep it open.
export function attachFieldHelp(dialog){
  const doc=dialog.ownerDocument,view=doc.defaultView;
  let active=null,pinned=false,timer;
  const clear=()=>{view.clearTimeout(timer);};
  function close(){
    clear();if(!active)return;
    const {button,tooltip}=active;
    if(tooltip.hidePopover&&tooltip.matches(':popover-open'))tooltip.hidePopover();
    tooltip.hidden=true;tooltip.removeAttribute('data-help-open');
    button.setAttribute('aria-expanded','false');active=null;pinned=false;
  }
  function position(){
    if(!active)return;
    const {button,tooltip}=active,anchor=button.getBoundingClientRect(),bounds=dialog.getBoundingClientRect();
    if(anchor.bottom<bounds.top||anchor.top>bounds.bottom){close();return;}
    const box=tooltip.getBoundingClientRect(),gap=8;
    const left=Math.max(gap,bounds.left+gap),right=Math.min(view.innerWidth-gap,bounds.right-gap);
    const bottom=Math.min(view.innerHeight-gap,bounds.bottom-gap);
    tooltip.style.left=`${Math.max(left,Math.min(anchor.left,right-box.width))}px`;
    tooltip.style.top=`${Math.max(gap,anchor.bottom+gap+box.height<=bottom?anchor.bottom+gap:anchor.top-gap-box.height)}px`;
  }
  function open(button,keep=false){
    clear();
    if(active?.button!==button){
      close();const tooltip=doc.getElementById(button.getAttribute('aria-controls'));
      active={button,tooltip};tooltip.hidden=false;tooltip.setAttribute('data-help-open','');
      if(tooltip.showPopover)tooltip.showPopover();
      button.setAttribute('aria-expanded','true');
    }
    pinned=pinned||keep;position();
  }
  function leave(){
    clear();if(!active||pinned||doc.activeElement===active.button)return;
    timer=view.setTimeout(close,120);
  }
  for(const button of dialog.querySelectorAll('.field-help')){
    const tooltip=doc.getElementById(button.getAttribute('aria-controls'));
    button.addEventListener('pointerenter',event=>{if(event.pointerType==='mouse'&&!pinned)open(button);});
    button.addEventListener('pointerleave',leave);
    button.addEventListener('focus',()=>open(button));
    button.addEventListener('blur',event=>{if(active?.button===button&&!tooltip.contains(event.relatedTarget))close();});
    button.addEventListener('click',()=>{if(active?.button===button&&pinned)close();else open(button,true);});
    tooltip.addEventListener('pointerenter',clear);tooltip.addEventListener('pointerleave',leave);
  }
  doc.addEventListener('pointerdown',event=>{
    if(active&&!active.button.contains(event.target)&&!active.tooltip.contains(event.target))close();
  },true);
  doc.addEventListener('keydown',event=>{
    if(event.key==='Escape'&&active){event.preventDefault();event.stopPropagation();close();}
  },true);
  dialog.addEventListener('close',close);dialog.addEventListener('reset',close);
  view.addEventListener('resize',position);view.addEventListener('scroll',position,true);
  return close;
}
