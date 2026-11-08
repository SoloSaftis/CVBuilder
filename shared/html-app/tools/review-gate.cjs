'use strict';
const fs=require('node:fs'),crypto=require('node:crypto'),path=require('node:path');
const contract=require('../contract.js');
const roles={recruitment_intelligence:[],candidate_intelligence:[],technical_specialist:['candidate_intelligence','recruitment_intelligence'],achievement_strategist:['candidate_intelligence'],ats_strategist:['technical_specialist','achievement_strategist'],bilingual_editor:['ats_strategist'],creative_director:['bilingual_editor'],independent_qa_auditor:['creative_director'],cv_manager:['independent_qa_auditor']};
const canonical=value=>value&&typeof value==='object'?Array.isArray(value)?value.map(canonical):Object.fromEntries(Object.keys(value).sort().map(key=>[key,canonical(value[key])])):value;
const hash=value=>crypto.createHash('sha256').update(JSON.stringify(canonical(value))).digest('hex');
function check(input,reports){
 const errors=contract.ready(input);if(errors.length)throw Error(errors.join('\n'));
 if(!Array.isArray(reports)||reports.length!==Object.keys(roles).length)throw Error('Alle ni rapporter kreves.');
 const byRole=new Map();
 for(const report of reports){if(!report||!Object.hasOwn(roles,report.role)||byRole.has(report.role))throw Error('Ukjent eller duplisert rolle.');byRole.set(report.role,report);}
 for(const [role,dependencies]of Object.entries(roles)){
  const report=byRole.get(role);
  const keys=['role','input_sha256','status','reviewer','findings','blockers','dependencies'];
  if(Object.keys(report).sort().join()!==keys.sort().join()||typeof report.reviewer!=='string'||report.reviewer.length>200||!Array.isArray(report.findings)||report.findings.length>50||report.findings.some(text=>typeof text!=='string'||text.length>2000))throw Error('Feil rapportformat: '+role);
  if(report.input_sha256!==hash(input)||report.status!=='approved'||!Array.isArray(report.blockers)||report.blockers.length||!Array.isArray(report.findings)||!report.reviewer?.trim())throw Error('Utdatert, ufullstendig eller blokkert rapport: '+role);
  if(!report.dependencies||typeof report.dependencies!=='object'||Array.isArray(report.dependencies)||Object.keys(report.dependencies).sort().join()!==[...dependencies].sort().join())throw Error('Feil avhengigheter: '+role);
  for(const dependency of dependencies)if(report.dependencies[dependency]!==hash(byRole.get(dependency)))throw Error('Endret avhengig rapport: '+role);
 }
 return {status:'approved',input_sha256:hash(input),report_sha256:hash(reports),warning:'Lokale manuelle godkjenninger; dette autentiserer ikke uavhengige reviewer-identiteter.'};
}
function load(file){if(fs.statSync(file).size>3000000)throw Error('Filen er større enn 3 MB.');return JSON.parse(fs.readFileSync(file,'utf8'));}
if(require.main===module){try{const [command,file,reports]=process.argv.slice(2),input=load(file);if(command==='prepare'){const errors=contract.validate(input);if(errors.length)throw Error(errors.join('\n'));console.log(JSON.stringify({input_sha256:hash(input),roles,mode:'connected-host-or-manual; no automatic model execution'},null,2));}else if(command==='verify')console.log(JSON.stringify(check(input,load(reports)),null,2));else throw Error('Bruk: node tools/review-gate.cjs prepare prosjekt.json | verify prosjekt.json rapporter.json');}catch(error){console.error(error.message);process.exitCode=1;}}
module.exports={roles,hash,check};
