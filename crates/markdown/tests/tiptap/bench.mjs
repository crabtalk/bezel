// Times tiptap on the cases `cargo bench -p bezel-markdown --bench markdown`
// times this crate on, with the same timing loop, and writes JSON.
//
//   node bench.mjs > tiptap.json
//
// Three stages per case: markdown → document, document → HTML, and
// document → markdown. The documents differ in shape — a ProseMirror tree
// here, a flat block list there — and the HTML differs in markup, so this
// times the two and compares no output.
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { getSchema } from '@tiptap/core';
import { MarkdownManager } from '@tiptap/markdown';
import { Node } from '@tiptap/pm/model';
import StarterKit from '@tiptap/starter-kit';
import Image from '@tiptap/extension-image';
import { TaskItem, TaskList } from '@tiptap/extension-list';
import { TableKit } from '@tiptap/extension-table';
import { renderToHTMLString } from '@tiptap/static-renderer/pm/html-string';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..', '..', '..');
const engine = 'tiptap 3.32.0';
const extensions = [StarterKit, Image, TaskList, TaskItem, TableKit];
const manager = new MarkdownManager({ extensions });
const schema = getSchema(extensions);

// The workspace's own READMEs, in the order the Rust bench reads them.
const docs = ['README.md', 'CONTRIBUTING.md'];
for (const crate of readdirSync(join(root, 'crates')).sort()) {
  if (existsSync(join(root, 'crates', crate, 'README.md'))) docs.push(join('crates', crate, 'README.md'));
}
// The spec's examples, read as `tests/spec/examples.rs` reads them.
const spec = readFileSync(join(here, '..', 'spec', 'commonmark.txt'), 'utf8');
const fence = '`'.repeat(32);
const examples = [];
{
  const lines = spec.split('\n');
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].trimEnd() !== `${fence} example`) continue;
    let markdown = '';
    while (++i < lines.length && lines[i] !== '.') markdown += lines[i].replaceAll('→', '\t') + '\n';
    while (++i < lines.length && lines[i] !== fence);
    examples.push(markdown);
  }
}
const cases = [
  ['workspace READMEs', docs.map((file) => readFileSync(join(root, file), 'utf8'))],
  ['CommonMark spec examples', examples],
  ['CommonMark spec', [spec]],
];

// Pele's loop, as in the mermaid bench: one warm-up pass, then passes until
// 300 ms are spent and at least 5 are taken (at most 2000), and the median.
function time(inputs, stage) {
  const run = () => {
    let done = 0;
    for (const input of inputs) {
      try {
        stage(input);
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

const rows = [];
for (const [name, sources] of cases) {
  const docs = sources.map((source) => manager.parse(source));
  // The document as an editor holds it, as the Rust bench renders a `Doc`:
  // JSON would be read into one against a fresh schema on every render.
  const nodes = docs.map((doc) => Node.fromJSON(schema, doc));
  const stages = [
    ['parse', sources, (source) => manager.parse(source)],
    ['html', nodes, (node) => renderToHTMLString({ extensions, content: node })],
    ['serialize', docs, (doc) => manager.serialize(doc)],
  ];
  for (const [stage, inputs, run] of stages) {
    rows.push({ case: `${stage}: ${name}`, inputs: inputs.length, ...time(inputs, run) });
  }
}
console.log(JSON.stringify({ engine, rows }, null, 1));
