import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { parse } from 'yaml';
import { fact, interpolate } from './facts.mjs';
import { walk } from '../../scripts/lib/pages.mjs';
import { body as stripFrontmatter } from '../../scripts/lib/frontmatter.mjs';

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


test('the snapshot id is not typed by hand', () => {
  const bad = [];
  for (const p of walk(fileURLToPath(new URL('content/docs/', root)))) {
    if (p.endsWith('/news.mdx')) continue;
    const body = stripFrontmatter(readFileSync(p, 'utf8')).replace(/```[\s\S]*?```/g, '');
    if (body.includes(snapshotId)) bad.push(p);
  }
  if (readFileSync(new URL('data/status.yml', root), 'utf8').includes(snapshotId)) bad.push('status.yml');
  assert.deepEqual(bad, []);
});
