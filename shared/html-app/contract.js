(function(root){
 'use strict';
 const string=(max=200,extra={})=>({type:'string',maxLength:max,...extra});
 const array=(items,maxItems)=>({type:'array',items,maxItems});
 const object=properties=>({type:'object',additionalProperties:false,required:Object.keys(properties),properties});
 const bool={type:'boolean'},month=string(7,{pattern:'^$|^[12][0-9]{3}(-(0[1-9]|1[0-2]))?$'}),year=string(4,{pattern:'^$|^[12][0-9]{3}$'});
 const line=string(500),tags=array(string(60),12),id=string(60,{pattern:'^[a-zA-Z0-9_-]+$'});
 const schema={
  $schema:'https://json-schema.org/draft/2020-12/schema',title:'CVBuilder candidate intake version 2',
  ...object({
   version:{const:2},language:{enum:['no','en']},
   candidate:object({name:string(),email:string(),phone:string(),city:string(),headline:string(),linkedin:string(200),summary:string(1500),photo:string(750000,{pattern:'^$|^data:image/jpeg;base64,[A-Za-z0-9+/=]+$'})}),
   target:object({role:string(),company:string(),url:string(1000),description:string(15000),keywords:array(string(80),30)}),
   experience:array(object({id,company:string(),title:string(),location:string(100),start:month,end:{anyOf:[month,{type:'null'}]},bullets:array(line,5),evidence:string(1000),confirmed:bool}),12),
   education:array(object({qualification:string(),institution:string(),start:year,end:year}),8),
   skills:array(object({group:string(60),name:string(),evidence:string(500),confirmed:bool}),30),
   languages:array(object({name:string(80),level:string(100)}),8),
   certifications:array(object({name:string(),issuer:string(),year:year}),10),
   interests:array(object({id,label:string(120),status:{enum:['current','former','interest']},description:string(400),category:{enum:['technology','outdoors','sport','creative','community','other']},tags,confirmed:bool,specificity:{type:'integer',minimum:0,maximum:2}}),20),
   public_context:array(object({topic:string(500),tags,source_url:string(1000),checked_on:string(10,{pattern:'^$|^[12][0-9]{3}-[0-9]{2}-[0-9]{2}$'}),scope:{enum:['employer_activity','professional_bio']},verified:bool}),8),
   options:object({layout:{enum:['classic','modern','nordic','compact']},photo:bool,include_interests:bool,interest_ids:{...array(id,3),uniqueItems:true},page_limit:{const:2}}),
   approvals:object({facts_confirmed:bool})
  })
 };
 const blank=()=>({version:2,language:'no',candidate:{name:'',email:'',phone:'',city:'',headline:'',linkedin:'',summary:'',photo:''},target:{role:'',company:'',url:'',description:'',keywords:[]},experience:[],education:[],skills:[],languages:[],certifications:[],interests:[],public_context:[],options:{layout:'classic',photo:false,include_interests:true,interest_ids:[],page_limit:2},approvals:{facts_confirmed:false}});
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
 function upgrade(input){
  if(!input||typeof input!=='object'||![1,2].includes(input.version))return input;
  const value=structuredClone(input);value.version=2;
  if(value.options&&typeof value.options==='object'&&!Object.hasOwn(value.options,'layout'))value.options.layout='classic';
  if(value.candidate&&typeof value.candidate==='object'&&!Object.hasOwn(value.candidate,'linkedin'))value.candidate.linkedin='';
  if(Array.isArray(value.experience))value.experience.forEach(job=>{if(job&&typeof job==='object'&&!Object.hasOwn(job,'location'))job.location='';});
  if(Array.isArray(value.skills))value.skills.forEach(skill=>{if(skill&&typeof skill==='object'&&!Object.hasOwn(skill,'group'))skill.group='';});
  if(!Object.hasOwn(value,'certifications'))value.certifications=[];
  return value;
 }
 const https=value=>!value||/^https:\/\/[^\s]+$/.test(value);
 const placeholder=value=>/\[(?:fyll|insert|navn|name|tall|antall|number)|<[^>]+>|\b(?:lorem ipsum|todo|tbd)\b/i.test(value);
 function ready(data){
  const errors=validate(data);if(errors.length)return errors.map(friendly);
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
  data.experience.forEach((job,index)=>{
   ids.push(job.id);const name=`Jobb ${index+1}${job.company.trim()?' ('+job.company.trim()+')':''}`;
   if(!job.company.trim()||!job.title.trim())errors.push(name+': fyll inn arbeidsgiver og stillingstittel.');
   if(!job.start)errors.push(name+': fyll inn når du startet (for eksempel 09.2021).');
   if(job.end==='')errors.push(name+': fyll inn når du sluttet, eller kryss av for «Nåværende jobb».');
   if(job.end&&job.start&&job.end<job.start)errors.push(name+': sluttdatoen er før startdatoen.');
   if(!job.bullets.length)errors.push(name+': skriv minst ett punkt om hva du gjorde.');
  });
  data.education.forEach((item,index)=>{const name=`Utdanning ${index+1}`;if(!item.qualification.trim()||!item.institution.trim())errors.push(name+': fyll inn utdanning og lærested.');if(!item.end)errors.push(name+': fyll inn sluttår (forventet år hvis du fortsatt studerer).');if(item.start&&item.end&&item.end<item.start)errors.push(name+': sluttåret er før startåret.');});
  data.skills.forEach((item,index)=>{if(!item.name.trim())errors.push(`Kompetanse ${index+1} mangler navn.`);});
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
 const api={schema,blank,validate,ready,upgrade,repair,friendly,parseMonth,parseYear,showMonth,isNow};root.CVContract=api;if(typeof module!=='undefined'&&module.exports)module.exports=api;
})(typeof globalThis!=='undefined'?globalThis:this);
