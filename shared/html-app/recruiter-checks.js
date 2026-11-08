(function(root){
 'use strict';
 // Open, explainable recruiter and ATS heuristics. No score here predicts a hiring outcome;
 // each check mirrors common advice from recruiters and applicant-tracking-system vendors.
 const normal=value=>String(value||'').normalize('NFKC').toLocaleLowerCase().replace(/[^\p{L}\p{N}+#]+/gu,' ').replace(/\s+/g,' ').trim();
 const escape=value=>value.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
 const words=value=>String(value||'').trim().split(/\s+/).filter(Boolean);
 const STOP=new Set(['no','en','nb','ca','as','asa','og','or','and','the','vi','du','we','you','it','is','ok','cv','pdf','eu','hr','ceo','cto']);
 const DICTIONARY=['idriftsettelse','igangkjøring','feilsøking','kundestøtte','teknisk støtte','teknisk support','applikasjonsstøtte','kursing','opplæring','prosjektledelse','dokumentasjon','automasjon','pls','plc','scada','hmi','feltbuss','feltbusser','nettverk','motion','maskinsikkerhet','funksjonell sikkerhet','programmering','commissioning','troubleshooting','customer support','technical support','training','project management','fieldbus','networking','machine safety','functional safety','kundeservice','salg','regnskap','budsjett','ledelse','koordinering','administrasjon','sertifisering','fagbrev','førerkort','truckførerbevis','excel','power bi','sql','python','javascript','typescript','c++','c#','java','html','css','json','react','docker','linux','azure','aws','git'];
 const COMMON=new Set(['arbeidsoppgaver','kvalifikasjoner','egenskaper','stillingen','muligheter','arbeidsmiljø','arbeidsmiljøet','søknadsfrist','informasjon','personlige','selvstendig','engasjement','engasjert','arbeidsdag','arbeidsplass','kontaktperson','spørsmål','tilsettingen','ansettelse','konkurransedyktige','betingelser','forsikringer','pensjonsordning','kollegaer','fremtidsrettet','utviklingsmuligheter','internasjonal','avdelingen','organisasjonen','virksomheten','tiltredelse','stillingsandel','experience','requirements','responsibilities','opportunity','opportunities','development','environment','application','applications','candidates','qualifications','including','international','information','competitive']);
 const CLICHES=['løsningsorientert','resultatorientert','teamplayer','team player','lagspiller','hardworking','hardtarbeidende','arbeidsom','strukturert og selvstendig','selvgående','motivert','engasjert','dynamisk','dynamic','detail-oriented','detaljorientert','go-getter','synergi','synergy','think outside the box','passionate','brennende interessert'];
 const WEAK_START=/^(jeg|i|my|min|mitt|mine|ansvarlig for|responsible for|duties included|arbeidsoppgaver|oppgaver)\b/i;
 const SENSITIVE=/\b(født|fødselsdato|fødselsnummer|personnummer|sivilstand|sivilstatus|gift|ugift|samboer|religion|date of birth|born on|marital status|social security|nationality|nasjonalitet)\b/i;
 // A number counts as a result only when it measures something (%, money, time, volume), not a version or model number.
 const RESULT=/(^|[^\p{L}\p{N}.\-])\d+(?:[.,]\d+)?\s?(?:%|prosent|percent|kr\b|nok\b|mnok|mill|million|timer|hours?|dager|days?|uker|weeks?|måneder|months?|år\b|years?|ganger|times|x\b|kunder|customers|anlegg|plants|systemer|systems|prosjekter|projects|personer|people|brukere|users|ansatte|employees|signaler|signals|tags|i\/o|meter|km|tonn|tonnes|mw|kw)/iu;
 function suggestKeywords(description,limit=25){
  const text=String(description||''),found=new Map();
  const add=(term,weight)=>{const clean=term.replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}+#]+$/gu,'').trim();const key=normal(clean);if(clean.length<2||STOP.has(key)||/^\d+$/.test(key))return;const entry=found.get(key)||{term:clean,weight:0};entry.weight+=weight;found.set(key,entry);};
  for(const match of text.matchAll(/\b([A-ZÆØÅ][A-Za-zÆØÅæøå]*)\s+(\d[\d.\-]*\d|\d)\b/g))if(/[A-ZÆØÅ]{2,}|[a-zæøå][A-ZÆØÅ]/.test(match[1]))add(match[0],3);
  for(const match of text.matchAll(/(?<![\p{L}\p{N}])([A-ZÆØÅ][A-Za-z0-9ÆØÅæøå]*[A-ZÆØÅ][A-Za-z0-9ÆØÅæøå]*|[A-Za-z]+(?:\+\+|#)|C\+\+|C#)(?![\p{L}\p{N}])/gu))add(match[1],2);
  const lower=normal(text);
  // Long Norwegian and English content words (often compound nouns such as «legemiddelhåndtering») that recur in the ad.
  const counts=new Map();for(const word of lower.split(' '))if(word.length>=9&&/^\p{L}+$/u.test(word)&&!COMMON.has(word))counts.set(word,(counts.get(word)||0)+1);
  for(const [word,times]of counts)if(times>=2||word.length>=12)add(word,.5+times*.5);
  for(const term of DICTIONARY)if(new RegExp('(^| )'+escape(normal(term))+'( |$)').test(lower))add(term,1);
  const terms=[...found.values()],keys=new Set(found.keys());
  const redundant=item=>{const key=normal(item.term),parts=key.split(' ');
   if(parts.length===2&&/^\d$/.test(parts[1])&&keys.has(parts[0]))return true; // "TwinCAT 3" when "TwinCAT" exists
   return key.length<=3&&[...keys].some(other=>other!==key&&other.startsWith(key+' '));}; // "IEC" when "IEC 61131 3" exists
  return terms.filter(item=>!redundant(item)).sort((a,b)=>b.weight-a.weight||a.term.localeCompare(b.term)).slice(0,limit).map(item=>item.term);
 }
 function cvText(data){
  return [data.candidate.headline,data.candidate.summary,...data.skills.flatMap(item=>[item.group,item.name]),...data.experience.flatMap(item=>[item.title,item.company,...item.bullets]),...data.education.flatMap(item=>[item.qualification,item.institution]),...(data.certifications||[]).flatMap(item=>[item.name,item.issuer])].join('\n');
 }
 function contains(haystack,keyword){
  const options=String(keyword).split(/\s*\/\s*|\s*\|\s*/).map(normal).filter(Boolean);
  return options.some(option=>option.length<=3?new RegExp('(^| )'+escape(option)+'( |$)').test(haystack):new RegExp('(^| )'+escape(option)).test(haystack));
 }
 function keywordMatch(data){
  const haystack=normal(cvText(data)),keywords=[...new Set(data.target.keywords.map(item=>item.trim()).filter(Boolean))];
  const found=keywords.filter(item=>contains(haystack,item)),missing=keywords.filter(item=>!contains(haystack,item));
  return {found,missing,coverage:keywords.length?Math.round(100*found.length/keywords.length):null};
 }
 // A bare year counts from January when a job starts and to December when it ends.
 function monthIndex(value,end=false){if(!value)return null;const [year,month]=value.split('-').map(Number);return year*12+(month?month-1:end?11:0);}
 function gaps(data,today=new Date()){
  const jobs=data.experience.filter(job=>job.start).map(job=>({job,start:monthIndex(job.start),end:job.end===null?today.getFullYear()*12+today.getMonth():monthIndex(job.end,true)})).filter(item=>item.end!==null).sort((a,b)=>a.start-b.start);
  const result=[];let reach=null,last=null;
  for(const item of jobs){if(reach!==null&&item.start-reach>6)result.push({after:last.job.company,before:item.job.company,months:item.start-reach});if(reach===null||item.end>reach){reach=item.end;last=item;}}
  return result;
 }
 function audit(data,{pages=1}={}){
  const checks=[];
  // check(ok, when it passes, when it does not, how to fix it, level when it does not pass)
  const check=(ok,pass,fail,tip='',level='warn')=>checks.push(ok?{level:'pass',text:pass,tip:''}:{level,text:fail,tip});
  const bullets=data.experience.flatMap(job=>job.bullets.map(text=>({job,text})));
  const role=normal(data.target.role),headline=normal(data.candidate.headline),summary=data.candidate.summary.trim();
  check(Boolean(data.candidate.name&&data.candidate.email&&data.candidate.phone&&data.candidate.city),'Kontaktinformasjon står som vanlig tekst øverst','Kontaktinformasjon mangler','Fyll inn navn, e-post, telefon og by. Det står som vanlig tekst øverst i CV-en, ikke i topptekst eller bilde.');
  if(data.candidate.phone)check(/^\s*(\+|00)\d/.test(data.candidate.phone),'Telefonnummeret har landskode','Telefonnummeret mangler landskode','Skriv for eksempel +47 412 34 567. Utenlandske arbeidsgivere og søknadssystemer trenger landskoden.','info');
  check(Boolean(data.candidate.linkedin),'LinkedIn-profil er oppgitt','LinkedIn-profil mangler','Rekrutterere sjekker nesten alltid LinkedIn. Legg inn full adresse, og hold profilen lik CV-en.','info');
  if(!headline)check(false,'','Faglig overskrift mangler','Skriv en kort linje under navnet, gjerne med stillingens egne ord: for eksempel «Sykepleier | Akuttmottak | Legemiddelhåndtering».');
  else{const roleWords=role.split(' ').filter(item=>item.length>3);check(!role||roleWords.some(item=>headline.includes(item.slice(0,Math.max(4,item.length-3))))||data.target.keywords.some(item=>contains(headline,item)),'Overskriften speiler stillingen','Overskriften bruker ikke stillingens ord','La overskriften bruke stillingens tittel eller to–tre fagord fra annonsen, uten å late som du har tittelen fra før.');}
  const count=words(summary).length;
  check(count>=30&&count<=70,`Profilen har ${count} ord`,count?`Profilen har ${count} ord`:'Profiltekst mangler','Sikt mot 30–70 ord: hvem du er faglig, hva du kan best, og hva du vil bidra med i denne jobben.');
  const match=keywordMatch(data);
  if(match.coverage===null)check(false,'','Ingen fagord fra annonsen er lagt inn','Lim inn annonsen under «Stillingen» og trykk «Foreslå fagord». Søknadssystemer og rekrutterere leter etter annonsens egne ord.','info');
  else check(match.coverage>=70,`${match.found.length} av ${match.found.length+match.missing.length} fagord fra annonsen finnes i CV-en`,`Bare ${match.found.length} av ${match.found.length+match.missing.length} fagord fra annonsen finnes i CV-en`,'Mangler: '+match.missing.join(', ')+'. Bruk annonsens ordlyd der du faktisk har erfaringen. Legg aldri inn ord du ikke kan stå for.');
  check(data.skills.length>=6,`${data.skills.length} kompetanser i kompetanseseksjonen`,data.skills.length?`Bare ${data.skills.length} kompetanser`:'Ingen kompetanser lagt inn','En kort, gruppert kompetanseliste tidlig i CV-en gjør fagordene lette å finne. Sikt mot 6–20.');
  if(data.skills.length>1)check(data.skills.every(item=>item.group.trim()),'Kompetansene er gruppert','Noen kompetanser mangler gruppe','Grupper dem, for eksempel «Fag», «Verktøy», «Språk og sertifikater». Grupperte linjer leses raskere.','info');
  if(bullets.length){
   const long=bullets.filter(item=>item.text.length>220);
   check(!long.length,'Punktene er korte nok til å skannes',`${long.length} punkt er for lange`,'Hold hvert punkt til én–to linjer. Gjelder: '+long.slice(0,2).map(item=>'«'+item.text.slice(0,45)+'…»').join(', '));
   const weak=bullets.filter(item=>WEAK_START.test(item.text.trim()));
   check(!weak.length,'Punktene starter med handling',`${weak.length} punkt starter med «Jeg» eller «Ansvarlig for»`,'Start med et verb: «Ledet …», «Innførte …», «Behandlet …». Gjelder: '+weak.slice(0,2).map(item=>'«'+item.text.slice(0,40)+'…»').join(', '));
   const starts=new Map();for(const item of bullets){const first=normal(item.text).split(' ')[0];if(first)starts.set(first,(starts.get(first)||0)+1);}
   const repeated=[...starts].filter(([,value])=>value>3).map(([key])=>key);
   check(!repeated.length,'Variert språk i punktene','Mange punkter starter med samme ord','Varier verbene. Gjentatt: '+repeated.join(', ')+'.','info');
   const quantified=data.experience.filter(job=>job.bullets.some(text=>RESULT.test(text)));
   check(quantified.length>=Math.min(2,data.experience.length),`${quantified.length} av ${data.experience.length} jobber viser et målbart resultat`,`${quantified.length} av ${data.experience.length} jobber viser et målbart resultat`,'Rekrutterere legger merke til resultater: «Oppnådde X, målt ved Y, ved å gjøre Z». Bruk bare tall du kan dokumentere.','info');
  }
  const text=[summary,...bullets.map(item=>item.text)].join(' '),cliches=CLICHES.filter(item=>normal(text).includes(normal(item)));
  check(!cliches.length,'Ingen tomme moteord','Moteord som ikke beviser noe',`Vis heller med eksempler enn å skrive: ${cliches.join(', ')}.`,'info');
  check(!SENSITIVE.test([summary,data.candidate.headline,...bullets.map(item=>item.text)].join(' ')),'Ingen unødvendige personopplysninger','Personopplysninger som ikke hører hjemme i en CV','Fjern fødselsdato, personnummer, sivilstand, religion og nasjonalitet.');
  const holes=gaps(data);
  check(!holes.length,'Ingen uforklarte opphold over seks måneder','Opphold over seks måneder i tidslinjen',holes.map(item=>`${item.months} måneder mellom ${item.after} og ${item.before}`).join('; ')+'. Forklar kort, for eksempel studier, permisjon eller omstilling.','info');
  if(data.experience.length)check(data.experience.every(job=>job.location.trim()),'Arbeidssted er oppgitt for jobbene','Arbeidssted mangler for noen jobber','Oppgi by for hver jobb. Mange søknadssystemer sorterer på sted.','info');
  check(pages<=2,`${pages} A4-side${pages===1?'':'r'}`,`${pages} A4-sider; maks to`,'Kort ned eller velg utformingen Kompakt; legg prosjektliste i eget vedlegg.');
  checks.push({level:'pass',text:'Én tekstspalte, vanlige overskrifter og ekte tekst',tip:''});
  return {checks,match};
 }
 function period(start,end,no){const format=value=>{const [year,month]=value.split('-');return month?month+'.'+year:year;};return start?format(start)+' – '+(end===null?(no?'nå':'Present'):end?format(end):''):'';}
 function plainText(data){
  const no=data.language!=='en',lines=[],section=title=>{lines.push('',title.toUpperCase());};
  lines.push(data.candidate.name,data.candidate.headline,[data.candidate.email,data.candidate.phone,data.candidate.city,data.candidate.linkedin].filter(Boolean).join(' | '));
  if(data.candidate.summary){section(no?'Profil':'Profile');lines.push(data.candidate.summary);}
  if(data.skills.length){section(no?'Kjernekompetanse':'Core skills');for(const [group,items]of groupSkills(data.skills))lines.push((group?group+': ':'')+items.join(', '));}
  if(data.experience.length){section(no?'Arbeidserfaring':'Professional experience');for(const job of sortJobs(data.experience)){lines.push('',job.title+' | '+job.company+(job.location?' | '+job.location:''),period(job.start,job.end,no));job.bullets.forEach(text=>lines.push('- '+text));}}
  if(data.education.length){section(no?'Utdanning':'Education');for(const item of sortEducation(data.education))lines.push(item.qualification+' | '+item.institution+' | '+[item.start,item.end].filter(Boolean).join(' – '));}
  if(data.certifications.length){section(no?'Kurs og sertifiseringer':'Courses and certifications');for(const item of sortCertifications(data.certifications))lines.push([item.name,item.issuer,item.year].filter(Boolean).join(' | '));}
  if(data.languages.length){section(no?'Språk':'Languages');lines.push(data.languages.map(item=>item.name+': '+item.level).join(' | '));}
  return lines.join('\n').replace(/\n{3,}/g,'\n\n').trim()+'\n';
 }
 function groupSkills(skills){const groups=new Map();for(const item of skills){const key=item.group.trim();if(!groups.has(key))groups.set(key,[]);groups.get(key).push(item.name.trim());}return [...groups];}
 function sortJobs(jobs){return [...jobs].sort((a,b)=>(b.end??'9999-99').localeCompare(a.end??'9999-99')||b.start.localeCompare(a.start));}
 function sortEducation(items){return [...items].sort((a,b)=>b.end.localeCompare(a.end));}
 function sortCertifications(items){return [...items].sort((a,b)=>(b.year||'0').localeCompare(a.year||'0'));}
 const api={suggestKeywords,keywordMatch,audit,gaps,plainText,period,groupSkills,sortJobs,sortEducation,sortCertifications,normal};
 root.CVRecruiter=api;if(typeof module!=='undefined'&&module.exports)module.exports=api;
})(typeof globalThis!=='undefined'?globalThis:this);
