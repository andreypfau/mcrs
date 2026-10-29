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
  `/` at the start (`commands-packet.md`, CMDP-EXEC-001).
- **Character**: one Unicode scalar value of the command line. A character outside the Basic
  Multilingual Plane is one character.
- **Code unit**: one UTF-16 code unit. A character outside the Basic Multilingual Plane is two
  code units. A rule that counts code units says so.
- **Position**: an offset into the command line, counted in characters. Position 0 is before
  the first character. Every position in an error and in a suggestion is a position in this
  sense.
- **Cursor**: the position where the reader reads next.
- **Separator**: the character U+0020. No other whitespace character separates two nodes.
- **Whitespace**: a character in the set of CMD-TEXT-002. Only argument types skip whitespace.
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
- **Source**: the sender of a command, with the data that the requirement and the argument
  types read. For a player, the data is the player, its permissions, its position and its
  rotation.
- **Executable**: a node that has a command. A parse that ends after an executable node can run.
- **Context**: the record of one parse level. It holds the source and the parsed nodes with
  their ranges. It also holds the parsed values by argument name, the command of the last
  parsed node, and the child context after a redirect.
- **Range**: a pair of positions `[start, end]` with `start ≤ end`.
- **Parse result**: a context, a final cursor and a list of errors (CMD-PARSE-006).
- **Error**: an error kind, a position and arguments (CMD-ERR-001).
- **Suggestion**: a range and a replacement text, with an optional tooltip.
- **Identifier**: a resource name of the game, `namespace:path`.
- **Example**: a sample input that an argument type declares. Examples find ambiguities
  (CMD-TREE-006).

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply, with one more:

- `library_source`: taken from the source of Brigadier `1.3.11`. Brigadier is a separate
  library under the MIT licence, not game code. An implementer can read it too
  (`docs/clean-room.md`). The parity fuzzer of section 10 is the test for each rule of this
  kind. No run of the fuzzer exists yet.

The sources jar of `com.mojang:brigadier:1.3.11` from Maven and the `master` branch of
`github.com/Mojang/brigadier` at commit `9ba4f13` have identical sources.

### 0.2.1 Kinds of rules

Each rule states its kind on its provenance line: wire, client agreement, semantic or
unspecified. The kinds are those of `README.md`, section "Semantics, not accidents". A rule
that corrects a defect of the reference has an entry in section 11.

