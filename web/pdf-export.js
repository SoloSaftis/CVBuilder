(function(root){
 'use strict';
 // Builds a text-based A4 PDF with jsPDF. Every word is real, selectable text in reading order,
 // so applicant-tracking systems extract the same content as from the Word file.
 const MM=72/25.4,W=210*MM,H=297*MM,M=17*MM,BOTTOM=H-16*MM,TEXT=W-2*M;
 function build({data,items,layout,period,contact,linkedinText,heading}){
  const {jsPDF}=root.jspdf;const doc=new jsPDF({unit:'pt',format:'a4',compress:true});
  const L=layout.pdf,size=L.size,lead=size*1.38,gap=L.gap;let y=M;
  const color=rgb=>doc.setTextColor(rgb[0],rgb[1],rgb[2]);
  const font=(face,style='normal',points=size)=>{doc.setFont(face,style);doc.setFontSize(points);};
  const newPage=()=>{doc.addPage();y=M;};
  const ensure=height=>{if(y+height>BOTTOM)newPage();};
  const wrap=(text,width)=>doc.splitTextToSize(String(text),width);
  // Draws "bold label" followed by wrapped regular text that continues on the same line.
  function rich(boldPart,rest,{x=M,width=TEXT,after=2}={}){
   font(L.body,'bold');const labelWidth=boldPart?doc.getTextWidth(boldPart):0;font(L.body,'normal');
   const words=String(rest).split(' '),lines=[];let current='',limit=width-labelWidth;
   for(const word of words){const trial=current?current+' '+word:word;if(doc.getTextWidth(trial)>limit&&current){lines.push(current);current=word;limit=width;}else current=trial;}
   if(current)lines.push(current);
   ensure(lines.length*lead);
   lines.forEach((line,index)=>{const baseline=y+size;if(index===0&&boldPart){font(L.body,'bold');color(L.ink);doc.text(boldPart,x,baseline);font(L.body,'normal');doc.text(line,x+labelWidth,baseline);}else{color(L.ink);doc.text(line,x,baseline);}y+=lead;});
   y+=after*gap;
  }
  function twoColumn(left,right,{style='bold',tint=L.ink,points=size}={}){
   font(L.body,'normal',size-1);const rightWidth=right?doc.getTextWidth(right):0;
   font(L.body,style,points);const lines=wrap(left,TEXT-rightWidth-12);
   lines.forEach((line,index)=>{color(tint);doc.text(line,M,y+points);if(index===0&&right){font(L.body,'normal',size-1);color(L.muted);doc.text(right,W-M,y+points,{align:'right'});font(L.body,style,points);}y+=points*1.32;});
  }
  // Header
  const photo=data.options.photo&&data.candidate.photo,photoW=25*MM,photoH=32*MM,headerWidth=photo?TEXT-photoW-8*MM:TEXT;
  if(photo)doc.addImage(data.candidate.photo,'JPEG',W-M-photoW,M,photoW,photoH);
  font(L.head,L.nameStyle||'bold',L.name);color(L.ink);for(const line of wrap(data.candidate.name,headerWidth)){doc.text(line,M,y+L.name*.9);y+=L.name*1.12;}
  y+=3;font(L.body,'bold',size+1.5);color(L.accent);for(const line of wrap(data.candidate.headline,headerWidth)){doc.text(line,M,y+size+1.5);y+=(size+1.5)*1.3;}
  y+=3;font(L.body,'normal',size-1);color(L.muted);
  const parts=contact(),line=parts.join('  ·  ');
  for(const text of wrap(line,headerWidth)){doc.text(text,M,y+size-1);y+=(size-1)*1.4;}
  if(data.candidate.linkedin){const label=linkedinText(data.candidate.linkedin),index=line.indexOf(label);if(index>=0&&doc.getTextWidth(line)<=headerWidth){doc.link(M+doc.getTextWidth(line.slice(0,index)),y-(size-1)*1.4,doc.getTextWidth(label),size,{url:data.candidate.linkedin});}}
  if(photo)y=Math.max(y,M+photoH+2*MM);
  // Body
  let previous=null;
  for(const item of items.slice(1)){
   if(item.kind==='heading'){
    ensure(size*4.2);y+=(previous?9:6)*gap;font(L.head,'bold',size+.5);color(L.accent);
    const text=heading(item.text);
    if(L.headingStyle==='bar'){doc.setFillColor(L.accent[0],L.accent[1],L.accent[2]);doc.rect(M,y+1,2.4,size+1,'F');doc.text(text,M+8,y+size+.5);}
    else doc.text(text,M,y+size+.5);
    y+=size*1.5;
    if(L.headingStyle==='rule'){doc.setDrawColor(L.rule[0],L.rule[1],L.rule[2]);doc.setLineWidth(.6);doc.line(M,y,W-M,y);}
    y+=6*gap;
   }else if(item.kind==='skills')rich(item.group?item.group+': ':'',item.names.join(', '));
   else if(item.kind==='job'){
    font(L.body,'normal');const firstBullet=item.bullets.length?wrap(item.bullets[0],TEXT-12).length*lead:0;
    ensure(size*2.9+firstBullet);
    twoColumn(item.title,period(item));twoColumn(item.company,item.location,{style:'normal',tint:L.accent});y+=2;
    font(L.body,'normal');
    for(const bullet of item.bullets){const lines=wrap(bullet,TEXT-12);ensure(lines.length*lead);color(L.ink);doc.text('•',M+2,y+size);lines.forEach(text=>{doc.text(text,M+12,y+size);y+=lead;});y+=1.5;}
    y+=5*gap;
   }else if(item.kind==='education'){ensure(size*3);twoColumn(item.qualification,[item.start,item.end].filter(Boolean).join(' – '));twoColumn(item.institution,'',{style:'normal',tint:L.accent});y+=5*gap;}
   else if(item.kind==='cert'){font(L.body,'normal');const rest=[item.issuer,item.year].filter(Boolean).join(' · ');rich(rest?item.name+' ':item.name,rest?'· '+rest:'');}
   else{font(L.body,'normal');const lines=wrap(item.text,TEXT);for(const text of lines){ensure(lead);color(L.ink);doc.text(text,M,y+size);y+=lead;}y+=3;}
   previous=item;
  }
  const total=doc.getNumberOfPages();
  for(let page=1;page<=total;page++){doc.setPage(page);font(L.body,'normal',8);color(L.muted);doc.text(data.candidate.name,M,H-9*MM);doc.text(`${page} / ${total}`,W-M,H-9*MM,{align:'right'});}
  doc.setProperties({title:data.candidate.name+' – CV',subject:[data.target.role,data.target.company].filter(Boolean).join(' – '),author:data.candidate.name,keywords:data.target.keywords.join(', '),creator:'CVBuilder'});
  doc.setLanguage(data.language==='en'?'en-GB':'nb-NO');
  return {blob:doc.output('blob'),pages:total};
 }
 const api={build};root.CVPdf=api;if(typeof module!=='undefined'&&module.exports)module.exports=api;
})(typeof globalThis!=='undefined'?globalThis:this);
