import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { parse } from 'yaml';
import { suspicious } from './lib/issues.mjs';

const registry = parse(readFileSync(new URL('../src/data/issues.yml', import.meta.url), 'utf8'));
const list = JSON.parse(execFileSync('gh', ['issue', 'list', '-R', 'andreypfau/mcrs', '--state', 'all', '--limit', '2000', '--json', 'number,state'], { encoding: 'utf8' }));
const gh = Object.fromEntries(list.map((i) => [i.number, i.state]));
const rows = suspicious(registry, gh);

console.log('## Issues whose registry state may be stale\n');
if (rows.length === 0) console.log('None.');
for (const r of rows) console.log(`- #${r.issue}: registry says \`${r.state}\`, GitHub says ${r.github}`);
