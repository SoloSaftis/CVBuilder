(function(root){
 'use strict';
 // A fictional example so the builder opens in a working state. Every person and company is invented.
 function example(contract){
  const value=contract.blank();
  value.candidate={name:'Ingrid Haugland',email:'ingrid.haugland@example.no',phone:'+47 400 00 000',city:'Trondheim, Norge',headline:'Prosjektkoordinator | Innkjøp | Kundeoppfølging',linkedin:'https://www.linkedin.com/in/eksempel',summary:'Prosjektkoordinator med sju års erfaring fra bygg og eiendom. Planlegger leveranser, følger opp leverandører og holder kunder oppdatert fra kontrakt til overlevering. Har redusert forsinkelser med faste ukeplaner og tydelige avviksrutiner, og vil bruke dette som koordinator i en voksende entreprenør.',photo:''};
  value.target={role:'Prosjektkoordinator',company:'Eksempel Entreprenør AS',url:'',description:'Vi søker en strukturert prosjektkoordinator med erfaring fra bygg. Du vil ha ansvar for fremdriftsplaner, innkjøp, oppfølging av underleverandører, HMS-dokumentasjon og kundekontakt. Erfaring med Excel, ISY ProArc og Power BI er en fordel.',keywords:['prosjektkoordinator','fremdriftsplan','innkjøp','underleverandører','HMS','kundekontakt','Excel','Power BI','ISY ProArc']};
  value.skills=[
   ['Prosjekt','fremdriftsplaner'],['Prosjekt','innkjøp og bestilling'],['Prosjekt','oppfølging av underleverandører'],['Prosjekt','HMS-dokumentasjon'],
   ['Verktøy','Excel (pivot, oppslag)'],['Verktøy','Power BI'],['Verktøy','MS Project'],
   ['Kunde','kundekontakt og avklaringer'],['Kunde','overleveringsdokumentasjon']
  ].map(([group,name])=>({group,name,evidence:'Eksempel: beskriv hvor du har brukt dette.',confirmed:true}));
  value.experience=[
   {id:'ex_1',company:'Fjellvegg Bygg AS (fiktiv)',title:'Prosjektkoordinator',location:'Trondheim',start:'2021-03',end:null,bullets:['Koordinerer innkjøp og leveranser for 6 boligprosjekter samtidig, med til sammen 140 leiligheter.','Innførte felles ukeplan for underleverandører og reduserte forsinkede leveranser med 30 % på ett år.','Utarbeider HMS- og FDV-dokumentasjon og leder overleveringsmøter med kunder.'],evidence:'Eksempel.',confirmed:true},
   {id:'ex_2',company:'Byggvare Midt AS (fiktiv)',title:'Innkjøpsmedarbeider',location:'Trondheim',start:'2018-01',end:'2021-02',bullets:['Fulgte opp 40 faste leverandører på pris, leveringstid og reklamasjoner.','Bygget rapport i Power BI som ga ledelsen ukentlig oversikt over lagerverdi.'],evidence:'Eksempel.',confirmed:true}
  ];
  value.education=[{qualification:'Bachelor i økonomi og administrasjon',institution:'Eksempel høgskole',start:'2015',end:'2018'}];
  value.certifications=[{name:'Prosjektledelse, grunnkurs',issuer:'Eksempel kursleverandør',year:'2022'}];
  value.languages=[{name:'Norsk',level:'morsmål'},{name:'Engelsk',level:'flytende'}];
  value.interests=[{id:'ex_i1',label:'Langrenn',status:'current',description:'Eksempel.',category:'sport',tags:['ski'],confirmed:true,specificity:1},{id:'ex_i2',label:'Frivillig i idrettslag',status:'current',description:'Eksempel.',category:'community',tags:['frivillig'],confirmed:true,specificity:2}];
  value.options.interest_ids=['ex_i1','ex_i2'];
  return value;
 }
 const api={example};root.CVExample=api;if(typeof module!=='undefined'&&module.exports)module.exports=api;
})(typeof globalThis!=='undefined'?globalThis:this);
