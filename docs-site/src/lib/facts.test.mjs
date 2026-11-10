import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { parse } from 'yaml';
import { fact, interpolate } from './facts.mjs';

const root = new URL('../', import.meta.url);
const snapshotId = JSON.parse(readFileSync(new URL('../../../assets/minecraft/version.json', import.meta.url), 'utf8')).id;

test('fact returns a value from facts.yml', () => { assert.equal(typeof fact('crates'), 'number'); });
test('snapshot comes from version.json', () => { assert.equal(fact('snapshot'), snapshotId); });
test('an unknown fact throws with its id', () => { assert.throws(() => fact('nope'), /nope/); });
test('interpolate replaces placeholders', () => { assert.equal(interpolate('{crates} crates'), `${fact('crates')} crates`); });
test('interpolate throws on an unknown placeholder', () => { assert.throws(() => interpolate('{nope}'), /nope/); });

test('every fact says how it is measured', () => {
  const facts = parse(readFileSync(new URL('data/facts.yml', root), 'utf8'));
  for (const [id, f] of Object.entries(facts)) {
    assert.ok(typeof f.how === 'string' && f.how.trim(), `fact ${id} has no how`);
    assert.ok(f.value !== undefined, `fact ${id} has no value`);
  }
});

function* walk(dir) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = new URL(e.name + (e.isDirectory() ? '/' : ''), dir);
    if (e.isDirectory()) yield* walk(p);
    else if (/\.mdx$/.test(e.name)) yield p;
  }
}

test('the snapshot id is not typed by hand', () => {
  const bad = [];
  for (const p of walk(new URL('content/docs/', root))) {
    if (p.pathname.endsWith('/news.mdx')) continue;
    const body = readFileSync(p, 'utf8').replace(/^---\n[\s\S]*?\n---\n/, '').replace(/```[\s\S]*?```/g, '');
    if (body.includes(snapshotId)) bad.push(p.pathname);
  }
  if (readFileSync(new URL('data/status.yml', root), 'utf8').includes(snapshotId)) bad.push('status.yml');
  assert.deepEqual(bad, []);
});
