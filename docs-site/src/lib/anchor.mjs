// A multi-line anchor matches consecutive lines: the first part ends the first line, the others start the next lines.
function matchAt(lines, i, anchor) {
  const parts = anchor.split("\n");
  if (parts.length === 1) return lines[i].includes(anchor);
  return parts.every((p, j) => lines[i + j] !== undefined && (j === 0 ? lines[i + j].endsWith(p) : lines[i + j].startsWith(p)));
}

export function extractByAnchor(text, from, to) {
  const lines = text.replace(/\n$/, "").split("\n");
  const hits = lines.flatMap((_, i) => (matchAt(lines, i, from) ? [i] : []));
  if (hits.length === 0) throw new Error(`anchor not found: ${JSON.stringify(from)}`);
  if (hits.length > 1) throw new Error(`anchor ${JSON.stringify(from)} found more than once (${hits.length} lines)`);
  const start = hits[0];
  let end = -1;
  if (to !== undefined) {
    const found = lines.findIndex((_, i) => i > start && matchAt(lines, i, to));
    if (found < 0) throw new Error(`end anchor ${JSON.stringify(to)} not found after ${JSON.stringify(from)}`);
    end = found + to.split("\n").length - 1;
  } else {
    const last = start + from.split("\n").length - 1;
    let depth = 0;
    for (let i = start; i < lines.length && end < 0; i++) {
      for (const ch of lines[i]) depth += ch === "{" ? 1 : ch === "}" ? -1 : 0;
      if (i >= last && depth === 0) end = i;
    }
    if (end < 0) throw new Error(`block opened at ${JSON.stringify(from)} never closes`);
  }
  const slice = lines.slice(start, end + 1);
  const indent = Math.min(...slice.filter((l) => l.trim()).map((l) => /^\s*/.exec(l)[0].length));
  return { code: slice.map((l) => l.slice(indent)).join("\n"), from: start + 1, to: end + 1 };
}
