// What Pele draws and parses, for the engine's tests to compare against.
//
//   node render.mjs svg <out-dir> [corpus-file…]
//     Each corpus entry drawn to <out-dir>/<file>/<index>.svg, or
//     <index>.error.json with Pele's error. Every corpus file by default.
//   node render.mjs model <inputs.json> <out.json>
//     Each source in <inputs.json> (an array of strings, or a file this
//     wrote) parsed, written as
//     [{ source, model } | { source, error: { code, message } }].
//
// `npm install` here first; Pele is pinned in package.json.
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parse, render } from 'pele';

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
} else if (mode === 'model') {
  const [inputs, out] = rest;
  // An array of sources, or a file this wrote, read again for its sources.
  const sources = JSON.parse(readFileSync(inputs, 'utf8')).map((c) => (typeof c === 'string' ? c : c.source));
  const cases = sources.map((source) => {
    try {
      return { source, model: JSON.parse(JSON.stringify(parse(source), plain)) };
    } catch (error) {
      return { source, error: failure(error) };
    }
  });
  writeFileSync(out, JSON.stringify(cases, null, 1) + '\n');
} else {
  console.error('usage: node render.mjs svg <out-dir> [corpus-file…] | model <inputs.json> <out.json>');
  process.exit(2);
}
