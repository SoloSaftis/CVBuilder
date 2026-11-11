// Tools for the bundled interface languages (assets/languages/<code>.cvlang.json).
//   node lang-tools.cjs template <out-dir>      every text as chunk files: {"<Norwegian key>": "<English>"}
//   node lang-tools.cjs missing <code> <out>    the texts a bundled language still lacks, as one chunk
//   node lang-tools.cjs merge <code> <name> <part.json …>   join translated chunks into the language file
//   node lang-tools.cjs check [code …]          check bundled files: every key, placeholders kept
//   node lang-tools.cjs cvdata <dir or files>   join CV language data into assets/cv-languages.json
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
    // A row may be on one line or spread over several: ("key", "English") or ( "key", "English", ).
    const row = /\(\s*("(?:[^"\\]|\\.)*")\s*,\s*("(?:[^"\\]|\\.)*")\s*,?\s*\)/g;
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
  // Keys no longer in i18n.rs are kept, last, so a merge never drops a translation.
  const known = keys().map(([k]) => k).filter((k) => typeof strings[k] === 'string');
  const order = [...known, ...Object.keys(strings).filter((k) => !known.includes(k))];
  write(file, { format: FORMAT, code, name, made_by: 'CVBuilder', strings: Object.fromEntries(order.map((k) => [k, strings[k]])) });
  const r = check(code);
  console.log(JSON.stringify(r, null, 1));
} else if (cmd === 'cvdata') {
  // Joins CV language files (one <code>.json each: code, name, headings[9], import, months[12],
  // now) into assets/cv-languages.json, checked; a file with a code already there replaces it.
  const out = path.join(ROOT, 'assets/cv-languages.json');
  const all = new Map(JSON.parse(fs.readFileSync(out, 'utf8')).map((l) => [l.code, l]));
  const files = args.flatMap((a) => (fs.statSync(a).isDirectory() ? fs.readdirSync(a).filter((f) => f.endsWith('.json')).map((f) => path.join(a, f)) : [a]));
  let bad = 0;
  for (const f of files) {
    const l = JSON.parse(fs.readFileSync(f, 'utf8'));
    const problems = [];
    if (!l.code || !l.name) problems.push('code or name missing');
    if (!Array.isArray(l.headings) || l.headings.length !== 9 || l.headings.some((h) => typeof h !== 'string' || !h.trim())) problems.push('headings must be 9 filled-in strings');
    if (!Array.isArray(l.months) || l.months.length !== 12) problems.push('months must be 12 lists');
    if (problems.length) { bad++; console.log(`${f}: ${problems.join('; ')}`); continue; }
    const lower = (xs) => [...new Set((xs || []).map((x) => String(x).trim().toLowerCase()).filter(Boolean))];
    const imp = {};
    for (const [k, v] of Object.entries(l.import || {})) imp[k] = lower(v);
    all.set(l.code, { code: l.code, name: l.name.trim(), english_name: (l.english_name || '').trim(), headings: l.headings.map((h) => h.trim()), import: imp, months: l.months.map(lower), now: lower(l.now) });
  }
  const list = [...all.values()].sort((a, b) => a.code.localeCompare(b.code));
  const names = list.map((l) => l.name.toLowerCase());
  for (const n of names.filter((n, i) => names.indexOf(n) !== i)) { bad++; console.log(`two languages are called «${n}»`); }
  fs.writeFileSync(out, JSON.stringify(list, null, 1) + '\n');
  console.log(`${list.length} CV languages in assets/cv-languages.json`);
  process.exit(bad ? 1 : 0);
} else if (cmd === 'check') {
  const codes = args.length ? args : fs.readdirSync(DIR).filter((f) => f.endsWith('.cvlang.json')).map((f) => f.replace('.cvlang.json', ''));
  let bad = 0;
  for (const c of codes) {
    const r = check(c);
    if (r.missing || r.problems.length) bad++;
    console.log(`${c} (${r.name}): ${r.total - r.missing}/${r.total}, unknown ${r.unknown}${r.problems.length ? '\n  ' + r.problems.join('\n  ') : ''}`);
  }
  process.exit(bad ? 1 : 0);
} else if (cmd === 'names') {
  // Every language's name in every interface language (assets/language-names.json), from the
  // Unicode CLDR data built into Node: «de» is «Tysk» in Norwegian and «German» in English.
  const packs = fs.readdirSync(DIR).filter((f) => f.endsWith('.cvlang.json')).map((f) => f.replace('.cvlang.json', ''));
  const cv = JSON.parse(fs.readFileSync(path.join(ROOT, 'assets/cv-languages.json'), 'utf8')).map((l) => l.code);
  const named = [...new Set(['nb', 'en', 'en-US', ...packs, ...cv])].sort();
  const out = {};
  for (const ui of ['no', 'en', ...packs].sort()) {
    const locale = ui === 'no' ? 'nb' : ui;
    const dn = new Intl.DisplayNames([locale, 'en'], { type: 'language', languageDisplay: 'standard', fallback: 'none' });
    out[ui] = {};
    for (const code of named) {
      const name = dn.of(code);
      if (name) out[ui][code] = name.charAt(0).toLocaleUpperCase(locale) + name.slice(1);
    }
  }
  fs.writeFileSync(path.join(ROOT, 'assets/language-names.json'), JSON.stringify(out, null, 1) + '\n');
  console.log(`${named.length} language names in ${Object.keys(out).length} interface languages`);
} else {
  console.log('usage: template <dir> | missing <code> <out> | merge <code> <name> <parts…> | check [codes…] | names');
}
