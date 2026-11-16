const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { test } = require('node:test');

const source = fs.readFileSync(path.join(__dirname, '../src/view/mod.rs'), 'utf8');
const template = source.match(/r#"(void \(async \(\) => \{\{[\s\S]*?)"#/)[1]
  .replaceAll('{{', '{').replaceAll('}}', '}');

async function evaluate(expression, backend) {
  let respond;
  const answer = new Promise((resolve) => { respond = resolve; });
  const bridge = { postMessage: (value) => respond(JSON.parse(value)) };
  const window = backend === 'webkit' ? { webkit: { messageHandlers: { ipc: bridge } } } :
    backend === 'webview2' ? { chrome: { webview: bridge } } : { ipc: bridge };
  vm.runInNewContext(template.replaceAll('{id}', '42').replace('{script}', expression), { window });
  return answer;
}

for (const backend of ['wry', 'webkit', 'webview2']) {
  test(`${backend} posts the awaited value with its evaluation id`, async () => {
    const reply = await evaluate('Promise.resolve({ready: true})', backend);
    assert.equal(reply.evaluation, 42);
    assert.deepEqual(JSON.parse(reply.value), { ready: true });
  });
}

test('async replies carry stack traces for rejected promises', async () => {
  const reply = await evaluate('Promise.reject(new Error("request failed"))', 'wry');
  assert.match(reply.error, /Error: request failed/);
  assert.equal(reply.value, undefined);
});

test('async replies reject values without a JSON representation', async () => {
  const reply = await evaluate('() => 1', 'wry');
  assert.match(reply.error, /no JSON representation/);
});