The vanilla client parses each command line against the tree that the server sends. The server
must agree with that parse on the input that a player types in normal use. On other input, the
server can be stricter than the client and reject the input with a syntax error. The server
must not accept input that the client rejects. The server must not give a different meaning to
input that both accept.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CMD-TEXT` | characters, positions, whitespace |
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

In scope: the reader, the tree, the parse, the six library argument types and suggestions.
Also in scope: error kinds and their positions, usage text, and the order of contexts in a
redirect chain.

Out of scope: the argument types of the game (`argument-*.md`), the `commands` packet and the
suggestion exchange (`commands-packet.md`), and permissions (`command-permissions.md`). Also
out of scope: the command executor of the game, with its results and its limits on chain
length and forks. No
specification of the executor exists yet (section 12).

---

## 1. Characters and positions

### CMD-TEXT-001: Positions count characters

- **Behavior:**
  - Every position of this document is a count of characters from the start of the command
    line. These positions are error positions, suggestion ranges and node ranges.
  - An implementation can store a position in another unit, for example a UTF-8 byte offset.
    The stored position is always on a character boundary.
  - A position on the wire is a count of code units. Only the codec of a packet converts a
    position between characters and code units (`commands-packet.md`, CMDP-SUG-002).
- **Edge cases:** the reader operations of section 2 never put the cursor inside a character.
  The context text of CMD-ERR-003 always starts at a character boundary (CMD-DEV-006).

Provenance: `library_source`. Kind: semantic (CMD-DEV-006, CMD-DEV-013). Confidence: high.

### CMD-TEXT-002: Whitespace

A character is whitespace when it has the Unicode property `White_Space` and is not one of
U+00A0, U+0085, U+2007 and U+202F. The whitespace set is:

| Code points | Note |
|-------------|------|
| U+0009 to U+000D | tab, line feed, line tabulation, form feed, carriage return |
| U+0020 | space |
| U+1680, U+2000 to U+2006, U+2008 to U+200A, U+205F, U+3000 | space separators |
| U+2028, U+2029 | line and paragraph separators |

U+00A0, U+2007 and U+202F are not whitespace. U+0085 and U+180E are not whitespace. U+001C to
U+001F are not whitespace (CMD-DEV-014).

No argument type of the command library skips whitespace. Argument types of the game do. A
player cannot send a character below U+0020 in a chat command: the server disconnects the
player. Command blocks and functions can contain these characters.

Provenance: `library_source`. Kind: client agreement, with a deviation (CMD-DEV-014).
Confidence: high.

---

## 2. The reader

The reader holds the command line and a cursor. Each operation below starts at the cursor.
"Next" is the character at the cursor. "At the end" means the cursor equals the length.

### CMD-RD-001: Number characters

A number character is `0` to `9`, `.` or `-`. A number token is the longest run of number
characters at the cursor. A number token can be empty.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

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

Provenance: `library_source`. Kind: client agreement. Confidence: high.

### CMD-RD-003: Read a long

As CMD-RD-002, with the 64-bit signed range and the kinds `reader_expected_long` and
`reader_invalid_long`.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

### CMD-RD-004: Read a double

- **Behavior:**
  1. Read the number token.
  2. If the token is empty, fail with `reader_expected_double` at the cursor.
  3. The token must match `-?([0-9]+(\.[0-9]*)?|\.[0-9]+)`. The result is the binary64 value
     nearest to the decimal value, with ties to even under IEEE 754 overflow rules. The result
     must be finite. The cursor stays after the token.
  4. If the token does not match, or the result is not finite, set the cursor back to the
     start. Fail with `reader_invalid_double`, with the token as its argument, at the start
     (CMD-DEV-012).
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
| a `1` and 400 `0` | `reader_invalid_double` with the token |

Provenance: `library_source`. Kind: client agreement for the grammar and the errors, semantic
for the value. Confidence: high.

### CMD-RD-005: Read a float

As CMD-RD-004, with these changes. The result is the binary32 value nearest to the decimal
value, rounded one time from the decimal text, not through binary64. The kinds are
`reader_expected_float` and `reader_invalid_float`.

| Text at cursor | Result |
|----------------|--------|
| `1.00000017881393432617187499` | the binary32 value `1.0000001`, not `1.0000002` |

Provenance: `library_source`. Kind: client agreement for the grammar and the errors, semantic
for the value. Confidence: high.

### CMD-RD-006: Unquoted string

An unquoted character is `0` to `9`, `A` to `Z`, `a` to `z`, `_`, `-`, `.` or `+`. An unquoted
string is the longest run of unquoted characters at the cursor. It can be empty. Reading it
never fails.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

### CMD-RD-007: Quoted string body

- **Inputs:** a terminator, `"` or `'`. The cursor is after the first quote.
- **Behavior:** read characters one at a time.
  - A `\` starts an escape. The next character must be the terminator or `\`. The result
    receives that character.
  - After a `\`, any other character fails with `reader_invalid_escape`, with that character as
    its argument. The position is that character, not the `\`.
  - The terminator ends the string. The cursor stays after it.
  - Any other character goes into the result.
  - At the end of the command line, fail with `reader_expected_end_of_quote` at the end.
- **Edge cases:** in a string in `'`, the text `\"` fails: `"` is not the terminator. In a
  string in `"`, the text `'` needs no escape.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

### CMD-RD-008: Read a quoted string

- **Behavior:**
  1. At the end, the result is the empty string. No error.
  2. If next is not `"` or `'`, fail with `reader_expected_start_of_quote` at the cursor.
  3. Otherwise skip the quote and read a quoted string body (CMD-RD-007) with that quote as the
     terminator.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

### CMD-RD-009: Read a string

- **Behavior:**
  1. At the end, the result is the empty string. No error.
  2. If next is `"` or `'`, skip it and read a quoted string body with it as the terminator.
  3. Otherwise read an unquoted string (CMD-RD-006).

Provenance: `library_source`. Kind: client agreement. Confidence: high.

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

Provenance: `library_source`. Kind: client agreement. Confidence: high.

### CMD-RD-011: Expect a symbol

To expect a character `c`: if next is `c`, skip it. Otherwise, or at the end, fail with
`reader_expected_symbol`, with `c` as its argument, at the cursor.

Provenance: `library_source`. Kind: semantic. Confidence: high.

### CMD-RD-012: Skip whitespace

To skip whitespace, move the cursor past each whitespace character (CMD-TEXT-002) until next is
not whitespace or the reader is at the end.

Provenance: `library_source`. Kind: semantic. Confidence: high.

---

## 3. The tree

### CMD-TREE-001: Node kinds

A tree has one root. Every other node is a literal or an argument. Each node has:

- a name: the word of a literal, or the name of an argument;
- children, in order;
- a requirement over the source;
- a command, or no command;
- a redirect target, or no redirect;
- for a node with a redirect: a source modifier, or none, and a fork flag (CMD-EXEC-002).

An argument has an argument type and, as an option, a suggestion source that replaces the
suggestions of its type (CMD-SUG-004).

A node with a redirect has no children. A tree that gives children to such a node is an error
when the tree is built (CMD-DEV-007).

Provenance: `library_source`. Kind: semantic. Confidence: high.

