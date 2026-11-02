#!/usr/bin/env node
// Checks every site/i18n/*.json against the keys index.html uses. Run: node scripts/check-site-i18n.mjs
import { existsSync, readdirSync, readFileSync } from 'node:fs';

const site = new URL('../site/', import.meta.url);
const read = (path) => readFileSync(new URL(path, site), 'utf8');
const html = read('index.html');
const problems = [];

const block = html.match(/<script type="application\/json" id="i18n-en">([\s\S]*?)<\/script>/);
if (!block) {
  console.error('site/index.html: no #i18n-en block');
  process.exit(1);
}
const english = JSON.parse(block[1]);

const VOID_TAGS = new Set(['br', 'hr', 'img', 'input', 'link', 'meta']);
const childCount = (start) => {
  let depth = 0;
  let count = 0;
  for (const [, closing, name] of html.slice(start).matchAll(/<(\/?)([a-z][a-z0-9]*)\b[^>]*>/gi)) {
    if (closing) {
      if (depth === 0) return count;
      depth -= 1;
    } else {
      if (depth === 0) count += 1;
      if (!VOID_TAGS.has(name.toLowerCase())) depth += 1;
    }
  }
  return count;
};

const markup = [...html.matchAll(/<[a-z][a-z0-9]*\b[^>]*\sdata-i18n="([^"]+)"[^>]*>/gi)]
  .map((m) => ({ key: m[1], children: childCount(m.index + m[0].length) }));
const attributeKeys = [...html.matchAll(/\sdata-i18n-[a-z-]+="([^"]+)"/g)].map((m) => m[1]);
const markupKeys = new Set([...markup.map((m) => m.key), ...attributeKeys]);
const keys = new Set([...markupKeys, ...Object.keys(english)]);

const i18nJs = read('i18n.js');
const attrs = [...i18nJs.match(/ATTRS\s*=\s*\[([^\]]*)\]/)[1].matchAll(/'([^']+)'/g)].map((m) => m[1]);
for (const [, attr] of html.matchAll(/\sdata-i18n-([a-z-]+)="[^"]+"/g)) {
  if (!attrs.includes(attr)) problems.push(`site/index.html: data-i18n-${attr} is not in ATTRS in site/i18n.js`);
}

// t('literal') and t(`template.${x}`) in site.js; each ${...} stands for any text.
const escapeRegExp = (text) => text.replace(/[.*+?^$()[\]{}|\\]/g, '\\$&');
const uses = [...read('site.js').matchAll(/\bt\(\s*(?:'([^']+)'|`([^`]+)`)/g)].map(([, literal, template]) => {
  const source = literal ?? template;
  return { source, pattern: new RegExp(`^${source.split(/\$\{[^}]*\}/).map(escapeRegExp).join('.+')}$`) };
});
for (const { source, pattern } of uses) {
  if (!Object.keys(english).some((key) => pattern.test(key))) problems.push(`site/site.js: t(${source}) matches no key in #i18n-en`);
}

// What site.js fills its templates with. Stages and layers come from the markup; a new shard state or lane needs a line here.
const stages = [...html.matchAll(/data-go="(\d+)"/g)].map((m) => m[1]);
const built = [
  ...stages.flatMap((n) => [`stage.${n}.name`, `stage.${n}.desc`]),
  ...stages.filter((n) => n !== '4').flatMap((n) => ['read1', 'read2'].flatMap((r) => [`stage.${n}.${r}.key`, `stage.${n}.${r}.value`])),
  ...['apart', 'linked', 'splitting'].map((state) => `shards.act.${state}`),
  ...['apart', 'linked'].flatMap((state) => [`shards.read.${state}`, `shards.outcome.${state}`]),
  ...['a', 'b', 'ab'].map((shard) => `shard.${shard}`),
  ...['core', 'edge', 'nodeA', 'nodeB'].map((lane) => `lane.${lane}`),
  ...[...html.matchAll(/data-layer="([^"]+)"/g)].map((m) => `layer.${m[1]}`),
];
for (const key of new Set(built)) {
  if (!(key in english)) problems.push(`site/index.html: #i18n-en has no ${key}, which site/site.js builds at run time`);
}
for (const key of Object.keys(english)) {
  if (!markupKeys.has(key) && !uses.some(({ pattern }) => pattern.test(key))) problems.push(`site/index.html: #i18n-en key ${key} is used by nothing`);
}

const forms = (value) => (typeof value === 'string' ? [value] : Object.values(value));
const placeholders = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join();
const TOKEN = /<(\d+)(?:\/>|>(.*?)<\/\1>)/gs; // the same tokens build() in site/i18n.js renders
const children = new Map(markup.map((m) => [m.key, m.children]));

const codes = [...i18nJs.match(/LANGUAGES\s*=\s*\[([\s\S]*?)\];/)[1].matchAll(/code:\s*'([^']+)'/g)].map((m) => m[1]);
const files = readdirSync(new URL('i18n/', site)).filter((f) => f.endsWith('.json')).map((f) => f.slice(0, -'.json'.length));
for (const code of codes) {
  if (code !== 'en' && !existsSync(new URL(`i18n/${code}.json`, site))) problems.push(`LANGUAGES has ${code} but site/i18n/${code}.json does not exist`);
}
for (const code of files) {
  if (!codes.includes(code)) problems.push(`site/i18n/${code}.json has no entry in LANGUAGES`);
}

for (const code of files) {
  const label = `${code}.json`;
  const dict = JSON.parse(read(`i18n/${code}.json`));

  for (const key of keys) if (!(key in dict)) problems.push(`${label}: missing key ${key}`);
  for (const key of Object.keys(dict)) if (!keys.has(key)) problems.push(`${label}: stale key ${key}`);

  const categories = new Intl.PluralRules(code).resolvedOptions().pluralCategories;
  for (const [key, value] of Object.entries(dict)) {
    if (typeof value === 'object') {
      for (const category of categories) if (!(category in value)) problems.push(`${label}: ${key} has no plural form "${category}"`);
    }
    if (key in english) {
      const expected = placeholders(forms(english[key])[0]);
      for (const form of forms(value)) {
        if (placeholders(form) !== expected) problems.push(`${label}: ${key} has placeholders {${placeholders(form)}}, English has {${expected}}: "${form}"`);
      }
    }
  }

  for (const [key, value] of Object.entries(dict)) {
    if ((typeof value === 'object') !== (typeof english[key] === 'object')) problems.push(`${label}: ${key} is ${typeof value === 'object' ? 'a plural, English is not' : 'not a plural, English is'}`);
    const expected = Array.from({ length: children.get(key) ?? 0 }, (_, i) => i).join();
    for (const form of forms(value)) {
      if (!form.trim()) problems.push(`${label}: ${key} is empty`);
      if (/<\/?\d/.test(form.replace(TOKEN, ''))) problems.push(`${label}: ${key} has a broken child tag: "${form}"`);
      const used = [...form.matchAll(TOKEN)].map((m) => Number(m[1])).sort((a, b) => a - b).join();
      if (used !== expected) problems.push(`${label}: ${key} uses child tags [${used}], its element has children [${expected}]`);
    }
  }
}

if (problems.length) {
  console.error(`${problems.length} problem(s):\n${problems.map((p) => `  ${p}`).join('\n')}`);
  process.exit(1);
}
console.log(`OK: ${keys.size} keys, languages: ${codes.join(', ')}`);
