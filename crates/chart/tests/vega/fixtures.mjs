// Runs each `*.vl.json` spec in ../transform through Vega-Lite and Vega,
// and writes the data each mark draws, in mark order, to `<name>.vega.json`.
//
//   npm run fixtures
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { compile } from 'vega-lite';
import { parse, View } from 'vega';

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

for (const file of readdirSync(dir).filter((f) => f.endsWith('.vl.json')).sort()) {
  const spec = JSON.parse(readFileSync(join(dir, file), 'utf8'));
  const vega = compile(spec).spec;
  const view = new View(parse(vega), { renderer: 'none' });
  await view.runAsync();
  const out = sources(vega.marks).map((name) => view.data(name).map((row) => ({ ...row })));
  writeFileSync(join(dir, file.replace('.vl.json', '.vega.json')), JSON.stringify(out, null, 1) + '\n');
  console.log(file, out.map((rows) => rows.length));
}