### CMD-TREE-002: Children are keyed by name

- **Behavior:** the children of one node have distinct names. A literal and an argument with
  the same name collide. The children of one node have one fixed order. The server selects
  that order.
- **Invariants:** the `commands` packet, the parse, the suggestions and the usage text visit
  the children in that same order.

Provenance: `library_source`. Kind: semantic for the order, wire for the packet. Confidence:
high.

### CMD-TREE-003: Merge of a node with the same name

- **Inputs:** a node `N` that is added as a child of `P`, when `P` already has a child `C` with
  the name of `N`.
- **Behavior:** `C` keeps its position. If `N` has a command, it replaces the command of `C`.
  Each child of `N` is added to `C` by this same rule, in order.
- **Postconditions:** a merge where a field of `N` differs from the field of `C` is an error.
  The fields are the kind, the argument type, the requirement, the redirect, the source
  modifier, the fork flag and the suggestion source (CMD-DEV-002).

Provenance: `library_source`. Kind: semantic. Confidence: high.

### CMD-TREE-004: Redirect

- **Behavior:** when the parse reaches a node with a redirect, the parse continues at the
  children of the target. The target can be any node, the root included, and an ancestor
  included.
- **Preconditions:** the parse follows a redirect only when the target is usable by the
  source. A node is usable when its requirement and the requirement of each node on its path
  from the root pass for the source. The root is always usable. When the target is not usable,
  the node acts as a node with no redirect and no children (CMD-DEV-004).
- **Invariants:** when a node has an argument child with a redirect, the node has no other
  argument child.
- **Edge cases:** a target that is a literal or an argument contributes only its children. The
  parse does not read the target itself again.

The vanilla server removes a redirect from the tree of a player when the target is not in that
tree (`commands-packet.md`). The precondition makes the parse of the server agree with the parse
of the client. The invariant prevents the case where a redirect discards a better result of an
earlier sibling (CMD-PARSE-004).

Provenance: `library_source`. Kind: client agreement. Confidence: high.

### CMD-TREE-005: Requirement

A requirement is a predicate over the source. The parse (CMD-PARSE-002) and the suggestions
(CMD-SUG-003) skip a child whose requirement fails. The projection of the tree for one player
is in `commands-packet.md`, CMDP-TREE-001.

Provenance: `library_source`. Kind: semantic. Confidence: high.

### CMD-TREE-006: Ambiguity

An implementation can report an ambiguity when it builds the tree. Let `A` and `B` be two
children of one node. An input `x` is ambiguous between them when `x` is an example of `A`.
Also, `B` must read `x` up to its end or up to a separator.

Provenance: `library_source`. Kind: unspecified. Confidence: high.

---

## 4. The parse

### CMD-PARSE-001: Start

A parse starts at a position `p` of the command line with the root of the tree and a new
context. The range of the new context is `[p, p]`. A command line from the client starts at
position 0. A suggestion request with a `/` at the start starts at position 1
(`commands-packet.md`, CMDP-SUG-002).

Provenance: `library_source`. Kind: wire and client agreement. Confidence: high.

### CMD-PARSE-002: Candidate children

- **Inputs:** a node `N` and the cursor.
- **Behavior:**
  1. Remove each child of `N` whose requirement fails for the source.
  2. If no literal child remains, the candidates are the argument children that remain, in
     order.
  3. Otherwise, take the token from the cursor up to the next separator or the end. If a
     literal child that remains has exactly this token as its name, it is the only candidate.
  4. Otherwise the candidates are the argument children that remain, in order.
- **Edge cases:**
  - The match of step 3 is case-sensitive.
  - When the token names a literal whose requirement fails, the argument siblings are
    candidates (CMD-DEV-003).
  - A literal that only starts the token is not a candidate: `gamemodes` does not match
    `gamemode`.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

### CMD-PARSE-003: Try one candidate

- **Inputs:** a candidate `C`, a copy of the context, a copy of the cursor at position `s`.
- **Behavior:**
  1. A literal reads its name. An argument reads a value with its type. An argument type fails
     only with a syntax error. Then the candidate fails with that error (CMD-DEV-005).
  2. If the reader is not at the end and next is not a separator, the candidate fails with
     `dispatcher_expected_argument_separator` at the cursor after the node.
  3. On success, add `C` with the range `[s, cursor]` to the context. An argument also stores
     its value under its name. The range of the context grows to include the new range. The
     command of the context becomes the command of `C`, or none.
  4. On failure, note the pair (`C`, error). A failed candidate changes nothing else.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

### CMD-PARSE-004: Continue after a candidate

