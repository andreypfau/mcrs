// Runs hand-written expressions through the runner. Each case pins the molangjs result. A
// case with `mol` states the value of the MOL rule where molangjs differs (README.md).
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const cases = [
  { input: { expr: '1 + 2 * 3' }, js: 7 },
  { input: { expr: '(1 + 2) * -(3 - 5)' }, js: 6 },
  { input: { expr: '10 - 3 - 4' }, js: 3 },
  { input: { expr: '1 / 0' }, js: 0 },
  { input: { expr: 'v.unset + 1' }, js: 1 },
  { input: { expr: '(v.unset + 1) ?? 5' }, js: 5 },
  { input: { expr: 'v.x = 0; return v.x ?? 5;' }, js: 0 },
  { input: { expr: 'v.a = 1; v.a ? { v.b = 2; }; return v.b;' }, js: 2 },
  { input: { expr: 'q.is_baby ? 1 : 2', queries: { is_baby: 1 } }, js: 1 },
  { input: { expr: 'q.variant == \'pig\'', queries: { variant: 'pig' } }, js: 1 },
  {
    input: { expr: 'q.head_x_rotation(1)', queries: { head_x_rotation: { default: 3, cases: [{ args: [1], value: 7 }] } } },
    js: 7,
  },
  { input: { expr: 'this * 2', variables: { this: 4 } }, js: 8 },
  { input: { expr: 'math.cos(180) + math.atan2(1, 0)' }, js: 89 },
  { input: { expr: 'v.x = 7' }, js: 0, variables: { 'variable.x': 7 } },
  { input: { expr: 'math.random(0, 1)' }, error: 'random' },
  { input: { expr: '1 || 0 && 0' }, js: 0, mol: 1 },
  { input: { expr: '1 < 2 > 0' }, js: 0, mol: 1 },
  { input: { expr: '1 ? 0 ? 5 : 6 : 7' }, js: 0, mol: 6 },
  { input: { expr: 'v.x = 1; v.x * 2' }, js: 2, mol: 0 },
  { input: { expr: 'math.sqrt(-1) ? 1 : 2' }, js: 2, mol: 1 },
  { input: { expr: 'math.sqrt(-1) + 1' }, js: 0, mol: 'NaN' },
  { input: { expr: 'q.unknown_fn(1) ?? 5' }, js: 0, mol: 5 },
  { input: { expr: '\'A\' == \'a\'' }, js: 1, mol: 0 },
  { input: { expr: 'math.round(-2.5)' }, js: -2, mol: -3 },
  { input: { expr: 'math.sign(0)' }, js: 0, mol: -1 },
  { input: { expr: 'math.clamp(5, 10, 0)' }, js: 10, mol: 0 },
  { input: { expr: 'math.lerprotate(10, 350, 0.25)' }, js: 355, mol: 5 },
  { input: { expr: 'math.ease_in_bounce(0, 1, 0.25)' }, js: 0.1171875, mol: 0.02734375 },
  { input: { expr: '1f + .5' }, js: 0, mol: 1.5 },
  { input: { expr: 't.i = 0; loop(2.5, { t.i = t.i + 1; }); return t.i;' }, js: 3, mol: 2 },
  { input: { expr: '0.1 + 0.2 == 0.3' }, js: 0, mol: 1 },
];

const runner = fileURLToPath(new URL('./runner.mjs', import.meta.url));
const stdin = cases.map((testCase) => JSON.stringify(testCase.input)).join('\n');
const run = spawnSync(process.execPath, [runner], { input: `${stdin}\n`, encoding: 'utf8' });
if (run.status !== 0) {
  console.error(run.stderr);
  process.exit(1);
}
const outputs = run.stdout.trim().split('\n').map((line) => JSON.parse(line));

let failures = 0;
cases.forEach((testCase, index) => {
  const output = outputs[index];
  const ok =
    testCase.error !== undefined
      ? output?.error === testCase.error
      : output?.value === testCase.js &&
        (testCase.variables === undefined ||
          JSON.stringify(output.variables) === JSON.stringify(testCase.variables));
  if (!ok) {
    failures += 1;
    console.error(`FAIL ${testCase.input.expr}: got ${JSON.stringify(output)}`);
  }
});
if (outputs.length !== cases.length) {
  failures += 1;
  console.error(`FAIL: ${outputs.length} output lines for ${cases.length} cases`);
}
const differences = cases.filter((testCase) => testCase.mol !== undefined).length;
console.log(`${cases.length - failures}/${cases.length} cases pass; ${differences} pin a known difference from MOL`);
process.exit(failures === 0 ? 0 : 1);
