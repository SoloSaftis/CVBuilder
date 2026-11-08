// Writes the shared fictional example CV for the Rust app (shared/cv-core/src/example.json).
const fs=require('node:fs'),path=require('node:path'),contract=require('../contract.js'),example=require('../example.js');
fs.writeFileSync(path.resolve(__dirname,'../../cv-core/src/example.json'),JSON.stringify(example.example(contract),null,1));
