# The command parser: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Command library | Brigadier `1.3.11`, the version that the game names in its library list |

Source of the game values: `assets/minecraft/version.json`.

This document states how a command line becomes a parse result, an error or a list of
suggestions, given a command tree. The vanilla client parses each command line itself, against
the tree that the server sends, to colour it, to underline errors and to complete it. The server
parses the same line again when the client sends it. The two parses must agree, so this document
states the behaviour of the vanilla parser exactly.

The node kinds and the argument types of the game are in other documents. This document states
the tree, the reader, the six argument types of the command library, the parse, the suggestions
and the errors. It does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Command line**: the text that the parser reads. A command line that the client sends has no
  leading `/` (`commands-packet.md`, CMDP-EXEC-001).
- **Code unit**: one UTF-16 code unit of the command line. A character outside the Basic
  Multilingual Plane is two code units.
- **Position**: an offset into the command line, counted in code units. Position 0 is before
  the first code unit. Every position on the wire and in an error is a position in this sense.
- **Cursor**: the position where the reader reads next.
- **Separator**: the code unit U+0020. No other whitespace code unit separates two nodes.
- **Whitespace**: a code unit in the set of CMD-TEXT-002. Only argument types skip whitespace.
- **Tree**: a directed graph of nodes with one root.
- **Root**: the node where a parse starts. It reads no text.
- **Literal**: a node that matches one fixed word, its name.
- **Argument**: a node with a name and an argument type. It reads a value from the command line.
- **Argument type**: the rule that reads a value. CMD-ARG states the six types of the command
  library. Other documents state the types of the game.
- **Child**: a node that can follow another node. The children of a node have an order: the
  order of registration (CMD-TREE-002).
- **Redirect**: a link from a node to a node that is not its child. After the node, the parse
  continues at the children of the redirect target.
- **Requirement**: a predicate of a node over the source. A node whose requirement fails does
  not exist for that source in a parse.
- **Source**: the sender of a command, with what the requirement and the argument types read:
  for a player, the player, its permissions, its position and its rotation.
- **Executable**: a node that has a command. A parse that ends after an executable node can run.
- **Context**: the record of one parse level: the source, the parsed nodes with their ranges,
  the parsed values by argument name, the command of the last parsed node, and the child
  context after a redirect.
- **Range**: a pair of positions `[start, end]` with `start ≤ end`.
- **Parse result**: a context, a final cursor and a list of errors (CMD-PARSE-006).
- **Error**: an error kind, a position and arguments (CMD-ERR-001).
- **Suggestion**: a range and a replacement text, with an optional tooltip.
- **Example**: a sample input that an argument type declares. Examples find ambiguities
  (CMD-TREE-006).

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply, with one more:

- `library_source`: taken from the source of Brigadier `1.3.11`. Brigadier is a separate
  library under the MIT licence, not game code. An implementer can read it too
  (`docs/clean-room.md`). The parity fuzzer of section 10 confirms each rule of this kind.

