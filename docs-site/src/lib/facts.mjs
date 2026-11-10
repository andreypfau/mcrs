import { readFileSync } from 'node:fs';
import { parse } from 'yaml';

// Both npm test and astro build run with the working directory at docs-site/.
const facts = parse(readFileSync('src/data/facts.yml', 'utf8'));
const version = () => JSON.parse(readFileSync('../assets/minecraft/version.json', 'utf8'));

export function fact(id) {
  if (id === 'snapshot') return version().id;
  const f = facts[id];
  if (!f || f.value === undefined) throw new Error(`unknown fact "${id}" (see src/data/facts.yml)`);
  return f.value;
}

export function interpolate(text) {
  return text.replace(/\{([a-z_0-9]+)\}/g, (_, id) => String(fact(id)));
}
