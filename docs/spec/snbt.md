# SNBT, the text form of NBT: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |
| Parser ids | `nbt_compound_tag` 21, `nbt_tag` 22, `nbt_path` 23 (`commands-packet.md`, CMDP-ARG-001) |

Source of the version values: `assets/minecraft/version.json`. Source of the parser ids:
`assets/mcrs/reports/registries.json`, registry `minecraft:command_argument_type`.

This document states SNBT, the text form of NBT, as the game reads it and writes it. It states
the text that the read accepts, the tag that each text gives, the errors with their positions,
the suggestions, and the two printed forms. Section 8 states how the command arguments use the
read. The document does not prescribe an architecture. The grammar of section 2 describes the
accepted language and the order of the errors. It is not a design for a parser.

## 0. Conventions

### 0.1 Terms

- **Code unit**, **position**, **cursor**, **whitespace**, **context text**: as
  `command-parser.md`, section 0.1, CMD-TEXT-002 and CMD-ERR-003, defines them.
- **Command line**: the text that holds the SNBT text. Every position in this document is a
  position in the command line. The examples give positions as offsets from the first code unit
  of the SNBT text.
- **SNBT text**: the part of the command line that the read starts at.
- **Read**: the operation that converts SNBT text to one tag (SNBT-GR-001).
- **Tag**: one NBT value. A tag has one of twelve **tag types**: byte, short, int, long, float,
  double, string, list, compound, byte array, int array, long array.
- **Numeric tag**: a tag of type byte, short, int, long, float or double.
- **Compound**: a tag that maps keys to tags. A **key** is a string.
- **List**: a tag that holds an ordered sequence of tags. The tags of one list can have
  different types.
- **Array**: a byte array, an int array or a long array.
- **Token**: the unit of the grammar that the read takes from the text. SNBT-TOK-002 to
  SNBT-TOK-006 state the kinds of token.
- **Character token**: a token of one code unit from a fixed set (SNBT-TOK-003).
- **Digit run**: a token of digits and `_` (SNBT-TOK-004).
- **Word**: a token of unquoted characters (SNBT-TOK-005).
- **Production**: one named rule of the grammar of section 2, for example `float`.
- **Check**: a test that a production applies after it reads its last token (section 2.10).
- **Record**: a pair of a position and an error that the read keeps when a token or a check
  fails (SNBT-ERR-001).
- **Reported error**: the error that the read gives when it fails (SNBT-ERR-002).
- **Compact form**: the printed form of a tag as one plain text (SNBT-PRINT-003).
- **Text form**: the printed form of a tag as a styled text component (SNBT-PRINT-004).

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents".

One artefact applies: `crates/mcrs_minecraft_snbt/src/fixtures/snbt.json`. The game wrote it with
its own read, its compact form and its number text. It lists texts that the read accepts, with
the compact form of each result, and texts that the read rejects. It has no error keys and no
positions. A rule that cites it has the label `source_suggested_black_box_confirmed` for the
accepted set and the values, and `source_suggested` for its errors.

None of the experiments has been run. The experiments use these commands as an operator:

- `data modify storage t:s v set value <text>`: an `nbt_tag` argument (SNBT-ARG-001). Then
  `data get storage t:s v` shows the result in the text form.
- `data merge storage t:s <text>`: an `nbt_compound_tag` argument (SNBT-ARG-002).

The vanilla client reads SNBT itself, to colour and to underline the command line. Thus the
rules of the read are of the kind client agreement.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `SNBT-TOK` | tokens and whitespace |
| `SNBT-GR` | the grammar |
| `SNBT-VAL` | the tag that a text gives |
| `SNBT-ERR` | errors, their positions and the choice of the reported error |
| `SNBT-LIM` | limits |
| `SNBT-SUG` | suggestions |
| `SNBT-PRINT` | the compact form and the text form |
| `SNBT-ARG` | the command arguments and the other readers |
| `SNBT-DEV` | intentional deviations |

### 0.4 Scope

In scope: the read of one tag from a command line, its errors, its suggestions, the compact
form and the text form.

Out of scope: the binary NBT format and its limits; the path syntax of `nbt_path`; the commands
`data`, `summon` and `execute store`; the structure file text that the development tools write.
The data pack fields that hold SNBT text use the full-text read of SNBT-ARG-004. Their load
errors are out of scope.

### 0.5 Notation of the grammar

| Notation | Meaning |
|----------|---------|
| `'c'` | a character token that accepts the code unit `c` |
| `'a\|A'` | a character token that accepts `a` or `A`. It is one token with one error |
| `DEC`, `HEX`, `BIN` | a digit run (SNBT-TOK-004) |
| `WORD` | a word (SNBT-TOK-005) |
| `TEXT`, `HEX2`, `HEX4`, `HEX8`, `NAME` | the tokens in a quoted string (SNBT-TOK-006) |
| `a b` | `a`, then `b` |
| `a \| b` | ordered choice: try `a`; if `a` fails, go back and try `b` |
| `[ a ]` | `a` or nothing |
| `{ a }` | `a` zero or more times |
| `&a` | look ahead: the choice continues only if `a` can be read next. Nothing is read and nothing is recorded |
| `↑` | commit: if the alternative fails after this mark, the nearest choice of this production fails and tries no later alternative |
| `fail(k)` | record the error `k` at the cursor, then fail |
| `ε` | read nothing |

A choice tries its alternatives in the written order. When an alternative fails, the cursor
goes back to the start of that alternative.

---

## 1. Tokens

### SNBT-TOK-001: Input

- **Behavior:** the read starts at the cursor of the command line. It never reads before the
  cursor. It can read up to the end of the command line.
- **Postconditions:** after a read that succeeds, the cursor is after the last token of the
  tag. The read does not skip whitespace after the tag.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: run
`data modify storage t:s v set value 1x`. The reply is `command.expected.separator`: the
argument ends after `1`, and `x` is not a separator.

### SNBT-TOK-002: Whitespace before a token

- **Behavior:** before each character token, each digit run and each word, the read skips
  whitespace (CMD-TEXT-002). A look ahead also looks past whitespace. The tokens in a quoted
  string (SNBT-TOK-006) do not skip whitespace.
