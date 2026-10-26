// Times Pele on the cases `cargo bench -p bezel-mermaid --bench mermaid`
// times the engine on, with the same timing loop, and writes JSON.
//
//   node bench.mjs > pele.json
//
// The cases are Pele's own `scripts/bench.mjs` cases — the flowchart
// documentation examples and generated trees and meshes — and then every
// corpus file, one case each.
import { readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { render } from 'pele';

const here = dirname(fileURLToPath(import.meta.url));
const corpus = join(here, '..', 'corpus');
const engine = 'pele 1.6.0';
const options = { idPrefix: 'bezel' };
const read = (file) => JSON.parse(readFileSync(join(corpus, file), 'utf8'));

function chain(n, fan) {
  let s = 'flowchart TD\n';
  for (let i = 1; i < n; i++) s += `  n${Math.floor((i - 1) / fan)}[Node ${Math.floor((i - 1) / fan)}] --> n${i}[Node ${i}]\n`;
  return s;
}

function mesh(n, extra) {
  let s = 'flowchart TD\n';
  let seed = 7;
  const rnd = () => (seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296;
  for (let i = 1; i < n; i++) s += `  n${Math.floor(rnd() * i)} --> n${i}\n`;
  for (let i = 0; i < extra; i++) s += `  n${Math.floor(rnd() * n)} -->|label ${i}| n${Math.floor(rnd() * n)}\n`;
  return s;
}

const cases = [
  ['docs: all flowchart examples', read('flowchart-docs.json')],
  ['tree: 50 nodes', [chain(50, 3)]],
  ['mesh: 50 nodes, 20 extra labelled edges', [mesh(50, 20)]],
  ['tree: 500 nodes', [chain(500, 3)]],
  ['mesh: 500 nodes, 200 extra labelled edges', [mesh(500, 200)]],
];
for (const file of readdirSync(corpus).sort()) {
  if (!file.endsWith('-docs.json')) continue;
  cases.push([`corpus: ${file.replace(/-docs\.json$/, '')}`, read(file)]);
}

// Pele's loop: one warm-up pass, then passes until 300 ms are spent and at
// least 5 are taken (at most 2000), and the median pass.
function time(inputs) {
  const run = () => {
    let done = 0;
    for (const src of inputs) {
      try {
        render(src, options);
        done++;
      } catch {
        continue;
      }
    }
    return done;
  };
  const done = run();
  let budget = 300;
  const samples = [];
  while (budget > 0 || samples.length < 5) {
    const t = performance.now();
    run();
    const ms = performance.now() - t;
    samples.push(ms);
    budget -= ms;
    if (samples.length >= 2000) break;
  }
  samples.sort((a, b) => a - b);
  return { ms: samples[samples.length >> 1], done };
}

const rows = cases.map(([name, inputs]) => ({ case: name, inputs: inputs.length, ...time(inputs) }));
console.log(JSON.stringify({ engine, rows }, null, 1));
