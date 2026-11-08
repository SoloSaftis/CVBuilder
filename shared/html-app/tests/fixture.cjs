const contract=require('../contract.js');
module.exports=()=>{
 const value=contract.blank();
 value.candidate={name:'Alex Nord',email:'alex@example.org',phone:'+47 55501234',city:'Oslo, Norge',headline:'Kundeservice og administrasjon',linkedin:'',summary:'Serviceorientert medarbeider med erfaring fra kundebehandling, koordinering og administrative oppgaver. Har jobbet med henvendelser, dokumentasjon og oppfølging av leveranser. Ønsker å bruke denne erfaringen i en rolle med tett kundekontakt og tydelig samarbeid.',photo:''};
 value.target={role:'Kunderådgiver',company:'Demo Virksomhet AS',url:'https://example.org/job',description:'Kundeservice, koordinering og administrasjon.',keywords:['kundeservice','koordinering']};
 value.experience=[{id:'job_1',company:'Demo Service AS',title:'Kundebehandler',location:'Oslo',start:'2022-01',end:null,bullets:['Besvarte kundehenvendelser og fulgte opp leveranser.','Oppdaterte dokumentasjon og koordinerte oppgaver med kolleger.'],evidence:'Syntetisk testeksempel; ingen reell kandidat eller arbeidsgiver.',confirmed:true}];
 value.education=[{qualification:'Administrasjon',institution:'Demo fagskole',start:'2019',end:'2021'}];
 value.skills=[{group:'Kundeservice',name:'Kundeoppfølging',evidence:'Syntetisk arbeidsoppgave i testeksemplet.',confirmed:true}];
 value.languages=[{name:'Norsk',level:'Flytende'},{name:'Engelsk',level:'Godt arbeidsnivå'}];
 value.interests=[{id:'i_1',label:'Turgåing',status:'current',description:'Kun syntetiske testdata.',category:'outdoors',tags:['natur'],confirmed:true,specificity:1},{id:'i_2',label:'Svømming',status:'former',description:'Kun syntetiske testdata.',category:'sport',tags:['swimming'],confirmed:true,specificity:2}];
 value.options.interest_ids=['i_2'];value.approvals.facts_confirmed=true;return value;
};
