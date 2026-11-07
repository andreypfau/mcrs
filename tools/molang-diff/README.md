# molang-diff

A second-opinion Molang evaluator for differential tests of `mcrs_molang`. The runner evaluates
expressions with [molangjs](https://github.com/JannisX11/molangjs) (MIT), the Molang
implementation of Blockbench. The dependency is pinned to `1.7.1`. The `src/` of the published
`1.7.1` package is equal to commit `0b17bc32fa12` of the repository, except for line endings.

molangjs is not a reference. It has defects, and it uses other number rules than MOL
(`docs/spec/molang.md`). A fuzz harness must skip or normalise each difference of section 3.
A difference that is not in that section is a finding: check it against MOL first.

## 1. Use

```sh
cd tools/molang-diff
npm ci
node runner.mjs < cases.jsonl > results.jsonl
npm test   # the smoke test
```

## 2. Protocol

The runner reads one JSON object for each line of stdin. It writes one JSON object for each
input line, in input order, on stdout.

### 2.1 Input

| Member | Type | Meaning |
|--------|------|---------|
| `expr` | string or number | the expression |
| `variables` | object, optional | initial values: `v.x`, `variable.x`, `t.x`, `c.x` or `this` → number or string; `null` is missing |
| `queries` | object, optional | query values: `name` or `q.name` → number, string or `{ "default": value, "cases": [{ "args": [...], "value": value }] }` |

- A name can use the short or the long namespace. The runner folds names to lower case.
- A query with a number or a string value answers each call, with or without arguments.
- A query with `cases` answers a call with the value of the first case whose `args` are equal
  to the arguments. Each other call gets `default`. A string argument has no quotes.
- A string value goes to molangjs as a string literal, with its quotes. molangjs compares a
  string only in this form.
- `this` has no special meaning in molangjs. The runner reads it as the variable `this`.

### 2.2 Output

| Output | Meaning |
|--------|---------|
| `{"value": v, "variables": {...}}` | the result, and each `variable.` value that the run stored |
| `{"error": "random"}` | the expression calls `math.random`, `math.random_integer`, `math.die_roll` or `math.die_roll_integer` |
| `{"error": "non-number result"}` | molangjs gave a value that is not a number |
| `{"error": "bad input line"}` | the line is not a JSON object with `expr` |
| `{"error": "exception: …"}` | molangjs threw an exception |

A number is a JSON number, or one of the strings `"NaN"`, `"Infinity"`, `"-Infinity"` and
`"-0"`.

### 2.3 Determinism

- Each line gets a new parser with no cache. No variable goes from one line to the next.
- `Math.random` throws. An expression with a random call gives the error `random` and no value.
  A parity test must not compare random draws (MOL-RAND-001).

## 3. Known differences from MOL

The IDs `JS-nn` are local to this file. Each entry names the MOL or MOLQ rule, the behaviour of
molangjs and the action of a fuzz harness. The smoke test pins one example of most entries.

### 3.1 Numbers

| ID | Rule | molangjs | Example: molangjs / MOL | Harness action |
|----|------|----------|-------------------------|----------------|
| JS-01 | MOL-NUM-001 | each value is an f64; literals and operations do not round to f32 | `0.1 + 0.2 == 0.3`: 0 / 1; `16777217 + 0`: 16777217 / 16777216 | compare within an f32 tolerance; skip exact-equality tests of fractions and values outside the f32 range |
| JS-02 | MOL-NUM-002, MOL-EVAL-002 (`-a`) | the result of the whole expression is `x \|\| 0`, so NaN and `-0` become 0; a bare number literal keeps `-0` | `math.sqrt(-1) + 1`: 0 / NaN; `-(0)`: 0 / -0 | map a MOL result NaN or -0 to 0 before the compare |
| JS-03 | MOL 0.1 "Truthy" | NaN is not truthy in `? :`, `&&`, `\|\|` and the binary `?` | `math.sqrt(-1) ? 1 : 2`: 2 / 1 | skip a case where a condition operand is NaN |
| JS-04 | MOL-EVAL-002 (`-a`) | `-a` is `0 - a`, so `-0` of a variable is `+0` | `v.z = 0; return -v.z;`: 0 / -0 | covered by JS-02 at the top; inside, compare `+0` and `-0` as equal |

### 3.2 Syntax

| ID | Rule | molangjs | Example: molangjs / MOL | Harness action |
|----|------|----------|-------------------------|----------------|
| JS-05 | MOL-SYN-002 | `&&` has a lower precedence than `\|\|` | `1 \|\| 0 && 0`: 0 / 1 | put parentheses around each `&&` operand of `\|\|` and each `\|\|` operand of `&&` |
| JS-06 | MOL-SYN-002 | a chain of comparisons splits by operator kind (`==`, `!=`, `<=`, `<`, `>=`, `>`), not left to right | `1 < 2 > 0`: 0 / 1; `2 == 2 != 0`: 0 / 1 | put parentheses around each comparison that is an operand of a comparison |
| JS-07 | MOL-SYN-001 (`conditional`) | the first top-level `:` ends the then-branch | `1 ? 0 ? 5 : 6 : 7`: 0 / 6 | put parentheses around a conditional in a then-branch |
| JS-08 | MOL-LEX-002 | `1f` and `.5` are not numbers; they read as unknown variables | `1f + .5`: 0 / 1.5 | write `1` and `0.5`; `0.5f` works |
| JS-09 | MOL-LEX-001, MOL-LEX-002 item 4 | the parser folds the whole text to lower case and removes each whitespace character, also inside a string literal | `'A' == 'a'`: 1 / 0; `'a b' == 'ab'`: 1 / 0 | use string literals with lower-case letters and no whitespace |
| JS-10 | MOLQ-HOST-002 item 5 | `q.name()` with an empty argument list reads the variable `query.name()` | `q.is_baby() + 1` with `is_baby = 1`: 1 without the runner alias / 2 | the runner defines the alias for number and string queries; for `cases`, write `q.name` |
| JS-11 | MOL-LOAD-004 | no load errors: an unknown `math.` name, an unknown name, an assignment to `c.` and a string operand of arithmetic give 0 or have no effect | `math.foo(1)`: 0 / load error; `'a' + 1`: 1 / load error | skip each case where MOL reports a load error |
| JS-12 | MOL-SYN-001 (`->`, arrays, resources), MOL-STMT-005 | `->`, `for_each`, `array.`, `geometry.`, `material.` and `texture.` are not supported | — | do not generate these constructs |

### 3.3 Statements and variables

| ID | Rule | molangjs | Example: molangjs / MOL | Harness action |
|----|------|----------|-------------------------|----------------|
| JS-13 | MOL-STMT-001 item 4, MOL-SYN-001 postcondition 4 | a statement list gives the value of its last statement; a final `;` is dropped, so `1;` is simple | `v.x = 1; v.x * 2`: 2 / 0; `1;`: 1 / 0 | end each complex expression with `return`; a single assignment gives 0 in both |
| JS-14 | MOL-EVAL-004 item 4, MOL-STMT-003 | a block gives the value of its last statement | `(1 ? { 5; }) + 1`: 6 / 1 | do not use the value of a block |
| JS-15 | MOL-EVAL-005 item 2, MOL-EVAL-006 item 2 | a query call with no value gives 0, not a missing value; only a variable or a query with no argument list can be missing | `q.unknown_fn(1) ?? 5`: 0 / 5 | supply each query call that the case uses, or skip `??` over a query call |
| JS-16 | MOL-STMT-002 item 4 | an assignment of a missing value stores 0, so the variable is not missing | `v.y = v.unset; return v.y ?? 5;`: 0 / 5 | skip `??` over a variable that a missing value wrote; MOL keeps this rule as an open question |
| JS-17 | MOL-VAR-005 | `v.a.b = x` is not an assignment; the statement reads `v.a.b` | `v.a.b = 3; return v.a.b;`: 0 / 3 | do not write struct paths |
| JS-18 | MOL-VAR-004 | the parser stores `temp.` values with the entity variables, so they survive to the next run | — | the runner makes a new parser for each line; a direct user of molangjs must reset the variables |
| JS-19 | MOL-STMT-004 item 2 | the loop runs `ceil(n)` times for a fraction `n`, not `trunc(n)` | `loop(2.5, …)`: 3 passes / 2 | use integer counts |

### 3.4 Math functions

| ID | Rule | molangjs | Example: molangjs / MOL | Harness action |
|----|------|----------|-------------------------|----------------|
| JS-20 | MOL-MATH-002 `round` | a half rounds toward +∞ | `math.round(-2.5)`: -2 / -3 | skip arguments with a fraction of exactly 0.5 |
| JS-21 | MOL-MATH-002 `mod` | `mod(a, 0)` is NaN | `math.mod(5, 0) + 1`: 0 (NaN, JS-02) / 1 | skip a zero divisor |
| JS-22 | MOL-MATH-002 `min`, `max` | a NaN operand gives NaN | `math.min(math.sqrt(-1), 3)`: 0 (NaN) / 3 | skip NaN operands |
| JS-23 | MOL-MATH-002 `clamp` | NaN gives 0; with `lo > hi` the result is `lo` | `math.clamp(5, 10, 0)`: 10 / 0 | use `lo ≤ hi` and no NaN |
| JS-24 | MOL-MATH-002 `sign` | `sign(0)` is 0, `sign(-0)` is -0, `sign(NaN)` is NaN | `math.sign(0)`: 0 / -1 | skip arguments that are not greater or less than 0 |
| JS-25 | MOL-MATH-002 `copy_sign` | the result is `abs(a) * sign(b)`, so `b = 0` gives 0 | `math.copy_sign(3, 0)`: 0 / 3 | skip `b = ±0` and NaN |
| JS-26 | MOL-MATH-002 `inverse_lerp` | `b = a` gives ±∞ or NaN | `math.inverse_lerp(2, 2, 3) + 1`: Infinity / 1 | skip `b = a` |
| JS-27 | MOL-MATH-002 `lerprotate`, `min_angle` | `lerprotate` uses another algorithm: it starts at the smaller angle of the pair, and the range is `[0, 360)` | `math.lerprotate(10, 350, 0.25)`: 355 / 5 | skip `lerprotate`; `min_angle` agrees for finite arguments |
| JS-28 | MOL-MATH-002 `pow` | `**` of JavaScript, not `powf`: `pow(1, NaN)` and `pow(-1, ±∞)` are NaN | `math.pow(1, math.sqrt(-1)) == 1`: 0 / 1 | skip a NaN or infinite exponent |
| JS-29 | MOL-MATH-003 | `ease_in_bounce` computes `ease_in_out_bounce`; `ease_out_elastic`, `ease_in_out_elastic` and `ease_in_out_back` use other formulas | `math.ease_in_bounce(0, 1, 0.25)`: 0.1171875 / 0.02734375 | skip these four functions; the other easing functions agree |
| JS-30 | MOL-NUM-004 | trigonometry in f64 with no range reduction | `math.cos(360000.5)` | compare within the error of MOL-NUM-004 |
| JS-31 | MOL-RAND-001 | random functions | — | the runner gives the error `random` |

### 3.5 Agreements

molangjs agrees with MOL on these rules, within JS-01 and JS-02. The harness can compare them
directly:

- division by `0` or `-0` gives 0 (MOL-EVAL-002);
- a comparison gives 1 or 0, and a comparison with NaN gives 0, except `!=` (MOL-EVAL-002);
- `!NaN` is 0 (MOL-EVAL-002);
- `&&`, `||` and `? :` evaluate only the operands that MOL evaluates (MOL-EVAL-003,
  MOL-EVAL-004);
- the binary `c ? a` gives 0 when `c` is not truthy (MOL-EVAL-004);
- `??` selects the right side for a missing variable at any depth, and keeps a stored 0
  (MOL-EVAL-005);
- an unknown variable reads as 0 (MOL-EVAL-006);
- `return` in a block ends the run (MOL-STMT-003);
- the `math.` functions take and give degrees (MOL-MATH-002), and `atan2(y, x)` has the MOL
  argument order;
- `in_range`, `approx_eq` (tolerance `0.0000001`), `all` and `any` (MOLQ-PURE-001);
- the easing functions of MOL-MATH-003 other than those of JS-29.
