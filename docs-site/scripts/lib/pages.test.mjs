import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { laggingTranslations, loadPages, missingSources, pagesHitBy, stalePages } from './pages.mjs';

const en = { file: 'docs-site/src/content/docs/en/a/b.mdx', locale: 'en', slug: 'a/b', sources: ['crates/x.rs'], verifiedCommit: 'aaa1111' };
const ru = { file: 'docs-site/src/content/docs/ru/a/b.mdx', locale: 'ru', slug: 'a/b', sources: [], translatedFrom: 'aaa1111' };

test('pagesHitBy reports a page whose source changed', () => {
  assert.deepEqual(pagesHitBy([en], new Set(['crates/x.rs'])), [{ page: en, hit: ['crates/x.rs'] }]);
});
test('stalePages counts commits to sources after verified.commit', () => {
  assert.deepEqual(stalePages([en], () => ['c1', 'c2']), [{ page: en, commits: 2 }]);
});
test('a page with unchanged sources is not stale', () => {
  assert.deepEqual(stalePages([en], () => []), []);
});
test('an unknown verified commit is reported, not thrown', () => {
  const git = () => { throw new Error('bad revision'); };
  assert.deepEqual(stalePages([en], git), [{ page: en, commits: 'unknown commit' }]);
});
test('a commit that touched both languages is not a lag', () => {
  const git = (args) => args.at(-1).includes('/en/') ? ['c1', 'c2'] : ['c1'];
  assert.deepEqual(laggingTranslations([en, ru], git), [{ page: ru, commits: 1 }]);
});
test('an unknown translated_from is reported, not thrown', () => {
  const git = () => { throw new Error('bad revision'); };
  assert.deepEqual(laggingTranslations([ru], git), [{ page: ru, commits: 'unknown commit' }]);
});
test('missingSources lists paths that do not exist', () => {
  assert.deepEqual(missingSources([en], () => false), [{ page: en, missing: ['crates/x.rs'] }]);
});
test('every source path of the real tree exists', () => {
  const siteRoot = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
  const repoRoot = resolve(siteRoot, '..');
  const pages = loadPages(join(siteRoot, 'src/content/docs'), repoRoot);
  assert.deepEqual(missingSources(pages, (p) => existsSync(join(repoRoot, p))), []);
});
