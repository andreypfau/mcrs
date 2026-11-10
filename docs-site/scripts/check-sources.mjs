#!/usr/bin/env node
// Warns (never fails) about doc pages whose frontmatter `sources` changed.
// Usage: node check-sources.mjs [changed files...] warns by PR diff against CHECK_SOURCES_BASE.
//        node check-sources.mjs --stale prints a Markdown report of stale pages, lagging translations and missing sources.
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { laggingTranslations, loadPages, missingSources, pagesHitBy, stalePages } from './lib/pages.mjs';

const siteRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const docsDir = join(siteRoot, 'src/content/docs');
const repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], { cwd: siteRoot, encoding: 'utf8' }).trim();
const run = (args, stderr) => execFileSync('git', args, { cwd: repoRoot, encoding: 'utf8', stdio: ['ignore', 'pipe', stderr] }).split('\n').filter(Boolean);
const git = (args) => run(args, 'ignore');

if (!existsSync(docsDir)) process.exit(0);
const pages = loadPages(docsDir, repoRoot);
const args = process.argv.slice(2);

if (args.includes('--stale')) {
  const section = (title, rows) => [`## ${title}`, '', ...(rows.length ? rows : ['None.']), ''];
  const commits = (n) => (typeof n === 'number' ? `${n} commit(s)` : n);
  const lines = [
    '# Docs freshness',
    '',
    ...section('Stale English pages', stalePages(pages, git).map((r) => `- \`${r.page.file}\`: ${commits(r.commits)} to sources since \`${r.page.verifiedCommit}\``)),
    ...section('Lagging Russian pages', laggingTranslations(pages, git).map((r) => `- \`${r.page.file}\`: ${commits(r.commits)} to English since \`${r.page.translatedFrom}\``)),
    ...section('Missing source paths', missingSources(pages, (p) => existsSync(join(repoRoot, p))).map((r) => `- \`${r.page.file}\`: ${r.missing.map((m) => `\`${m}\``).join(', ')}`)),
  ];
  console.log(lines.join('\n'));
  process.exit(0);
}

let changed = args;
if (changed.length === 0) {
  const base = process.env.CHECK_SOURCES_BASE || 'origin/main';
  changed = run(['diff', '--name-only', `${base}...HEAD`], 'inherit');
}
const hits = pagesHitBy(pages, new Set(changed));
for (const { page, hit } of hits) {
  console.log(`::warning file=${page.file}::Source changed in this PR: ${hit.join(', ')}. Check that this page still matches, or update it in the same PR.`);
}
console.log(`check-sources: ${hits.length} page(s) to review`);
