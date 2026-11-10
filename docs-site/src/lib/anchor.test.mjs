import test from "node:test";
import assert from "node:assert/strict";
import { extractByAnchor } from "./anchor.mjs";

const src = ['use a;', 'pub fn one() {', '    let x = 1;', '    if x > 0 {', '        y();', '    }', '}', '', 'const N: u8 = 1;', 'fn two() {}'].join('\n');

test('without `to` takes the block opened on the anchor line', () => {
  assert.deepEqual(extractByAnchor(src, 'pub fn one'), { code: 'pub fn one() {\n    let x = 1;\n    if x > 0 {\n        y();\n    }\n}', from: 2, to: 7 });
});
test('a line without a block is one line', () => {
  assert.deepEqual(extractByAnchor(src, 'const N'), { code: 'const N: u8 = 1;', from: 9, to: 9 });
});
test('a one-line block is one line', () => {
  assert.deepEqual(extractByAnchor(src, 'fn two'), { code: 'fn two() {}', from: 10, to: 10 });
});
test('`to` ends the fragment and the common indent is dropped', () => {
  assert.deepEqual(extractByAnchor(src, 'let x', 'y();'), { code: 'let x = 1;\nif x > 0 {\n    y();', from: 3, to: 5 });
});
test('a missing anchor names itself', () => {
  assert.throws(() => extractByAnchor(src, 'fn nope'), /fn nope/);
});
test('an anchor on two lines throws', () => {
  assert.throws(() => extractByAnchor(src, 'fn '), /fn .*(twice|2|more than once)/);
});
test('`to` that never comes throws', () => {
  assert.throws(() => extractByAnchor(src, 'let x', 'nope'), /nope/);
});
test('a block that never closes throws', () => {
  assert.throws(() => extractByAnchor('fn a() {\n  b();', 'fn a'), /fn a/);
});
test('a multi-line `to` ends on the last of its lines', () => {
  assert.deepEqual(extractByAnchor(src, 'let x', '    }\n}'), { code: '    let x = 1;\n    if x > 0 {\n        y();\n    }\n}', from: 3, to: 7 });
});
test('a multi-line `from` must match consecutive lines', () => {
  assert.deepEqual(extractByAnchor(src, 'use a;\npub fn one', 'let x'), { code: 'use a;\npub fn one() {\n    let x = 1;', from: 1, to: 3 });
  assert.throws(() => extractByAnchor(src, 'use a;\nfn two'), /fn two/);
});
test('a multi-line `from` opens the block on its last line', () => {
  assert.deepEqual(extractByAnchor('#[test]\nfn a() {\n    b();\n}\nfn c() {}', '#[test]\nfn a'), { code: '#[test]\nfn a() {\n    b();\n}', from: 1, to: 4 });
  assert.deepEqual(extractByAnchor('#[derive(X)]\nstruct S(u8);\nfn c() {}', '#[derive(X)]\nstruct S'), { code: '#[derive(X)]\nstruct S(u8);', from: 1, to: 2 });
});
