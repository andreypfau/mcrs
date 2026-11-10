import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFrontmatter } from './frontmatter.mjs';

test('reads nested fields', () => {
  const fm = readFrontmatter('---\ntitle: A\nsources:\n  - a.rs\n  - issue:5\nverified:\n  commit: abc1234\n---\nbody');
  assert.deepEqual(fm.sources, ['a.rs', 'issue:5']);
  assert.equal(fm.verified.commit, 'abc1234');
});
test('no frontmatter gives an empty object', () => {
  assert.deepEqual(readFrontmatter('just text'), {});
});
test('CRLF line endings', () => {
  assert.equal(readFrontmatter('---\r\ntitle: A\r\n---\r\n').title, 'A');
});