The sources jar of `com.mojang:brigadier:1.3.11` from Maven and the `master` branch of
`github.com/Mojang/brigadier` at commit `9ba4f13` have identical sources.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CMD-TEXT` | code units, positions, whitespace |
| `CMD-RD` | the reader |
| `CMD-TREE` | nodes, children, merges, redirects |
| `CMD-PARSE` | the parse and the choice of a result |
| `CMD-ARG` | the argument types of the command library |
| `CMD-SUG` | suggestions |
| `CMD-ERR` | errors |
| `CMD-EXEC` | contexts in a chain, redirect and fork at run time |
| `CMD-USAGE` | usage text |
| `CMD-DEV` | intentional deviations |

### 0.4 Scope

In scope: the reader, the tree, the parse, the six library argument types, suggestions, error
kinds and their positions, usage text, and the order of contexts in a redirect chain.

Out of scope: the argument types of the game (`argument-*.md`), the `commands` packet and the
suggestion exchange (`commands-packet.md`), permissions (`command-permissions.md`), and the
command executor of the game with its limits on chain length and forks (section 11).

---

## 1. Code units and positions

### CMD-TEXT-001: Positions count code units

- **Behavior:** every position of this document is a count of UTF-16 code units from the start
  of the command line. A reader that stores the command line in another encoding must convert
  each position that leaves the parser: an error position, a suggestion range and a node range.
- **Edge cases:** a range can end between the two code units of one surrogate pair. Only
  CMD-ERR-003 can make such a range, when it takes the last 10 code units before a position.

Provenance: `library_source`. Confidence: high.

### CMD-TEXT-002: Whitespace

The whitespace set is:

| Code points | Note |
|-------------|------|
| U+0009 to U+000D | tab, line feed, line tabulation, form feed, carriage return |
| U+001C to U+001F | information separators |
| U+0020 | space |
| U+1680, U+2000 to U+2006, U+2008 to U+200A, U+205F, U+3000 | space separators |
| U+2028, U+2029 | line and paragraph separators |

U+00A0, U+2007 and U+202F are not whitespace. U+0085 and U+180E are not whitespace.

Provenance: `library_source`. Confidence: high. The set is the whitespace class of the Java
platform for the Unicode version of Java 25.

---

## 2. The reader

The reader holds the command line and a cursor. Each operation below starts at the cursor.
"Next" is the code unit at the cursor. "At the end" means the cursor equals the length.

### CMD-RD-001: Number characters

A number character is `0` to `9`, `.` or `-`. A number token is the longest run of number
characters at the cursor. A number token can be empty.

Provenance: `library_source`. Confidence: high.

### CMD-RD-002: Read an integer

- **Behavior:**
  1. Read the number token.
  2. If the token is empty, fail with `reader_expected_int` at the cursor.
  3. If the token matches `-?[0-9]+` and its value is in the 32-bit signed range, the result is
     that value. The cursor stays after the token.
  4. Otherwise, set the cursor back to the start. Fail with `reader_invalid_int`, with the token
     as its argument, at the start.
- **Edge cases:**

| Text at cursor | Result |
|----------------|--------|
| `12 3` | 12, cursor after `12` |
| `-0` | 0 |
| `007` | 7 |
| `1.5` | `reader_invalid_int` with `1.5` |
| `-` | `reader_invalid_int` with `-` |
| `2147483648` | `reader_invalid_int` with `2147483648` |
| `+5` | `reader_expected_int`: `+` is not a number character |
| `1-2` | `reader_invalid_int` with `1-2` |

Provenance: `library_source`. Confidence: high.

### CMD-RD-003: Read a long

As CMD-RD-002, with the 64-bit signed range and the kinds `reader_expected_long` and
`reader_invalid_long`.

Provenance: `library_source`. Confidence: high.

### CMD-RD-004: Read a double

- **Behavior:**
  1. Read the number token.
  2. If the token is empty, fail with `reader_expected_double` at the cursor.
  3. If the token matches `-?([0-9]+(\.[0-9]*)?|\.[0-9]+)`, the result is the IEEE 754
     binary64 value nearest to the decimal value, with ties to even. A value too large for a
     finite result is an infinity of the same sign. The cursor stays after the token.
  4. Otherwise, set the cursor back to the start. Fail with `reader_invalid_double`, with the
     token as its argument, at the start.
- **Edge cases:**

| Text at cursor | Result |
|----------------|--------|
| `.5` | 0.5 |
| `5.` | 5.0 |
| `-.5` | -0.5 |
| `-0` | negative zero |
| `.` | `reader_invalid_double` with `.` |
| `1.2.3` | `reader_invalid_double` with `1.2.3` |
| `1e5` | 1.0, cursor before `e`: `e` is not a number character |
| a `1` and 400 `0` | positive infinity, no error |

Provenance: `library_source`. Confidence: high.

### CMD-RD-005: Read a float

As CMD-RD-004, with these changes. The result is the IEEE 754 binary32 value nearest to the
decimal value, rounded one time from the decimal text, not through binary64. The kinds are
`reader_expected_float` and `reader_invalid_float`.

Provenance: `library_source`. Confidence: high.

### CMD-RD-006: Unquoted string

An unquoted character is `0` to `9`, `A` to `Z`, `a` to `z`, `_`, `-`, `.` or `+`. An unquoted
string is the longest run of unquoted characters at the cursor. It can be empty. Reading it
never fails.

Provenance: `library_source`. Confidence: high.

### CMD-RD-007: Quoted string body

- **Inputs:** a terminator, `"` or `'`. The cursor is after the opening quote.
- **Behavior:** read code units one at a time.
  - A `\` starts an escape. The next code unit must be the terminator or `\`. The result
    receives that code unit.
  - After a `\`, any other code unit fails with `reader_invalid_escape`, with that code unit as
    its argument. The position is that code unit, not the `\`.
  - The terminator ends the string. The cursor stays after it.
  - Any other code unit goes into the result.
  - At the end of the command line, fail with `reader_expected_end_of_quote` at the end.
- **Edge cases:** in a string in `'`, the text `\"` fails: `"` is not the terminator. In a
  string in `"`, the text `'` needs no escape.

