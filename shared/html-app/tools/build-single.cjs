// Builds dist/CVBuilder.html: one self-contained file with every style and script inlined.
// Double-click it in Edge or Chrome. No server, install or internet connection needed.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const root=path.resolve(__dirname,'..'),out=path.join(root,'dist');
let html=fs.readFileSync(path.join(root,'index.html'),'utf8');
const references=(html.match(/<script src="[^"]+"><\/script>|<link rel="stylesheet" href="[^"]+">/g)||[]).length;let inlined=0;
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
html=html.replace(/<link rel="stylesheet" href="([^"]+)">/g,(_,file)=>{inlined++;return `<style>\n${read(file)}</style>`;});
html=html.replace(/<script src="([^"]+)"><\/script>/g,(_,file)=>{inlined++;return `<script>/* ${file} */\n${read(file).replace(/<\/script/gi,'<\\/script')}\n</script>`;});
if(inlined!==references||!references)throw Error(`Inlined ${inlined} of ${references} file references`);
const version=new Date().toISOString().slice(0,10);
html=html.replace('<title>CVBuilder Familie</title>',`<title>CVBuilder Familie</title><meta name="generator" content="CVBuilder Familie ${version}">`);
fs.mkdirSync(out,{recursive:true});
const target=path.join(out,'CVBuilder.html');fs.writeFileSync(target,html);
const hash=crypto.createHash('sha256').update(html).digest('hex');
fs.writeFileSync(path.join(out,'CVBuilder.html.sha256'),hash+'  CVBuilder.html\n');
console.log(`Built ${path.relative(root,target)} (${(html.length/1024/1024).toFixed(2)} MB), sha256 ${hash.slice(0,16)}…`);
