// Tools for the bundled interface languages (assets/languages/<code>.cvlang.json).
//   node lang-tools.cjs template <out-dir>      every text as chunk files: {"<Norwegian key>": "<English>"}
//   node lang-tools.cjs missing <code> <out>    the texts a bundled language still lacks, as one chunk
//   node lang-tools.cjs merge <code> <name> <part.json …>   join translated chunks into the language file
//   node lang-tools.cjs check [code …]          check bundled files: every key, placeholders kept
// The keys come from the EN and DYN tables in crates/app/src/i18n.rs.
const fs = require('fs');
const path = require('path');
const ROOT = path.join(__dirname, '..');
const I18N = path.join(ROOT, 'crates/app/src/i18n.rs');
const DIR = path.join(ROOT, 'assets/languages');
const FORMAT = 'cvbuilder-language/1';
const CHUNK = 220;

/// (key, English) from the EN and DYN tables.
function keys() {
  const src = fs.readFileSync(I18N, 'utf8');
  const out = [];
  for (const table of ['EN', 'DYN']) {
    const start = src.indexOf(`static ${table}: &[(&str, &str)] = &[`);
    if (start < 0) throw Error('no table ' + table);
    const end = src.indexOf('\n];', start);
    const body = src.slice(start, end);
    const row = /^\s*\(\s*("(?:[^"\\]|\\.)*")\s*,\s*("(?:[^"\\]|\\.)*")\s*\),?\s*$/gm;
    let m;
    while ((m = row.exec(body))) out.push([lit(m[1]), lit(m[2])]);
  }
  return out;
}

/// A Rust string literal as a JS string (the escapes the tables use: \" \\ \n \t \u{…}).
function lit(s) {
  return JSON.parse(s.replace(/\\u\{([0-9a-fA-F]+)\}/g, (_, h) => String.fromCodePoint(parseInt(h, 16)) === '"' ? '\\"' : String.fromCodePoint(parseInt(h, 16))).replace(/\\'/g, "'"));
}

const holders = (s) => [...s.matchAll(/\{[a-z_]+\}/g)].map((m) => m[0]).sort().join(',');

function write(file, obj) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(obj, null, 1) + '\n');
}

function check(code) {
  const file = path.join(DIR, `${code}.cvlang.json`);
  const pack = JSON.parse(fs.readFileSync(file, 'utf8'));
  const problems = [];
  if (pack.format !== FORMAT) problems.push('format');
  if (pack.code !== code) problems.push(`code ${pack.code}`);
  if (!pack.name) problems.push('no name');
  const all = keys();
  const known = new Set(all.map(([k]) => k));
  let missing = 0;
  for (const [k] of all) {
    const v = pack.strings[k];
    if (typeof v !== 'string' || !v.trim()) { missing++; continue; }
    if (holders(v) !== holders(k)) problems.push(`placeholders: ${JSON.stringify(k).slice(0, 70)} → ${JSON.stringify(v).slice(0, 70)}`);
  }
  const unknown = Object.keys(pack.strings).filter((k) => !known.has(k)).length;
  return { code, name: pack.name, total: all.length, missing, unknown, problems };
}

const [cmd, ...args] = process.argv.slice(2);
if (cmd === 'template') {
  const all = keys();
  for (let i = 0; i * CHUNK < all.length; i++) {
    write(path.join(args[0], `chunk-${i + 1}.json`), Object.fromEntries(all.slice(i * CHUNK, (i + 1) * CHUNK)));
  }
  console.log(`${all.length} texts in ${Math.ceil(all.length / CHUNK)} chunks`);
} else if (cmd === 'missing') {
  const [code, out] = args;
  const file = path.join(DIR, `${code}.cvlang.json`);
  const have = fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, 'utf8')).strings : {};
  const left = keys().filter(([k]) => typeof have[k] !== 'string' || !have[k].trim());
  write(out, Object.fromEntries(left));
  console.log(`${left.length} texts missing in ${code}`);
} else if (cmd === 'merge') {
  const [code, name, ...parts] = args;
  const file = path.join(DIR, `${code}.cvlang.json`);
  const old = fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, 'utf8')).strings : {};
  const strings = { ...old };
  for (const p of parts) Object.assign(strings, JSON.parse(fs.readFileSync(p, 'utf8')));
  // Keys this script can't read from i18n.rs (rows split over several lines) are kept, last.
  const known = keys().map(([k]) => k).filter((k) => typeof strings[k] === 'string');
  const order = [...known, ...Object.keys(strings).filter((k) => !known.includes(k))];
  write(file, { format: FORMAT, code, name, made_by: 'CVBuilder', strings: Object.fromEntries(order.map((k) => [k, strings[k]])) });
  const r = check(code);
  console.log(JSON.stringify(r, null, 1));
} else if (cmd === 'check') {
  const codes = args.length ? args : fs.readdirSync(DIR).filter((f) => f.endsWith('.cvlang.json')).map((f) => f.replace('.cvlang.json', ''));
  let bad = 0;
  for (const c of codes) {
    const r = check(c);
    if (r.missing || r.problems.length) bad++;
    console.log(`${c} (${r.name}): ${r.total - r.missing}/${r.total}, unknown ${r.unknown}${r.problems.length ? '\n  ' + r.problems.join('\n  ') : ''}`);
  }
  process.exit(bad ? 1 : 0);
} else {
  console.log('usage: template <dir> | missing <code> <out> | merge <code> <name> <parts…> | check [codes…]');
}