Provenance: `library_source`. Confidence: high.

### CMD-RD-008: Read a quoted string

- **Behavior:**
  1. At the end, the result is the empty string. No error.
  2. If next is not `"` or `'`, fail with `reader_expected_start_of_quote` at the cursor.
  3. Otherwise skip the quote and read a quoted string body (CMD-RD-007) with that quote as the
     terminator.

Provenance: `library_source`. Confidence: high.

### CMD-RD-009: Read a string

- **Behavior:**
  1. At the end, the result is the empty string. No error.
  2. If next is `"` or `'`, skip it and read a quoted string body with it as the terminator.
  3. Otherwise read an unquoted string (CMD-RD-006).

Provenance: `library_source`. Confidence: high.

### CMD-RD-010: Read a boolean

- **Behavior:**
  1. Note the start. Read a string (CMD-RD-009).
  2. If the string is empty, fail with `reader_expected_bool` at the cursor. The cursor is not
     set back.
  3. If the string is exactly `true` or `false`, the result is that value.
  4. Otherwise set the cursor back to the start. Fail with `reader_invalid_bool`, with the
     string as its argument.
- **Edge cases:**

| Text at cursor | Result |
|----------------|--------|
| `"true"` | true |
| `True` | `reader_invalid_bool` with `True` at the start |
| `""` | `reader_expected_bool` at the position after the second `"` |

Provenance: `library_source`. Confidence: high.

### CMD-RD-011: Expect a symbol

To expect a code unit `c`: if next is `c`, skip it. Otherwise, or at the end, fail with
`reader_expected_symbol`, with `c` as its argument, at the cursor.

Provenance: `library_source`. Confidence: high.

### CMD-RD-012: Skip whitespace

To skip whitespace, move the cursor past each whitespace code unit (CMD-TEXT-002) until next is
not whitespace or the reader is at the end.

Provenance: `library_source`. Confidence: high.

---

## 3. The tree

### CMD-TREE-001: Node kinds

A tree has one root. Every other node is a literal or an argument. Each node has:

- a name: the word of a literal, or the name of an argument;
- children, in order;
- a requirement over the source;
- a command, or no command;
- a redirect target, or no redirect. A node with a redirect has no children.
- for a node with a redirect: a source modifier, or none, and a fork flag (CMD-EXEC-002).

An argument has an argument type and, as an option, a suggestion source that replaces the
suggestions of its type (CMD-SUG-004).

Provenance: `library_source`. Confidence: high.

### CMD-TREE-002: Children are keyed by name

- **Behavior:** the children of one node have distinct names. A literal and an argument with
  the same name collide. The order of the children is the order in which each name was first
  added.
- **Invariants:** the order of the children is the order of every operation in this document
  that visits children: the parse, the suggestions, the usage text and the `commands` packet.

Provenance: `library_source`. Confidence: high.

### CMD-TREE-003: Merge of a node with the same name

- **Inputs:** a node `N` that is added as a child of `P`, when `P` already has a child `C` with
  the name of `N`.
- **Behavior:** the library keeps `C` at its position. If `N` has a command, it replaces the
  command of `C`. Each child of `N` is added to `C` by this same rule, in order. The library
  ignores the kind, the argument type, the requirement and the redirect of `N`.
- **Postconditions:** no error occurs.

An implementation can reject the merge where the library silently ignores a field of `N`: a
different kind, a different argument type, a different requirement or a different redirect
(CMD-DEV-002).

Provenance: `library_source`. Confidence: high.

### CMD-TREE-004: Redirect

