'use strict';
let data=CVContract.blank(),sequence=0,pages=[];
const $=id=>document.getElementById(id),el=(tag,cls,text)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined)node.textContent=text;return node;};
const id=prefix=>prefix+'_'+Date.now().toString(36)+'_'+(++sequence),lines=value=>value.split('\n').map(item=>item.trim()).filter(Boolean);
const R=window.CVRecruiter,LAY=window.CVLayouts,STORE='cvbuilder-utkast-v2',PREF='cvbuilder-husk',BACKUP='cvbuilder-sikkerhetskopi';
let remember=true,isExample=false;
function storageGet(key){try{return localStorage.getItem(key);}catch{return null;}}
function storageSet(key,value){try{if(value===null)localStorage.removeItem(key);else localStorage.setItem(key,value);return true;}catch{return false;}}
function persist(){if(!remember||isExample)return;const copy=structuredClone(data);copy.approvals.facts_confirmed=false;storageSet(STORE,JSON.stringify(copy));}
function layout(){return LAY.get(data.options.layout);}
const LABELS={
 no:{profile:'Profil',skills:'Kjernekompetanse',experience:'Arbeidserfaring',education:'Utdanning',certifications:'Kurs og sertifiseringer',languages:'Språk',interests:'Interesser',portrait:'Portrett',page:'A4-side(r)'},
 en:{profile:'Profile',skills:'Core skills',experience:'Professional experience',education:'Education',certifications:'Courses and certifications',languages:'Languages',interests:'Interests',portrait:'Portrait',page:'A4 page(s)'}
};
const label=key=>LABELS[data.language==='en'?'en':'no'][key];
function message(value,action){const node=$('status');node.replaceChildren(document.createTextNode(value));if(action){const button=el('button','status-action',action.label);button.type='button';button.onclick=action.run;node.append(button);}}
function changed(){data.approvals.facts_confirmed=false;$('approval').checked=false;if(isExample){isExample=false;banner();message('Nå er dette din CV. Den huskes i denne nettleseren mens du skriver.');}persist();render();}
function banner(){const node=$('example-banner');if(node)node.hidden=!isExample;}
function field(parent,text,value,set,{type='text',multi=false,choices=null,max=200,placeholder=''}={}){
 const wrapper=el('label','field',text),input=el(choices?'select':multi?'textarea':'input');
 if(choices)for(const [key,name]of choices)input.append(new Option(name,key));else if(!multi)input.type=type;
 input.value=value??'';if(!choices)input.maxLength=max;if(placeholder)input.placeholder=placeholder;if(multi&&max>2000)input.className='long';
 input.addEventListener('input',()=>{set(input.value);changed();});wrapper.append(input);parent.append(wrapper);return input;
}
// A text field for a month (kind 'month', stored YYYY-MM) or a year (kind 'year', stored YYYY).
// It accepts 09.2021, 9/2021, «mars 2019» and similar, and never stores a value it cannot read.
function dateField(parent,text,value,set,kind,onNow=null){
 const wrapper=el('label','field',text),input=el('input'),note=el('small','field-error');input.type='text';input.inputMode=kind==='year'?'numeric':'text';input.autocomplete='off';input.placeholder=kind==='year'?'2021':onNow?'09.2021, 2010 eller nå':'09.2021 eller 2010';input.value=kind==='month'?CVContract.showMonth(value):value;note.hidden=true;
 const commit=()=>{if(kind==='month'&&onNow&&CVContract.isNow(input.value)){onNow();return;}const parsed=kind==='month'?CVContract.parseMonth(input.value):CVContract.parseYear(input.value);if(parsed===null){note.textContent=kind==='month'?(onNow?'Skriv 09.2021 eller bare 2010. Jobber du der fortsatt, skriv «nå» eller kryss av for «Nåværende jobb».':'Skriv 09.2021, eller bare årstallet, for eksempel 2010.'):'Skriv årstall med fire siffer, for eksempel 2021.';note.hidden=false;input.setAttribute('aria-invalid','true');return;}note.hidden=true;input.removeAttribute('aria-invalid');if(parsed!==value){value=parsed;set(parsed);changed();}input.value=kind==='month'?CVContract.showMonth(parsed):parsed;};
 input.addEventListener('change',commit);input.addEventListener('blur',commit);wrapper.append(input,note);parent.append(wrapper);return input;
}
function check(parent,text,value,set){const wrapper=el('label','check'),input=el('input');input.type='checkbox';input.checked=value;input.addEventListener('change',()=>{set(input.checked);changed();});wrapper.append(input,document.createTextNode(text));parent.append(wrapper);return input;}
function button(parent,text,action,cls=''){const item=el('button',cls,text);item.type='button';item.onclick=action;parent.append(item);return item;}
function section(title,hint=''){const node=el('section');node.append(el('h2','',title));if(hint)node.append(el('p','hint',hint));$('form').append(node);return node;}
function records(parent,key,title,create,fill){
 const holder=el('div');parent.append(holder);
 data[key].forEach((item,index)=>{const card=el('div','record');card.append(el('h3','',`${title} ${index+1}`));fill(card,item,index);button(card,'Fjern',()=>{const before=structuredClone(data);data[key].splice(index,1);if(key==='interests')data.options.interest_ids=data.options.interest_ids.filter(value=>value!==item.id);changed();form();message(`${title} ${index+1} er fjernet.`,{label:'Angre',run:()=>{load(before);message('Det er tilbake.');}});},'remove');holder.append(card);});
 button(parent,'+ Legg til '+title.toLowerCase(),()=>{data[key].push(create());changed();form();},'add');
}
function form(){
 $('form').replaceChildren();
 const design=section('Utforming','Alle malene følger de samme strenge reglene: én tekstspalte, vanlige overskrifter, ekte tekst og maks to sider. Teksten som ATS-systemer leser, er lik i alle maler; bare typografi, farge og luft endres.');
 const choices=el('div','layout-choices');choices.setAttribute('role','radiogroup');choices.setAttribute('aria-label','Velg utforming');
 for(const key of LAY.ids){const info=LAY.get(key),option=el('label','layout-choice'+(data.options.layout===key?' selected':'')),input=el('input');input.type='radio';input.name='layout';input.value=key;input.id='layout-'+key;input.checked=data.options.layout===key;input.onchange=()=>{data.options.layout=key;persist();form();message('Utforming: '+info.name+'. Innholdet er uendret.');document.getElementById('layout-'+key)?.focus();};const swatch=el('span','layout-swatch');swatch.style.setProperty('--swatch',info.web.accent);swatch.style.fontFamily=info.web.head;swatch.textContent='Aa';const text=el('span','layout-text');text.append(el('strong','',info.name),el('small','',info.description));option.append(input,swatch,text);choices.append(option);}
 design.append(choices);
 const identity=section('01  Kontakt og profil','Navn, e-post, telefon, sted og faglig overskrift må fylles ut. Alt står som vanlig tekst i CV-en, slik at ATS-systemer kan lese det.');
 field(identity,'CV-språk',data.language,value=>{data.language=value;},{choices:[['no','Bokmål'],['en','English']]});
 identity.append(el('p','hint','Språkvalget bytter overskrifter og datoord (for eksempel «nå» / «Present»). Teksten du skriver, må du oversette selv. Lagre én fil per språk.'));
 for(const [key,text,type,placeholder]of [['name','Fullt navn','text',''],['email','E-post','email',''],['phone','Telefon med landskode','tel','+47 ...'],['city','By og land','text','Bergen, Norge'],['linkedin','LinkedIn · full https-adresse (anbefalt)','url','https://www.linkedin.com/in/...'],['headline','Faglig overskrift · bruk stillingens ord','text','For eksempel: Automasjonsingeniør | PLS | Idriftsettelse']])field(identity,text,data.candidate[key],value=>data.candidate[key]=value,{type,placeholder});
 field(identity,'Kort profil · 30–70 ord: hvem du er, kjernekompetanse og hva du vil bidra med',data.candidate.summary,value=>data.candidate.summary=value,{multi:true,max:1500});
 const upload=field(identity,'Valgfritt portrett · JPEG eller PNG','',()=>{},{type:'file'});upload.removeAttribute('value');upload.accept='image/jpeg,image/png';
 upload.onchange=async event=>{try{const file=event.target.files[0];if(!file)return;if(!['image/jpeg','image/png'].includes(file.type)||file.size>10000000)throw Error('Velg JPEG eller PNG under 10 MB.');const bitmap=await createImageBitmap(file);if(bitmap.width*bitmap.height>40000000){bitmap.close();throw Error('Bildet er for stort. Velg et mindre bilde.');}data.candidate.photo=portrait(bitmap);bitmap.close();data.options.photo=true;changed();form();message('Portrett lagt til lokalt. Originalfilen er uendret.');}catch(error){message(error.message);}};
 check(identity,'Vis portrett øverst til høyre på første side (vanlig i Norge; utelat ved søknad til land som fraråder bilde, som USA og Storbritannia)',data.options.photo,value=>data.options.photo=value);
 const target=section('02  Stillingen','Lim inn hele annonsen. «Foreslå fagord» henter ut teknologier og nøkkelord, og rekrutterersjekken viser hvilke som mangler i CV-en. Legg bare inn ord du faktisk har dekning for.');
 for(const [key,text]of [['role','Stilling du søker'],['company','Arbeidsgiver'],['url','Annonselenke · https']])field(target,text,data.target[key],value=>data.target[key]=value);
 field(target,'Stillingsannonse',data.target.description,value=>data.target.description=value,{multi:true,max:15000});
 const keywords=field(target,'Viktige fagord fra annonsen · ett per linje',data.target.keywords.join('\n'),value=>data.target.keywords=lines(value),{multi:true,max:2500});
 button(target,'Foreslå fagord fra annonsen',()=>{const merged=[...new Set([...data.target.keywords,...R.suggestKeywords(data.target.description)])].slice(0,30);data.target.keywords=merged;keywords.value=merged.join('\n');changed();message('Fagord foreslått. Fjern ord som ikke er relevante før du bruker listen.');},'add');
 const skills=section('03  Kjernekompetanse','Står rett etter profilen, gruppert. Bruk annonsens ordlyd der den stemmer. Hver kompetanse må ha konkret faktagrunnlag; studiekunnskap og interesse skal ikke presenteres som yrkeserfaring.');
 records(skills,'skills','Kompetanse',()=>({group:'',name:'',evidence:'',confirmed:true}),(card,item)=>{field(card,'Gruppe · for eksempel Fag, Verktøy eller Sertifikater',item.group,value=>item.group=value,{max:60});field(card,'Kompetanse eller verktøy',item.name,value=>item.name=value);});
 const experience=section('04  Arbeidserfaring','Nyeste først. Datoer: 09.2021, eller bare 2010 hvis du ikke husker måneden. 2–5 korte punkter per jobb som starter med et verb og viser resultat: «Oppnådde X, målt ved Y, ved å gjøre Z». Ikke skriv «Jeg» eller «Ansvarlig for».');
 records(experience,'experience','Jobb',()=>({id:id('job'),company:'',title:'',location:'',start:'',end:'',bullets:[],evidence:'',confirmed:true}),(card,item)=>{
  for(const [key,text]of [['company','Arbeidsgiver'],['title','Faktisk stillingstittel'],['location','Sted · by']])field(card,text,item[key],value=>item[key]=value);
  dateField(card,'Fra · måned og år',item.start,value=>item.start=value,'month');
  check(card,'Nåværende jobb',item.end===null,value=>{item.end=value?null:'';form();});
  if(item.end!==null)dateField(card,'Til · måned og år',item.end,value=>item.end=value,'month',()=>{item.end=null;changed();form();message('Merket som nåværende jobb.');});
  field(card,'Ditt arbeid og resultat · ett punkt per linje',item.bullets.join('\n'),value=>item.bullets=lines(value),{multi:true,max:2500});
  field(card,'Notat til deg selv · vises ikke i CV-en (valgfritt)',item.evidence,value=>item.evidence=value,{multi:true,max:1000,placeholder:'For eksempel hvor tallene kommer fra, eller hva du vil si i intervjuet.'});
 });
 const education=section('05  Utdanning','År har fire siffer. Bruk forventet sluttår for pågående utdanning og skriv dette i utdanningstittelen.');
 records(education,'education','Utdanning',()=>({qualification:'',institution:'',start:'',end:''}),(card,item)=>{field(card,'Grad, fagbrev eller studium',item.qualification,value=>item.qualification=value);field(card,'Lærested',item.institution,value=>item.institution=value);dateField(card,'Fra · år',item.start,value=>item.start=value,'year');dateField(card,'Til · år (forventet år hvis du studerer nå)',item.end,value=>item.end=value,'year');});
 const certifications=section('06  Kurs og sertifiseringer','Ta med det som er relevant for stillingen: sertifikater, fagbrev, sikkerhetskurs, leverandørkurs. Navn og utsteder slik de står på beviset.');
 records(certifications,'certifications','Kurs',()=>({name:'',issuer:'',year:''}),(card,item)=>{field(card,'Kurs eller sertifisering',item.name,value=>item.name=value);field(card,'Utsteder',item.issuer,value=>item.issuer=value);dateField(card,'År',item.year,value=>item.year=value,'year');});
 const languages=section('07  Språk');
 records(languages,'languages','Språk',()=>({name:'',level:''}),(card,item)=>{field(card,'Språk',item.name,value=>item.name=value,{max:80});field(card,'Nivå · for eksempel morsmål, flytende eller B2',item.level,value=>item.level=value,{max:100});});
 const interests=section('08  Interesser','Bevar hele listen her. CV-en skal bare ha et kort utvalg. Vurderingen er regelbasert og viser begrunnelsen; den er ingen ATS-score.');
 records(interests,'interests','Interesse',()=>({id:id('interest'),label:'',status:'current',description:'',category:'other',tags:[],confirmed:true,specificity:1}),(card,item)=>{
  field(card,'Interesse · slik den kan stå på CV-en',item.label,value=>item.label=value,{max:120});
  field(card,'Status',item.status,value=>item.status=value,{choices:[['current','Nåværende aktivitet'],['former','Tidligere aktivitet'],['interest','Interesse uten påstand om praksis']]});
  field(card,'Kort beskrivelse · bare for vurderingen',item.description,value=>item.description=value,{multi:true,max:400});
  field(card,'Kategori',item.category,value=>item.category=value,{choices:[['technology','Teknologi'],['outdoors','Friluftsliv'],['sport','Sport'],['creative','Kreativt'],['community','Frivillighet'],['other','Annet']]});
  field(card,'Temaord · ett per linje',item.tags.join('\n'),value=>item.tags=lines(value),{multi:true,max:600});
 });
 check(interests,'Ta med utvalgte interesser i CV-en',data.options.include_interests,value=>data.options.include_interests=value);
 const result=el('div');result.id='interest-results';interests.append(result);
 button(interests,'Bruk foreslått utvalg',()=>{const selected=CVInterests.recommend(CVInterests.evaluate(data.interests,data.target,data.public_context),{language:data.language});data.options.interest_ids=selected.map(item=>item.id);changed();message('Utvalget er oppdatert. Kontroller formuleringene før du godkjenner.');});
 const context=section('09  Avansert: kilder om arbeidsgiveren','Bruk arbeidsgiverens egen side eller en åpent publisert jobbprofil. Oppgi konkret kilde og lesedato. Ikke registrer private eller sensitive opplysninger. Byggeren søker ikke på nettet.');
 records(context,'public_context','Kilde',()=>({topic:'',tags:[],source_url:'',checked_on:'',scope:'employer_activity',verified:false}),(card,item)=>{
  field(card,'Hva sier kilden konkret?',item.topic,value=>item.topic=value,{multi:true,max:500});
  field(card,'Temaord · ett per linje',item.tags.join('\n'),value=>item.tags=lines(value),{multi:true,max:600});
  field(card,'Kildeadresse · https',item.source_url,value=>item.source_url=value,{max:1000});
  field(card,'Kontrollert dato',item.checked_on,value=>item.checked_on=value,{type:'date',max:10});
  field(card,'Kildetype',item.scope,value=>item.scope=value,{choices:[['employer_activity','Arbeidsgiver eller åpen teamaktivitet'],['professional_bio','Åpen profesjonell presentasjon']]});
  check(card,'Jeg har lest kilden og bekrefter at opplysningen står der',item.verified,value=>item.verified=value);
 });
 $('approval').checked=data.approvals.facts_confirmed;render();
}
function interestText(item){const no=data.language==='no',lower=/^\p{Ll}/u.test(item.label);if(item.status==='former'&&!/^(tidligere|former)\b/i.test(item.label))return (no?'Tidligere':'Former')+(lower?' ':': ')+item.label;if(item.status==='interest'&&!/^(interesse for|interest in)\b/i.test(item.label))return (no?'Interesse for ':'Interest in ')+item.label;return item.label;}
function selection(){return data.interests.filter(item=>item.confirmed&&data.options.interest_ids.includes(item.id));}
// The CV portrait box is 25 x 32 mm. Crop the centre of any photo to that shape so the preview,
// Word and PDF all show the same undistorted picture; slightly above centre keeps the face in view.
const PHOTO_RATIO=25/32,PHOTO_HEIGHT=512;
function portrait(source){const width=source.width,height=source.height;let cropW=width,cropH=height;if(width/height>PHOTO_RATIO)cropW=height*PHOTO_RATIO;else cropH=width/PHOTO_RATIO;const x=(width-cropW)/2,y=Math.max(0,(height-cropH)*.35);const canvas=document.createElement('canvas');canvas.height=Math.min(PHOTO_HEIGHT,Math.round(cropH));canvas.width=Math.round(canvas.height*PHOTO_RATIO);const context=canvas.getContext('2d');context.fillStyle='white';context.fillRect(0,0,canvas.width,canvas.height);context.imageSmoothingQuality='high';context.drawImage(source,x,y,cropW,cropH,0,0,canvas.width,canvas.height);return canvas.toDataURL('image/jpeg',.9);}
async function normalisePhoto(){const photo=data.candidate.photo;if(!photo)return;try{const image=new Image();image.src=photo;await image.decode();if(Math.abs(image.naturalWidth/image.naturalHeight-PHOTO_RATIO)<.02)return;if(data.candidate.photo!==photo)return;data.candidate.photo=portrait(image);persist();render();}catch{}}
function linkedinText(url){return url.replace(/^https:\/\/(www\.)?/,'').replace(/\/$/,'');}
function contactLine(){return [data.candidate.email,data.candidate.phone,data.candidate.city,data.candidate.linkedin&&linkedinText(data.candidate.linkedin)].filter(Boolean).join(' · ');}
function period(item){return R.period(item.start,item.end,data.language!=='en');}
function descriptors(){
 const items=[{kind:'header'}],add=(heading,records)=>{if(records.length)items.push({kind:'heading',text:heading},...records);};
 if(data.candidate.summary)add(label('profile'),[{kind:'paragraph',text:data.candidate.summary}]);
 add(label('skills'),R.groupSkills(data.skills).map(([group,names])=>({kind:'skills',group,names})));
 add(label('experience'),R.sortJobs(data.experience).map(item=>({kind:'job',...item})));
 add(label('education'),R.sortEducation(data.education).map(item=>({kind:'education',...item})));
 add(label('certifications'),R.sortCertifications(data.certifications).map(item=>({kind:'cert',...item})));
 add(label('languages'),data.languages.length?[{kind:'paragraph',text:data.languages.map(item=>item.name+': '+item.level).join(' · ')}]:[]);
 const chosen=selection();if(data.options.include_interests&&chosen.length)add(label('interests'),[{kind:'paragraph',text:chosen.map(interestText).join(' · ')}]);
 return items;
}
function block(item){
 const node=el('div','cv-block');node._cv=item;
 if(item.kind==='header'){
  node.className+=' cv-header'+(data.options.photo&&data.candidate.photo?'':' no-photo');
  if(data.options.photo&&data.candidate.photo){const image=el('img');image.src=data.candidate.photo;image.alt=label('portrait');node.append(image);}
  node.append(el('div','cv-name',data.candidate.name||'Ditt navn'),el('div','cv-role',data.candidate.headline||'Din faglige overskrift'),el('div','cv-contact',contactLine()));
 }else if(item.kind==='heading'){node.className+=' cv-heading';node.textContent=LAY.heading(item.text,data.options.layout);}
 else if(item.kind==='skills'){node.className+=' cv-skill';if(item.group)node.append(el('strong','',item.group+': '));node.append(document.createTextNode(item.names.join(', ')));}
 else if(item.kind==='job'&&item.continued){node.className+=' cv-job cv-continued';const list=el('ul');item.bullets.forEach(text=>list.append(el('li','',text)));node.append(list);}
 else if(item.kind==='job'){node.className+=' cv-job';const top=el('div','cv-line');top.append(el('span','cv-jobtitle',item.title),el('span','cv-period',period(item)));const sub=el('div','cv-line');sub.append(el('span','cv-company',item.company),el('span','cv-period',item.location));node.append(top,sub);const list=el('ul');item.bullets.forEach(text=>list.append(el('li','',text)));node.append(list);}
 else if(item.kind==='education'){node.className+=' cv-edu';const top=el('div','cv-line');top.append(el('span','cv-jobtitle',item.qualification),el('span','cv-period',[item.start,item.end].filter(Boolean).join(' – ')));node.append(top,el('div','cv-company',item.institution));}
 else if(item.kind==='cert'){node.className+=' cv-cert';node.append(el('strong','',item.name),document.createTextNode([item.issuer,item.year].filter(Boolean).map(text=>' · '+text).join('')));}
 else{node.className+=' cv-profile';node.textContent=item.text;}
 return node;
}
function renderRecruiter(audit){
 const panel=$('recruiter');if(!panel)return;panel.replaceChildren();
 for(const item of audit.checks){const row=el('div','rc rc-'+item.level);row.append(el('strong','',(item.level==='pass'?'✓ ':item.level==='warn'?'! ':'i ')+item.text));if(item.level!=='pass'&&item.tip)row.append(el('small','',item.tip));panel.append(row);}
 if(audit.match.found.length){const row=el('div','rc-tags');for(const word of audit.match.found)row.append(el('span','tag tag-found',word));for(const word of audit.match.missing)row.append(el('span','tag tag-missing',word));panel.append(row);}
}
// Lays the CV out on A4 sheets inside `holder` and returns the pages; used for the preview and for fit checks.
function paginate(holder,layoutId=data.options.layout){
 const pages=[],info=LAY.get(layoutId),web=info.web;holder.replaceChildren();
 function newPage(){const sheet=el('article','sheet layout-'+layoutId+' heading-'+web.headingStyle);for(const [key,value]of [['--cv-body',web.body],['--cv-head',web.head],['--cv-size',web.size+'pt'],['--cv-name',web.name+'pt'],['--cv-accent',web.accent],['--cv-ink',web.ink],['--cv-muted',web.muted],['--cv-rule',web.rule],['--cv-gap',web.gap]])sheet.style.setProperty(key,value);const content=el('div','page-content'),footer=el('footer');sheet.append(content,footer);holder.append(sheet);const page={sheet,content,items:[]};pages.push(page);return page;}
 let page=newPage(),oversized=false;
 for(const item of descriptors()){
  const node=block(item);page.content.append(node);
  if(page.content.scrollHeight>page.content.clientHeight+1){
   node.remove();
   if(item.kind==='job'&&page.items.length&&item.bullets.length>1){
    // Let a long job continue on the next page at bullet level instead of leaving a gap.
    let split=false;
    for(let keep=item.bullets.length-1;keep>=1&&!split;keep--){
     const first={...item,bullets:item.bullets.slice(0,keep)},trial=block(first);page.content.append(trial);
     if(page.content.scrollHeight<=page.content.clientHeight+1){page.items.push(first);const rest={...item,bullets:item.bullets.slice(keep),continued:true};page=newPage();page.content.append(block(rest));page.items.push(rest);split=true;}
     else trial.remove();
    }
    if(split)continue;
   }
   if(!page.items.length){page.content.append(node);page.items.push(item);oversized=true;continue;}
   let heading=null;if(page.items.at(-1)?.kind==='heading'){heading=page.items.pop();page.content.lastElementChild.remove();}
   page=newPage();if(heading){page.content.append(block(heading));page.items.push(heading);}page.content.append(node);
   if(page.content.scrollHeight>page.content.clientHeight+1)oversized=true;
  }
  page.items.push(item);
 }
 return {pages,oversized};
}
function fitsIn(layoutId){const probe=el('div','probe');probe.setAttribute('aria-hidden','true');document.body.append(probe);const result=paginate(probe,layoutId);probe.remove();return !result.oversized&&result.pages.length<=2;}
function render(){
 const result=paginate($('pages')),oversized=result.oversized;pages=result.pages;
 pages.forEach((page,index)=>{const footer=page.sheet.querySelector('footer');footer.append(el('span','',data.candidate.name),el('span','',`${index+1} / ${pages.length}`));if(oversized||pages.length>2)page.sheet.classList.add('overflow');});
 $('page-count').textContent=data.language==='en'?`${pages.length} A4 page${pages.length===1?'':'s'}`:`${pages.length} A4-side${pages.length===1?'':'r'}`;
 const errors=CVContract.ready(data);
 if(oversized||pages.length>2){const alternatives=LAY.ids.filter(id=>id!==data.options.layout&&fitsIn(id)).map(id=>LAY.get(id).name);errors.push(`CV-en blir ${pages.length} sider i ${layout().name}. `+(alternatives.length?`Den får plass på to sider i ${alternatives.join(' og ')}, eller kort ned teksten.`:'Kort ned teksten; maksimalt to sider.'));}
 if(selection().map(interestText).join(' ').split(/\s+/).filter(Boolean).length>35)errors.push('Kort ned interesseutvalget til maksimalt 35 ord.');
 const blocking=errors.filter(error=>!/^approvals:/.test(error));
 $('validation').replaceChildren();for(const error of blocking.slice(0,12))$('validation').append(el('p','error',error));if(blocking.length>12)$('validation').append(el('p','error',`… og ${blocking.length-12} til.`));
 if(!blocking.length)$('validation').append(el('p','pass',data.approvals.facts_confirmed?'Alt er klart. Du har bekreftet opplysningene.':'Alt nødvendig er fylt ut. Du bekrefter opplysningene når du laster ned.'));
 fitSheets();
 for(const name of ['word','pdf','text'])$(name).disabled=!!blocking.length;
 const hint=$('export-hint');hint.replaceChildren();hint.hidden=!blocking.length;if(blocking.length){hint.append(document.createTextNode(blocking.length===1?'1 ting må ordnes før nedlasting.':`${blocking.length} ting må ordnes før nedlasting.`));const go=el('button','','Vis');go.type='button';go.onclick=()=>{document.body.dataset.view='form';$('validation').scrollIntoView({behavior:'smooth',block:'center'});};hint.append(go);}
 renderRecruiter(R.audit(data,{pages:pages.length}));
 const results=$('interest-results');if(results){results.replaceChildren();const ranked=CVInterests.evaluate(data.interests,data.target,data.public_context),recommended=new Set(CVInterests.recommend(ranked,{language:data.language}).map(item=>item.id));for(const item of ranked){const row=el('div','suggestion');row.append(el('strong','',item.label+(recommended.has(item.id)?' · foreslått':item.confirmed?' · reserve':' · må bekreftes')),el('small','',item.reasons.join(' ')));const selected=el('input');selected.type='checkbox';selected.checked=data.options.interest_ids.includes(item.id);selected.disabled=!item.confirmed;selected.setAttribute('aria-label','Ta med '+item.label);selected.onchange=()=>{const values=new Set(data.options.interest_ids);if(selected.checked&&values.size>=3){selected.checked=false;message('Velg høyst tre interesser.');return;}if(selected.checked)values.add(item.id);else values.delete(item.id);data.options.interest_ids=[...values];changed();};row.prepend(selected);results.append(row);}}
 return errors;
}
function save(blob,name){const url=URL.createObjectURL(blob),anchor=el('a');anchor.href=url;anchor.download=name;anchor.click();setTimeout(()=>URL.revokeObjectURL(url),3000);}
function filename(){const clean=value=>(value||'').normalize('NFKD').replace(/[̀-ͯ]/g,'').replace(/ø/g,'o').replace(/Ø/g,'O').replace(/æ/g,'ae').replace(/Æ/g,'Ae').replace(/[^a-zA-Z0-9]+/g,'_').replace(/^_|_$/g,'');return [clean(data.candidate.name)||'CV','CV',clean(data.target.company),data.language].filter(Boolean).join('_').slice(0,100);}
async function exportWord(){
 const errors=render();if(errors.length)throw Error(errors[0]);
 const {Document,Paragraph,TextRun,ImageRun,Packer,TextWrappingType,HeadingLevel,ExternalHyperlink,LevelFormat,AlignmentType,TabStopType,Footer,PageNumber,BorderStyle}=window.docx;
 const Wd=layout().word,INK=Wd.ink,MUTED=Wd.muted,ACCENT=Wd.accent,SIZE=Wd.size,RIGHT=9978; // text width in twips (A4 minus 17 mm margins)
 const run=(text,options={})=>new TextRun({text,font:options.font??Wd.body,size:options.size??SIZE,bold:options.bold??false,color:options.color??INK});
 const space=value=>Math.round(value*Wd.gap);
 const line=(left,right,{bold=false,size=SIZE,color=INK,after=20,keepNext=true}={})=>new Paragraph({keepNext,keepLines:true,spacing:{after},tabStops:[{type:TabStopType.RIGHT,position:RIGHT}],children:[run(left,{bold,size,color}),...(right?[run('\t'+right,{size:SIZE-3,color:MUTED})]:[])]});
 const children=[],no=data.language!=='en';
 if(data.options.photo&&data.candidate.photo){const source=Uint8Array.from(atob(data.candidate.photo.split(',')[1]),character=>character.charCodeAt(0));children.push(new Paragraph({children:[new ImageRun({data:source,type:'jpg',altText:{name:label('portrait'),title:label('portrait'),description:label('portrait')+' · '+data.candidate.name},transformation:{width:90,height:114},floating:{horizontalPosition:{relative:'margin',align:'right'},verticalPosition:{relative:'margin',offset:0},wrap:{type:TextWrappingType.NONE},allowOverlap:true}})],spacing:{after:0,line:1},run:{size:2}}));}
 children.push(new Paragraph({heading:HeadingLevel.TITLE,spacing:{after:80},children:[run(data.candidate.name,{size:Wd.name,bold:true,font:Wd.head})]}));
 children.push(new Paragraph({spacing:{after:80},children:[run(data.candidate.headline,{size:SIZE+3,bold:true,color:ACCENT})]}));
 const small=SIZE-2,contact=[run(data.candidate.email,{size:small,color:MUTED}),run('  ·  '+data.candidate.phone+'  ·  '+data.candidate.city,{size:small,color:MUTED})];
 if(data.candidate.linkedin)contact.push(run('  ·  ',{size:small,color:MUTED}),new ExternalHyperlink({link:data.candidate.linkedin,children:[new TextRun({text:linkedinText(data.candidate.linkedin),font:Wd.body,size:small,color:ACCENT,underline:{}})]}));
 children.push(new Paragraph({spacing:{after:data.options.photo&&data.candidate.photo?520:160},children:contact}));
 const border=Wd.headingStyle==='rule'?{bottom:{color:Wd.rule,style:BorderStyle.SINGLE,size:6,space:4}}:Wd.headingStyle==='bar'?{left:{color:Wd.rule,style:BorderStyle.SINGLE,size:24,space:6}}:undefined;
 const heading=text=>new Paragraph({heading:HeadingLevel.HEADING_1,keepNext:true,spacing:{before:space(200),after:space(110)},indent:Wd.headingStyle==='bar'?{left:120}:undefined,border,children:[run(LAY.heading(text,data.options.layout,'word'),{bold:true,size:SIZE+1,color:ACCENT,font:Wd.head})]});
 const body=(text,after=120)=>new Paragraph({spacing:{after},children:[run(text)]});
 for(const item of descriptors().slice(1)){
  if(item.kind==='heading')children.push(heading(item.text));
  else if(item.kind==='skills')children.push(new Paragraph({spacing:{after:50},children:[...(item.group?[run(item.group+': ',{bold:true})]:[]),run(item.names.join(', '))]}));
  else if(item.kind==='job'){children.push(line(item.title,period(item),{bold:true}),line(item.company,item.location,{color:ACCENT,after:60}));item.bullets.forEach((text,index)=>children.push(new Paragraph({numbering:{reference:'cv-bullets',level:0},keepLines:true,spacing:{after:index===item.bullets.length-1?150:40},children:[run(text)]})));}
  else if(item.kind==='education')children.push(line(item.qualification,[item.start,item.end].filter(Boolean).join(' – '),{bold:true}),new Paragraph({spacing:{after:120},children:[run(item.institution,{color:ACCENT})]}));
  else if(item.kind==='cert')children.push(new Paragraph({spacing:{after:50},children:[run(item.name,{bold:true}),run([item.issuer,item.year].filter(Boolean).map(text=>' · '+text).join(''),{color:MUTED})]}));
  else children.push(body(item.text));
 }
 const footer=new Footer({children:[new Paragraph({tabStops:[{type:TabStopType.RIGHT,position:RIGHT}],children:[run(data.candidate.name,{size:16,color:MUTED}),new TextRun({children:['\t',PageNumber.CURRENT,' / ',PageNumber.TOTAL_PAGES],font:Wd.body,size:16,color:MUTED})]})]});
 const document=new Document({
  creator:data.candidate.name,title:data.candidate.name+' – CV',subject:[data.target.role,data.target.company].filter(Boolean).join(' – '),keywords:data.target.keywords.join(', '),description:data.candidate.headline,lastModifiedBy:data.candidate.name,
  numbering:{config:[{reference:'cv-bullets',levels:[{level:0,format:LevelFormat.BULLET,text:'•',alignment:AlignmentType.LEFT,style:{paragraph:{indent:{left:340,hanging:220}}}}]}]},
  styles:{default:{document:{run:{font:Wd.body,size:SIZE,color:INK},paragraph:{spacing:{line:Math.round(264*Math.min(1,Wd.gap+.1))}}}},paragraphStyles:[{id:'Title',name:'Title',basedOn:'Normal',next:'Normal',run:{font:Wd.head,size:Wd.name,bold:true,color:INK}},{id:'Heading1',name:'Heading 1',basedOn:'Normal',next:'Normal',quickFormat:true,run:{font:Wd.head,size:SIZE+1,bold:true,color:ACCENT}}]},
  sections:[{properties:{page:{size:{width:11906,height:16838},margin:{top:964,bottom:850,left:964,right:964,footer:420}}},footers:{default:footer},children}]
 });
 return Packer.toBlob(document);
}
$('form').onsubmit=event=>event.preventDefault();
$('approval').onchange=()=>{data.approvals.facts_confirmed=$('approval').checked;render();};
$('save').onclick=()=>{save(new Blob([JSON.stringify(data,null,2)],{type:'application/json'}),filename()+'_utkast.json');message('Utkastet er lagret som fil. Den inneholder dine personopplysninger; del den bare med folk du stoler på.');};
$('open').onclick=()=>$('project-file').click();
$('project-file').onchange=async event=>{
 const file=event.target.files[0];event.target.value='';if(!file)return;
 try{
  if(!/\.json$/i.test(file.name)){message(`«${file.name}» er ikke et CVBuilder-utkast. Her åpner du bare .json-filer du har lagret med «Lagre som fil». Word- og PDF-filer kan ikke leses inn.`);return;}
  if(file.size>3000000){message('Filen er større enn 3 MB og er ikke et CVBuilder-utkast.');return;}
  let value;try{value=CVContract.upgrade(JSON.parse(await file.text()));}catch{message(`«${file.name}» kunne ikke leses. Filen er skadet eller ikke et CVBuilder-utkast.`);return;}
  const before=structuredClone(data),wasExample=isExample,{value:fixed,fixed:notes}=CVContract.repair(value);
  fixed.approvals.facts_confirmed=false;load(fixed);
  offerUndo(before,wasExample,notes.length?`Utkastet er åpnet. Disse feltene hadde feil format og ble tømt: ${notes.join('; ')}.`:'Utkastet er åpnet.');
 }catch(error){message('Kunne ikke åpne utkastet: '+CVContract.friendly(error.message));}
};
// Downloads ask for one confirmation at the moment of download instead of blocking the buttons.
let pending=null;
function withApproval(run){if(data.approvals.facts_confirmed){run();return;}pending=run;$('confirm-bar').hidden=false;$('confirm-yes').focus();}
$('confirm-yes').onclick=()=>{data.approvals.facts_confirmed=true;$('approval').checked=true;$('confirm-bar').hidden=true;render();const run=pending;pending=null;if(run)run();};
document.addEventListener('keydown',event=>{if(event.key==='Escape'&&!$('confirm-bar').hidden)$('confirm-no').click();});
$('confirm-no').onclick=()=>{pending=null;$('confirm-bar').hidden=true;message('Ingenting er lastet ned. Se gjennom CV-en og prøv igjen.');};
$('word').onclick=()=>withApproval(async()=>{try{save(await exportWord(),filename()+'.docx');message('Word-filen er lastet ned. Åpne den i Word og se over sidebrytingen før du sender den.');}catch(error){message(error.message);}});
$('text').onclick=()=>withApproval(async()=>{const text=R.plainText(data);try{await navigator.clipboard.writeText(text);message('CV-teksten er kopiert. Lim den inn der søknadsportalen ber om CV som tekst, og sjekk feltene portalen fyller ut.');}catch{save(new Blob([text],{type:'text/plain;charset=utf-8'}),filename()+'.txt');message('Kopiering var ikke tillatt her, så teksten ble lastet ned som .txt-fil i stedet.');}});
function exportPdf(){const errors=render().filter(error=>!/^approvals:/.test(error));if(errors.length)throw Error(errors[0]);return CVPdf.build({data,items:descriptors(),layout:layout(),period,contact:()=>[data.candidate.email,data.candidate.phone,data.candidate.city,data.candidate.linkedin&&linkedinText(data.candidate.linkedin)].filter(Boolean),linkedinText,heading:text=>LAY.heading(text,data.options.layout,'pdf')});}
$('pdf').onclick=()=>withApproval(()=>{if(!window.jspdf||!window.CVPdf){message('PDF-modulen mangler. Velg Lagre som PDF i utskriftsvinduet: A4, ingen topp- og bunntekst, bakgrunnsgrafikk på.');window.print();return;}try{const result=exportPdf();save(result.blob,filename()+'.pdf');message(result.pages>2?`PDF-en ble ${result.pages} sider. Kort ned teksten eller velg Kompakt.`:'PDF-en er lastet ned, med ekte, søkbar tekst.');}catch(error){message(error.message);}});
function load(value,{example=false,note=''}={}){const upgraded=CVContract.upgrade(value),errors=CVContract.validate(upgraded);if(errors.length)throw Error(errors[0]);data=structuredClone(upgraded);isExample=example;banner();form();if(!example)persist();if(note)message(note);normalisePhoto();}
// Replacing the CV can always be undone, and the replaced CV is also kept as a backup in this browser.
function offerUndo(before,wasExample,text){if(!wasExample)storageSet(BACKUP,JSON.stringify(before));restoreButton();message(text,{label:'Angre',run:()=>{load(before,{example:wasExample});message(wasExample?'Eksemplet er tilbake.':'CV-en din er tilbake.');}});}
$('new-cv').onclick=()=>{const before=structuredClone(data),wasExample=isExample,value=CVContract.blank();value.options.layout=data.options.layout;load(value);offerUndo(before,wasExample,wasExample?'Tom CV klar. Start med kontaktinformasjonen.':'Tom CV klar. Den forrige CV-en kan hentes tilbake med Angre.');window.scrollTo({top:0,behavior:'smooth'});};
$('load-example').onclick=()=>{const before=structuredClone(data),wasExample=isExample;load(CVExample.example(CVContract),{example:true});offerUndo(before,wasExample,wasExample?'Eksemplet er lastet på nytt.':'Du ser eksemplet. CV-en din kan hentes tilbake med Angre.');};
$('remember').onchange=()=>{remember=$('remember').checked;storageSet(PREF,remember?'1':'0');if(remember){persist();message('Utkastet huskes i denne nettleseren på denne maskinen.');}else message('Endringer huskes ikke lenger. Det som allerede er lagret, ligger der til du sletter det.',{label:'Slett lagret utkast',run:()=>{storageSet(STORE,null);message('Det lagrede utkastet er slettet fra denne nettleseren.');}});};
function restoreButton(){const node=$('restore');if(node)node.hidden=!storageGet(BACKUP);}
$('restore').onclick=()=>{const saved=storageGet(BACKUP);if(!saved)return;try{const before=structuredClone(data),wasExample=isExample,{value}=CVContract.repair(CVContract.upgrade(JSON.parse(saved)));load(value);offerUndo(before,wasExample,'Den forrige CV-en er hentet tilbake.');}catch{message('Sikkerhetskopien kunne ikke åpnes.');}};
for(const [button,view]of [['show-form','form'],['show-cv','cv']])$(button).onclick=()=>{document.body.dataset.view=view;$('show-form').setAttribute('aria-pressed',String(view==='form'));$('show-cv').setAttribute('aria-pressed',String(view==='cv'));window.scrollTo({top:0});fitSheets();};
// The A4 sheet scales to the width available so it is never cut off on a phone.
function fitSheets(){const holder=$('pages');if(!holder)return;const width=holder.clientWidth-32,zoom=Math.min(1,Math.max(.3,width/794));holder.style.setProperty('--zoom',zoom.toFixed(3));}
window.addEventListener('resize',fitSheets);
window.CVFamily={getData:()=>structuredClone(data),setData:value=>load(value),render,exportWord,exportPdf,plainText:()=>R.plainText(data),audit:()=>R.audit(data,{pages:pages.length}),isExample:()=>isExample};
(function start(){
 remember=storageGet(PREF)!=='0';$('remember').checked=remember;document.body.dataset.view='form';restoreButton();
 const stored=storageGet(STORE);
 if(stored){
  try{const {value,fixed}=CVContract.repair(CVContract.upgrade(JSON.parse(stored)));load(value,{note:fixed.length?`Velkommen tilbake. Noen felt hadde feil format og ble tømt: ${fixed.join('; ')}.`:'Velkommen tilbake. Utkastet ditt er hentet fra denne nettleseren.'});fitSheets();return;}
  catch{storageSet(BACKUP,stored);}
 }
 load(CVExample.example(CVContract),{example:true,note:stored?'Det lagrede utkastet kunne ikke åpnes, men er tatt vare på som sikkerhetskopi i denne nettleseren. Du ser nå eksemplet.':'Dette er et eksempel. Velg «Ny tom CV» for å starte på din egen, eller skriv rett inn i eksemplet.'});fitSheets();
})();
