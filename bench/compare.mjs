// Medians side by side, from the JSON two or more benches wrote: each a run
// of one engine, `{ engine, rows: [{ case, inputs, ms, done }] }`. The first
// file is the baseline.
//
//   node compare.mjs theirs.json ours.json [more.json…]
import { readFileSync } from 'node:fs';
import { basename } from 'node:path';

const files = process.argv.slice(2);
const runs = files.map((file) => JSON.parse(readFileSync(file, 'utf8')));
// Two runs of one engine, such as two profiles, are told apart by their files.
const shared = runs.map((run) => runs.filter((other) => other.engine === run.engine).length > 1);
for (const [i, run] of runs.entries()) {
  if (shared[i]) run.engine = `${run.engine} (${basename(files[i], '.json')})`;
}
if (runs.length < 2) {
  console.error('usage: node compare.mjs theirs.json ours.json [more.json…]');
  process.exit(2);
}

const ms = (value) => (value < 1 ? value.toFixed(3) : value.toFixed(1)) + ' ms';
const [base] = runs;
const table = base.rows.map((row) => {
  const out = { case: row.case, inputs: row.inputs };
  for (const run of runs) {
    const match = run.rows.find((r) => r.case === row.case);
    if (!match) continue;
    out[run.engine] = ms(match.ms) + (match.done !== match.inputs ? ` (${match.done} done)` : '');
  }
  for (const run of runs.slice(1)) {
    const match = run.rows.find((r) => r.case === row.case);
    if (match && match.ms > 0) out[`${base.engine} ÷ ${run.engine}`] = (row.ms / match.ms).toFixed(1) + '×';
  }
  return out;
});
console.table(table);
const ratios = runs.slice(1).map((run) => {
  const logs = base.rows.flatMap((row) => {
    const match = run.rows.find((r) => r.case === row.case);
    return match && match.ms > 0 && row.ms > 0 ? [Math.log(row.ms / match.ms)] : [];
  });
  const mean = logs.reduce((sum, x) => sum + x, 0) / logs.length;
  return { engine: run.engine, [`${base.engine} ÷ engine, geometric mean`]: Math.exp(mean).toFixed(2) + '×' };
});
console.table(ratios);