- **Behavior:** a node with a redirect has no children. When the parse reaches it, the parse
  continues at the children of its target. The target can be any node, the root included, and
  an ancestor included. A parse can therefore come back to a node that it already passed: for
  example, a child that redirects to its own parent. This loop goes through a redirect and a
  child link. The redirect links alone never form a loop in a vanilla tree
  (`commands-packet.md`, CMDP-PKT-002).
- **Edge cases:** a target that is a literal or an argument contributes only its children. The
  parse does not read the target itself again.

Provenance: `library_source`. Confidence: high.

### CMD-TREE-005: Requirement

A requirement is a predicate over the source. The parse (CMD-PARSE-002) skips a child whose
requirement fails. Suggestions do not test requirements in the library (CMD-SUG-003,
CMD-DEV-001). The projection of the tree for one player is in `commands-packet.md`, CMDP-TREE-001.

Provenance: `library_source`. Confidence: high.

### CMD-TREE-006: Ambiguity

- **Inputs:** a node `P`, and two different children `A` and `B` of `P`.
- **Behavior:** an input `x` is ambiguous between `A` and `B` if `x` is an example of `A` and
  `x` is valid for `B`. A literal has its name as its only example. An argument has the examples
  of its type.
  - `x` is valid for a literal when `x` starts with the name and the name is followed by the end
    of `x` or by a separator.
  - `x` is valid for an argument when its type reads `x` without an error and the reader is then
    at the end of `x` or before a separator.
- **Postconditions:** an ambiguity is a diagnostic only. The game logs it at start and runs.

Provenance: `library_source`. Confidence: high.

---

## 4. The parse

### CMD-PARSE-001: Start

A parse starts at a position `p` of the command line with the root of the tree and a new
context. The range of the new context is `[p, p]`. A command line from the client starts at
position 0. A suggestion request with a leading `/` starts at position 1
(`commands-packet.md`, CMDP-SUG-002).

Provenance: `library_source`. Confidence: high.

### CMD-PARSE-002: Candidate children

- **Inputs:** a node `N` and the cursor.
- **Behavior:**
  1. If `N` has no literal child, the candidates are the argument children of `N`, in order.
  2. Otherwise, take the token from the cursor up to the next separator or the end. If a
     literal child of `N` has exactly this token as its name, it is the only candidate.
  3. Otherwise the candidates are the argument children of `N`, in order.
  4. Remove each candidate whose requirement fails for the source.
- **Edge cases:**
  - The match of step 2 is case-sensitive.
  - When the token names a literal whose requirement fails, no candidate remains. The argument
    siblings are not tried.
  - A literal that only starts the token is not a candidate: `gamemodes` does not match
    `gamemode`.

Provenance: `library_source`. Confidence: high.

### CMD-PARSE-003: Try one candidate

- **Inputs:** a candidate `C`, a copy of the context, a copy of the cursor at position `s`.
- **Behavior:**
  1. A literal reads its name. An argument reads a value with its type. When the argument type
     fails, the candidate fails with the error of the type.
  2. If the reader is not at the end and next is not a separator, the candidate fails with
     `dispatcher_expected_argument_separator` at the cursor after the node.
  3. On success, add `C` with the range `[s, cursor]` to the context. An argument also stores
     its value under its name. The range of the context grows to include the new range. The
     command of the context becomes the command of `C`, or none.
  4. On failure, note the pair (`C`, error). The cursor goes back to `s`.

Provenance: `library_source`. Confidence: high.

### CMD-PARSE-004: Continue after a candidate

- **Inputs:** a candidate `C` that succeeded, with the cursor at position `e` after it.
- **Behavior:**
  - **`C` has a redirect, and at least one code unit remains.** Skip the separator. Start a
    new context at the cursor, with the redirect target as its root. Parse the target from
    there (CMD-PARSE-002 onwards). Attach the new context as the child context. The result of
    this level is: this context, the cursor of the nested parse, and the errors of the nested
    parse. The candidates after `C` are not tried.
  - **`C` has no redirect, and at least two code units remain.** Skip the separator. Parse the
    children of `C` from there. The nested result is a potential result.
  - **Otherwise.** The potential result is this context, with the cursor at `e` and no errors.
