// Interface language only. Never translate editor contents or dictionary data.
import {messages,ruleGuides,familyGuides,fieldNames,posNames} from './khmer-ui.js';
export const UI_LANGUAGE_KEY='kmeow-manager-language-v1';
let locale='en';
export const getUiLanguage=()=>locale;
export function setUiLanguage(value){locale=value==='km'?'km':'en';return locale;}
export function readUiLanguage(storage,preferred='en'){
  try{const saved=storage.getItem(UI_LANGUAGE_KEY);if(saved==='en'||saved==='km')return saved;}catch{}
  return String(preferred).toLowerCase().startsWith('km')?'km':'en';
}
// Only explicitly marked UI parameters are translated; word data stays literal.
export const uiMessage=(key,values={})=>({type:'ui-message',key,values});
const isUiMessage=value=>value?.type==='ui-message';
export function t(key,values={}){
  if(isUiMessage(key)){values=key.values;key=key.key;}
  const source=String(key??''),template=locale==='km'?(messages[source]??source):source;
  return template.replace(/\{(\w+)\}/g,(match,name)=>Object.hasOwn(values,name)?isUiMessage(values[name])?t(values[name]):String(values[name]):match);
}
export function setUiText(node,key,values={}){
  node.setAttribute('data-i18n',key);node.setAttribute('data-i18n-values',JSON.stringify(values));
  node.textContent=t(key,values);
}
export function translatePage(doc){
  for(const attribute of ['', '-aria-label','-placeholder','-title']){
    for(const node of doc.querySelectorAll(`[data-i18n${attribute}]`)){
      const name=`data-i18n${attribute}`,target=attribute.slice(1);
      const key=node.getAttribute(name)||(target?node.getAttribute(target):node.textContent);
      node.setAttribute(name,key);
      const values=JSON.parse(node.getAttribute('data-i18n-values')||'{}');
      if(target)node.setAttribute(target,t(key,values));else node.textContent=t(key,values);
    }
  }
}
export function ruleGuide(id,title,description){return locale==='km'&&ruleGuides[id]?ruleGuides[id]:[title,description];}
export function familyGuide(family){
  const translated=locale==='km'&&familyGuides[family.id];
  return translated?{...family,title:translated[0],description:translated[1]}:family;
}
export function fieldLabel(key,fallback){return locale==='km'?(fieldNames[key]??ruleGuides[key]?.[0]??t(fallback)):fallback;}
export function posLabel(parts){return parts.map(code=>locale==='km'?(posNames[code]??code):code).join(', ');}
