// The sources Mermaid's parser specs feed a diagram type, for the compat
// goldens: every string and template literal in Pele's ported specs that
// Pele detects as the type.
//
//   node specs.mjs <pele-checkout> <kind> <out.json>
//
// <pele-checkout> is github.com/obsidianmd/pele at 44da152 (1.6.0); <kind>
// is a folder under its tests/compat. Literals with `${` are skipped.
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { detectType } from 'pele';

const [checkout, kind, out] = process.argv.slice(2);
if (!checkout || !kind || !out) {
  console.error('usage: node specs.mjs <pele-checkout> <kind> <out.json>');
  process.exit(2);
}

// Each quoted or backquoted literal in `code`, read as JavaScript would.
function literals(code) {
  const found = [];
  let i = 0;
  while (i < code.length) {
    const c = code[i];
    if (c === '/' && code[i + 1] === '/') {
      i = code.indexOf('\n', i);
      if (i === -1) break;
      continue;
    }
    if (c === '/' && code[i + 1] === '*') {
      i = code.indexOf('*/', i + 2);
      if (i === -1) break;
      i += 2;
      continue;
    }
    if (c !== '"' && c !== "'" && c !== '`') {
      i++;
      continue;
    }
    let j = i + 1;
    let interpolated = false;
    while (j < code.length && code[j] !== c) {
      if (code[j] === '\\') j++;
      else if (c === '`' && code[j] === '$' && code[j + 1] === '{') interpolated = true;
      else if (c !== '`' && code[j] === '\n') break;
      j++;
    }
    const raw = code.slice(i, j + 1);
    if (!interpolated && code[j] === c) {
      try {
        found.push(c === '`' ? String.raw({ raw: [raw.slice(1, -1)] }).replace(/\\(.)/gs, unescape) : JSON.parse(c === "'" ? '"' + raw.slice(1, -1).replace(/\\'/g, "'").replace(/"/g, '\\"') + '"' : raw));
      } catch {
        // A literal JSON cannot read is left out.
      }
    }
    i = j + 1;
  }
  return found;
}

function unescape(_match, ch) {
  return { n: '\n', t: '\t', r: '\r', '\\': '\\', '`': '`', $: '$' }[ch] ?? ch;
}

const dir = join(checkout, 'tests', 'compat', kind);
const sources = new Set();
for (const file of readdirSync(dir).sort()) {
  if (!/\.(spec|test)\.[jt]s$/.test(file)) continue;
  for (const text of literals(readFileSync(join(dir, file), 'utf8'))) {
    let type = null;
    try {
      type = detectType(text);
    } catch {
      type = null;
    }
    if (type !== null && text.length < 20000) sources.add(text);
  }
}
writeFileSync(out, JSON.stringify([...sources], null, 1) + '\n');
console.log(`${sources.size} sources`);