- **Postconditions:** whitespace can occur between any two tokens. This includes the inside of
  a number and the place after `\` in a string.
- **Edge cases:**

| Text | Result |
|------|--------|
| `{ a : 1 , b : "x" , }` | the compound `{a:1,b:"x"}` |
| `{a:1 b}` | `a` is the byte 1 |
| `{a:1 .5}` | `a` is the double 1.5 |
| `- 5` | the int −5 |
| `1 u b` | the byte 1 |
| `1 e 5` | the double 100000.0 |
| `[B; 1 , 2 ]` | the byte array `[B;1B,2B]` |
| `"a\ nb"` | a string with a line feed between `a` and `b` |
| `{a:1 000}` | an error: `1` ends the value, and `0` is not `,` or `}` |

Provenance: `source_suggested_black_box_confirmed`
(`crates/mcrs_minecraft_snbt/src/fixtures/snbt.json`, the cases `{a:1 b}`, `{a:1 .5}` and
`[B; 1 , 2 ]`). Kind: client agreement. Confidence: medium for the string case, high for the
other cases. Experiment: run `data modify storage t:s v set value "a\ nb"`. Run
`data get storage t:s v`. The string has a line feed, shown as `\n`.

### SNBT-TOK-003: Character tokens

- **Behavior:** a character token accepts one code unit from its set. When the next code unit
  is not in the set, or the read is at the end, the token fails. It records
  `argument.literal.incorrect` at the cursor after the whitespace. The key argument is the set,
  each code unit as text, joined with `|`, for example `f|F`.
- **Postconditions:** the order of the code units in the key argument is the order of section
  2: the lower case letter comes first.

Provenance: `source_suggested`. Kind: wire for the key and its argument, client agreement for
the position. Confidence: medium. Experiment: run `data merge storage t:s {a b:1}`. The reply is
"Expected literal :", and its context text ends after `{a b`.

### SNBT-TOK-004: Digit runs

- **Behavior:** a digit run reads the longest sequence of its code units at the cursor:

| Run | Code units |
|-----|-----------|
| `DEC` | `0` to `9`, `_` |
| `HEX` | `0` to `9`, `a` to `f`, `A` to `F`, `_` |
| `BIN` | `0`, `1`, `_` |

  1. If the sequence is empty, the run fails. It records `snbt.parser.expected_decimal_numeral`,
     `snbt.parser.expected_hex_numeral` or `snbt.parser.expected_binary_numeral` at its start.
  2. If the sequence starts or ends with `_`, the run fails. It records
     `snbt.parser.underscore_not_allowed` at its start. The run does not give back the `_`.
  3. Otherwise the run succeeds. A `_` inside the run has no value.
- **Edge cases:** `1__000` is the int 1000. `1_` is not a number: the run `1_` fails, and the
  read does not try the run `1`.

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `{a:1__0}` and `{a:0_1}`).
Kind: client agreement. Confidence: medium. Experiment: run
`data modify storage t:s v set value 1__000`. `data get storage t:s v` shows `1000`.

### SNBT-TOK-005: Words

- **Behavior:** a word reads the longest sequence of unquoted characters at the cursor
  (`command-parser.md`, CMD-RD-006): `0` to `9`, `A` to `Z`, `a` to `z`, `_`, `-`, `.` and `+`.
  If the sequence is empty, the word fails. It records `snbt.parser.expected_unquoted_string`
  at the cursor after the whitespace.
- **Edge cases:** a word can start with a digit, `+`, `-` or `.`. A key can be such a word. A
  value cannot (SNBT-GR-008).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: run
`data merge storage t:s {1st:2}`. Run `data get storage t:s`. The reply shows `1st: 2`.

### SNBT-TOK-006: Tokens in a quoted string

These tokens do not skip whitespace.

| Token | Code units | Error on failure | Position |
|-------|-----------|------------------|----------|
| `TEXT` | one or more code units that are not `"`, `'` or `\` | `snbt.parser.invalid_string_contents` | its start |
| `HEX2`, `HEX4`, `HEX8` | exactly 2, 4 or 8 code units `0`–`9`, `a`–`f`, `A`–`F` | `snbt.parser.expected_hex_escape`, with the text `2`, `4` or `8` | its start |
| `NAME` | one or more of `-`, `A`–`Z`, `a`–`z`, `0`–`9` and space | `snbt.parser.invalid_character_name` | its start |

`HEXn` reads at most `n` code units. When fewer than `n` hex digits are next, it fails. `TEXT`
includes whitespace and line feeds.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: run
`data merge storage t:s {a:"\x4"}`. The reply has `snbt.parser.expected_hex_escape` with `2`,
and its context text ends after `"\x`.

---

## 2. Grammar

The grammar below is complete. Section 2.10 lists the checks. Section 3 states the tag that each
production gives.

```
value        = &number-start ( float | integer )             check: integer range
             | &( '"' | "'" ) ↑ quoted
             | &'{' ↑ compound
             | &'[' ↑ list
             | word-value

compound     = '{' [ entry { ',' entry } [ ',' ] ] '}'
entry        = key ':' value                                  check: key
key          = quoted | WORD

list         = '[' ( array-type ';' [ integer { ',' integer } [ ',' ] ]
                   | [ value { ',' value } [ ',' ] ] ) ']'    check: array elements
array-type   = 'B' | 'L' | 'I'

quoted       = '"' ↑ { TEXT | '\' escape | "'" } '"'
             | "'" { TEXT | '\' escape | '"' } "'"
escape       = 'b' | 's' | 't' | 'n' | 'f' | 'r' | '\' | "'" | '"'
             | 'x' HEX2 | 'u' HEX4 | 'U' HEX8
             | 'N' '{' NAME '}'                               check: code point, name

float        = [ sign ] ( DEC '.' ↑ [ DEC ] [ exponent ] [ float-suffix ]
                        | '.' ↑ DEC [ exponent ] [ float-suffix ]
                        | DEC exponent ↑ [ float-suffix ]
                        | DEC [ exponent ] float-suffix )      check: finite
exponent     = 'e|E' [ sign ] DEC
float-suffix = 'f|F' | 'd|D'
sign         = '+' | '-'

integer      = [ sign ] ( '0' ↑ ( 'x|X' ↑ HEX
                                | 'b|B' BIN
                                | DEC ↑ fail(snbt.parser.leading_zero_not_allowed)
                                | ε )
                        | DEC ) [ int-suffix ]
int-suffix   = 'u|U' type-letter | 's|S' type-letter
             | 'b|B' | 's|S' | 'i|I' | 'l|L'
type-letter  = 'b|B' | 's|S' | 'i|I' | 'l|L'

word-value   = WORD [ '(' [ value { ',' value } [ ',' ] ] ')' ]  check: start, operation
```

`number-start` is a look ahead at one of `+`, `-`, `.` and `0` to `9`, after whitespace.

### SNBT-GR-001: Value

- **Behavior:** the read reads one `value`. The first code unit after whitespace selects the
  form:
  - `+`, `-`, `.` or a digit: the read tries `float`, then `integer`. If both fail, the read
    tries `word-value`, which then fails (SNBT-GR-008).
  - `"` or `'`: a quoted string. No other form is tried.
  - `{`: a compound. No other form is tried.
  - `[`: a list or an array. No other form is tried.
  - another code unit or the end: `word-value`.
- **Postconditions:** the float form comes before the integer form. A text that both can read
  is a float only when it has `.`, an exponent or a float suffix.
- **Edge cases:** an empty text, or a text of whitespace only, fails with
  `snbt.parser.expected_unquoted_string` at its end. `{` and `[` commit: a text that starts with
  `{` gives a compound or an error, never another type.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: run
`data modify storage t:s v set value 5` and `data get storage t:s v`; the reply shows `5`. Run
the same with `5.`; the reply shows `5.0d`.

### SNBT-GR-002: Compound

- **Behavior:** a compound is `{`, zero or more entries separated by `,`, an optional final
  `,`, and `}`. An entry is a key, `:` and a value. A key is a quoted string or a word.
- **Edge cases:**

| Text | Result |
|------|--------|
| `{}`, `{ }` | an empty compound |
| `{a:1,}` | one entry; the final comma is accepted |
| `{,}` | an error: an entry or `}` is necessary after `{` |
| `{a:1,,b:2}` | an error |
| `{true:1}` | the key is the string `true`, not a byte |
| `{1a:2}`, `{-:2}` | the keys `1a` and `-` |
| `{a}`, `{a:}` | an error |
| `{minecraft:stone:1}` | an error: a word has no `:` |

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `{a:1,}`, `{true:1}`,
`{,}` and `{a}`). Kind: client agreement. Confidence: high. Experiment: run
`data merge storage t:s {true:1}`. Run `data get storage t:s`. The reply shows the key `true`
with the value `1`.

### SNBT-GR-003: List and array

- **Behavior:** after `[`, the read first tries an array: `B`, `L` or `I`, then `;`, then zero
  or more integers separated by `,` with an optional final `,`. If that alternative fails at
  any token, the read goes back to the position after `[` and reads a list: zero or more values
  separated by `,`, with an optional final `,`. Then the read expects `]`.
- **Postconditions:** an array element is an `integer` (SNBT-GR-007). A float, a string, a
  list or a compound is not an array element.
- **Edge cases:**

