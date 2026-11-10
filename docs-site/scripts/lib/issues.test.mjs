import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { suspicious } from './issues.mjs';

test('a closed issue still marked wip is suspicious', () => {
  assert.deepEqual(suspicious({ 78: { state: 'wip' } }, { 78: 'CLOSED' }), [{ issue: 78, state: 'wip', github: 'CLOSED' }]);
});
test('an open declined issue is suspicious', () => {
  assert.equal(suspicious({ 437: { state: 'declined' } }, { 437: 'OPEN' }).length, 1);
});
test('open wip and closed works are fine', () => {
  assert.deepEqual(suspicious({ 78: { state: 'wip' }, 5: { state: 'works' } }, { 78: 'OPEN', 5: 'CLOSED' }), []);
});
test('an issue unknown to GitHub is reported as missing', () => {
  assert.equal(suspicious({ 9: { state: 'wip' } }, {})[0].github, 'missing');
});

const docs = fileURLToPath(new URL('../../src/content/docs', import.meta.url));
function* mdx(dir) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* mdx(p);
    else if (e.name.endsWith('.mdx')) yield p;
  }
}
const prose = (p) => readFileSync(p, 'utf8').replace(/```[\s\S]*?```/g, '').replace(/`[^`\n]*`/g, '');
const offenders = (re) => [...mdx(docs)].filter((p) => re.test(prose(p)));

test('no issue carries a state in the content', () => {
  assert.deepEqual(offenders(/issue=\{\d+\}\s+state=/), []);
});
test('no bare issue or PR links in the content', () => {
  assert.deepEqual(offenders(/\[(PR )?#\d+\]\(https:\/\/github\.com\/andreypfau\/mcrs\/(issues|pull)\/\d+\)/), []);
});