- **Edge cases:**
  - A single trailing separator after a node without a redirect is not consumed. The potential
    result has one code unit left, so it is not complete (CMD-PARSE-007).
  - A single trailing separator after a redirect node is consumed. The nested parse starts at
    the end of the command line.

Provenance: `library_source`. Confidence: high.

### CMD-PARSE-005: Choice of the result of one level

- **Inputs:** the potential results of one node, in the order of the candidates, and the noted
  errors.
- **Behavior:**
  - If there is at least one potential result, order them by a stable sort with this
    precedence, and take the first:
    1. a result with the cursor at the end comes before a result with code units left;
    2. among the remaining ties, a result with no errors comes before a result with errors.
  - If there is no potential result, the result of this level is the context before this level,
    the cursor before this level, and the noted errors in the order of the candidates.
- **Postconditions:** the errors of failed siblings are dropped when a sibling succeeds. The
  choice does not prefer a longer parse in any other way.

Provenance: `library_source`. Confidence: high.

### CMD-PARSE-006: Parse result

A parse result holds:

- the top context, with child contexts after each redirect;
- the final cursor;
- the errors of the level where the parse stopped, each with the node that it came from, in
  candidate order.

A parse never fails. Every outcome is a parse result.

Provenance: `library_source`. Confidence: high.

### CMD-PARSE-007: Validity and the reported error

- **Inputs:** a parse result.
- **Behavior:** the reported error is the first of these that applies:
  1. The final cursor is not at the end, and there is exactly one error: that error.
  2. The final cursor is not at the end, and the range of the top context is empty:
     `dispatcher_unknown_command` at the final cursor.
  3. The final cursor is not at the end: `dispatcher_unknown_argument` at the final cursor.
  4. The last context in the chain of child contexts has no command:
     `dispatcher_unknown_command` at the final cursor.
  5. No error: the command line can run.
- **Edge cases:**

| Tree | Command line | Reported error |
|------|--------------|----------------|
| `a` (executable) | `b` | `dispatcher_unknown_command` at 0 |
| `a` (executable) → `<n: integer>` (executable) | `a x` | `reader_expected_int` at 2 |
| `a` → `<n: integer>`, `<w: word>`, both executable | `a !` | `dispatcher_unknown_argument` at 2 |
| `a` (not executable) | `a` | `dispatcher_unknown_command` at 1 |
| `a` (executable) | `a ` | `dispatcher_unknown_argument` at 1 |
| `t` → redirect to `teleport` | `t ` | `dispatcher_unknown_command` at 2 |

  In the third row, `<n>` fails with `reader_expected_int`. `<w>` reads the empty string, and
  then fails with `dispatcher_expected_argument_separator`. Two errors and a top context that
  is not empty give case 3.

- **Invariants:** the client treats a command line as valid when the final cursor is at the
  end, the parse result has no errors, and the last context has a command. The client applies
  this test with its own permissions, and again with no permissions (`commands-packet.md`,
  CMDP-TREE-004).

Provenance: `library_source`. Confidence: high.

---

## 5. Argument types of the library

Each type reads at the cursor. Section 2 states the reader operations. The parser ids and the
wire properties are in `commands-packet.md`, CMDP-ARG-001.

### CMD-ARG-001: `bool`

- **Read:** read a boolean (CMD-RD-010).
- **Suggest:** with `r` the remaining input in lower case (CMD-SUG-005), suggest `true` if `true`
  starts with `r`, then `false` if `false` starts with `r`.
- **Examples:** `true`, `false`.

Provenance: `library_source`. Confidence: high.

### CMD-ARG-002: `integer`, `long`, `float`, `double`

- **Properties:** a minimum and a maximum. The defaults are the extreme values of the type:
  for `float` and `double`, the negative and the positive largest finite value.
- **Read:**
  1. Note the start. Read a value with CMD-RD-002, CMD-RD-003, CMD-RD-005 or CMD-RD-004.
  2. If the value is less than the minimum, set the cursor back to the start. Fail with
     `<type>_too_low`, with the value and the minimum.
  3. If the value is greater than the maximum, set the cursor back to the start. Fail with
     `<type>_too_high`, with the value and the maximum.
- **Suggest:** nothing.
- **Examples:** `0`, `123`, `-123` for `integer` and `long`. `0`, `1.2`, `.5`, `-1`, `-.5`,
  `-1234.56` for `float` and `double`.