| Text | Result |
|------|--------|
| `[]`, `[ ]` | an empty list |
| `[B;]`, `[I;]`, `[L;]` | an empty array |
| `[B ; 1]` | a byte array; whitespace is accepted before `B` and `;` |
| `[B]`, `[B,C]` | a list of the strings `B` and `C` |
| `[b;1]` | an error: the array letter is upper case only |
| `[1,2,]` | a list of two ints |
| `[,]`, `[1,,2]`, `[1 2]` | an error |
| `[1b,"a",{}]` | a list of a byte, a string and a compound |
| `[B;1.5]` | an error |

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `[B;]`, `[1,2,3,]`,
`["a",1]`, `[,]` and `[1 2]`). Kind: client agreement. Confidence: high for the fixture cases,
medium for `[B]`. Experiment: run `data modify storage t:s v set value [B]`.
`data get storage t:s v` shows `["B"]`.

### SNBT-GR-004: Quoted string

- **Behavior:** a quoted string starts with `"` or `'` and ends with the same code unit. Between
  them, the read reads `TEXT`, escapes, and the other quote code unit, in any order.
- **Postconditions:** in a string in `"`, the code unit `'` needs no escape, and the reverse.
  The escapes `\"` and `\'` are valid in both kinds of string.
- **Edge cases:** `""` and `''` are the empty string. A string that does not end before the
  end of the command line fails (SNBT-ERR-004).

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `{a:""}`, `{a:''}`,
`{a:"quote's"}` and `{a:'both "\''}`). Kind: client agreement. Confidence: high. Experiment: run
`data merge storage t:s {a:'x"y'}`. Run `data get storage t:s a`. The reply shows `'x"y'`.

### SNBT-GR-005: Escapes

- **Behavior:** after `\`, the read reads one escape letter and gives these code units:

| Escape | Result |
|--------|--------|
| `\b` | U+0008 |
| `\s` | U+0020, a space |
| `\t` | U+0009 |
| `\n` | U+000A |
| `\f` | U+000C |
| `\r` | U+000D |
| `\\`, `\'`, `\"` | the code unit after `\` |
| `\xHH` | the code point with the hex value `HH` |
| `\uHHHH` | the code point with the hex value `HHHH` |
| `\UHHHHHHHH` | the code point with the hex value `HHHHHHHH` |
| `\N{name}` | the code point with that Unicode character name (SNBT-VAL-007) |

- **Postconditions:** each escape letter is a character token. Whitespace before it is
  skipped (SNBT-TOK-002). The hex digits and the name are not character tokens.
