// Runs each `*.vl.json` spec in ../transform through Vega-Lite and Vega,
// and writes the data each mark draws, in mark order, to `<name>.vega.json`,
// a time as milliseconds since the epoch; and the domain of each of its
// discrete scales, by scale name, to `<name>.domains.json` where it has one.
// Times are read in UTC, with Vega's random generator seeded as
// `render.mjs` seeds it.
//
//   npm run fixtures
process.env.TZ = 'UTC';

import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { compile } from 'vega-lite';
import { parse, randomLCG, setRandom, View } from 'vega';

const dir = join(import.meta.dirname, '..', 'transform');

// The datasets marks read, in paint order. A faceted group (a line or area
// split by colour) reads its facet's source.
function sources(marks, out = []) {
  for (const mark of marks ?? []) {
    if (mark.from?.facet) out.push(mark.from.facet.data);
    else if (mark.from?.data) out.push(mark.from.data);
    else sources(mark.marks, out);
  }
  return out;
}

// A date as its time value.
function times(key, value) {
  return this[key] instanceof Date ? +this[key] : value;
}

for (const file of readdirSync(dir).filter((f) => f.endsWith('.vl.json')).sort()) {
  const spec = JSON.parse(readFileSync(join(dir, file), 'utf8'));
  const vega = compile(spec).spec;
  setRandom(randomLCG(123456789));
  const view = new View(parse(vega), { renderer: 'none' });
  await view.runAsync();
  const out = sources(vega.marks).map((name) => view.data(name).map((row) => ({ ...row })));
  writeFileSync(join(dir, file.replace('.vl.json', '.vega.json')), JSON.stringify(out, times, 1) + '\n');
  const domains = Object.fromEntries(
    (vega.scales ?? [])
      .filter((scale) => ['band', 'point', 'ordinal'].includes(scale.type))
      .map((scale) => [scale.name, view.scale(scale.name).domain()]),
  );
  if (Object.keys(domains).length > 0) {
    writeFileSync(join(dir, file.replace('.vl.json', '.domains.json')), JSON.stringify(domains, times, 1) + '\n');
  }
  console.log(file, out.map((rows) => rows.length));
}
