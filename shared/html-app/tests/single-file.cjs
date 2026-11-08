// QA for dist/CVBuilder.html opened straight from disk (file://), as a friend would use it.
// Usage: node tests/single-file.cjs [output-folder]
const {chromium}=require('playwright'),{pathToFileURL}=require('node:url'),path=require('node:path'),fs=require('node:fs'),os=require('node:os'),assert=require('node:assert/strict');
const fixture=require('./fixture.cjs'),layouts=require('../layouts.js');
const out=process.argv[2]||fs.mkdtempSync(path.join(os.tmpdir(),'cvbuilder-single-'));fs.mkdirSync(out,{recursive:true});
(async()=>{
 const browser=await chromium.launch({channel:process.env.CVSTUDIO_BROWSER_CHANNEL||(process.platform==='win32'?'msedge':undefined),headless:true});
 const report={layouts:{},errors:[],network:[]};
 try{
  const context=await browser.newContext({viewport:{width:1500,height:1000},acceptDownloads:true,permissions:['clipboard-read','clipboard-write']}),page=await context.newPage();
  page.on('pageerror',error=>report.errors.push(error.message));page.on('console',msg=>{if(msg.type()==='error')report.errors.push(msg.text());});
  page.on('request',request=>{if(/^https?:/.test(request.url()))report.network.push(request.url());});
  const url=pathToFileURL(path.resolve(__dirname,'../dist/CVBuilder.html')).href;
  await page.goto(url);await page.waitForFunction(()=>window.CVFamily&&window.docx&&window.jspdf);
  assert.equal(await page.evaluate(()=>CVFamily.isExample()),true,'First open shows the example');assert.equal(await page.locator('#confirm-bar').isVisible(),false,'No confirmation until a download is asked for');
  await page.screenshot({path:path.join(out,'01_first_open.png')});
  // Example data must be fictional and exportable after approval.
  await page.locator('#approval').check();assert.equal(await page.locator('#word').isDisabled(),false,'Approved example exports');
  // Every layout: preview fits, and Word, PDF and text all export with the same content.
  const value=fixture();value.candidate.linkedin='https://www.linkedin.com/in/alex-nord';value.certifications=[{name:'Kundeservicekurs',issuer:'Demo',year:'2023'}];
  for(const id of layouts.ids){
   const item=structuredClone(value);item.options.layout=id;
   const page=await context.newPage();page.on('pageerror',error=>report.errors.push(error.message));await page.goto(url);await page.waitForFunction(()=>window.CVFamily&&window.jspdf);
   await page.evaluate(v=>CVFamily.setData(v),item);await page.locator('#approval').check();
   assert.equal(await page.locator('#word').isDisabled(),false,id+' export enabled');
   const stem=path.join(out,'layout_'+id);
   await page.locator('#text').click();const copied=await page.evaluate(()=>navigator.clipboard.readText());assert.match(copied,/ARBEIDSERFARING/,id+': text copied');fs.writeFileSync(stem+'.txt',copied);
   for(const [button,ext] of [['#word','.docx'],['#pdf','.pdf']]){const download=page.waitForEvent('download',{timeout:15000});await page.locator(button).click();await(await download).saveAs(stem+ext);}
   const pdf=await page.evaluate(()=>CVFamily.exportPdf().pages);
   await page.locator('.sheet').first().screenshot({path:stem+'_preview.png'});
   report.layouts[id]={previewPages:await page.locator('.sheet').count(),pdfPages:pdf,docxBytes:fs.statSync(stem+'.docx').size,pdfBytes:fs.statSync(stem+'.pdf').size};
   const pdfText=fs.readFileSync(stem+'.pdf','latin1');assert.ok(!/\/Subtype\s*\/Image/.test(pdfText),id+': PDF has no images without a photo');
  }
  // Remembered draft survives a reload; approval does not.
  await page.evaluate(v=>CVFamily.setData(v),value);await page.getByLabel('Fullt navn',{exact:true}).fill('Alex Husket');
  await page.reload();await page.waitForFunction(()=>window.CVFamily);
  assert.equal(await page.evaluate(()=>CVFamily.getData().candidate.name),'Alex Husket','Draft restored from this browser');
  assert.equal(await page.locator('#approval').isChecked(),false,'Approval is never remembered');
  await page.locator('#remember').uncheck();await page.reload();await page.waitForFunction(()=>window.CVFamily);
  assert.equal(await page.evaluate(()=>CVFamily.getData().candidate.name),'Alex Husket','Unticking Husk keeps what is saved');await page.getByRole('button',{name:'Slett lagret utkast'}).count();
  // New blank CV keeps the chosen layout.
  await page.locator('#layout-nordic').check({force:true});await page.locator('#new-cv').click();
  assert.equal(await page.evaluate(()=>CVFamily.getData().options.layout),'nordic');assert.equal(await page.evaluate(()=>CVFamily.getData().candidate.name),'');
  await page.screenshot({path:path.join(out,'02_blank.png')});
  // Reviewer blockers: a badly typed year never deletes the draft; replacing the CV can be undone.
  await page.locator('#remember').check();await page.evaluate(v=>CVFamily.setData(v),value);
  await page.locator('#new-cv').click();assert.equal(await page.evaluate(()=>CVFamily.getData().candidate.name),'');
  await page.getByRole('button',{name:'Angre'}).click();assert.equal(await page.evaluate(()=>CVFamily.getData().candidate.name),'Alex Nord','Angre restores the CV');
  const year=page.getByLabel('Fra · år',{exact:true}).first();await year.fill('aug. 2016');await year.blur();
  assert.equal(await page.evaluate(()=>CVFamily.getData().education[0].start),'2016','Year read from aug. 2016');
  await year.fill('sekstenåtte');await year.blur();assert.equal(await page.locator('.field-error:visible').count(),1,'Unreadable year is flagged');
  const month=page.getByLabel('Fra · måned og år',{exact:true}).first();await month.fill('09.2021');await month.blur();
  assert.equal(await page.evaluate(()=>CVFamily.getData().experience[0].start),'2021-09','Month typed as 09.2021');
  await page.reload();await page.waitForFunction(()=>window.CVFamily);assert.equal(await page.evaluate(()=>CVFamily.getData().candidate.name),'Alex Nord','Draft survives a reload after a bad entry');
  const confirmDownload=page.waitForEvent('download');await page.locator('#word').click();await page.locator('#confirm-yes').click();await confirmDownload;
  await page.locator('#project-file').setInputFiles({name:'cv.docx',mimeType:'application/octet-stream',buffer:Buffer.from('PK')});assert.match(await page.locator('#status').innerText(),/ikke et CVBuilder-utkast/);
  // Round 5: bare years, «nå», restoring after a reload, unticking «Husk» keeps the draft, Esc.
  await page.locator('#remember').check();await page.evaluate(v=>CVFamily.setData(v),value);
  const start=page.getByLabel('Fra · måned og år',{exact:true}).first();await start.fill('2010');await start.blur();
  assert.equal(await page.evaluate(()=>CVFamily.getData().experience[0].start),'2010','Bare year accepted');
  await page.locator('#new-cv').click();await page.reload();await page.waitForFunction(()=>window.CVFamily);
  assert.equal(await page.locator('#restore').isVisible(),true,'Restore offered after reload');await page.locator('#restore').click();
  assert.equal(await page.evaluate(()=>CVFamily.getData().candidate.name),'Alex Nord','Previous CV restored after reload');
  await page.locator('#remember').uncheck();await page.reload();await page.waitForFunction(()=>window.CVFamily);
  assert.equal(await page.evaluate(()=>CVFamily.getData().candidate.name),'Alex Nord','Unticking Husk does not delete the draft');
  await page.locator('#remember').check();
  const confirmEsc=page.locator('#word');await confirmEsc.click();await page.keyboard.press('Escape');assert.equal(await page.locator('#confirm-bar').isVisible(),false,'Esc closes the confirmation');
  // Phone width and dark mode.
  await page.locator('#load-example').click();
  const phone=await browser.newContext({viewport:{width:390,height:844},colorScheme:'dark',isMobile:true}),mobile=await phone.newPage();
  mobile.on('pageerror',error=>report.errors.push('mobile: '+error.message));
  await mobile.goto(url);await mobile.waitForFunction(()=>window.CVFamily);
  {const long=require('../example.js').example(require('../contract.js'));for(let i=0;i<6;i++)long.experience.push({...long.experience[0],id:'long_'+i,start:String(2000+i)+'-01',end:String(2000+i)+'-12',bullets:Array(4).fill('Ekstra fiktivt testpunkt som gjør CV-en lengre, for å kontrollere at telefonvisningen teller sider riktig.')});
   await mobile.evaluate(v=>CVFamily.setData(v),long);const phonePages=await mobile.locator('.sheet').count();
   await page.evaluate(v=>CVFamily.setData(v),long);const desktopPages=await page.locator('.sheet').count();
   report.pageCountPhoneVsDesktop=[phonePages,desktopPages];assert.equal(phonePages,desktopPages,'Phone counts pages like desktop');
   await mobile.locator('#load-example').click();}
  report.mobileOverflow=await mobile.evaluate(()=>document.documentElement.scrollWidth-window.innerWidth);
  await mobile.screenshot({path:path.join(out,'03_phone_dark.png')});
  await mobile.locator('#show-cv').click();await mobile.waitForTimeout(200);report.mobileSheetOverflow=await mobile.evaluate(()=>{const s=document.querySelector('.sheet').getBoundingClientRect();return Math.round(s.right-window.innerWidth);});await mobile.screenshot({path:path.join(out,'05_phone_cv.png')});assert.ok(report.mobileSheetOverflow<=0,'CV sheet fits the phone screen');
  const dark=await browser.newContext({viewport:{width:1500,height:1000},colorScheme:'dark'}),desk=await dark.newPage();
  await desk.goto(url);await desk.waitForFunction(()=>window.CVFamily);await desk.screenshot({path:path.join(out,'04_desktop_dark.png')});
  assert.deepEqual(report.network,[],'No network requests');assert.deepEqual(report.errors,[],'No browser errors');
  assert.ok(report.mobileOverflow<=0,'No sideways scrolling on a phone');
  fs.writeFileSync(path.join(out,'report.json'),JSON.stringify(report,null,2));
  console.log('PASS single file: '+JSON.stringify(report.layouts)+'\nQA files: '+out);
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});
