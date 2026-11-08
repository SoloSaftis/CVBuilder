(function(root){
 'use strict';
 // Every layout follows the same strict rules: one text column, the same section order and
 // standard section names, real text (no tables, text boxes, icons or skill bars), contact
 // details in the document body, standard fonts, body text 10 pt or larger, at most two pages.
 // Layouts differ only in typography, colour and spacing, so the machine-read text is identical.
 const LAYOUTS={
  classic:{name:'Klassisk',description:'Rolig og tidløs. Passer de fleste bransjer.',
   web:{body:'Arial, Helvetica, sans-serif',head:'Arial, Helvetica, sans-serif',size:10.5,name:24,accent:'#2f4a66',ink:'#1f2c3a',muted:'#5b6b7d',rule:'#becad7',headingCase:'upper',headingStyle:'rule',gap:1},
   word:{body:'Arial',head:'Arial',size:21,name:48,accent:'2F4A66',ink:'1F2C3A',muted:'5B6B7D',rule:'BECAD7',headingCase:'upper',headingStyle:'rule',gap:1},
   pdf:{body:'helvetica',head:'helvetica',size:10.5,name:24,accent:[47,74,102],ink:[31,44,58],muted:[91,107,125],rule:[190,202,215],headingCase:'upper',headingStyle:'rule',gap:1}},
  modern:{name:'Moderne',description:'Moderne skrift og tydelig fargeaksent. Teknologi og konsulent.',
   web:{body:'Calibri, Carlito, Arial, sans-serif',head:'Calibri, Carlito, Arial, sans-serif',size:10.5,name:25,accent:'#1d6670',ink:'#18262b',muted:'#566a70',rule:'#1d6670',headingCase:'upper',headingStyle:'bar',gap:1.02},
   word:{body:'Calibri',head:'Calibri',size:21,name:50,accent:'1D6670',ink:'18262B',muted:'566A70',rule:'1D6670',headingCase:'upper',headingStyle:'bar',gap:1.02},
   pdf:{body:'helvetica',head:'helvetica',size:10.5,name:25,accent:[29,102,112],ink:[24,38,43],muted:[86,106,112],rule:[29,102,112],headingCase:'upper',headingStyle:'bar',gap:1.02}},
  nordic:{name:'Nordisk',description:'Serif i navn og overskrifter, dempede farger. Rådgivning, offentlig sektor, ledelse.',
   web:{body:'Arial, Helvetica, sans-serif',head:'Georgia, "Times New Roman", serif',size:10.5,name:26,accent:'#3e5c55',ink:'#1d2826',muted:'#5f6f6b',rule:'#c9d3cf',headingCase:'title',headingStyle:'rule',gap:1.04},
   word:{body:'Arial',head:'Georgia',size:21,name:52,accent:'3E5C55',ink:'1D2826',muted:'5F6F6B',rule:'C9D3CF',headingCase:'title',headingStyle:'rule',gap:1.04},
   pdf:{body:'helvetica',head:'times',nameStyle:'normal',size:10.5,name:26,accent:[62,92,85],ink:[29,40,38],muted:[95,111,107],rule:[201,211,207],headingCase:'title',headingStyle:'rule',gap:1.04}},
  compact:{name:'Kompakt',description:'Tettere og nøktern. Når lang erfaring må få plass på to sider.',
   web:{body:'Arial, Helvetica, sans-serif',head:'Arial, Helvetica, sans-serif',size:10,name:21,accent:'#222222',ink:'#1a1a1a',muted:'#555555',rule:'#9a9a9a',headingCase:'upper',headingStyle:'plain',gap:.82},
   word:{body:'Arial',head:'Arial',size:20,name:42,accent:'222222',ink:'1A1A1A',muted:'555555',rule:'9A9A9A',headingCase:'upper',headingStyle:'plain',gap:.82},
   pdf:{body:'helvetica',head:'helvetica',size:10,name:21,accent:[34,34,34],ink:[26,26,26],muted:[85,85,85],rule:[154,154,154],headingCase:'upper',headingStyle:'plain',gap:.82}}
 };
 const ids=Object.keys(LAYOUTS);
 const get=id=>LAYOUTS[id]||LAYOUTS.classic;
 const heading=(text,layout,format='web')=>get(layout)[format].headingCase==='upper'?text.toLocaleUpperCase('nb'):text;
 const api={LAYOUTS,ids,get,heading};
 root.CVLayouts=api;if(typeof module!=='undefined'&&module.exports)module.exports=api;
})(typeof globalThis!=='undefined'?globalThis:this);
