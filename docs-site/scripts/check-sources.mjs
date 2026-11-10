#!/usr/bin/env node
// Warns (never fails) about doc pages whose frontmatter `sources` include a file changed in the PR.
// Usage: node check-sources.mjs [changed files...]; without args uses git diff against CHECK_SOURCES_BASE.
import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const siteRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const docsDir = join(siteRoot, 'src/content/docs');
const repoRoot = execFileSync('git', ['rev-parse', '--show-toplevel'], { cwd: siteRoot, encoding: 'utf8' }).trim();

const base = process.env.CHECK_SOURCES_BASE || 'origin/main';
let changed = process.argv.slice(2);
if (changed.length === 0) {
  changed = execFileSync('git', ['diff', '--name-only', `${base}...HEAD`], { cwd: repoRoot, encoding: 'utf8' })
    .split('\n').filter(Boolean);
}
const changedSet = new Set(changed);

function* walk(dir) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* walk(p);
    else if (e.name.endsWith('.mdx')) yield p;
  }
}

// shortcut: line-based parse of the `sources:` list only, upgrade to a YAML parser if the schema gets nested.
function sourcesOf(text) {
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!m) return [];
  const out = [];
  let inList = false;
  for (const line of m[1].split(/\r?\n/)) {
    if (/^sources:\s*$/.test(line)) { inList = true; continue; }
    if (inList) {
      const item = line.match(/^\s+-\s+["']?([^"'#]+?)["']?\s*(#.*)?$/);
      if (item) out.push(item[1]);
      else if (/^\S/.test(line)) inList = false;
    }
  }
  return out;
}

let count = 0;
if (!existsSync(docsDir)) process.exit(0);
for (const file of walk(docsDir)) {
  const hit = sourcesOf(readFileSync(file, 'utf8')).filter((s) => changedSet.has(s));
  if (hit.length === 0) continue;
  count++;
  const rel = relative(repoRoot, file);
  console.log(`::warning file=${rel}::Source changed in this PR: ${hit.join(', ')}. Check that this page still matches, or update it in the same PR.`);
}
console.log(`check-sources: ${count} page(s) to review`);
