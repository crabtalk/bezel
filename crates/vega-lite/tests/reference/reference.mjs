// What Vega-Lite 6.5.0 and Vega 6.4.0 make of each corpus spec, for the
// `compile` and `scenegraph` checks to compare against.
//
//   node reference.mjs <out-dir> [spec…]
//
// For each spec in ../corpus/specs, `<name>.vg.json` is `compile(spec).spec`,
// `<name>.sg.json` the scenegraph Vega draws from it, as
// `view.scenegraph().toJSON()` writes it, and `<name>.view.json` the size of
// the view inside its padding, its `origin()` and its `padding()`;
// `<name>.error.json` holds the message where either throws. Every spec by
// default; a spec is named by its file name.
//
// `npm install` here first. A spec's `data/…` url is read from
// vega-datasets. Times are drawn in UTC, with Vega's random generator seeded
// as Vega-Lite's own `examples/compiled` are. Without the `canvas` package,
// text widths are Vega's estimate.
process.env.TZ = 'UTC';

import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as vega from 'vega';
import * as vegaLite from 'vega-lite';

const here = dirname(fileURLToPath(import.meta.url));
const specs = join(here, '..', 'corpus', 'specs');
const loader = vega.loader({ baseURL: join(here, 'node_modules', 'vega-datasets') + '/' });

// `JSON.stringify`'s replacer: a date as its time, before `toJSON` writes it.
function dates(key, value) {
  return this[key] instanceof Date ? this[key].getTime() : value;
}

// A scale as `scales.json` holds it: its domain values, and for a
// continuous domain the eight points between its ends, mapped.
function sample(scale) {
  const domain = scale.domain();
  const range = typeof scale.range === 'function' ? scale.range() : undefined;
  const continuous = domain.length >= 2 && domain.every((d) => typeof d === 'number' || d instanceof Date);
  const points = continuous
    ? Array.from({length: 9}, (_, i) => +domain[0] + (i / 8) * (+domain[domain.length - 1] - +domain[0]))
    : domain;
  return {
    type: scale.type,
    domain,
    range,
    bandwidth: scale.bandwidth ? scale.bandwidth() : undefined,
    samples: points.map((p) => [p, scale(p)]),
  };
}

const [out, ...files] = process.argv.slice(2);
if (out === undefined) {
  console.error('usage: node reference.mjs <out-dir> [spec…]');
  process.exit(2);
}
mkdirSync(out, { recursive: true });
const names = files.length > 0 ? files.map((f) => basename(f)) : readdirSync(specs).filter((f) => f.endsWith('.vl.json')).sort();
for (const file of names) {
  const name = file.replace(/\.vl\.json$/, '');
  let view;
  try {
    const { spec } = vegaLite.compile(JSON.parse(readFileSync(join(specs, file), 'utf8')));
    writeFileSync(join(out, `${name}.vg.json`), JSON.stringify(spec, null, 1) + '\n');
    vega.setRandom(vega.randomLCG(123456789));
    view = new vega.View(vega.parse(spec), { renderer: 'none', loader });
    await view.runAsync();
    writeFileSync(join(out, `${name}.sg.json`), JSON.stringify(JSON.parse(view.scenegraph().toJSON()), null, 1) + '\n');
    const placed = {width: view._viewWidth, height: view._viewHeight, origin: view.origin(), padding: view.padding()};
    writeFileSync(join(out, `${name}.view.json`), JSON.stringify(placed) + '\n');
    const data = Object.fromEntries((spec.data ?? []).map((d) => [d.name, view.data(d.name)]));
    writeFileSync(join(out, `${name}.data.json`), JSON.stringify(data, dates) + '\n');
    const scales = Object.fromEntries((spec.scales ?? []).map((s) => [s.name, sample(view.scale(s.name))]));
    writeFileSync(join(out, `${name}.scales.json`), JSON.stringify(scales, dates) + '\n');
  } catch (error) {
    writeFileSync(join(out, `${name}.error.json`), JSON.stringify({ message: String(error.message ?? error) }) + '\n');
  } finally {
    view?.finalize();
  }
}
