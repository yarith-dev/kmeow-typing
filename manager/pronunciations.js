import {t} from './i18n.js';

// A separate editor; existing grouped rule views stay read-only.
export function pronunciationSection({document,preview,onSave}){
  const node=(tag,text,cls)=>{const n=document.createElement(tag);if(text)n.textContent=text;if(cls)n.className=cls;return n;};
  const root=node('div');
  const hint=node('p',t('Read the alternate spelling; keep the original word and saved spellings.'),'helper');
  const list=node('div',undefined,'pronunciation-list'),form=node('form',undefined,'pronunciation-form');
  const field=(label,placeholder)=>{const wrap=node('label'),caption=node('span',t(label)),input=node('input');input.lang='km';input.required=true;input.maxLength=120;input.placeholder=placeholder;wrap.append(caption,input);form.append(wrap);return {input,caption};};
  const word=field('Word','មិត្ត'),alternate=field('Read as','មិត');
  const output=node('output',undefined,'pronunciation-preview');output.setAttribute('aria-live','polite');
  const save=node('button',t('Add rule'),'primary');save.type='submit';
  const cancel=node('button',t('Cancel'),'quiet');cancel.type='button';cancel.hidden=true;
  const actions=node('div',undefined,'actions');actions.append(save,cancel);
  const status=node('p',undefined,'helper');status.setAttribute('role','status');
  form.append(output,actions);root.append(hint,list,form,status);
  let rules=[],overrides=[],defaults=[],query='',editing=null,busy=false,ready=false;
  function reset(){form.reset();editing=null;cancel.hidden=true;output.textContent='';status.textContent='';save.textContent=t('Add rule');}
  function showPreview(){
    output.textContent='';save.disabled=busy||!ready;
    if(!alternate.input.value.trim())return;
    try{output.textContent=preview(alternate.input.value.trim()).join(' / ');}catch(error){output.textContent=error.message??String(error);save.disabled=true;}
  }
  alternate.input.oninput=showPreview;
  cancel.onclick=reset;
  async function persist(next){
    busy=true;render();status.textContent=t('Saving…');
    try{await onSave(next);reset();status.textContent=t('Pronunciation saved.');}
    catch(error){status.textContent=error.message??String(error);}
    finally{busy=false;render();}
  }
  form.onsubmit=event=>{
    event.preventDefault();if(busy||!ready)return;
    const rule={word:word.input.value.trim(),readAs:alternate.input.value.trim()};
    try{preview(rule.readAs);}catch(error){status.textContent=error.message??String(error);return;}
    if(rules.some(item=>item.word===rule.word&&item.word!==editing)){status.textContent=t('This word already has a pronunciation rule. Edit it below.');return;}
    const next=overrides.filter(item=>item.word!==editing);next.push(rule);return persist(next);
  };
  function render(){
    hint.textContent=t('Read the alternate spelling; keep the original word and saved spellings.');
    word.caption.textContent=t('Word');alternate.caption.textContent=t('Read as');cancel.textContent=t('Cancel');
    word.input.disabled=alternate.input.disabled=cancel.disabled=busy||!ready;
    save.textContent=t(editing?'Save rule':'Add rule');list.replaceChildren();
    let matches=0;
    for(const rule of rules){
      let inputs=[];try{inputs=preview(rule.readAs);}catch{}
      if(query&&![rule.word,rule.readAs,...inputs].join(' ').toLowerCase().includes(query))continue;
      matches++;
      const row=node('div',undefined,'pronunciation-row'),text=node('div'),km=node('span',`${rule.word} → ${rule.readAs}`);km.lang='km';
      const latin=node('small',inputs.join(' / '),'muted');latin.lang='en';text.append(km,latin);row.append(text);
      for(const [label,action] of [['Edit',()=>{editing=rule.word;word.input.value=rule.word;alternate.input.value=rule.readAs;cancel.hidden=false;status.textContent='';render();word.input.focus();}],[defaults.some(item=>item.word===rule.word)?'Reset':'Remove',()=>persist(overrides.filter(item=>item.word!==rule.word))]]){
        const button=node('button',t(label),'quiet');button.type='button';button.disabled=busy||!ready;button.setAttribute('aria-label',`${t(label)} ${rule.word}`);button.onclick=action;row.append(button);
      }
      list.append(row);
    }
    if(!matches&&ready)list.append(node('p',t(rules.length?'No matching pronunciations.':'No pronunciation rules yet.'),'helper'));
    showPreview();
  }
  return {root,update(value,search='',available=true,stock=[]){overrides=value;defaults=stock;rules=[...stock.filter(rule=>!value.some(item=>item.word===rule.word)),...value];query=search;ready=available;render();}};
}
