// What Vega and Vega-Lite draw for each corpus spec, for the importer's
// tests to compare against.
//
//   node render.mjs <out-dir> [spec…]
//     Each spec in ../vega_lite/specs drawn to <out-dir>/<name>.svg, or
//     <name>.error.json with the error. Every spec by default; a spec is
//     named by its file name.
//
// `npm install` here first; Vega, Vega-Lite and vega-datasets are pinned in
// package.json. A spec's `data/…` url is read from vega-datasets; an
// absolute url is fetched. Times are drawn in UTC, with Vega's random
// generator seeded as Vega-Lite's own `examples/compiled` are. Without the
// `canvas` package, text widths are Vega's estimate, so the drawings are the
// same on every machine and differ from `examples/compiled` where text
// sizes a layout.
process.env.TZ = 'UTC';

import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as vega from 'vega';
import * as vegaLite from 'vega-lite';

const here = dirname(fileURLToPath(import.meta.url));
const specs = join(here, '..', 'vega_lite', 'specs');
const loader = vega.loader({ baseURL: join(here, 'node_modules', 'vega-datasets') + '/' });

async function draw(text) {
  vega.setRandom(vega.randomLCG(123456789));
  const { spec } = vegaLite.compile(JSON.parse(text));
  const view = new vega.View(vega.parse(spec), { renderer: 'none', loader });
  try {
    return await view.toSVG();
  } finally {
    view.finalize();
  }
}

const [out, ...files] = process.argv.slice(2);
if (out === undefined) {
  console.error('usage: node render.mjs <out-dir> [spec…]');
  process.exit(2);
}
mkdirSync(out, { recursive: true });
const names = files.length > 0 ? files.map((f) => basename(f)) : readdirSync(specs).filter((f) => f.endsWith('.vl.json')).sort();
for (const file of names) {
  const name = file.replace(/\.vl\.json$/, '');
  try {
    writeFileSync(join(out, `${name}.svg`), await draw(readFileSync(join(specs, file), 'utf8')));
  } catch (error) {
    writeFileSync(join(out, `${name}.error.json`), JSON.stringify({ message: String(error.message ?? error) }));
  }
}
