#!/usr/bin/env node
// Reads JSON lines {expr, variables, queries} on stdin and writes one JSON line
// {value, variables} or {error} on stdout for each input line, in input order.
import { createInterface } from 'node:readline';
import Molang from 'molangjs/src/molang.js';

class RandomDraw extends Error {}

const RANDOM_CALL = /\bmath\.(random|random_integer|die_roll|die_roll_integer)\s*\(/i;

// A random draw makes a result that no other implementation can reproduce.
Math.random = () => {
  throw new RandomDraw('random');
};

const PREFIXES = { q: 'query', v: 'variable', t: 'temp', c: 'context' };

function longName(name) {
  const lower = name.toLowerCase();
  const dot = lower.indexOf('.');
  if (dot < 0) return lower;
  const head = lower.slice(0, dot);
  return (PREFIXES[head] ?? head) + lower.slice(dot);
}

// molangjs keeps a string value with its quotes, as the parser keeps a string literal.
function encodeInput(value) {
  return typeof value === 'string' ? `'${value}'` : value;
}

function encodeNumber(value) {
  if (Number.isNaN(value)) return 'NaN';
  if (value === Infinity) return 'Infinity';
  if (value === -Infinity) return '-Infinity';
  if (Object.is(value, -0)) return '-0';
  return value;
}

function sameArgs(left, right) {
  return left.length === right.length && left.every((arg, index) => arg === right[index]);
}

function queryValue(spec, args) {
  if (spec === null || typeof spec !== 'object') return encodeInput(spec);
  const hit = (spec.cases ?? []).find((entry) => sameArgs(entry.args ?? [], args));
  return encodeInput(hit ? hit.value : spec.default);
}

function evaluate(line) {
  let input;
  try {
    input = JSON.parse(line);
  } catch {
    return { error: 'bad input line' };
  }
  const { expr } = input;
  if (typeof expr !== 'string' && typeof expr !== 'number') return { error: 'bad input line' };
  if (typeof expr === 'string' && RANDOM_CALL.test(expr)) return { error: 'random' };

  const context = {};
  for (const [name, value] of Object.entries(input.variables ?? {})) {
    if (value !== null) context[longName(name)] = encodeInput(value);
  }
  const queries = {};
  for (const [name, spec] of Object.entries(input.queries ?? {})) {
    const key = longName(name.includes('.') ? name : `query.${name}`);
    queries[key] = spec;
    if (spec !== null && typeof spec !== 'object') {
      context[key] = encodeInput(spec);
      // molangjs reads `q.name()` as a variable with the name `query.name()`.
      context[`${key}()`] = encodeInput(spec);
    }
  }

  const parser = new Molang();
  parser.cache_enabled = false;
  parser.variableHandler = (name, _context, args) =>
    args !== undefined && name in queries ? queryValue(queries[name], args) : undefined;

  let value;
  try {
    value = parser.parse(expr, context);
  } catch (error) {
    return { error: error instanceof RandomDraw ? 'random' : `exception: ${error.message}` };
  }
  if (typeof value !== 'number') return { error: 'non-number result' };

  const variables = {};
  for (const [name, stored] of Object.entries(parser.variables)) {
    if (name.startsWith('variable.')) {
      variables[name] = typeof stored === 'number' ? encodeNumber(stored) : String(stored);
    }
  }
  return { value: encodeNumber(value), variables };
}

const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
for await (const line of lines) {
  process.stdout.write(`${JSON.stringify(evaluate(line))}\n`);
}