- **Edge cases:** an infinity from CMD-RD-004 or CMD-RD-005 fails the range test with the
  default bounds.

Provenance: `library_source`. Confidence: high.

### CMD-ARG-003: `string`

- **Properties:** a mode: `word`, `phrase` or `greedy`.
- **Read:**
  - `word`: read an unquoted string (CMD-RD-006). It can be empty.
  - `phrase`: read a string (CMD-RD-009).
  - `greedy`: the value is all code units from the cursor to the end. The cursor moves to the
    end. It never fails.
- **Suggest:** nothing.
- **Examples:** `word`: `word`, `words_with_underscores`. `phrase`: `"quoted phrase"`, `word`,
  `""`. `greedy`: `word`, `words with spaces`, `"and symbols"`.
- **Edge cases:** a `word` that reads the empty string at a code unit that is neither an
  unquoted character nor a separator fails at the next step: CMD-PARSE-003, step 2.

Provenance: `library_source`. Confidence: high.

---

## 6. Suggestions

### CMD-SUG-001: Request

A suggestion request has a parse result and a cursor position `c`. The server uses `c` = the
length of the command line. The suggestions describe what can replace the text before `c`.

Provenance: `library_source`. Confidence: high.

### CMD-SUG-002: Suggestion parent and start

- **Inputs:** a context `X` with range `[s, e]`, and the cursor position `c`. Start with the top
  context.
- **Behavior:**
  1. If `e < c`:
     - if `X` has a child context, apply this rule to the child context;
     - else, if `X` has parsed nodes, the parent is the last parsed node, and the start is the
       end of its range plus 1;
     - else, the parent is the root of `X`, and the start is `s`.
  2. If `s ≤ c ≤ e`: go through the parsed nodes of `X` in order, and keep the previous node,
     which is at first the root of `X`. At the first node whose range contains `c` at either
     end or inside, the parent is the previous node, and the start is the start of that range.
     If no range contains `c`, the parent is the last parsed node, and the start is `s`.
  3. The final start is the smaller of the start and `c`.
- **Postconditions:** the suggestions come from the children of the parent. The context for
  the suggestion sources is `X`, with the values that it parsed.

Provenance: `library_source`. Confidence: high.

### CMD-SUG-003: Suggestions of each child

- **Behavior:** the truncated input is the command line up to `c`. For each child of the
  parent, in order, collect the suggestions of that child for the truncated input and the
  start. A child that fails while it makes suggestions contributes nothing.
- **Edge cases:** the library does not test the requirement of a child here. A source can
  receive the name of a literal that its tree does not contain (CMD-DEV-001).

Provenance: `library_source`. Confidence: high.

### CMD-SUG-004: Suggestions of a node

- A literal suggests its name when its name, in lower case, starts with the remaining input in
  lower case.
- An argument with a suggestion source uses that source. An argument without one uses the
  suggestions of its type.

Provenance: `library_source`. Confidence: high.

### CMD-SUG-005: The suggestion builder

- **Inputs:** the truncated input and a start.
- **Behavior:**
  - The remaining input is the truncated input from the start to its end. Its lower case form
    uses the locale-independent Unicode lower case mapping.
  - A suggestion of text `t` has the range `[start, length of the truncated input]`.
  - A suggestion whose text equals the remaining input exactly, case included, is not added.
  - A builder can make a second builder at another start, for example a start before the
    current argument. Its suggestions join the first builder with their own ranges.
- **Edge cases:** the lower case mapping can change the length of a string, for example for
  U+0130. The library then takes the remaining lower case input at the start offset of the
  original input. The result for such input is in section 11.

Provenance: `library_source`. Confidence: high.

### CMD-SUG-006: Build one list

- **Inputs:** the full command line and a set of suggestions.
- **Behavior:**
  1. If the set is empty, the result is the empty list with the range `[0, 0]`.
  2. The range of the list is from the smallest start to the largest end of the suggestions.
  3. Expand each suggestion to the range of the list: put before its text the command line
     from the list start to its start, and put after its text the command line from its end to
     the list end.
  4. Remove duplicates. Two suggestions are duplicates when their range, text and tooltip are
     equal.
  5. Sort by text, without regard to case: compare code unit by code unit, and compare two
     different code units by their upper case forms, then by their lower case forms.
