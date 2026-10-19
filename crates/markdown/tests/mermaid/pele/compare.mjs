// Medians side by side, from what bench.mjs and the engine's bench wrote, and
// the size of each bundle that recorded one.
//
//   node compare.mjs pele.json ours.json [more.json…]
import { readFileSync } from 'node:fs';

const runs = process.argv.slice(2).map((file) => JSON.parse(readFileSync(file, 'utf8')));
if (runs.length < 2) {
  console.error('usage: node compare.mjs pele.json ours.json [more.json…]');
  process.exit(2);
}

const ms = (value) => (value < 1 ? value.toFixed(3) : value.toFixed(1)) + ' ms';
const [base] = runs;
const table = base.rows.map((row) => {
  const out = { case: row.case, diagrams: row.diagrams };
  for (const run of runs) {
    const match = run.rows.find((r) => r.case === row.case);
    if (!match) continue;
    out[run.engine] = ms(match.ms) + (match.done !== match.diagrams ? ` (${match.done} drew)` : '');
  }
  for (const run of runs.slice(1)) {
    const match = run.rows.find((r) => r.case === row.case);
    if (match && match.ms > 0) out[`${base.engine} ÷ ${run.engine}`] = (row.ms / match.ms).toFixed(1) + '×';
  }
  return out;
});
console.table(table);
const kb = (n) => (n / 1024).toFixed(0) + ' KB';
const sized = runs.filter((run) => run.bytes);
if (sized.length) {
  console.table(sized.map((run) => ({ engine: run.engine, raw: kb(run.bytes.raw), gzip: kb(run.bytes.gzip) })));
}
