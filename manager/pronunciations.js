import {t} from './i18n.js';

const spellings=value=>Array.isArray(value)?value:[value];
const formSpellings=value=>[...new Set(value.split(/[,，\n]+/).map(text=>text.trim()).filter(Boolean))];
const sameReadAs=(left,right)=>JSON.stringify(spellings(left))===JSON.stringify(spellings(right));

const actionIcons={
  Edit:'m16 3 5 5-13 13H3v-5L16 3ZM14 5l5 5',
  Remove:'M3 6h18M9 6V3h6v3M5 6l1 15h12l1-15M10 10v7m4-7v7',
  Reset:'M3 4v6h6M3.5 10a8.5 8.5 0 1 1 .7 7',
};

// A separate editor; existing grouped rule views stay read-only.
export function pronunciationSection({document,preview,onSave}){
  const node=(tag,text,cls)=>{const n=document.createElement(tag);if(text)n.textContent=text;if(cls)n.className=cls;return n;};
  const root=node('div');
  const hint=node('p',t('Separate multiple Read-As spellings with commas.'),'helper');
  const list=node('div',undefined,'pronunciation-list'),form=node('form',undefined,'pronunciation-form');
  const field=(label,placeholder)=>{const wrap=node('label'),caption=node('span',t(label)),input=node('input');input.lang='km';input.required=true;input.maxLength=120;input.placeholder=placeholder;wrap.append(caption,input);form.append(wrap);return {input,caption};};
  const word=field('Word','មិត្ត'),alternate=field('Read-As','មិត, មិត្ត');
  alternate.input.maxLength=121000;
  const output=node('output',undefined,'pronunciation-preview');output.setAttribute('aria-live','polite');
  const save=node('button',t('Add rule'),'primary');save.type='submit';
  const cancel=node('button',t('Cancel'),'quiet');cancel.type='button';cancel.hidden=true;
  const actions=node('div',undefined,'actions');actions.append(save,cancel);
  const status=node('p',undefined,'helper');status.setAttribute('role','status');
  form.append(output,actions);root.append(hint,list,form,status);
  let rules=[],overrides=[],defaults=[],query='',editing=null,busy=false,ready=false;
  function rowAction(label,rule,action){
    const button=node('button',undefined,'pronunciation-action');
    button.type='button';
    button.disabled=busy||!ready;
    button.title=t(`${label} {word}`,{word:rule.word});
    button.setAttribute('aria-label',button.title);
    const svgNamespace='http://www.w3.org/2000/svg';
    const icon=document.createElementNS(svgNamespace,'svg');
    const path=document.createElementNS(svgNamespace,'path');
    icon.setAttribute('viewBox','0 0 24 24');
    icon.setAttribute('aria-hidden','true');
    icon.setAttribute('focusable','false');
    path.setAttribute('d',actionIcons[label]);
    icon.append(path);
    button.append(icon);
    button.onclick=action;
    return button;
  }
  function reset(){form.reset();editing=null;cancel.hidden=true;output.textContent='';status.textContent='';save.textContent=t('Add rule');}
  function showPreview(){
    output.textContent='';save.disabled=busy||!ready;
    if(!alternate.input.value.trim())return;
    try{
      const values=formSpellings(alternate.input.value);
      output.textContent=preview(values.length===1?values[0]:values).join(' / ');
    }catch(error){output.textContent=error.message??String(error);save.disabled=true;}
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
    const values=formSpellings(alternate.input.value);
    const rule={word:word.input.value.trim(),readAs:values.length===1?values[0]:values};
    try{preview(rule.readAs);}catch(error){status.textContent=error.message??String(error);return;}
    if (editing && !rules.some(item => item.word === editing.word && sameReadAs(item.readAs,editing.readAs))) {
      status.textContent = t('The dictionary changed. Refresh it and reopen the word before saving again.');
      return;
    }
    if(rules.some(item=>item.word===rule.word&&item.word!==editing?.word)){status.textContent=t('This word already has a pronunciation rule. Edit it below.');return;}
    const next=overrides.filter(item=>item.word!==editing?.word);next.push(rule);return persist(next);
  };
  function render(){
    hint.textContent=t('Separate multiple Read-As spellings with commas.');
    word.caption.textContent=t('Word');alternate.caption.textContent=t('Read-As');cancel.textContent=t('Cancel');
    word.input.disabled=alternate.input.disabled=cancel.disabled=busy||!ready;
    save.textContent=t(editing?'Save rule':'Add rule');list.replaceChildren();
    let matches=0;
    for(const rule of rules){
      let inputs=[];try{inputs=preview(rule.readAs);}catch{}
      if(query&&![rule.word,...spellings(rule.readAs),...inputs].join(' ').toLowerCase().includes(query))continue;
      matches++;
      const row=node('div',undefined,'pronunciation-row'),text=node('div'),km=node('span',`${rule.word} → ${spellings(rule.readAs).join(' / ')}`);km.lang='km';
      const latin=node('small',inputs.join(' / '),'muted');latin.lang='en';text.append(km,latin);row.append(text);
      const edit=rowAction('Edit',rule,()=>{
        editing={word:rule.word,readAs:[...spellings(rule.readAs)]};
        word.input.value=rule.word;
        alternate.input.value=spellings(rule.readAs).join(', ');
        cancel.hidden=false;
        status.textContent='';
        render();
        word.input.focus();
      });
      const removeLabel=defaults.some(item=>item.word===rule.word)?'Reset':'Remove';
      const remove=rowAction(removeLabel,rule,()=>persist(overrides.filter(item=>item.word!==rule.word)));
      row.append(edit,remove);
      list.append(row);
    }
    if(!matches&&ready)list.append(node('p',t(rules.length?'No matching pronunciations.':'No pronunciation rules yet.'),'helper'));
    showPreview();
  }
  return {root,update(value,search='',available=true,stock=[]){overrides=value;defaults=[...new Map(stock.map(rule=>[rule.word,rule])).values()];rules=[...defaults.filter(rule=>!value.some(item=>item.word===rule.word)),...value];query=search;ready=available;render();}};
}
