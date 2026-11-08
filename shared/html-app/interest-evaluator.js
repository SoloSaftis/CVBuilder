(function(root){
 'use strict';
 const normal=value=>String(value||'').normalize('NFKC').toLocaleLowerCase().replace(/[^\p{L}\p{N}+#]+/gu,' ').trim();
 const words=value=>new Set(normal(value).split(/\s+/).filter(Boolean));
 const groups=[['technology','teknologi','technical','teknisk','automasjon','automation','programmering','programming','software','ingeniør','engineer'],['outdoors','friluftsliv','natur','nature','hiking','turgåing'],['sport','sports','idrett','trener','coach'],['maritime','maritim','marine','shipping'],['football','fotball'],['ai','kunstig intelligens','artificial intelligence'],['community','frivillig','volunteer']];
 function expand(values){const result=new Set(values.map(normal).filter(Boolean));for(const group of groups)if(group.some(term=>result.has(term)))for(const term of group)result.add(term);return result;}
 function match(tags,text){const tokens=words(text);return [...expand(tags)].filter(tag=>tag.includes(' ')?normal(text).includes(tag):tokens.has(tag));}
 function evaluate(interests,target,contexts=[],today=new Date().toISOString().slice(0,10)){
  const job=[target.role,target.title,target.company,target.description,...(target.keywords||[])].join(' ');
  return interests.map((item,index)=>{
   if(!item.confirmed)return {...item,recommendation:'needs_confirmation',rank:-100,reasons:['Må bekreftes av kandidaten før bruk.'],source_urls:[]};
   const candidateTags=[...(item.tags||[]),item.category],direct=match(candidateTags,job),reasons=[],source_urls=[];let rank=0;
   if(direct.length){rank+=6;reasons.push('Interessen har en tematisk kobling til stillingen; den er ikke dokumentasjon på fagkompetanse.');}
   for(const context of contexts){
    const date=Date.parse(context.checked_on),age=(Date.parse(today)-date)/86400000;
    const eligible=context.verified===true&&['employer_activity','professional_bio'].includes(context.scope)&&/^https:\/\//.test(context.source_url||'')&&Number.isFinite(age)&&age>=0&&age<=180;
    if(eligible&&!source_urls.includes(context.source_url)&&match(candidateTags,[context.topic,...(context.tags||[])].join(' ')).length)source_urls.push(context.source_url);
   }
   if(source_urls.length){rank+=1;reasons.push('En nylig, eksplisitt bekreftet offentlig opplysning gir kontekst. Dette er ingen garanti for personlig kjemi.');}
   rank+=Math.max(0,Math.min(2,item.specificity||0));
   if(!direct.length&&!source_urls.length)reasons.push('Mulig personlig samtalestarter. Ingen dokumentert kobling til arbeidsplassen.');
   if(item.status==='former')reasons.push('Behold tidligere/former i formuleringen; ikke beskriv dette som nåværende aktivitet.');
   if(item.status==='interest')reasons.push('Beskriv som interesse, ikke som sertifisering eller dokumentert aktivitet.');
   return {...item,rank,recommendation:'reserve',reasons,source_urls,_index:index};
  }).sort((a,b)=>b.rank-a.rank||(a._index??0)-(b._index??0));
 }
 function recommend(ranked,{maxItems=3,maxWords=35,language='no'}={}){
  const selected=[];let count=0;
  for(const item of ranked){if(!item.confirmed||selected.length>=maxItems)continue;const text=item['cv_'+language]||item.label;const amount=text.trim().split(/\s+/).filter(Boolean).length;if(amount&&count+amount<=maxWords){selected.push(item);count+=amount;}}
  return selected;
 }
 const api={evaluate,recommend};root.CVInterests=api;if(typeof module!=='undefined'&&module.exports)module.exports=api;
})(typeof globalThis!=='undefined'?globalThis:this);