- **Inputs:** a candidate `C` that succeeded, with the cursor at position `e` after it.
- **Behavior:**
  - **`C` has a redirect that the parse follows (CMD-TREE-004), and at least one character
    remains.** Skip the separator. Start a
    new context at the cursor, with the redirect target as its root. Parse the target from
    there (CMD-PARSE-002 onwards). Attach the new context as the child context. The result of
    this level is: this context, the cursor of the nested parse, and the errors of the nested
    parse. The candidates after `C` are not tried. The potential results of the candidates
    before `C` are discarded.
  - **The parse does not follow a redirect of `C`, and at least two characters remain.** Skip
    the separator. Parse the children of `C` from there. The nested result is a potential result.
  - **Otherwise.** The potential result is this context, with the cursor at `e` and no errors.
- **Edge cases:**
  - A single trailing separator after a node without a redirect is not consumed. The potential
    result has one character left, so it is not complete (CMD-PARSE-007).
  - A single trailing separator after a redirect node is consumed. The nested parse starts at
    the end of the command line.
  - The new context after a redirect has the source of this context. The parse does not
    apply the source modifier, so the requirements after a redirect test the original source.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

### CMD-PARSE-005: Choice of the result of one level

- **Inputs:** the potential results of one node, in the order of the candidates, and the noted
  errors.
- **Behavior:**
  - If there is at least one potential result, order them by a stable sort with this
    precedence, and take the first:
    1. a result with the cursor at the end comes before a result with characters left;
    2. among the remaining ties, a result with no errors comes before a result with errors.
  - If there is no potential result, the result of this level is the context and the cursor
    before this level. The errors of the result are the noted errors, in the order of the
    candidates.
- **Postconditions:** the errors of failed siblings are dropped when a sibling succeeds. The
  choice does not prefer a longer parse in any other way.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

### CMD-PARSE-006: Parse result

A parse result holds:

- the top context, with child contexts after each redirect;
- the final cursor;
- the errors of the level where the parse stopped, each with the node that it came from.

A parse never fails. Every outcome is a parse result. The number of errors is significant
(CMD-PARSE-007). Their order is unspecified.

Provenance: `library_source`. Kind: semantic. Confidence: high.

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

- **Invariants:**
  - The client treats a command line as valid when the final cursor is at the end. Also, the
    parse result must have no errors, and the last context must have a command.
  - The client applies this test to a command line that the player did not type. First it
    parses the line with its full tree. If that test passes and the line has no signed
    argument, it parses the line again without the restricted nodes (`commands-packet.md`,
    CMDP-TREE-004).
  - The client signs the arguments of the game type `message` that its parse finds. The parse
    of the server must find the same arguments, with the same names.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

---

## 5. Argument types of the library

Each type reads at the cursor. Section 2 states the reader operations. The parser ids and the
wire properties are in `commands-packet.md`, CMDP-ARG-001.

### CMD-ARG-001: `bool`

- **Read:** read a boolean (CMD-RD-010).
- **Suggest:** with `r` the remaining input in lower case (CMD-SUG-005), suggest `true` if `true`
  starts with `r`, then `false` if `false` starts with `r`.
- **Examples:** `true`, `false`.

Provenance: `library_source`. Kind: client agreement for the read, semantic for the
suggestions. Confidence: high.

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
- **Edge cases:** a decimal value outside the finite range of `float` or `double` fails in
  the reader (CMD-RD-004), before the range test.

Provenance: `library_source`. Kind: client agreement; the bounds are wire
(`commands-packet.md`, CMDP-ARG-001). Confidence: high.

### CMD-ARG-003: `string`

- **Properties:** a mode: `word`, `phrase` or `greedy`.
- **Read:**
  - `word`: read an unquoted string (CMD-RD-006). It can be empty.
  - `phrase`: read a string (CMD-RD-009).
  - `greedy`: the value is all characters from the cursor to the end. The cursor moves to the
    end. It never fails.
- **Suggest:** nothing.
- **Examples:** `word`: `word`, `words_with_underscores`. `phrase`: `"quoted phrase"`, `word`,
  `""`. `greedy`: `word`, `words with spaces`, `"and symbols"`.
- **Edge cases:** a `word` can read the empty string at a character that is neither an
  unquoted character nor a separator. Then it fails at the next step: CMD-PARSE-003, step 2.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

---

## 6. Suggestions

The vanilla client makes most suggestions itself, from its own tree. The server makes
suggestions only for a `command_suggestion` request. The client sends a request for the
arguments of `commands-packet.md`, CMDP-SUG-001. These are an `ask_server` argument, an argument
of a registry that the client does not have, and an argument of type `minecraft:objective`. The
client always uses the range of a reply that has suggestions. The client shows nothing for an
empty reply. When the parent on the client has more than one child, the client sorts the reply
again. Otherwise the client keeps the order of the reply. In both cases the client then shows
the suggestions that start with the last word of the input before the other suggestions.
`client-commands.md` states the client side: the parse while the player types, the display
order, the suggestion list and the hint lines.

