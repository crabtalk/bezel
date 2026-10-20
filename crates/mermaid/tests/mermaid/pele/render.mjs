// What Pele draws and parses, for the engine's tests to compare against.
//
//   node render.mjs svg <out-dir> [corpus-file…]
//     Each corpus entry drawn to <out-dir>/<file>/<index>.svg, or
//     <index>.error.json with Pele's error. Every corpus file by default.
//   node render.mjs compat <pele-checkout> <out-dir>
//     <out-dir>/<kind>.json for every kind in KINDS, written as
//     [{ source, model } | { source, error: { code, message } }]: each
//     string and template literal in Pele's ported specs for the kind that
//     Pele detects as it, and what Pele's `parse` made of it. <pele-checkout>
//     is github.com/obsidianmd/pele at 44da152 (1.6.0). Literals with `${`
//     are skipped.
//
// `npm install` here first; Pele is pinned in package.json.
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { detectType, parse, render } from 'pele';

const here = dirname(fileURLToPath(import.meta.url));
const corpus = join(here, '..', 'corpus');
const options = { idPrefix: 'bezel' };

// Maps and Sets in a model, as the engine serializes them.
function plain(_key, value) {
  if (value instanceof Map) return Object.fromEntries(value);
  if (value instanceof Set) return [...value];
  return value;
}

function failure(error) {
  return { code: error.code ?? 'internal', message: String(error.message) };
}

// Each compat file and the folders under Pele's `tests/compat` its sources
// come from, or the sources themselves where Pele's specs have none worth
// taking.
const KINDS = {
  agentflow: ['agentflow'],
  architecture: ['architecture'],
  block: ['block'],
  c4: ['c4'],
  class: ['class'],
  cynefin: ['cynefin'],
  er: ['er'],
  eventmodeling: ['eventmodeling'],
  flowchart: ['flowchart'],
  gantt: ['gantt'],
  git: ['git'],
  info: {
    sources: [
      'info',
      'info showInfo',
      'info unsupported',
      '  info\n',
      'info\ntitle My info',
      'info\naccTitle: Hello\naccDescr: World',
      'info\naccDescr {\n  multi\n    line\n}',
      '---\ntitle: Front\n---\ninfo',
      'info title',
      'info\n%% comment\ntitle  spaced   out ',
      '%%{init: {"theme": "dark"}}%%\ninfo',
      'infox',
    ],
  },
  ishikawa: ['ishikawa'],
  journey: ['journey'],
  kanban: ['kanban'],
  mindmap: ['mindmap'],
  packet: ['packet'],
  pie: ['pie'],
  quadrant: ['quadrant'],
  radar: ['radar'],
  railroad: ['railroad'],
  requirement: ['requirement'],
  sankey: ['sankey'],
  sequence: ['sequence'],
  state: ['state'],
  timeline: ['timeline'],
  treemap: ['treemap'],
  treeview: ['treeview'],
  usecase: ['usecase', 'usecase-parser'],
  venn: ['venn'],
  wardley: ['wardley'],
  xychart: ['xychart'],
};

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

// The literals in the specs under `folders` that Pele detects as a diagram,
// each once, in file order.
function specSources(checkout, folders) {
  const sources = new Set();
  for (const folder of folders) {
    const dir = join(checkout, 'tests', 'compat', folder);
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
  }
  return [...sources];
}

function parsed(source) {
  try {
    return { source, model: JSON.parse(JSON.stringify(parse(source), plain)) };
  } catch (error) {
    return { source, error: failure(error) };
  }
}

const [mode, ...rest] = process.argv.slice(2);
if (mode === 'svg') {
  const [out, ...files] = rest;
  const names = files.length > 0 ? files : readdirSync(corpus).filter((f) => f.endsWith('.json'));
  for (const name of names) {
    const file = basename(name);
    const dir = join(out, file.replace(/\.json$/, ''));
    mkdirSync(dir, { recursive: true });
    // A corpus entry is a source, or a reference entry with its `src`.
    const sources = JSON.parse(readFileSync(join(corpus, file), 'utf8')).map((c) => (typeof c === 'string' ? c : c.src));
    sources.forEach((source, i) => {
      try {
        writeFileSync(join(dir, `${i}.svg`), render(source, options).svg);
      } catch (error) {
        writeFileSync(join(dir, `${i}.error.json`), JSON.stringify(failure(error)));
      }
    });
  }
} else if (mode === 'compat') {
  const [checkout, out] = rest;
  mkdirSync(out, { recursive: true });
  for (const [kind, folders] of Object.entries(KINDS)) {
    const sources = Array.isArray(folders) ? specSources(checkout, folders) : folders.sources;
    writeFileSync(join(out, `${kind}.json`), JSON.stringify(sources.map(parsed), null, 1) + '\n');
  }
} else {
  console.error('usage: node render.mjs svg <out-dir> [corpus-file…] | compat <pele-checkout> <out-dir>');
  process.exit(2);
}