- **Edge cases:** any other code unit after `\` fails. The escape letters are case-sensitive:
  `\X` and `\S` fail. `\U` is the only escape with upper case.

Provenance: `source_suggested_black_box_confirmed` (the fixture cases with `\x41`, `é`,
`\U0001F600` and `\s`). Kind: client agreement. Confidence: high for the table, medium for `\N`.
Experiment: run `data merge storage t:s {a:"\N{SNOWMAN}"}`. Run `data get storage t:s a`. The
reply shows `"☃"`.

### SNBT-GR-006: Float form

- **Behavior:** a float has an optional sign and one of these forms:
  1. digits, `.`, optional digits, an optional exponent and an optional suffix;
  2. `.`, digits, an optional exponent and an optional suffix;
  3. digits, an exponent and an optional suffix;
  4. digits and a suffix.

  An exponent is `e` or `E`, an optional sign and digits. A suffix is `f`, `F`, `d` or `D`.
- **Postconditions:** digits alone, with no `.`, exponent or suffix, are not a float. A float
  has no hex or binary form.
- **Edge cases:**

| Text | Result |
|------|--------|
| `1.`, `.5`, `+.5` | the doubles 1.0, 0.5 and 0.5 |
| `5.e1` | the double 50.0 |
| `1e5`, `1E2`, `1e+5` | the doubles 100000.0, 100.0 and 100000.0 |
| `1f`, `1D` | the float 1.0 and the double 1.0 |
| `00.5`, `01e1` | the doubles 0.5 and 10.0; a zero at the start is valid in a float |
| `1_0.0_1` | the double 10.01 |
| `.` , `-.` | an error |
| `1e` | the int 1, and `e` is left |
| `1.5b` | the double 1.5, and `b` is left |
| `1e400` | the float form fails its check; the int 1 is read, and `e400` is left (SNBT-VAL-004) |

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `{a:5.e1}`, `{a:+.5}`,
`{a:00.5}`, `{a:1_0.0_1}`, `{a:.}` and `{a:1e}`). Kind: client agreement. Confidence: high.
Experiment: run `data merge storage t:s {a:00.5}`. Run `data get storage t:s a`. The reply shows
`0.5d`.

### SNBT-GR-007: Integer form

- **Behavior:** an integer has an optional sign, a body and an optional suffix. The body is one
  of:
  - `0x` or `0X` and a `HEX` run: the base is 16;
  - `0b` or `0B` and a `BIN` run: the base is 2;
  - `0` alone: the value 0, base 10;
  - a `DEC` run that does not start with `0`: base 10.

  The suffix is a type letter `b`, `s`, `i` or `l`, in either case. Before the type letter, the
  suffix can have `u` or `s`, in either case, for unsigned or signed.
- **Postconditions:**
  - After `0x`, a `HEX` run is necessary. `0x` alone fails.
  - After `0b`, if no `BIN` run follows, the `b` is the byte suffix. Thus `0b` is the byte 0.
  - `0` and then a `DEC` run fails with `snbt.parser.leading_zero_not_allowed`.
  - `s` alone is the short suffix. `s` before a type letter is the signed mark.
  - A hex run includes `b`. Thus `0x1b` is the int 27, not a byte.
- **Edge cases:**

| Text | Result |
|------|--------|
| `0b101`, `0B1L` | the int 5 and the long 1 |
| `0b`, `0b2` | the byte 0; for `0b2`, the `2` is left |
| `0xFFb` | the int 4091 |
| `0x10ub`, `0x10sb` | the byte 16 |
| `1sb`, `1ss`, `1si`, `1sl` | a byte, a short, an int and a long |
| `1u` | the int 1, and `u` is left |
| `01`, `00`, `-012` | an error |
| `_1` | not a number: a word (SNBT-GR-008) |

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `{a:0b}`, `{a:0x1b}`,
`{a:0b1L}`, `{a:1sb}`, `{a:01}` and `{a:_1}`). Kind: client agreement. Confidence: high.
Experiment: run `data merge storage t:s {a:0x1b}`. Run `data get storage t:s a`. The reply shows
`27`.

### SNBT-GR-008: Word value, boolean and operation

- **Behavior:** a word value is a word and an optional argument list in `(` and `)`. The
  arguments are zero or more values separated by `,`, with an optional final `,`. Then:
  1. If the word starts with `+`, `-`, `.` or a digit, the word value fails with
     `snbt.parser.invalid_unquoted_start`.
  2. If an argument list is present, the word is an operation name (SNBT-VAL-009).
  3. If the word is `true` or `false`, in any case, the tag is the byte 1 or 0
     (SNBT-VAL-008).
  4. Otherwise the tag is the string of the word.
- **Postconditions:** whitespace before `(` is skipped. Thus `b (1)` is a call of the
  operation `b`.
- **Edge cases:**

| Text | Result |
|------|--------|
| `abc`, `a.b-c+d_e`, `_1` | the strings of the words |
| `TRUE`, `tRuE` | the byte 1 |
| `false1`, `true.` | the strings `false1` and `true.` |
| `Infinity`, `NaN` | the strings `Infinity` and `NaN` |
| `a b` in a compound | an error after `a` |
| `@` | an error: `@` is not an unquoted character |

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `{a:tRuE}`, `{a:false1}`,
`{a:Infinity}` and `{a:a b}`). Kind: client agreement. Confidence: high. Experiment: run
`data merge storage t:s {a:TRUE}`. Run `data get storage t:s a`. The reply shows `1b`.

### SNBT-GR-009: Separators

- **Behavior:** in a compound, a list, an array and an argument list, a `,` separates two
  items. One `,` after the last item is accepted. A `,` with no item before it is an error.
- **Postconditions:** no other separator exists. `;` occurs only after the array letter. `=`
  is not a separator: `{foo=bar}` fails.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: run
`data merge storage t:s {foo=bar}`. The reply has `argument.literal.incorrect` with `:`.

### 2.10 Checks

A check applies when its production has read its last token. A check that fails records its
error at the cursor after that token, and the production fails.

| Production | Check | Error |
|------------|-------|-------|
| `value`, integer form | the integer fits its type (SNBT-VAL-002, SNBT-VAL-003) | `snbt.parser.expected_non_negative_number` or `snbt.parser.number_parse_failure` |
| `float` | the value is finite (SNBT-VAL-004) | `snbt.parser.infinity_not_allowed` |
| `entry` | the key is not empty | `snbt.parser.empty_key` |
| `list`, array form | each element in order (SNBT-VAL-005) | `snbt.parser.invalid_array_element_type`, `snbt.parser.expected_non_negative_number` or `snbt.parser.number_parse_failure` |
| `escape`, hex form | the code point is at most U+10FFFF | `snbt.parser.invalid_codepoint` |
| `escape`, name form | the name is known (SNBT-VAL-007) | `snbt.parser.invalid_character_name` |
| `word-value` | SNBT-GR-008 step 1, then SNBT-VAL-009 | `snbt.parser.invalid_unquoted_start`, `snbt.parser.no_such_operation`, `snbt.parser.expected_number_or_boolean`, `snbt.parser.expected_string_uuid` |

The check of `float` is part of the float form. When it fails, `value` tries the integer form
next. The check of `value` comes after the choice. When it fails, `value` fails.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: run
`data merge storage t:s {a:[B;300]}`. The reply has `snbt.parser.number_parse_failure`, and its
context text ends after `]`.

---

## 3. Values

### SNBT-VAL-001: Tag type of each form

| Form | Tag type |
|------|----------|
| compound | compound |
| list | list |
| `[B;...]`, `[I;...]`, `[L;...]` | byte array, int array, long array |
| quoted string, word | string |
| `true`, `false` | byte |
| float with `f` or `F` | float |
| float with `d`, `D` or no suffix | double |
| integer | byte, short, int or long (SNBT-VAL-002) |
| `bool(...)` | byte |
| `uuid(...)` | int array of 4 elements |

SNBT has no boolean type and no end tag. An empty list has no element type.

Provenance: `source_suggested_black_box_confirmed` (the fixture). Kind: semantic. Confidence:
high. Experiment: run `data merge storage t:s {a:1.5,b:1.5f,c:true}`. Run
`data get storage t:s`. The reply shows `1.5d`, `1.5f` and `1b`.

### SNBT-VAL-002: Type of an integer

- **Behavior:** the type letter selects the type: `b` byte, `s` short, `i` int, `l` long, in
  either case. An integer without a type letter is an int.
- **Edge cases:** `1I` and `1i` are ints. `1L` and `1l` are longs.

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `{a:1I}` and
`{a:1L,b:1l}`). Kind: semantic. Confidence: high. Experiment: run
`data merge storage t:s {a:1l}`. Run `data get storage t:s a`. The reply shows `1L`.

### SNBT-VAL-003: Sign and range of an integer

- **Inputs:** an integer with a sign, a base, digits and a suffix. A type `T` with `n` bits:
  8, 16, 32 or 64.
- **Behavior:**
  1. The integer is unsigned when the suffix has `u`. It is signed when the suffix has `s`.
     Without either mark, a base-10 integer is signed, and a base-16 or base-2 integer is
     unsigned.
  2. An unsigned integer with `-` fails with `snbt.parser.expected_non_negative_number`. This
     includes `-0x0`.
  3. The value is the digits without `_`, read in the base, with the sign.
  4. A signed value must be in `[−2^(n−1), 2^(n−1) − 1]`. An unsigned value must be in
     `[0, 2^n − 1]`. Otherwise the integer fails with `snbt.parser.number_parse_failure`.
  5. The tag holds the value as `n` bits in two's complement. An unsigned value at or above
     `2^(n−1)` thus gives a negative tag value.
- **Postconditions:** the key argument of `snbt.parser.number_parse_failure` is a message text
  (SNBT-ERR-003). `+` before an integer has no effect.
- **Edge cases:**

| Text | Tag |
|------|-----|
| `127b`, `-128b` | the bytes 127 and −128 |
| `128b`, `-129b` | an error |
| `255ub`, `1ub` | the bytes −1 and 1 |
| `-1ub` | `snbt.parser.expected_non_negative_number` |
| `200sb` | an error |
| `-0x80sb` | the byte −128 |
| `0xFFFFFFFF` | the int −1 |
| `0x100000000` | an error |
| `0x8000000000000000L` | the long −9223372036854775808 |
| `0x8000000000000000sl` | an error |
| `4294967295ui`, `65535us`, `18446744073709551615ul` | the int −1, the short −1, the long −1 |
| `12345678901` | an error: the type is int |
| `-0` | the int 0 |
| `-0x1sl` | the long −1 |

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `255ub`, `{a:-1ub}`,
`{a:200sb}`, `{a:0xFFFFFFFF}`, `{a:0x100000000}`, `{a:-0x1sl}` and `{a:12345678901}`). Kind:
semantic for the value, client agreement for the errors. Confidence: high. Experiment: run
`data merge storage t:s {a:255ub}`. Run `data get storage t:s a`. The reply shows `-1b`.

### SNBT-VAL-004: Float and double value

- **Behavior:**
  1. Make a decimal text from the sign, the digits before `.`, the digits after `.` and the
     exponent, without `_`.
  2. A double is the binary64 value nearest to that decimal value, with ties to even. A float
     is the binary32 value nearest to that decimal value, rounded one time from the decimal
     text, not through binary64.
  3. If the result is infinite, the float form fails with `snbt.parser.infinity_not_allowed`.
  4. A result of zero is positive zero. `-0.0` gives the double 0.0, not −0.0.
- **Postconditions:** a value too small for the type rounds to zero or to a subnormal value
  with no error. The text cannot give NaN.
- **Edge cases:**

| Text | Result |
|------|--------|
| `3.4028235e38f` | the float 3.4028235E38 |
| `1e39f`, `1e309` | the float form fails |
| `1e400d` | `snbt.parser.infinity_not_allowed` after `d` |
| `1e400` | the int 1 with `e400` left: the float form fails, then the integer form reads `1` |
| `-0.0`, `-0.0f` | positive zero |
| `1e-400` | the double 0.0 |

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `{a:3.4028235e38f}`,
`{a:1e39f}`, `{a:1e309}` and `{f:...,j:-0.0,...}`). Kind: semantic. Confidence: high.
Experiment: run `data merge storage t:s {a:-0.0}`. Run `data get storage t:s a`. The reply shows
`0.0d`.

### SNBT-VAL-005: Array elements

- **Behavior:** for each element of an array, in order:
  1. The element type is its type letter. Without a type letter, it is the type of the array:
     byte for `B`, int for `I`, long for `L`.
  2. The element type must be allowed. `B` allows byte. `I` allows int, byte and short. `L`
     allows long, byte, short and int. Otherwise the check fails with
     `snbt.parser.invalid_array_element_type`.
  3. Convert the element as an integer of the element type (SNBT-VAL-003).
  4. Widen the value to the type of the array, with its sign.
- **Postconditions:** the check stops at the first element that fails. The position of the
  error is after `]` (section 2.10), not at the element.
- **Edge cases:**

| Text | Result |
|------|--------|
| `[I;1,2b,3s]` | `[I;1,2,3]` |
| `[L;1,2,3L]` | `[L;1L,2L,3L]` |
| `[B;200ub]` | `[B;-56B]` |
| `[I;255ub]` | `[I;-1]`: the byte −1, widened |
| `[L;0xFFFFFFFF]` | `[L;4294967295L]` |
| `[L;0xFFFFFFFFi]` | `[L;-1L]` |
| `[I;0x10,0b1]` | `[I;16,1]` |
| `[B;200]`, `[B;-129]` | `snbt.parser.number_parse_failure` |
| `[I;1L]`, `[B;1s]` | `snbt.parser.invalid_array_element_type` |

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `[I;1,2b,3s]`,
`[B;200ub]`, `[I;0x10,0b1]`, `[B;200]` and `[I;1L]`). Kind: semantic for the value, client
agreement for the errors. Confidence: high. Experiment: run
`data merge storage t:s {a:[I;255ub]}`. Run `data get storage t:s a`. The reply shows `[I; -1]`.

### SNBT-VAL-006: String value

- **Behavior:** a string is a sequence of UTF-16 code units. `TEXT` gives its code units. An
  escape gives the code units of its code point: one code unit below U+10000, two code units
  above.
- **Postconditions:** `😀` gives the two code units of U+1F600, one character.
  SNBT-DEV-003 states the case of a surrogate without its pair.
- **Edge cases:** `\u0000` gives the code unit U+0000. `\U00110000` fails with
  `snbt.parser.invalid_codepoint`, with the text `U+00110000`. `\UFFFFFFFF` fails with the text
  `U+FFFFFFFF`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`data merge storage t:s {a:"😀"}`. Run `data get storage t:s a`. The reply shows `"😀"`.

### SNBT-VAL-007: Character name

- **Behavior:** the name of `\N{name}` is a Unicode character name, as the Name property of the
  Unicode Character Database states it. The comparison ignores the case of ASCII letters. It
  ignores spaces at the start and at the end of the name. An unknown name fails with
  `snbt.parser.invalid_character_name` after `}`.
- **Postconditions:** the algorithmic names are valid, for example `CJK UNIFIED IDEOGRAPH-4E00`.
  Name aliases are not valid. The Unicode version and the names of code points without a Name
  property are open (section 9).

Provenance: `source_suggested`. Kind: semantic for the names of the Name property, unspecified
for the other forms. Confidence: medium. Experiment: run
`data merge storage t:s {a:"\N{snowman}"}`. Run `data get storage t:s a`. The reply shows `"☃"`.

### SNBT-VAL-008: Duplicate key and boolean

- **Behavior:** when a compound has a key two times, the last value is the value of the key.
  `true` and `false` give the bytes 1 and 0.
- **Postconditions:** the compound keeps no order of its entries.

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `{a:1,a:2}` and
`{a:1,"a":2}`). Kind: semantic. Confidence: high. Experiment: run
`data merge storage t:s {a:1,a:2}`. Run `data get storage t:s a`. The reply shows `2`.

### SNBT-VAL-009: Operations

- **Behavior:** an operation has a name and a number of arguments. The name is case-sensitive.
  Two operations exist:

| Operation | Result |
|-----------|--------|
| `bool` with 1 argument | if the argument is a numeric tag, the byte 1 when its value is not zero, else the byte 0. Otherwise fail with `snbt.parser.expected_number_or_boolean` |
| `uuid` with 1 argument | if the argument is a string in the canonical UUID form (SNBT-DEV-002), an int array of 4 elements. Otherwise fail with `snbt.parser.expected_string_uuid` |

  Any other name or number of arguments fails with `snbt.parser.no_such_operation`. The key
  argument is the name, `/` and the number in decimal, for example `foo/1` or `bool/0`.
- **Postconditions:**
  - The int array of `uuid` holds the 128 bits of the UUID in four groups of 32 bits, the most
    significant group first. Each element is the group as a signed 32-bit value.
  - The argument of `uuid` can be a word, for example `uuid(f81d4fae-7dec-11d0-a765-00a0c91e6bf6)`.
    A UUID text that starts with a digit is not a word and must be quoted.
  - `bool(0.5)` and `bool(256)` give the byte 1. `bool(0.0)` gives the byte 0. `bool(true)`
    gives the byte 1.
- **Edge cases:** `bool()` fails with `bool/0`. `BOOL(1)` fails with `BOOL/1`. `true(1)` fails
  with `true/1`. `bool(1,)` is `bool` with 1 argument.

Provenance: `source_suggested_black_box_confirmed` (the fixture cases
`{u:uuid("00000000-0000-0000-0000-000000000000")}`, `{b:bool(1)}`, `{a:bool(0.0)}`,
`{a:bool("x")}`, `{a:foo(1)}` and `{a:uuid(1)}`). Kind: semantic. Confidence: high. Experiment:
run `data merge storage t:s {a:uuid("00000001-0002-0003-0004-000000000005")}`. Run
`data get storage t:s a`. The reply shows `[I; 1, 131075, 262144, 5]`.

---

## 4. Errors

### SNBT-ERR-001: Records

- **Behavior:** while it reads, the read keeps records:
  - a token that fails records its error at its position (section 1);
  - a check that fails records its error at the cursor after the last token of its production
    (section 2.10);
  - `fail(k)` records `k` at the cursor;
  - a look ahead records nothing.
- **Postconditions:** a token in an alternative that fails still keeps its record. A token in an
  optional part that is absent also keeps its record. The records come in the order of the
  reads: a sequence reads from left to right, and a choice tries its alternatives in the written
  order. A production that the read tries again at the same position gives the same result.
  This repetition is not observable.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: the
experiments of SNBT-ERR-004.

### SNBT-ERR-002: Reported error

- **Behavior:** when the read fails, the reported error is the first record at the largest
  position of all records. Its position is that largest position.
- **Postconditions:**
  - A record at a smaller position never becomes the reported error.
  - After a value, the read often tries more tokens at the same position: the rest of a
    number, `(` after a word, then `,` and `}` or `]`. These records come before the check of
    the production that holds the value. Thus an error of a check is often not the reported error.
  - When the read succeeds, no record has an effect.
- **Edge cases:** `{"":1}` fails with `argument.literal.incorrect` and `.` at offset 5, not with
  `snbt.parser.empty_key`. The float form first tries `.` after `1`. `{"":"a"}` fails with
  `snbt.parser.empty_key` at offset 7, because no token fails at offset 7 before the check.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: run
`data merge storage t:s {"":1}`. The reply is "Expected literal .", and its context text ends
after `{"":1`.

### SNBT-ERR-003: Error keys

The key arguments come in the order of the column. Each key argument is a plain text.

| Translation key | Key arguments | Position | Rule |
|-----------------|---------------|----------|------|
| `argument.literal.incorrect` | the code units of the token, joined with `\|` | the token, after whitespace | SNBT-TOK-003 |
| `snbt.parser.expected_unquoted_string` | none | the word, after whitespace | SNBT-TOK-005 |
| `snbt.parser.expected_decimal_numeral` | none | the run, after whitespace | SNBT-TOK-004 |
| `snbt.parser.expected_hex_numeral` | none | the run, after whitespace | SNBT-TOK-004 |
| `snbt.parser.expected_binary_numeral` | none | the run, after whitespace | SNBT-TOK-004 |
| `snbt.parser.underscore_not_allowed` | none | the start of the run | SNBT-TOK-004 |
| `snbt.parser.leading_zero_not_allowed` | none | after the `DEC` run that follows `0` | SNBT-GR-007 |
| `snbt.parser.invalid_string_contents` | none | the `TEXT` token | SNBT-TOK-006 |
| `snbt.parser.expected_hex_escape` | `2`, `4` or `8` | after the escape letter | SNBT-TOK-006 |
| `snbt.parser.invalid_character_name` | none | the `NAME` token, or after `}` | SNBT-TOK-006, SNBT-VAL-007 |
| `snbt.parser.invalid_codepoint` | `U+` and 8 upper case hex digits | after the hex digits | SNBT-VAL-006 |
| `snbt.parser.number_parse_failure` | a message | after the integer, or after `]` of an array | SNBT-VAL-003, SNBT-VAL-005 |
| `snbt.parser.expected_non_negative_number` | none | after the integer, or after `]` of an array | SNBT-VAL-003, SNBT-VAL-005 |
| `snbt.parser.invalid_array_element_type` | none | after `]` | SNBT-VAL-005 |
| `snbt.parser.infinity_not_allowed` | none | after the float | SNBT-VAL-004 |
| `snbt.parser.empty_key` | none | after the value of the entry | SNBT-GR-002 |
| `snbt.parser.invalid_unquoted_start` | none | after the word, or after `)` | SNBT-GR-008 |
| `snbt.parser.no_such_operation` | name `/` count | after `)` | SNBT-VAL-009 |
| `snbt.parser.expected_number_or_boolean` | none | after `)` | SNBT-VAL-009 |
| `snbt.parser.expected_string_uuid` | none | after `)` | SNBT-VAL-009 |
| `argument.nbt.expected.compound` | none | after the value | SNBT-ARG-002 |
| `argument.nbt.trailing` | none | the first code unit after the value that is not whitespace | SNBT-ARG-004 |
| `arguments.nbtpath.too_deep` | none | the `{` or `[` that opens the level 513 | SNBT-LIM-001 |

- **The message of `snbt.parser.number_parse_failure`:** the text is unspecified. An
  implementation should name the digits and the type. A parity test must not compare it.
- **Keys that no input raises:** the reference also defines `snbt.parser.expected_integer_type`
  and `snbt.parser.expected_float_type`. The grammar of section 2 cannot reach them. The keys
  `argument.nbt.expected.key`, `argument.nbt.expected.value`, `argument.nbt.list.mixed` and
  `argument.nbt.array.*` are in the language file, but the read does not raise them.
- **A key without a text:** the language file of the client has `snbt.parser.undescore_not_allowed`,
  not `snbt.parser.underscore_not_allowed`. The vanilla client thus shows the key itself for this
  error. The server sends the key `snbt.parser.underscore_not_allowed`.

Provenance: `source_suggested` (the language file `en_us.json` of the client jar, lines 367–377,
424 and 7196–7216). Kind: wire for the keys and the arguments, client agreement for the
positions, unspecified for the message text. Confidence: medium. Experiment: run
`data merge storage t:s {a:foo(1)}`. The chat line reads "No such operation: foo/1". Run
`data modify storage t:s v set value [B;0x1_]`. The chat line shows the key
`snbt.parser.underscore_not_allowed`.

### SNBT-ERR-004: Examples

These results follow from SNBT-ERR-001 and SNBT-ERR-002. The offset counts from the first code
unit of the SNBT text. "Literal `x`" means `argument.literal.incorrect` with the argument `x`.

| Text | Offset | Reported error |
|------|--------|----------------|
| `{` | 1 | literal `"` |
| `{,}` | 1 | literal `"` |
| `{a}` | 2 | literal `:` |
| `{a b:1}` | 3 | literal `:` |
| `{a:}` | 3 | `snbt.parser.expected_unquoted_string` |
| `{a:@}` | 3 | `snbt.parser.expected_unquoted_string` |
| `{a:1` | 4 | literal `.` |
| `{a:1a}` | 4 | literal `.` |
| `{a:1 000}` | 5 | literal `.` |
| `{a:012}` | 6 | literal `.` |
| `{a:b` | 4 | literal `(` |
| `{a:true` | 7 | literal `(` |
| `{a:"x"` | 6 | literal `,` |
| `{a:1b b:2}` | 6 | literal `,` |
| `{a:1.5b}` | 6 | literal `e\|E` |
| `{a:1.e}` | 6 | literal `+` |
| `{a:1e400}` | 8 | literal `f\|F` |
| `{a:1e400d}` | 9 | `snbt.parser.infinity_not_allowed` |
| `{"":1}` | 5 | literal `.` |
| `{"":"a"}` | 7 | `snbt.parser.empty_key` |
| `{a:-}` | 4 | `snbt.parser.expected_decimal_numeral` |
| `.` | 1 | `snbt.parser.expected_decimal_numeral` |
| `{a:0x}` | 5 | `snbt.parser.expected_hex_numeral` |
| `{a:0b2}` | 5 | `snbt.parser.expected_binary_numeral` |
| `[,]` | 1 | literal `B` |
| `[1,,2]` | 3 | `snbt.parser.expected_unquoted_string` |
| `[1 2]` | 3 | literal `.` |
| `[B; 1.5]` | 5 | literal `u\|U` |
| `[B;01]` | 5 | `snbt.parser.leading_zero_not_allowed` |
| `[B;1_]` | 3 | literal `+` |
| `[B;0x1_]` | 5 | `snbt.parser.underscore_not_allowed` |
| `[B;300]` | 7 | `snbt.parser.number_parse_failure` |
| `[B;1s]` | 6 | `snbt.parser.invalid_array_element_type` |
| `[B;-1ub]` | 8 | `snbt.parser.expected_non_negative_number` |
| `{a:128b}` | 7 | `snbt.parser.number_parse_failure` |
| `{a:-1ub}` | 7 | `snbt.parser.expected_non_negative_number` |
| `"abc` | 4 | `snbt.parser.invalid_string_contents` |
| `"\q"` | 2 | literal `b` |
| `{a:"\x4"}` | 6 | `snbt.parser.expected_hex_escape` with `2` |
| `"\x 41"` | 3 | `snbt.parser.expected_hex_escape` with `2` |
| `{a:"\U00110000"}` | 14 | `snbt.parser.invalid_codepoint` with `U+00110000` |
| `"\N{FOO}"` | 8 | `snbt.parser.invalid_character_name` |
| `-a()` | 4 | `snbt.parser.invalid_unquoted_start` |
| `{a:foo(1)}` | 9 | `snbt.parser.no_such_operation` with `foo/1` |
| `{a:b (1)}` | 8 | `snbt.parser.no_such_operation` with `b/1` |
| `{a:bool()}` | 9 | `snbt.parser.no_such_operation` with `bool/0` |
| `{a:bool("x")}` | 12 | `snbt.parser.expected_number_or_boolean` |
| `{a:uuid("x")}` | 12 | `snbt.parser.expected_string_uuid` |

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: for each
text with `{` at the start, run `data merge storage t:s <text>`. For each other text, run
`data modify storage t:s v set value <text>`. Compare the key, the key argument and the end of
the context text with the table.

---

## 5. Limits

### SNBT-LIM-001: Depth

- **Behavior:** the depth of the top tag is 1. An element of a list or an array, and a value of
  a compound, has the depth of its container plus 1. The read must reject a compound or a list
  at a depth above 512. The read then stops and reports `arguments.nbtpath.too_deep` at the `{`
  or `[` of that compound or list. SNBT-ERR-002 does not apply to this error.
- **Postconditions:** 512 is the depth limit of the binary NBT read. A tag that the SNBT read
  accepts can be saved and read again.

Provenance: `source_suggested`. Kind: semantic. Confidence: low. The reference has no limit in
the read (SNBT-DEV-001). Experiment: section 9.

### SNBT-LIM-002: No other limits

The read has no limit on the length of a string, the number of entries of a compound or the
number of elements of a list or an array. The limits of the command line apply
(`commands-packet.md`). The binary NBT format limits a string to 65535 bytes of modified UTF-8.
That limit applies when the game saves the tag, not in the read.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`data merge storage t:s {a:"<x>"}`, where `<x>` is 1000 code units `a`. Run
`data get storage t:s a`. The reply shows the string.

---

## 6. Suggestions

### SNBT-SUG-001: Suggestions of the `nbt_tag` argument

- **Inputs:** the text of the argument up to the cursor of the request (`command-parser.md`,
  CMD-SUG-001).
- **Behavior:**
  1. Read the text as SNBT-ARG-001 states.
  2. If the read keeps no record, there are no suggestions.
  3. Otherwise make a builder at the largest position of the records (CMD-SUG-005).
  4. For each record at that position, take its candidates:
     - a record of a character token: each code unit of its set, as one text. `f|F` gives `f`
       and `F`;
     - a record of `snbt.parser.invalid_unquoted_start`: `false`, `true`, `bool` and `uuid`;
     - any other record: no candidate.
  5. Filter the candidates with CMD-SUG-008 against the remaining input of the builder.
- **Postconditions:** the vanilla client makes these suggestions itself. The type
  `nbt_tag` has no suggestion type `minecraft:ask_server`, so the server receives no request
  for it. `nbt_compound_tag` makes no suggestions.
- **Edge cases:**

| Text before the cursor | Suggestions |
|------------------------|-------------|
| `{` | `"`, `'`, `}` |
| `{a:b` | `(`, `,`, `}` |
| `{a:1` | `.`, `e`, `E`, `f`, `F`, `d`, `D`, `u`, `U`, `s`, `S`, `b`, `B`, `i`, `I`, `l`, `L`, `,`, `}` |
| `[` | `B`, `L`, `I`, `]` |
| the empty text | none: the only record is `snbt.parser.expected_unquoted_string` |

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in the vanilla
client, type `/data modify storage t:s v set value {a:b`. The list shows `(`, `,` and `}`.

---

## 7. Printed forms

### SNBT-PRINT-001: Number text

- **Behavior:** an integer tag prints in decimal, with `-` for a negative value. A float or a
  double prints as follows. Let `R` be the set of decimals that round to the value in its
  binary format. Let `m` be the smallest number of significant digits in `R`. If `m ≥ 2`, the
  candidates are the decimals of `R` with `m` digits. If `m = 1`, the candidates are the
  decimals of `R` with 1 or 2 digits. The printed decimal is the candidate nearest to the value.
  For a tie, it is the candidate with an even last digit.
  1. If `10^−3 ≤ |v| < 10^7`, print the decimal in plain notation, with at least one digit
     after `.`.
  2. Otherwise print one digit, `.`, at least one more digit, `E` and the exponent in decimal.
     A negative exponent has `-`. A positive exponent has no sign.
  3. Zero prints `0.0`, and negative zero `-0.0`. NaN prints `NaN`. The infinities print
     `Infinity` and `-Infinity`.
- **Postconditions:** the SNBT read never gives NaN, an infinity or negative zero. A tag from
  another source can have them.
- **Edge cases:**

| Value | Text |
|-------|------|
| double 1.0E7 | `1.0E7` |
| double 9999999.0 | `9999999.0` |
| double 12345678.9 | `1.23456789E7` |
| double 0.001 | `0.001` |
| double 0.0001 | `1.0E-4` |
| float 16777216 | `1.6777216E7` |
| float 3.4028235E38 | `3.4028235E38` |

Provenance: `source_suggested_black_box_confirmed` (the number text of the Java platform; the
fixture lists `doubles` and `floats` and the case `{a:1.0E7,...}`). Kind: semantic. Confidence:
high. Experiment: run `data merge storage t:s {a:12345678.9}`. Run `data get storage t:s a`. The
reply shows `1.23456789E7d`.

### SNBT-PRINT-002: Quoted string

- **Behavior:** to quote a string `s`:
  1. Select the quote. If `s` has no `"` and no `'`, the quote is `"`. Otherwise find the first
     `"` or `'` in `s`. If it is `"`, the quote is `'`. If it is `'`, the quote is `"`.
  2. Write the quote. Then write each code unit of `s`:
     - `\` as `\\`;
     - the quote code unit as `\` and the quote;
     - the other quote code unit as it is;
     - U+0008, U+0009, U+000A, U+000C and U+000D as `\b`, `\t`, `\n`, `\f` and `\r`;
     - any other code unit below U+0020 as `\x` and two upper case hex digits;
     - any other code unit as it is.
  3. Write the quote.
- **Edge cases:** `it's "q"` prints as `"it's \"q\""`. `both "\'` prints as `'both "\''`.
  U+0001 prints as `\x01`. U+007F and U+00A0 print as they are.

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `{a:'both "\''}`,
`{ctrl:"\x01"}` and the `pretty` text). Kind: semantic. Confidence: high. Experiment: run
`data merge storage t:s {a:"it's \"q\""}`. Run `data get storage t:s a`. The reply shows
`"it's \"q\""`.

### SNBT-PRINT-003: Compact form

- **Behavior:**

| Tag | Compact form |
|-----|--------------|
| byte | number text and `b` |
| short | number text and `s` |
| int | number text |
| long | number text and `L` |
| float | number text and `f` |
| double | number text and `d` |
| string | the quoted string (SNBT-PRINT-002) |
| byte array | `[B;`, each element as number text and `B`, separated by `,`, then `]` |
| int array | `[I;`, each element as number text, separated by `,`, then `]` |
| long array | `[L;`, each element as number text and `L`, separated by `,`, then `]` |
| list | `[`, each element in its compact form, separated by `,`, then `]` |
| compound | `{`, each entry as key, `:` and value, separated by `,`, then `}` |

- **Keys:** the entries come in the order of their keys, compared code unit by code unit. A key
  prints as it is when it matches `[A-Za-z._][A-Za-z0-9._+-]*` and is not `true` or `false` in
  any case. Otherwise it prints as a quoted string.
- **Postconditions:** the compact form has no whitespace outside strings. The read of the
  compact form gives the same tag, except for NaN and the infinities.
- **Use:** the game uses the compact form for a tag in the key argument of an error message,
  and in a data pack field that holds a compound as text.

Provenance: `source_suggested_black_box_confirmed` (the fixture, column `snbt` and the case
`{x.y:1,_z:2,"1a":3,a-b:4,"a b":5}`). Kind: semantic. Confidence: high. Experiment: run
`data merge storage t:s {b:{x:1b,"1a":2}}`. Run `data modify storage t:s b append value 1`. The
reply has `commands.data.modify.expected_list` with `{"1a":2,x:1b}`.

### SNBT-PRINT-004: Text form

The text form is the styled text component that `data get` and the text component `nbt` show.

- **Behavior:**

| Tag | Text form |
|-----|-----------|
| byte, short, long, float, double | number text, then the suffix `b`, `s`, `L`, `f` or `d` |
| int | number text |
| string | the quoted string of SNBT-PRINT-002 |
| byte array | `[B;`, then for each element: a space, the number text and `b`, and `,` if it is not the last element; then `]` |
| int array | as a byte array, with `I` and no suffix |
| long array | as a byte array, with `L` and the suffix `L` |
| empty list | `[]` |
| list | `[`, the elements in their text form separated by `, `, then `]` |
| empty compound | `{}` |
| compound | `{`, the entries separated by `, `, then `}`. An entry is the key, `: ` and the value |

- **Keys:** a key prints as it is when it matches `[A-Za-z0-9._+-]+`. Otherwise it prints as a
  quoted string. The order of the entries is unspecified.
- **Postconditions:** an array element has the suffix `b`, not `B`. An int array element has no
  suffix. The text form of `[B;1B,2B]` is `[B; 1b, 2b]`. The text form of `[B;]` is `[B;]`.
  The text form of `{a:1,b:"x"}` is `{a: 1, b: "x"}` or `{b: "x", a: 1}`.
- **Edge cases:** the keys `1a` and `true` print without quotes in the text form, and with
  quotes in the compact form.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`data merge storage t:s {a:[B;1,2]}`. Run `data get storage t:s a`. The reply shows
`[B; 1b, 2b]`.

### SNBT-PRINT-005: Styles of the text form

| Part | Style |
|------|-------|
| a key, without its quotes | colour `aqua` |
| the content of a string, without its quotes | colour `green` |
| the number text | colour `gold` |
| the suffixes `b`, `s`, `L`, `f`, `d` and the array letters `B`, `I`, `L` | colour `red` |
| the fold mark `<...>` | colour `gray` |
| quotes, `[`, `]`, `{`, `}`, `;`, `:`, `,` and spaces | no style |

The text component `nbt` with `plain` set to true has no style on any part.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: run
`data merge storage t:s {a:1b}`. Run `data get storage t:s`. The key `a` is aqua, `1` is gold
and `b` is red.

### SNBT-PRINT-006: Folds of the text form

- **Behavior:**
  - The top tag has the level 0. Each element of a list and each value of a compound has the
    level of its container plus 1.
  - A list or a compound that is not empty and has a level of 64 or more prints as `[<...>]` or
    `{<...>}`.
  - An array prints at most its first 128 elements. When it has more, `<...>` follows the `,`
    after element 128, and then `]`.
- **Postconditions:** a list prints all its elements. Only the level folds a list.
- **Edge cases:** an int array of 200 elements prints as `[I; 0, 1, ..., 127,<...>]`, with no
  space before `<...>`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: make an int
array of 200 elements in storage with a function. Run `data get storage t:s a`. The reply ends
with `127,<...>]`.

---

## 8. Command arguments and other readers

### SNBT-ARG-001: `minecraft:nbt_tag`

- **Behavior:** the argument reads one tag (SNBT-GR-001). The value of the argument is that
  tag, of any type. The cursor stays after the tag (SNBT-TOK-001).
- **Postconditions:** the argument does not check the text after the tag. The command parser
  then expects a separator or the end (`command-parser.md`, CMD-PARSE-003). Suggestions:
  SNBT-SUG-001.
- **Examples:** the type declares `0`, `0b`, `0l`, `0.0`, `"foo"`, `{foo=bar}` and `[0]`
  (`command-parser.md`, CMD-TREE-006). `{foo=bar}` does not read without an error.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: run
`data modify storage t:s v set value 1e400`. The reply is `command.expected.separator`: the
argument reads `1`, and `e400` is left.

### SNBT-ARG-002: `minecraft:nbt_compound_tag` and the compound read

- **Behavior:** the compound read reads one tag (SNBT-GR-001). If the tag is not a compound,
  the read fails with `argument.nbt.expected.compound`. The position is the cursor after the
  tag.
- **Postconditions:** the compound read does not check the text after the compound. A text that
  starts with `{` gives a compound or an SNBT error. It never gives
  `argument.nbt.expected.compound`. The argument type makes no suggestions.
- **Examples:** the type declares `{}` and `{foo=bar}`. `{foo=bar}` does not read without an
  error.
- **Edge cases:** `5` fails with `argument.nbt.expected.compound` after `5`. `"{}"` fails after
  the second `"`.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: run
`data merge storage t:s 5`. The reply has `argument.nbt.expected.compound`, and its context text
ends after `5`.

### SNBT-ARG-003: Other command readers of a compound

These readers use the compound read of SNBT-ARG-002:

- the data compound of `minecraft:block_predicate` and `minecraft:block_state`
  (`argument-block.md`, ARG-BLK-007);
- the selector option `nbt` of `minecraft:entity` (`argument-entity.md`, section 3);
- the compound filters of `minecraft:nbt_path`. No specification of that type exists yet.

The values of the argument types `minecraft:component`, `minecraft:style`,
`minecraft:particle`, `minecraft:item_stack` and `minecraft:item_predicate` also contain SNBT.
Their specifications state where the SNBT starts and what they do with the tag.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: run
`execute if entity @e[nbt=5]`. The reply has `argument.nbt.expected.compound`.

### SNBT-ARG-004: Full-text read

- **Behavior:** a full-text read reads one tag from a text that holds only SNBT. After the tag,
  it skips whitespace. If a code unit is left, the read fails with `argument.nbt.trailing` at
  that code unit.
- **Postconditions:** a data pack field that holds SNBT as a string uses the full-text read, for
  example the text form of `minecraft:custom_data`. A field that needs a compound also rejects
  a tag of another type.

Provenance: `source_suggested_black_box_confirmed` (the fixture cases `{a:1}  ` and `{a:1} x`).
Kind: semantic. Confidence: high. Experiment: give an item the component `minecraft:custom_data`
with the string value `"{a:1} x"` in a data pack. The load of the pack reports "Unexpected
trailing data".

---

## 9. Open questions

- **A deep tag in the reference.** The reference read has no depth limit (SNBT-LIM-001). Its
  behaviour for a deep text is not known: an error, a disconnection or a crash. The vanilla
  client reads the command line too, so the client can fail first. Experiment: put the line
  `data merge storage t:s {a:` and 20000 `[` into a function, and run the function. Read the
  server log and the reply.
- **Unicode version of `\N`.** SNBT-VAL-007 depends on the Unicode version of the Java runtime
  that runs the game. The reference also accepts a name of the form "block name, space, hex
  code point" for a code point without a Name property. Experiment: run
  `data merge storage t:s {a:"\N{BASIC LATIN 1F}"}` and a name that is new in Unicode 16.0.
  Read the replies.
- **The order of the records.** SNBT-ERR-004 follows from the grammar. No case of it has been
  run in the game. Experiment: run each case of the table in the vanilla client and on a
  vanilla server. Compare the key and the context text.
- **The key `snbt.parser.underscore_not_allowed`.** A later snapshot can correct the key or the
  language file. Experiment: in each new snapshot, compare the key in the reference with the
  language file.

---

## 10. Intentional deviations

### SNBT-DEV-001: The depth of a tag is limited

- **Rule:** SNBT-LIM-001.
- **Reference:** the read has no depth limit. Each level of depth uses stack space of the
  thread that reads the command line.
- **Visible to the client:** only for a text with more than 512 levels. The client can accept
  such a text that the server rejects.
- **Test:** do not compare a text with more than 512 levels.

### SNBT-DEV-002: `uuid` accepts only the canonical form

- **Rule:** SNBT-VAL-009.
- **Reference:** the read accepts a text of at most 36 code units with exactly four `-`. Each
  group can have any number of hex digits, a sign at its start, and digits of other scripts.
  The value of each group is cut to the bits of its field.
- **The canonical form:** 8, 4, 4, 4 and 12 hex digits `0`–`9`, `a`–`f`, `A`–`F`, separated by
  `-`. 36 code units in total.
- **Visible to the client:** yes, but only for unusual input. The client accepts
  `uuid("0-0-0-0-0")`. The server rejects it with `snbt.parser.expected_string_uuid`.
- **Test:** use only the canonical form in a differential test of `uuid`.

### SNBT-DEV-003: A surrogate without its pair is an error

- **Rule:** SNBT-VAL-006.
- **Reference:** an escape can give one surrogate code unit, U+D800 to U+DFFF. The string then
  holds an unpaired surrogate. A high surrogate escape and a low surrogate escape directly
  after it give one character.
- **The rule of the server:** a high surrogate from an escape must be followed directly by a
  low surrogate, from an escape or from the text. A low surrogate from an escape must come
  directly after a high surrogate. Otherwise the escape fails with
  `snbt.parser.invalid_codepoint`, with the text of the surrogate code point, for example
  `U+0000D800`. The position is after the escape that gives the unpaired surrogate.
- **Visible to the client:** yes, but only for unusual input. The client accepts `"\uD800"`.
  The server rejects it.
- **Test:** do not compare a text that gives an unpaired surrogate.

---

## 11. Differences of the current implementation

`crates/mcrs_minecraft_snbt` differs from this document in these points:

- **The text form.** It has no text form (SNBT-PRINT-004 to SNBT-PRINT-006).
- **Suggestions.** It makes no suggestions (SNBT-SUG-001). The read keeps only the reported
  error, not all records at the furthest position.
- **Operation arguments.** The depth of an argument of `bool` or `uuid` is the depth of the
  operation plus 1. An argument list that opens the level 513 fails with
  `arguments.nbtpath.too_deep` at its `(`. SNBT-LIM-001 states only compounds and lists.