### CMD-SUG-001: Request

A suggestion request has a parse result and a cursor position `c`. The server uses `c` = the
length of the command line. The suggestions describe what can replace the text before `c`.

Provenance: `library_source`. Kind: wire. Confidence: high.

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
     which is at first the root of `X`. Stop at the first node whose range contains `c`, at
     either end or inside. The parent is the previous node, and the start is the start of that
     range.
     If no range contains `c`, the parent is the last parsed node, and the start is `s`. If
     `X` has no parsed nodes, the parent is the root of `X`, and the start is `s`.
  3. The final start is the smaller of the start and `c`.
- **Postconditions:** the suggestions come from the children of the parent. The context for
  the suggestion sources is `X`, with the values that it parsed.

Provenance: `library_source`. Kind: wire: the client uses the range of the reply. Confidence:
high.

### CMD-SUG-003: Suggestions of each child

- **Behavior:** the truncated input is the command line up to `c`. Take the children of the
  parent whose requirement passes for the source (CMD-DEV-001). For each one, in order,
  collect its suggestions for the truncated input and the start.
- **Postconditions:** a child that fails contributes no suggestions. The request always gets a
  reply (CMD-DEV-008).

Provenance: `library_source`. Kind: semantic. Confidence: high.

### CMD-SUG-004: Suggestions of a node

- A literal suggests its name when its name, in lower case, starts with the remaining input in
  lower case.
- An argument with a suggestion source uses that source. An argument without one uses the
  suggestions of its type.

Provenance: `library_source`. Kind: semantic. Confidence: high.

### CMD-SUG-005: The suggestion builder

- **Inputs:** the truncated input and a start.
- **Behavior:**
  - The remaining input is the truncated input from the start to its end. Its lower case form
    is the locale-independent Unicode lower case form of the remaining input (CMD-DEV-009).
  - A suggestion of text `t` has the range `[start, length of the truncated input]`.
  - A suggestion whose text equals the remaining input exactly, case included, is not added
    (CMD-DEV-010).
  - A builder can make a second builder at another start, for example a start before the
    current argument. Its suggestions join the first builder with their own ranges.

Provenance: `library_source`. Kind: semantic. Confidence: high.

### CMD-SUG-006: Build one list

- **Inputs:** the full command line and a set of suggestions.
- **Behavior:**
  1. If the set is empty, the result is the empty list. Its range is unspecified
     (`commands-packet.md`, CMDP-SUG-002).
  2. The range of the list is from the smallest start to the largest end of the suggestions.
  3. Expand each suggestion to the range of the list. Put before its text the command line
     from the list start to its start. Put after its text the command line from its end to the
     list end.
  4. Remove duplicates. Two suggestions are duplicates when their range, text and tooltip are
     equal.
  5. Sort the suggestions by text, without regard to case, code point by code point. When one
     text starts with the other, the shorter text comes first.
- **Edge cases:** the order of two texts that are equal without regard to case, for example
  `A` and `a`, is unspecified.

Provenance: `library_source`. Kind: wire for steps 2 to 4, unspecified for the range of
step 1. Semantic for step 5: the client sees the order of the server only for an argument of
CMDP-SUG-001 with no sibling. Confidence: high.

### CMD-SUG-007: Merge the lists of the children

- **Behavior:** the suggestions of a request are the suggestions of all children of the
  parent. CMD-SUG-006 builds them into one list over the full command line (CMD-DEV-011).

Provenance: `library_source`. Kind: wire for the range and the texts, semantic for the order.
Confidence: high.

### CMD-SUG-008: Substring filter of the game

Many suggestion sources of the game filter a list of candidates with one rule. The argument
documents cite this rule.

- **Inputs:** the remaining input `r` of a builder (CMD-SUG-005), and a list of candidates.
- **Behavior:**
  1. Map `r` to lower case with the locale-independent mapping of the full string.
  2. For a candidate that is a text, map the text to lower case in the same way. For a
     candidate that is an identifier, keep the text of the identifier as it is.
  3. A pattern `p` matches a text `t` when `t` starts with `p`. It also matches when `p`
     occurs in `t` directly after a `.`, a `_` or a `/`. The comparison is exact, character
     by character.
  4. A text candidate matches when `r` matches its text from step 2.
  5. An identifier candidate matches as follows:
     - if `r` contains a `:`, when `r` matches the full identifier `namespace:path`;
     - otherwise, when `r` matches the namespace or `r` matches the path.
  6. The source suggests each candidate that matches, with its original text. CMD-SUG-005
     then drops a text that equals the remaining input. CMD-SUG-006 sets the final order.
- **Edge cases:**
  - An empty `r` matches every candidate.
  - A `-`, a `:` or a space in `t` does not start a new match position.

