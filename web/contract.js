(function(root){
 'use strict';
 const string=(max=200,extra={})=>({type:'string',maxLength:max,...extra});
 const array=(items,maxItems)=>({type:'array',items,maxItems});
 // `optional`: fields an older file (version 2) does not have; `upgrade` fills them in.
 const object=(properties,optional=[])=>({type:'object',additionalProperties:false,required:Object.keys(properties).filter(key=>!optional.includes(key)),properties});
 const bool={type:'boolean'},month=string(7,{pattern:'^$|^[12][0-9]{3}(-(0[1-9]|1[0-2]))?$'}),year=string(4,{pattern:'^$|^[12][0-9]{3}$'});
 const line=string(500),tags=array(string(60),12),id=string(60,{pattern:'^[a-zA-Z0-9_-]+$'});
 const schema={
  $schema:'https://json-schema.org/draft/2020-12/schema',title:'CVBuilder candidate intake version 3',
  ...object({
   version:{enum:[2,3]},language:{enum:['no','en']},
   candidate:object({name:string(),email:string(),phone:string(),city:string(),headline:string(),linkedin:string(200),summary:string(1500),photo:string(750000,{pattern:'^$|^data:image/jpeg;base64,[A-Za-z0-9+/=]+$'})}),
   target:object({role:string(),company:string(),url:string(1000),description:string(15000),keywords:array(string(80),30)}),
   experience:array(object({id,company:string(),title:string(),location:string(100),start:month,end:{anyOf:[month,{type:'null'}]},bullets:array(line,5),bullet_ids:{...array(id,5),uniqueItems:true},evidence:string(1000),confirmed:bool},['bullet_ids']),12),
   education:array(object({id,qualification:string(),institution:string(),start:year,end:year},['id']),8),
   skills:array(object({id,group:string(60),name:string(),evidence:string(500),confirmed:bool},['id']),30),
   languages:array(object({id,name:string(80),level:string(100)},['id']),8),
   certifications:array(object({id,name:string(),issuer:string(),year:year},['id']),10),
   interests:array(object({id,label:string(120),status:{enum:['current','former','interest']},description:string(400),category:{enum:['technology','outdoors','sport','creative','community','other']},tags,confirmed:bool,specificity:{type:'integer',minimum:0,maximum:2}}),20),
   public_context:array(object({topic:string(500),tags,source_url:string(1000),checked_on:string(10,{pattern:'^$|^[12][0-9]{3}-[0-9]{2}-[0-9]{2}$'}),scope:{enum:['employer_activity','professional_bio']},verified:bool}),8),
   options:object({layout:{enum:['classic','modern','nordic','compact']},photo:bool,include_interests:bool,interest_ids:{...array(id,3),uniqueItems:true},page_limit:{const:2},hidden:{...array(id,400),uniqueItems:true},copy_of:string(260)},['hidden','copy_of']),
   approvals:object({facts_confirmed:bool})
  })
 };
 const blank=()=>({version:3,language:'no',candidate:{name:'',email:'',phone:'',city:'',headline:'',linkedin:'',summary:'',photo:''},target:{role:'',company:'',url:'',description:'',keywords:[]},experience:[],education:[],skills:[],languages:[],certifications:[],interests:[],public_context:[],options:{layout:'classic',photo:false,include_interests:true,interest_ids:[],page_limit:2,hidden:[],copy_of:''},approvals:{facts_confirmed:false}});
 function validate(value,spec=schema,path='$',errors=[]){
  const fail=message=>errors.push(path+': '+message);
  if(spec.anyOf){if(!spec.anyOf.some(branch=>validate(value,branch,path,[]).length===0))fail('ugyldig verdi');return errors;}
  if(Object.hasOwn(spec,'const')&&value!==spec.const)fail('feil fast verdi');
  if(spec.enum&&!spec.enum.includes(value))fail('verdi utenfor tillatte valg');
  if(spec.type==='object'){
   if(!value||typeof value!=='object'||Array.isArray(value)){fail('må være et objekt');return errors;}
   for(const key of spec.required||[])if(!Object.hasOwn(value,key))errors.push(path+'.'+key+': mangler');
   for(const key of Object.keys(value)){if(!Object.hasOwn(spec.properties,key))errors.push(path+'.'+key+': ukjent felt');else validate(value[key],spec.properties[key],path+'.'+key,errors);}
  }else if(spec.type==='array'){
   if(!Array.isArray(value)){fail('må være en liste');return errors;}if(value.length>spec.maxItems){fail('for mange elementer');return errors;}if(spec.uniqueItems&&new Set(value.map(item=>JSON.stringify(item))).size!==value.length)fail('dupliserte valg');value.forEach((item,index)=>validate(item,spec.items,`${path}[${index}]`,errors));
  }else if(spec.type==='string'){
   if(typeof value!=='string'){fail('må være tekst');return errors;}if(value.length>spec.maxLength)fail('teksten er for lang');if(spec.pattern&&!new RegExp(spec.pattern).test(value))fail('feil format');
  }else if(spec.type==='null'&&value!==null)fail('må være null');
  else if(spec.type==='boolean'&&typeof value!=='boolean')fail('må være sann/usann');
  else if(spec.type==='integer'&&(!Number.isInteger(value)||value<spec.minimum||value>spec.maximum))fail('ugyldig heltall');
  return errors;
 }
 const MONTHS={jan:1,januar:1,january:1,feb:2,februar:2,february:2,mar:3,mars:3,march:3,apr:4,april:4,mai:5,may:5,jun:6,juni:6,june:6,jul:7,juli:7,july:7,aug:8,august:8,sep:9,sept:9,september:9,okt:10,oktober:10,oct:10,october:10,nov:11,november:11,des:12,desember:12,dec:12,december:12};
 // Reads the ways people write a month: 09.2021, 9/2021, 2021-09, «sep. 2021», «mars 2019». Returns YYYY-MM or null.
 function parseMonth(input){const text=String(input||'').trim().toLowerCase();if(!text)return '';if(/^(19|20)\d{2}$/.test(text))return text;let m=text.match(/^(\d{4})\s*[-./]\s*(\d{1,2})$/);if(m)return pad(m[1],m[2]);m=text.match(/^(\d{1,2})\s*[-./ ]\s*(\d{4})$/);if(m)return pad(m[2],m[1]);m=text.match(/^([a-zæøå]+)\.?\s*(\d{4})$/);if(m&&MONTHS[m[1]])return pad(m[2],MONTHS[m[1]]);return null;}
 function pad(year,month){month=Number(month);year=Number(year);if(month<1||month>12||year<1900||year>2100)return null;return year+'-'+String(month).padStart(2,'0');}
 // Reads a year from «2016», «aug. 2016» or «2016-08». Returns YYYY or null.
 function parseYear(input){const text=String(input||'').trim();if(!text)return '';const m=text.match(/(?:^|\D)((?:19|20)\d{2})(?:\D|$)/);return m?m[1]:null;}
 function showMonth(value){if(!value)return '';const [year,month]=value.split('-');return month?month+'.'+year:year;}
 // «nå», «now», «d.d.», «present» typed as an end date mean a current job.
 const isNow=input=>/^(nå|naa|now|present|d\.?\s?d\.?|dags dato|i dag|today|pågående|ongoing)$/i.test(String(input||'').trim());
 const FIELD_NAMES={name:'Fullt navn',email:'E-post',phone:'Telefon',city:'By og land',headline:'Faglig overskrift',linkedin:'LinkedIn',summary:'Profil',role:'Stilling du søker',url:'Annonselenke',start:'Fra',end:'Til',year:'År',company:'Arbeidsgiver',title:'Stillingstittel',qualification:'Utdanning',institution:'Lærested'};
 const LIST_NAMES={experience:'Jobb',education:'Utdanning',skills:'Kompetanse',certifications:'Kurs',languages:'Språk',interests:'Interesse',public_context:'Kilde'};
 // Turns «$.education[0].start: feil format» into «Utdanning 1, Fra: feil format».
 function friendly(error){const m=String(error).match(/^\$\.(?:([a-z_]+)\[(\d+)\]\.)?(?:candidate\.|target\.)?([a-z_]+)(?:\[\d+\])?: (.*)$/);if(!m)return error;const [,list,index,key,problem]=m;const where=list?`${LIST_NAMES[list]||list} ${Number(index)+1}, ${FIELD_NAMES[key]||key}`:(FIELD_NAMES[key]||key);return where+': '+problem;}
 // Clears or shortens fields that break the strict format, so a saved draft is never thrown away.
 function repair(input){const value=structuredClone(input),fixed=[];for(const error of validate(value)){const m=error.match(/^\$((?:\.[a-z_]+|\[\d+\])+): (feil format|teksten er for lang)/);if(!m)continue;const keys=[...m[1].matchAll(/\.([a-z_]+)|\[(\d+)\]/g)].map(part=>part[1]??Number(part[2]));let parent=value;for(const key of keys.slice(0,-1))parent=parent?.[key];const last=keys.at(-1);if(parent&&typeof parent[last]==='string'){parent[last]=m[2]==='feil format'?'':parent[last].slice(0,180);fixed.push(friendly(error));}}return {value,fixed};}
 // Version 3 gave every item a stable id (bullets: `bullet_ids`, at the same position) and added
 // `options.hidden`, the ids left out of this CV. The rules are the Rust app's (crates/cv-core/src/ids.rs):
 // ids from an older file are `{prefix}_{n}`, the same every time; ids for new items are never reused.
 const PREFIX={job:'job',bullet:'b',education:'edu',skill:'skill',language:'lang',certification:'cert',interest:'interest',section:'section',item:'item',contact:'contact'};
 let sequence=0;
 const base36=n=>Math.max(0,Math.floor(n)).toString(36);
 function freshId(prefix){return prefix+'_'+base36(Date.now())+base36(sequence++);}
 function allIds(value){const out=[];const push=id=>{if(typeof id==='string'&&id)out.push(id);};
  for(const x of value.candidate?.extra||[])push(x?.id);
  for(const j of value.experience||[]){push(j?.id);(j?.bullet_ids||[]).forEach(push);}
  for(const list of ['education','skills','languages','certifications','interests'])for(const x of value[list]||[])push(x?.id);
  for(const s of value.custom_sections||[]){push(s?.id);for(const i of s?.items||[]){push(i?.id);(i?.bullet_ids||[]).forEach(push);}}
  return out;}
 // Every item gets an id, unique in the project; good ids never change.
 function assignIds(value,deterministic=false){
  const fit=item=>{if(!Array.isArray(item.bullets))return;const ids=Array.isArray(item.bullet_ids)?item.bullet_ids.slice(0,item.bullets.length):[];while(ids.length<item.bullets.length)ids.push('');item.bullet_ids=ids.map(id=>typeof id==='string'?id:'');};
  for(const j of value.experience||[])if(j&&typeof j==='object')fit(j);
  for(const s of value.custom_sections||[])for(const i of s?.items||[])if(i&&typeof i==='object')fit(i);
  const taken=new Set(allIds(value)),next={},seen=new Set();
  const newId=prefix=>{for(;;){let id;if(deterministic){next[prefix]=next[prefix]||1;id=prefix+'_'+next[prefix]++;}else id=freshId(prefix);if(!taken.has(id)){taken.add(id);return id;}}};
  const fix=(get,set,prefix,rename)=>{let id=get();if(typeof id!=='string'||!id||(rename&&seen.has(id))){id=newId(prefix);set(id);}seen.add(id);};
  const record=(item,prefix,rename)=>{if(item&&typeof item==='object')fix(()=>item.id,id=>item.id=id,prefix,rename);};
  const bullets=item=>{if(Array.isArray(item?.bullet_ids))item.bullet_ids.forEach((_,k)=>fix(()=>item.bullet_ids[k],id=>item.bullet_ids[k]=id,PREFIX.bullet,true));};
  for(const x of value.candidate?.extra||[])record(x,PREFIX.contact,false);
  for(const j of value.experience||[]){record(j,PREFIX.job,true);bullets(j);}
  for(const [list,prefix] of [['education',PREFIX.education],['skills',PREFIX.skill],['languages',PREFIX.language],['certifications',PREFIX.certification]])for(const x of value[list]||[])record(x,prefix,true);
  for(const x of value.interests||[])record(x,PREFIX.interest,false);
  for(const s of value.custom_sections||[]){record(s,PREFIX.section,false);for(const i of s?.items||[]){record(i,PREFIX.item,true);bullets(i);}}
  if(value.options&&typeof value.options==='object'){const kept=new Set();value.options.hidden=(Array.isArray(value.options.hidden)?value.options.hidden:[]).filter(id=>seen.has(id)&&!kept.has(id)&&kept.add(id));}
  return value;
 }
 // The ids for `next` lines, given the `previous` lines and their ids: an unchanged line keeps its id
 // (also when moved), a changed line takes the id of the line it replaced, new lines get new ids.
 function carryIds(previous,ids,next){
  const key=text=>String(text).trim(),used=previous.map(()=>false),out=next.map(()=>null);
  next.forEach((text,n)=>{const o=previous.findIndex((old,k)=>!used[k]&&ids[k]&&key(old)===key(text));if(o>=0){used[o]=true;out[n]=ids[o];}});
  const freeOld=previous.map((_,k)=>k).filter(k=>!used[k]&&ids[k]),freeNew=next.map((_,n)=>n).filter(n=>out[n]===null);
  freeNew.forEach((n,k)=>{if(k<freeOld.length)out[n]=ids[freeOld[k]];});
  return out.map(id=>id||freshId(PREFIX.bullet));
 }
 const isHidden=(data,id)=>!!id&&(data.options?.hidden||[]).includes(id);
 // The CV as it is shown, printed and checked: without the items hidden in this copy.
 function visible(data){
  const hidden=new Set(data.options?.hidden||[]);if(!hidden.size)return data;
  const shown=item=>!hidden.has(item.id),value=structuredClone(data);
  const keep=item=>{const ids=item.bullet_ids||[];item.bullets=item.bullets.filter((_,k)=>!ids[k]||!hidden.has(ids[k]));item.bullet_ids=ids.filter(id=>!hidden.has(id));};
  value.experience=value.experience.filter(shown);value.experience.forEach(keep);
  for(const list of ['education','skills','languages','certifications'])value[list]=value[list].filter(shown);
  if(Array.isArray(value.custom_sections)){value.custom_sections=value.custom_sections.filter(shown);for(const s of value.custom_sections){s.items=s.items.filter(shown);s.items.forEach(keep);}}
  value.options.hidden=[];
  return value;
 }
 function upgrade(input){
  if(!input||typeof input!=='object'||![1,2,3].includes(input.version))return input;
  const value=structuredClone(input);value.version=3;
  if(value.options&&typeof value.options==='object'&&!Object.hasOwn(value.options,'layout'))value.options.layout='classic';
  if(value.candidate&&typeof value.candidate==='object'&&!Object.hasOwn(value.candidate,'linkedin'))value.candidate.linkedin='';
  if(Array.isArray(value.experience))value.experience.forEach(job=>{if(job&&typeof job==='object'&&!Object.hasOwn(job,'location'))job.location='';});
  if(Array.isArray(value.skills))value.skills.forEach(skill=>{if(skill&&typeof skill==='object'&&!Object.hasOwn(skill,'group'))skill.group='';});
  if(!Object.hasOwn(value,'certifications'))value.certifications=[];
  if(value.options&&typeof value.options==='object'){if(!Object.hasOwn(value.options,'hidden'))value.options.hidden=[];if(!Object.hasOwn(value.options,'copy_of'))value.options.copy_of='';}
  return assignIds(value,true);
 }
 const https=value=>!value||/^https:\/\/[^\s]+$/.test(value);
 const placeholder=value=>/\[[^\]\n]{1,30}\]|<[^>]+>|\b(?:lorem ipsum|todo|tbd)\b/i.test(value);
 function ready(full){
  const errors=validate(full);if(errors.length)return errors.map(friendly);
  // Hidden items are not in the CV, so they cannot hold up the download; items keep their numbers.
  const data=visible(full),shown=item=>!isHidden(full,item.id);
  for(const key of ['name','email','phone','city','headline'])if(!data.candidate[key].trim())errors.push(FIELD_NAMES[key]+' må fylles ut.');
  if(data.candidate.email&&!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(data.candidate.email))errors.push('E-postadressen ser ikke riktig ut.');
  if(data.candidate.phone&&(!/^\+?[0-9 ()-]{6,30}$/.test(data.candidate.phone)||data.candidate.phone.replace(/\D/g,'').length<6))errors.push('Telefonnummeret ser ikke riktig ut. Skriv for eksempel +47 412 34 567.');
  if(!data.target.role.trim())errors.push('Skriv hvilken stilling du søker (under «Stillingen»).');
  if(!https(data.target.url))errors.push('Annonselenken må starte med https://.');
  if(!https(data.candidate.linkedin))errors.push('LinkedIn må være full adresse, for eksempel https://www.linkedin.com/in/navn.');
  for(const item of data.certifications)if(!item.name.trim())errors.push('Et kurs mangler navn.');
  if(data.candidate.summary.trim().split(/\s+/).filter(Boolean).length>90)errors.push('Profilen er for lang. Kort den ned til høyst 90 ord, helst 30–70.');
  if(!data.experience.length&&!data.education.length)errors.push('Legg til minst én jobb eller utdanning.');
  const ids=[];
  full.experience.forEach((job,index)=>{
   ids.push(job.id);if(!shown(job))return;const name=`Jobb ${index+1}${job.company.trim()?' ('+job.company.trim()+')':''}`;
   if(!job.company.trim()||!job.title.trim())errors.push(name+': fyll inn arbeidsgiver og stillingstittel.');
   if(!job.start)errors.push(name+': fyll inn når du startet (for eksempel 09.2021).');
   if(job.end==='')errors.push(name+': fyll inn når du sluttet, eller kryss av for «Nåværende jobb».');
   if(job.end&&job.start&&job.end<job.start)errors.push(name+': sluttdatoen er før startdatoen.');
   if(!job.bullets.length)errors.push(name+': skriv minst ett punkt om hva du gjorde.');
  });
  full.education.forEach((item,index)=>{if(!shown(item))return;const name=`Utdanning ${index+1}`;if(!item.qualification.trim()||!item.institution.trim())errors.push(name+': fyll inn utdanning og lærested.');if(!item.end)errors.push(name+': fyll inn sluttår (forventet år hvis du fortsatt studerer).');if(item.start&&item.end&&item.end<item.start)errors.push(name+': sluttåret er før startåret.');});
  full.skills.forEach((item,index)=>{if(!shown(item))return;if(!item.name.trim())errors.push(`Kompetanse ${index+1} mangler navn.`);});
  for(const item of data.languages)if(!item.name.trim()||!item.level.trim())errors.push('Et språk mangler navn eller nivå.');
  for(const item of data.interests){ids.push(item.id);if(!item.label.trim())errors.push('En interesse mangler navn.');}
  if(new Set(ids).size!==ids.length)errors.push('id: dupliserte identifikatorer');
  for(const context of data.public_context)if(!https(context.source_url)||!context.source_url||!context.topic.trim()||!context.checked_on||!Number.isFinite(Date.parse(context.checked_on)))errors.push('public_context: kilde, dato og presis opplysning kreves');
  for(const id of data.options.interest_ids)if(!data.interests.some(item=>item.id===id&&item.confirmed))errors.push('interest_ids: valget er ukjent eller ikke bekreftet');
  if(data.options.photo&&!data.candidate.photo)errors.push('photo: legg til bilde eller slå av bildevalget');
  if(data.candidate.photo){try{const decoded=atob(data.candidate.photo.split(',')[1]);if(decoded.charCodeAt(0)!==255||decoded.charCodeAt(1)!==216||decoded.charCodeAt(decoded.length-2)!==255||decoded.charCodeAt(decoded.length-1)!==217)errors.push('photo: ugyldig JPEG');}catch{errors.push('photo: ugyldig bildeformat');}}
  const exported=[data.candidate.name,data.candidate.headline,data.candidate.summary,...data.experience.flatMap(item=>[item.company,item.title,...item.bullets]),...data.skills.flatMap(item=>[item.group,item.name]),...data.certifications.flatMap(item=>[item.name,item.issuer]),...data.education.flatMap(item=>[item.qualification,item.institution]),...data.interests.filter(item=>data.options.interest_ids.includes(item.id)).map(item=>item.label)];
  if(exported.some(placeholder))errors.push('Teksten inneholder plassholdere som [tall] eller [fyll inn], eller kode som <…>. Fyll inn ekte tall og tekst, eller fjern dem.');
  if(!data.approvals.facts_confirmed)errors.push('approvals: bekreft at opplysningene er riktige');
  return [...new Set(errors)];
 }
 const api={schema,blank,validate,ready,upgrade,repair,friendly,parseMonth,parseYear,showMonth,isNow,assignIds,carryIds,freshId,allIds,isHidden,visible};root.CVContract=api;if(typeof module!=='undefined'&&module.exports)module.exports=api;
})(typeof globalThis!=='undefined'?globalThis:this);