- **Edge cases:** the order of two texts that are equal without regard to case, for example
  `A` and `a`, is not specified. A custom suggestion source can make integer suggestions.
  Integer suggestions compare by value with each other.

Provenance: `library_source`. Confidence: high.

### CMD-SUG-007: Merge the lists of the children

- **Behavior:**
  - No child: the empty list with the range `[0, 0]`.
  - One child: the list of that child, as it is.
  - More than one child: take every suggestion of every list, and build one list from them with
    CMD-SUG-006, over the full command line.

Provenance: `library_source`. Confidence: high.

---

## 7. Errors

### CMD-ERR-001: Error data

An error has a kind, a position and zero or more arguments. Where a rule says that the cursor
goes back to a start, the position is that start. Otherwise the position is the cursor at the
moment of the error. An error that a command raises when it runs has no position.

Provenance: `library_source`. Confidence: high.

### CMD-ERR-002: Error kinds

The game shows each kind with a translation key. The arguments go into the key in the order
of the last column. The key arguments of the range kinds come in the order bound, then value.

| Kind | Translation key | Key arguments |
|------|-----------------|---------------|
| `double_too_low` | `argument.double.low` | minimum, value |
| `double_too_high` | `argument.double.big` | maximum, value |
| `float_too_low` | `argument.float.low` | minimum, value |
| `float_too_high` | `argument.float.big` | maximum, value |
| `integer_too_low` | `argument.integer.low` | minimum, value |
| `integer_too_high` | `argument.integer.big` | maximum, value |
| `long_too_low` | `argument.long.low` | minimum, value |
| `long_too_high` | `argument.long.big` | maximum, value |
| `literal_incorrect` | `argument.literal.incorrect` | literal |
| `reader_expected_start_of_quote` | `parsing.quote.expected.start` | none |
| `reader_expected_end_of_quote` | `parsing.quote.expected.end` | none |
| `reader_invalid_escape` | `parsing.quote.escape` | code unit |
| `reader_invalid_bool` | `parsing.bool.invalid` | text |
| `reader_expected_bool` | `parsing.bool.expected` | none |
| `reader_invalid_int` | `parsing.int.invalid` | token |
| `reader_expected_int` | `parsing.int.expected` | none |
| `reader_invalid_long` | `parsing.long.invalid` | token |
| `reader_expected_long` | `parsing.long.expected` | none |
| `reader_invalid_double` | `parsing.double.invalid` | token |
| `reader_expected_double` | `parsing.double.expected` | none |
| `reader_invalid_float` | `parsing.float.invalid` | token |
| `reader_expected_float` | `parsing.float.expected` | none |
| `reader_expected_symbol` | `parsing.expected` | symbol |
| `dispatcher_unknown_command` | `command.unknown.command` | none |
| `dispatcher_unknown_argument` | `command.unknown.argument` | none |
| `dispatcher_expected_argument_separator` | `command.expected.separator` | none |
| `dispatcher_parse_exception` | `command.exception` | message |

The game inserts each argument as plain text. `literal_incorrect` cannot occur in a parse:
CMD-PARSE-002 offers a literal only when its name matches.

Provenance: `source_suggested` for the keys and the order of the key arguments.
`library_source` for the kinds. Confidence: high. Experiment: as an operator, run
`effect give @s minecraft:speed 0`, which has an integer argument with the minimum 1. Read the
chat line of `integer_too_low`, and look for the order "1" then "0".

### CMD-ERR-003: Context text

- **Inputs:** an error with a position `q` over a command line of length `n`.
- **Behavior:** let `k` = the smaller of `q` and `n`. The context text is the code units from
  `max(0, k − 10)` to `k`. When `k > 10`, it starts with an ellipsis. `commands-packet.md`,
  CMDP-EXEC-003, states how the game shows it.

Provenance: `library_source`. Confidence: high.

---

## 8. Contexts in a chain

### CMD-EXEC-001: Chain

- **Behavior:** a parse result is a chain of contexts: the top context, then each child
  context. The command line can run only when the last context has a command (CMD-PARSE-007,
  case 4). Each context before the last one is a modifier stage. The last one is the run stage.
