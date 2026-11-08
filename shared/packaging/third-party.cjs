// Writes THIRD-PARTY.txt: every crate in the build with its licence, from `cargo metadata`.
const {execSync}=require('node:child_process'),fs=require('node:fs');
const meta=JSON.parse(execSync('cargo metadata --format-version 1 --locked',{cwd:__dirname+'/../..',maxBuffer:1<<28}).toString());
const ours=new Set(['cv-core','cv-ai','cvbuilder-app','cvbuilder-windows','cvbuilder-linux','cvbuilder-macos']);
const rows=meta.packages.filter(p=>!ours.has(p.name)).map(p=>`${p.name} ${p.version}  —  ${p.license||'see '+(p.repository||'crate')}`).sort();
const head='CVBuilder bundles these open-source components. Each keeps its own licence.\nWordCraft, Craft AI integrations and Craft libraries: MIT OR Apache-2.0, © the ArtCraft/storytold contributors.\n\n';
fs.writeFileSync(process.argv[2],head+[...new Set(rows)].join('\n')+'\n');console.log(rows.length+' components listed');