| `r` | Candidate | Match |
|-----|-----------|-------|
| `oak` | text `oak_log` | yes |
| `log` | text `oak_log` | yes, after `_` |
| `ak` | text `oak_log` | no |
| `OAK` | text `Oak_Log` | yes: both sides are in lower case |
| `stone` | identifier `minecraft:smooth_stone` | yes, after `_` in the path |
| `craft` | identifier `minecraft:stone` | no |
| `minecraft:st` | identifier `minecraft:stone` | yes |
| `minecraft:st` | identifier `minecraft:smooth_stone` | no |

#### Identifiers with a prefix

Some sources put a fixed prefix before each identifier, for example `#` before a tag. For
these sources, the filter changes:

1. If `r` is empty, every candidate matches.
2. Otherwise, find the longest common start of `r` and the prefix. If it is empty, no
   candidate matches.
3. Otherwise, remove that common start from `r`. Filter the identifiers with the rest, as in
   step 5.
4. The suggested text is the prefix, then the identifier.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the
vanilla client, type `/give @s log`, then `/give @s ak`. Read the suggestions. The first list
contains `minecraft:oak_log`. The second list does not contain `minecraft:oak_log`. The client
makes these suggestions itself with the same filter, so the experiment checks the filter, not
the server.

---

## 7. Errors

### CMD-ERR-001: Error data

An error has a kind, a position and zero or more arguments. Where a rule says that the cursor
goes back to a start, the position is that start. Otherwise the position is the cursor at the
moment of the error.

An error that a command raises when it runs has no position. The server shows only the message
of such an error, with no context text (CMD-ERR-003).

Provenance: `library_source`. Kind: client agreement for parse errors, semantic for errors at
run time. Confidence: high.

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
| `reader_invalid_escape` | `parsing.quote.escape` | character |
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

The game keeps an argument that is a number, a boolean or a text as a typed value of the
translation. The bound and the value of the eight range kinds are numbers: a 32-bit integer, a
64-bit integer, a binary32 value or a binary64 value. The client shows each number with the
decimal form of the Java platform, for example `1.7976931348623157E308`. A literal node never
raises `literal_incorrect`: CMD-PARSE-002 offers a literal only when its name matches. The
SNBT read of an argument raises it (`snbt.md`, SNBT-TOK-003). The kind
`dispatcher_parse_exception` of the library does not exist here (CMD-DEV-005).

Provenance: `source_suggested` for the keys and the order of the key arguments.
`library_source` for the kinds. Kind: wire. Confidence: high. Experiment: as an operator, run
`effect give @s minecraft:speed 0`, which has an integer argument with the minimum 1. Read the
chat line of `integer_too_low`, and look for the order "1" then "0".

### CMD-ERR-003: Context text

- **Inputs:** an error with a position `q` over a command line of length `n`.
- **Behavior:**
  1. Let `k` = the smaller of `q` and `n`, and let `j` = `max(0, k − 10)`. `q`, `n`, `j` and
     `k` count characters (CMD-DEV-006).
  2. The context text is the characters from `j` to `k`. When `k > 10`, it starts with an
     ellipsis.
- `commands-packet.md`, CMDP-EXEC-003, states how the server shows the context text.

Provenance: `library_source`. Kind: wire, through the chat message of CMDP-EXEC-003.
Confidence: high.

---

## 8. Contexts in a chain

### CMD-EXEC-001: Chain

- **Behavior:** a parse result is a chain of contexts: the top context, then each child
  context. The command line can run only when the last context has a command (CMD-PARSE-007,
  case 4). Each context before the last one is a modifier stage. The last one is the run stage.
- **Postconditions:** the values of a context are visible only to that context. A value with
  the same name in two contexts of one chain does not overwrite the other.

Provenance: `library_source`. Kind: semantic. Confidence: high.

### CMD-EXEC-002: Redirect, modifier and fork

A modifier stage maps one source to zero or more sources, and each source runs the next stage.
In a chain with a fork node, an error of one source stops only that source.

The results of a chain and the limits on its length and its forks belong to a specification
of the command executor. That specification does not exist yet (section 12).

Provenance: `library_source`. Kind: semantic. Confidence: high.

---

## 9. Usage text

### CMD-USAGE-001: Usage text of a node

A literal has its name as its usage text. An argument has `<`, its name and `>`. The root has
the empty text.

Provenance: `library_source`. Kind: semantic. Confidence: high.

### CMD-USAGE-002: Short usage

