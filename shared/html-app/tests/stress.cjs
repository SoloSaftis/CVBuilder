// Stress test: a full real-world project in every layout. Exports Word and PDF and records the
// page count in the preview and in the PDF. Usage: node tests/stress.cjs project.json out-folder
const {chromium}=require('playwright'),{pathToFileURL}=require('node:url'),path=require('node:path'),fs=require('node:fs');
const layouts=require('../layouts.js');
const [input,out]=process.argv.slice(2);fs.mkdirSync(out,{recursive:true});
(async()=>{
 const browser=await chromium.launch({channel:'msedge',headless:true}),result={};
 try{
  for(const id of layouts.ids){
   const context=await browser.newContext({viewport:{width:1500,height:1000},acceptDownloads:true}),page=await context.newPage(),errors=[];
   page.on('pageerror',error=>errors.push(error.message));
   await page.goto(pathToFileURL(path.resolve(__dirname,'../dist/CVBuilder.html')).href);await page.waitForFunction(()=>window.CVFamily&&window.jspdf);
   const value=JSON.parse(fs.readFileSync(input,'utf8'));value.options.layout=id;
   await page.evaluate(v=>CVFamily.setData(v),value);await page.locator('#approval').check();
   const preview=await page.locator('.sheet').count(),blocked=await page.locator('#word').isDisabled();
   const entry={preview,blocked,errors};
   if(!blocked){
    for(const [button,ext] of [['#word','.docx'],['#pdf','.pdf']]){const download=page.waitForEvent('download');await page.locator(button).click();await(await download).saveAs(path.join(out,id+ext));}
    entry.pdf=await page.evaluate(()=>CVFamily.exportPdf().pages);
   }else entry.reason=await page.locator('#validation').innerText();
   result[id]=entry;await context.close();
  }
 }finally{await browser.close();}
 fs.writeFileSync(path.join(out,'stress.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
})().catch(error=>{console.error(error);process.exitCode=1;});
