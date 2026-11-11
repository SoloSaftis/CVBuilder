const fs=require('fs');
const path=require('path');
const s=fs.readFileSync(path.resolve(__dirname,'../recruiter-checks.js'),'utf8');
let out='// Generated from recruiter-checks.js by web/tools/rust-wordlists.cjs; keep in step with the HTML builder.\n';
for(const name of ['STOP','DICTIONARY','COMMON']){
 const start=s.indexOf(' const '+name+'=');const open=s.indexOf('[',start);const close=s.indexOf(']',open);
 const items=Function('return '+s.slice(open,close+1))();
 out+=`pub(crate) const ${name}: &[&str] = &[${items.map(x=>JSON.stringify(x)).join(', ')}];\n`;
}
fs.writeFileSync(path.resolve(__dirname,'../../crates/cv-core/src/wordlists.rs'),out);console.log(out.length);
