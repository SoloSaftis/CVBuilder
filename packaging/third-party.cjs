// Writes THIRD-PARTY.txt: every crate in the build with its licence, from `cargo metadata`, then the
// full text of every licence whose notice must go with each copy of the program (the word lists,
// fonts and design remakes built into the exe). Every package ships this one file.
const {execSync}=require('node:child_process'),fs=require('node:fs');
const meta=JSON.parse(execSync('cargo metadata --format-version 1 --locked',{cwd:__dirname+'/..',maxBuffer:1<<28}).toString());
const ours=new Set(['cv-core','cv-ai','cv-mcp','cvbuilder-app','cvbuilder-windows','cvbuilder-linux','cvbuilder-macos']);
const rows=meta.packages.filter(p=>!ours.has(p.name)).map(p=>`${p.name} ${p.version}  —  ${p.license||'see '+(p.repository||'crate')}`).sort();
// The commit of wooorm/dictionaries the bundled and downloaded dictionaries come from.
const COMMIT=/pub const COMMIT: &str = "([0-9a-f]{40})"/.exec(fs.readFileSync(__dirname+'/../crates/cv-core/src/spelling.rs','utf8'))[1];
const dicts=`https://github.com/wooorm/dictionaries/tree/${COMMIT}/dictionaries`;
const head=`CVBuilder bundles these open-source components. Each keeps its own licence.
WordCraft, Craft AI integrations and Craft libraries: MIT OR Apache-2.0, © the ArtCraft/storytold contributors.
DM Sans font: SIL Open Font License 1.1, © 2014-2017 Indian Type Foundry, © 2019 Google LLC (full text at the end of this file).
Phosphor icons: MIT, © 2020-2021 Phosphor Icons (full text at the end of this file).
BIZ UDPGothic (© Morisawa Inc.), Noto Sans CJK SC, Noto Sans Arabic, Hebrew, Thai, Devanagari, Georgian, Symbols, Symbols 2 and the other Noto Sans scripts listed below (© Google LLC and Adobe), NanumGothic (© NAVER Corporation): SIL Open Font License 1.1, through craft-fonts (https://github.com/storytold/craft-fonts); each licence text is at the end of this file.
Designs marked «Reactive Resume» are remakes of templates from Reactive Resume (MIT, © 2026 Amruth Pillai); designs marked «RenderCV» are remakes of RenderCV themes (MIT, © 2023 to present Sina Atalay and individual contributors). Licence texts at the end of this file.
Spelling dictionaries for English (US and UK; from SCOWL, MIT AND BSD, © Kevin Atkinson and others), French (MPL-2.0, Grammalecte, © Olivier R.) and Spanish (RLA-ES, used under MPL-1.1 from GPL-3.0 OR LGPL-3.0 OR MPL-1.1) come with the app, taken unchanged from wooorm/dictionaries; their licence texts are in full at the end of this file. The French and Spanish dictionaries are covered by the Mozilla Public License; their source form, which CVBuilder uses unchanged, is at ${dicts}/fr and ${dicts}/es. Dictionaries for other languages are downloaded from ${dicts} only when the person asks, keep their own licences (named in the app) and are not part of this distribution. The spellbook crate that reads them is MPL-2.0.

`;
const A=__dirname+'/../assets/';
const fontDirs=fs.readdirSync(A+'craft-fonts/fonts',{withFileTypes:true}).filter(d=>d.isDirectory()&&fs.existsSync(A+`craft-fonts/fonts/${d.name}/OFL.txt`)).map(d=>d.name).sort();
const texts=[
  ['Spelling dictionary, English (US), from SCOWL','licenses/dictionaries/en.txt'],
  ['Spelling dictionary, English (UK), from SCOWL','licenses/dictionaries/en-GB.txt'],
  ['Spelling dictionary, French (Grammalecte)','licenses/dictionaries/fr.txt'],
  ['Spelling dictionary, Spanish (RLA-ES)','licenses/dictionaries/es.txt'],
  ['Reactive Resume','licenses/Reactive-Resume-LICENSE.txt'],
  ['RenderCV','licenses/RenderCV-LICENSE.txt'],
  ['DM Sans font','fonts/OFL-DMSans.txt'],
  ['Phosphor icons','fonts/Phosphor-LICENSE.txt'],
  ...fontDirs.map(n=>[`Font ${n}`,`craft-fonts/fonts/${n}/OFL.txt`]),
];
// Identical texts are printed once, under every name they cover.
const groups=new Map();
for(const [name,file] of texts){
  const t=fs.readFileSync(A+file,'utf8').replace(/\r\n/g,'\n').trim();
  if(!groups.has(t))groups.set(t,[]);
  groups.get(t).push(name);
}
const rule=c=>c.repeat(72)+'\n';
const full='\n\n'+rule('=')+'FULL LICENCE TEXTS\n'+rule('=')+[...groups].map(([t,names])=>'\n'+rule('-')+names.join('; ')+'\n'+rule('-')+'\n'+t+'\n').join('');
fs.writeFileSync(process.argv[2],head+[...new Set(rows)].join('\n')+'\n'+full);console.log(rows.length+' components listed, '+groups.size+' licence texts in full');