- **Inputs:** a node `N`, a source, a flag `optional` and a flag `deep`.
- **Behavior:** if the requirement of `N` fails, there is no usage. Otherwise:
  1. `self` is the usage text of `N`, in `[` and `]` when `optional` is true.
  2. If `deep` is true, the result is `self`.
  3. If `N` has a redirect, the result is `self`, a separator and a redirect text. The
     redirect text is `...` when the target is the root. Otherwise it is `-> ` and the usage
     text of the target.
  4. Let `K` be the children of `N` whose requirement passes, and `o` true when `N` is
     executable.
     - One child: the result is `self`, a separator, and the short usage of the child with
       `optional = o` and `deep = o`, if that usage exists.
     - More than one child: compute the short usage of each child with `optional = o` and
       `deep = true`. If they give one distinct text, the result is `self`, a separator, and
       that text, in `[` and `]` when `o` is true. If they give more than one distinct text,
       the result is `self`, a separator, and the usage texts of all of `K` joined by `|`. The
       joined text is in `[` and `]` when `o` is true, or in `(` and `)` when `o` is false.
  5. Otherwise the result is `self`.
- **Postconditions:** the short usage of a node `P` for a source is the short usage of each
  child of `P`. For each child, `optional` is true when `P` is executable, and `deep` is false.

The client computes its own usage hint. On the server, the short usage is the text of `help`.

Provenance: `library_source`. Kind: semantic. Confidence: high.

---

## 10. Parity test

The parity fuzzer runs the same tree and the same command lines through Brigadier `1.3.11` from
Maven and through the implementation. For each command line it compares:

- the final cursor and the reported error of CMD-PARSE-007, with its kind and its position;
- the client validity test of CMD-PARSE-007;
- the range and the set of texts of the suggestion list of CMD-SUG-007 at the end of the line.

The fuzzer does not compare the order of the suggestions or the order of the errors of
CMD-PARSE-006. It does not compare any other item that a rule calls unspecified. When the
fuzzer finds a difference, it looks for the case in the entries of section 11. A difference
that an entry states is the expected result. Any other difference is a defect.

The trees use the six library argument types, literals, redirects to the root and to inner
nodes, and requirements. The command lines include the cases of CMD-RD-002, CMD-RD-004,
CMD-RD-010 and CMD-PARSE-007. They also include text outside the Basic Multilingual Plane, the
whitespace of CMD-TEXT-002, and runs of separators.

---

## 11. Intentional deviations

### CMD-DEV-001: Suggestions skip hidden children

- **Rule:** CMD-SUG-003.
- **Reference:** the library collects suggestions from every child of the parent, also from a
  child whose requirement fails. A crafted `command_suggestion` request can show the name of a
  hidden command.
- **Visible to the client:** only through an `ask_server` argument whose parent has a hidden
  child that matches the input. For other suggestions, the client uses its own tree, which has
  no hidden child. Whether a vanilla tree has such a parent is not known (section 12).
- **Test:** expect the hidden child in the reference reply and not in the reply of the
  implementation.

### CMD-DEV-002: Merge conflicts are errors

- **Rule:** CMD-TREE-003.
- **Reference:** the library ignores each field of the added node that differs from the node
  that the parent already has.
- **Visible to the client:** no. The merge occurs when the server builds its tree.
- **Test:** do not build a tree with a merge that has a conflict.

### CMD-DEV-003: A hidden literal does not hide its argument siblings

- **Rule:** CMD-PARSE-002.
- **Reference:** the library matches the token against every literal child, then removes the
  match if its requirement fails. No argument sibling is tried, and the parse fails.
- **Visible to the client:** yes. The tree of the client has no hidden literal, so the client
  tries the argument siblings and can accept the line. The reference server then rejects the
  line that the client accepted.
- **Test:** expect the reference to fail where the implementation tries the argument siblings.

### CMD-DEV-004: A redirect to a hidden target is not followed

- **Rule:** CMD-TREE-004.
- **Reference:** the library follows a redirect without a test of the requirements of the
  target and its path. The requirement of each child of the target still applies.
- **Visible to the client:** yes. The vanilla server removes such a redirect from the tree of
  the player. The client then parses the node as a node with no redirect and no children. The
  reference server can accept a line that the client rejects.
- **Test:** expect the result of the implementation to equal the parse of the client tree.

### CMD-DEV-005: Argument types fail only with syntax errors

- **Rule:** CMD-PARSE-003.
- **Reference:** when an argument type throws an error that is not a syntax error, the library
  makes the error `dispatcher_parse_exception` with the key `command.exception`. The argument
  is the message of the error, or the text `null`. The position is the cursor at the moment of
  the error.
- **Visible to the client:** no. The client shows such an error only for a defect in its own
  argument types.
- **Test:** an implementation argument type that fails without a syntax error is a defect.

### CMD-DEV-006: The context text and the error position count characters

- **Rule:** CMD-TEXT-001 and CMD-ERR-003.
- **Reference:** the error position and the context text count code units. The context text
  starts 10 code units before the error position, also when that start is inside a surrogate
  pair.
- **Visible to the client:** yes, but only for a command line with a character outside the
  Basic Multilingual Plane before the error. The position number in the error message is
  smaller. The context text holds 10 characters, not 10 code units, and never starts with an
  unpaired surrogate.
