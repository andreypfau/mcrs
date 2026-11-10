const closedStates = new Set(['wip', 'partial', 'undecided']);

export function suspicious(registry, gh) {
  const out = [];
  for (const [key, { state }] of Object.entries(registry)) {
    const github = gh[key] ?? 'missing';
    if (github === 'missing' || (github === 'CLOSED' && closedStates.has(state)) || (github === 'OPEN' && state === 'declined')) {
      out.push({ issue: Number(key), state, github });
    }
  }
  return out;
}
