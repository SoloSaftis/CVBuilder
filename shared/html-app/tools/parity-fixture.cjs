// Records what the HTML builder says about a few projects, for the Rust app's parity test.
const fs=require('node:fs'),path=require('node:path');
const contract=require('../contract.js'),recruiter=require('../recruiter-checks.js'),example=require('../example.js');
// Fictional data only: this fixture is committed with the Rust code.
const long=example.example(contract);
long.candidate.photo='';long.options.photo=false;
long.experience.push(
 {id:'ex_3',company:'Nordlys Montasje AS (fiktiv)',title:'Montør',location:'',start:'2012',end:'2015-06',bullets:['Jeg var ansvarlig for montasje på 12 byggeplasser.','Monterte og testet tavler.','Monterte ventilasjon.','Monterte belysning i 3 skoler.','Monterte solcellepaneler.'],evidence:'',confirmed:true},
 {id:'ex_4',company:'Fjordkyst Elektro AS (fiktiv)',title:'Lærling',location:'Ålesund',start:'2009-08',end:'2011-06',bullets:['Født 1990. Lærte faget hos dyktige og motiverte kolleger, og var løsningsorientert.'],evidence:'',confirmed:true});
long.target.keywords.push('TwinCAT','C/C++','maskinsikkerhet');
long.target.description+=' TwinCAT 3 og CODESYS er en fordel. Legemiddelhåndtering. Legemiddelhåndtering.';
const projects={long,example:example.example(contract),blank:contract.blank()};
const out={};
for(const [name,value]of Object.entries(projects)){const v=contract.upgrade(value);v.approvals.facts_confirmed=false;const audit=recruiter.audit(v,{pages:2});out[name]={project:v,ready:contract.ready(v),checks:audit.checks.map(item=>({level:item.level,text:item.text})),match:audit.match,plain:recruiter.plainText(v)};}
const target=path.resolve(__dirname,'../../cv-core/tests/parity.json');fs.writeFileSync(target,JSON.stringify(out,null,1));console.log('wrote',target);