- **Postconditions:** the values of a context are visible only to that context. A value with
  the same name in two contexts of one chain does not overwrite the other.

Provenance: `library_source`. Confidence: high.

### CMD-EXEC-002: Redirect, modifier and fork

- **Behavior:** a modifier stage has the source modifier of the redirect node that ends it, or
  none.
  - With no modifier, the source passes to the next stage unchanged.
  - A modifier maps one source to zero or more sources. Each source runs the next stage.
  - If any stage so far comes from a fork node, the chain is forked. In a forked chain, an error
    of one source stops only that source, and the result is the number of successful runs.
  - In a chain that is not forked, the first error stops the command.
  - When a modifier stage produces no source, the command stops with the result 0.
- **Edge cases:** `tp` is a redirect to `teleport` with no modifier and no fork
  (`command-teleport.md`). Its chain has one modifier stage that passes the source unchanged.

Provenance: `library_source`. Confidence: high. The game runs chains in its own executor. Its
limits on chain length and forks are in section 11.

---

## 9. Usage text

### CMD-USAGE-001: Usage text of a node

A literal has its name as its usage text. An argument has `<`, its name and `>`. The root has
the empty text.

Provenance: `library_source`. Confidence: high.

### CMD-USAGE-002: Short usage

- **Inputs:** a node `N`, a source, a flag `optional` and a flag `deep`.
- **Behavior:** if the requirement of `N` fails, there is no usage. Otherwise:
  1. `self` is the usage text of `N`, in `[` and `]` when `optional` is true.
  2. If `deep` is true, the result is `self`.
  3. If `N` has a redirect, the result is `self`, a separator, and `...` when the target is the
     root, or `-> ` and the usage text of the target.
  4. Let `K` be the children of `N` whose requirement passes, and `o` true when `N` is
     executable.
     - One child: the result is `self`, a separator, and the short usage of the child with
       `optional = o` and `deep = o`, if that usage exists.
     - More than one child: compute the short usage of each child with `optional = o` and
       `deep = true`. If they give one distinct text, the result is `self`, a separator, and
       that text, in `[` and `]` when `o` is true. If they give more than one distinct text,
       the result is `self`, a separator, and the usage texts of all of `K` joined by `|`, in
       `[` and `]` when `o` is true, or in `(` and `)` when `o` is false.
  5. Otherwise the result is `self`.
- **Postconditions:** the short usage of a node `P` for a source is the short usage of each
  child of `P`, with `optional` true when `P` is executable and `deep` false.

Provenance: `library_source`. Confidence: high.

---

## 10. Parity test

The parity fuzzer runs the same tree and the same command lines through Brigadier `1.3.11` from
Maven and through the implementation. For each command line it compares:

- the final cursor and the reported error of CMD-PARSE-007, with its kind and its position;
- the client validity test of CMD-PARSE-007;
- the suggestion list of CMD-SUG-007 at the end of the line: its range, its texts and their
  order where CMD-SUG-006 specifies the order.

The trees use the six library argument types, literals, redirects to the root and to inner
nodes, and requirements. The command lines include the cases of CMD-RD-002, CMD-RD-004,
CMD-RD-010 and CMD-PARSE-007, text outside the Basic Multilingual Plane, the whitespace of
CMD-TEXT-002, and runs of separators.

---

## 11. Intentional deviations

### CMD-DEV-001: Suggestions test requirements

The library collects suggestions from every child of the parent (CMD-SUG-003). A player can
then learn the name of a command that its tree hides, with a crafted `command_suggestion`
request. An implementation can skip a child whose requirement fails for the source. The vanilla
client cannot see a difference, because the tree that it parses has no such child.

### CMD-DEV-002: Merge conflicts are errors

The library ignores conflicting fields when it merges two nodes (CMD-TREE-003). An
implementation can reject the merge instead, when the tree is built. No player can see a
difference.

---

## 12. Open questions

- The game runs a command chain in its own executor, with the game rules
  `max_command_sequence_length` and `max_command_forks`. Its order, its limits and its results
  for forked chains need a specification before a command with a modifier or a fork exists.
- CMD-SUG-005: the result for remaining input whose lower case form has a different length.
  Experiment: in the vanilla client, type `/gamemode İ` and read the suggestions.
