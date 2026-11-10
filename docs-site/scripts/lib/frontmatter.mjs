import { parse } from 'yaml';

export function readFrontmatter(text) {
  const m = /^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/.exec(text);
  return m ? (parse(m[1]) ?? {}) : {};
}