- **Test:** for such a command line, compare the error kind and ignore the position number and
  the context text.

### CMD-DEV-007: A redirect node has no children

- **Rule:** CMD-TREE-001.
- **Reference:** the node builders refuse children on a node with a redirect, but a merge can
  add them. The parse and the suggestions never visit these children.
- **Visible to the client:** no.
- **Test:** do not build such a tree.

### CMD-DEV-008: A suggestion request always gets a reply

- **Rule:** CMD-SUG-003.
- **Reference:** the suggestion source of a child can throw an error that is not a syntax
  error. It can also complete with an error later. In both cases the request stops, and the
  server sends no reply.
- **Visible to the client:** yes. With no reply, the client shows no new suggestions for the
  argument.
- **Test:** expect a reply from the implementation where the reference sends none.

### CMD-DEV-009: The lower case form of the remaining input

- **Rule:** CMD-SUG-005.
- **Reference:** the library takes the lower case form of the whole truncated input, then cuts
  it at the start. When a character before the start changes length in lower case, for example
  U+0130, the lower case remaining input moves.
- **Visible to the client:** only for a server suggestion after such a character.
- **Test:** do not compare suggestions for an input with such a character before the start.

### CMD-DEV-010: Every suggestion is a text

- **Rule:** CMD-SUG-005 and CMD-SUG-006.
- **Reference:** the library also has integer suggestions. They skip the test against the
  remaining input. They compare by value with each other, and by text in a case-sensitive way
  with a text suggestion. They become text suggestions when CMD-SUG-006 expands their range.
- **Visible to the client:** no. The integer values of a block property are integer
  suggestions (`argument-block.md`, ARG-BLK-016), but only the client makes them: those
  arguments have no suggestion type `minecraft:ask_server`. No suggestion source of the server
  makes integer suggestions, and the `command_suggestions` packet carries only texts.
- **Test:** do not use integer suggestions.

### CMD-DEV-011: The list of one child is sorted

- **Rule:** CMD-SUG-007.
- **Reference:** when the parent has exactly one child, the library returns the list of that
  child as it is, without CMD-SUG-006.
- **Visible to the client:** only when the suggestion source of that child returns a list that
  is not sorted. The order of the reply is then different.
- **Test:** do not compare the order of suggestions (section 10).

### CMD-DEV-012: A number outside the finite range is a syntax error

- **Rule:** CMD-RD-004 and CMD-RD-005.
- **Reference:** the library reads such a token as an infinity, with no error. An argument
  type with a range test then fails with `double_too_high` or `float_too_high`, with the text
  `Infinity` as the value. Coordinates and the selector options `x`, `y`, `z`, `dx`, `dy` and
  `dz` have no range test and keep the infinity. A command then decides the result. For
  example, `teleport` fails at run time with `commands.teleport.invalidPosition`.
- **Visible to the client:** yes, but only for unusual input. The client parse accepts the
  token or reports a range error. The implementation rejects the token with a syntax error at
  its start. The grammar policy of section 0.2.1 allows this difference.
- **Test:** expect `reader_invalid_double` or `reader_invalid_float` from the implementation
  where the reference reads an infinity.

### CMD-DEV-013: Positions inside the implementation are not code units

- **Rule:** CMD-TEXT-001.
- **Reference:** the library stores the command line as UTF-16 and counts every position in
  code units.
- **Visible to the client:** no. The codec of `command_suggestions` converts the range to code
  units (`commands-packet.md`, CMDP-SUG-002). An error position reaches the client only as text
  (CMD-DEV-006).
- **Test:** a differential test converts each suggestion range to code units before it compares
  the range.

### CMD-DEV-014: The information separators are not whitespace

- **Rule:** CMD-TEXT-002.
- **Reference:** U+001C to U+001F are whitespace.
- **Visible to the client:** no for a player, who cannot send a character below U+0020
  (`commands-packet.md`, CMDP-EXEC-002). A command block or a function that skips such a
  character in an argument of the game fails with a syntax error. The server is then stricter
  than the client, as section 0.2.1 allows.
  An SNBT text in a data pack field fails in the same way (`snbt.md`, SNBT-ARG-004).
- **Test:** do not compare a command line with a character from U+001C to U+001F.

---

## 12. Open questions

- The game runs a command chain in its own executor, with the game rules
  `max_command_sequence_length` and `max_command_forks`. Its order, its limits and its results
  need a specification before a command with a modifier or a fork exists.
- CMD-DEV-001: whether a vanilla tree has a node with an `ask_server` argument child and a
  child that a requirement hides from some players. Experiment: list each such node of the
  server tree. For each one, send a `command_suggestion` request as a player without
  permissions, and compare the reply with the tree of that player.
