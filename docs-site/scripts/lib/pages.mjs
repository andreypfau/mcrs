import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { readFrontmatter } from './frontmatter.mjs';

function* walk(dir) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) yield* walk(p);
    else if (e.name.endsWith('.mdx')) yield p;
  }
}

const isPath = (s) => !/^(issue|pr):\d+$/.test(s);

export function loadPages(docsDir, repoRoot) {
  const pages = [];
  for (const locale of ['en', 'ru']) {
    const dir = join(docsDir, locale);
    for (const abs of walk(dir)) {
      const fm = readFrontmatter(readFileSync(abs, 'utf8'));
      pages.push({
        file: relative(repoRoot, abs).split('\\').join('/'),
        locale,
        slug: relative(dir, abs).split('\\').join('/').replace(/\.mdx$/, ''),
        sources: (fm.sources ?? []).map(String).filter(isPath),
        verifiedCommit: fm.verified?.commit ? String(fm.verified.commit) : undefined,
        translatedFrom: fm.translated_from ? String(fm.translated_from) : undefined,
      });
    }
  }
  return pages;
}

export function pagesHitBy(pages, changedSet) {
  return pages
    .map((page) => ({ page, hit: page.sources.filter((s) => changedSet.has(s)) }))
    .filter((r) => r.hit.length > 0);
}

function commitsSince(git, from, paths) {
  try {
    return git(['log', '--format=%h', `${from}..HEAD`, '--', ...paths]);
  } catch {
    return 'unknown commit';
  }
}

export function stalePages(pages, git) {
  const out = [];
  for (const page of pages) {
    if (page.locale !== 'en' || !page.verifiedCommit || page.sources.length === 0) continue;
    const r = commitsSince(git, page.verifiedCommit, page.sources);
    if (r === 'unknown commit') out.push({ page, commits: r });
    else if (r.length > 0) out.push({ page, commits: r.length });
  }
  return out;
}

export function laggingTranslations(pages, git) {
  const out = [];
  for (const page of pages) {
    if (page.locale !== 'ru' || !page.translatedFrom) continue;
    const enFile = page.file.replace('/ru/', '/en/');
    const en = commitsSince(git, page.translatedFrom, [enFile]);
    const ru = commitsSince(git, page.translatedFrom, [page.file]);
    if (en === 'unknown commit' || ru === 'unknown commit') {
      out.push({ page, commits: 'unknown commit' });
      continue;
    }
    const n = en.filter((c) => !ru.includes(c)).length;
    if (n > 0) out.push({ page, commits: n });
  }
  return out;
}

export function missingSources(pages, exists) {
  return pages
    .map((page) => ({ page, missing: page.sources.filter((s) => !exists(s)) }))
    .filter((r) => r.missing.length > 0);
}
