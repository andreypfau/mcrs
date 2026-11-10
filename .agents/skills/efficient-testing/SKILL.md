---
name: efficient-testing
description: Select and write high-value regression tests for code changes without slowing development. Use when implementing features, fixing bugs, refactoring, reviewing generated tests, or deciding what tests to run.
---

# Efficient Testing

## Objective

Maximize detection of meaningful regressions per unit of generation, execution, and maintenance cost. Passing tests, coverage, and mutation scores are signals, not proof of correctness.

## Workflow

1. **Classify the change.** Identify externally observable contracts, affected callers, and realistic failure modes. Use the lowest adequate verification tier:
   - **Non-behavioral** (docs, comments, formatting): no new tests; run only relevant static checks when appropriate.
   - **Behavior-preserving refactor**: run existing tests; add a test only if an important contract is currently unprotected.
   - **Bug fix**: prefer a focused regression test that fails on the old implementation and passes on the fix.
   - **New behavior**: cover the new contract and the most important boundary or failure mode.
   - **High risk** (parsers, protocols, concurrency, persistence, security, money, complex algorithms): add property, differential, integration, or targeted mutation checks where they provide distinct value.
2. **Inspect existing coverage first.** Search for tests that already detect the relevant failure. Do not create duplicates just to increase counts.
3. **Choose an independent oracle.** Derive expected outcomes from requirements, protocol specifications, known vectors, independently implemented references, or valid invariants. Never use the current implementation as the sole source of expected values.
4. **Write the smallest discriminating test.** Each new test must be able to fail for a plausible incorrect implementation. Prefer public behavior to private implementation details. Use mocks only at genuine boundaries; verify interactions only when they are contractual.
5. **Run narrow checks first.** Run affected tests and appropriate type/lint/build checks; expand scope for shared contracts and integration risks. Do not repeatedly rerun an unchanged full suite without a reason.
6. **Diagnose failures, do not game them.** Distinguish production defects, wrong assertions, stale requirements, flaky environments, and tooling issues. Never weaken an assertion, alter fixtures, patch the application from a test, or modify production behavior solely to manufacture a pass.
7. **Stop when marginal value is low.** Do not add more tests if they cover no distinct contract, boundary, invariant, or regression risk. Report what was checked and what remains unverified.

## Test acceptance filter

Before adding a test, answer internally:

- **Contract:** What externally meaningful claim does it check?
- **Counterexample:** What plausible bug would cause it to fail?
- **Oracle:** Why is its expected result correct independently of the implementation?
- **Novelty:** Which important failure does it catch that existing tests would miss?
- **Cost:** Is its runtime and maintenance burden justified?

If there is no convincing answer, revise or omit the test. Do not add verbose justifications to test source files.

## Guardrails

- Do not require a fixed number of tests per change, or target a coverage percentage as an end in itself.
- `is_ok()`, non-empty assertions, mocks, and snapshots are not intrinsically bad; they are insufficient only when they fail to check the actual contract.
- Prefer a small set of complementary cases over many near-identical examples; parameterize meaningful equivalence classes where useful.
- Do not lock tests to incidental ordering, internal call counts, or private structure unless the behavior is contractual.
- For regressions, verify red-before-green when practical; if not possible, explain the limitation rather than claim it happened.
- Property-based tests require valid properties. Round-trip properties alone can miss matching encoder/decoder defects; supplement with known vectors or independent references when warranted.
- Surviving mutants may be equivalent or irrelevant. Inspect them; never chase a mutation score blindly.
- For high-risk changes, a fast local feedback budget is not permission to skip mandatory checks. Run longer verification in CI before merge when required.
- Separate test design from implementation details when practical, but do not spawn additional agents merely to write tests.

## Output

Keep completion reports concise:

- **Verified:** observable behavior and checks run.
- **Not verified:** meaningful gaps or unrun checks.
- **Tests added:** only when they provide distinct fault-detection value.

## Examples

See [bad-tests.md](examples/bad-tests.md) and [good-tests.md](examples/good-tests.md).
