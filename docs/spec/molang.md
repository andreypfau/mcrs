# Molang: behavioural specification

| Field | Value |
|-------|-------|
| Language | Molang of Bedrock Edition, rules of `min_engine_version` 1.19.60 and later |
| Reference pack | `Mojang/bedrock-samples`, resource pack `1.26.60.5` |

This document states the syntax and the meaning of Molang, the expression language of Bedrock
Edition resource packs. The client uses Molang in entity geometry, animations, animation
controllers, render controllers and client entity files. The block definitions under
`assets/mcrs/block_definition/` use Molang in their permutation conditions. One implementation
serves all of these consumers. `molang-queries.md` states the queries that the host supplies.
The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Expression**: the Molang text of one JSON value, or the joined text of one script array
  (MOL-LOAD-002).
- **Simple expression**: an expression that is one operand with no top-level `;`
  (MOL-STMT-001).
- **Complex expression**: an expression that is a sequence of statements (MOL-STMT-001).
- **Consumer**: the part of the client that compiles an expression and runs it, for example
  the animation system or the block definition loader. Each consumer has a specification.
- **Run**: one evaluation of one expression from its start to its end.
- **Subject**: the entity or the block state for which a run occurs.
- **Host**: the part of the client that supplies query values for a subject
  (`molang-queries.md`).
- **Load**: the time when the client compiles the expressions of a resource set. A reload of
  resources is a new load.
- **Load error**: a defect that the compiler finds at load. The expression does not run.
- **Load warning**: a defect that the compiler reports at load. The expression runs.
- **Missing**: the state of a value that has no assignment, or that comes from an error. A
  missing value reads as `0.0`. `??` (MOL-EVAL-005) can detect it.
- **Truthy**: a number that is not `0.0`. NaN is truthy.
- **Milestone**: the implementation milestone of a rule, as in the other specifications. `1`
  is the pig (`mob-pig.md`) and the block definitions. `2` is the frog (`mob-frog.md`).
  `later` is a milestone that no specification uses yet. Before the milestone of a construct
  or of a function, the compiler must parse it and must reject the expression with the load
  error `unsupported` (MOL-LOAD-004).
- **Lane**: one subject in a batch evaluation. Many lanes run the same expression together.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". `documentation` refers to the public Bedrock
documentation: bedrock.dev (`/docs/stable/Molang`), Microsoft Learn (Molang reference and
syntax guide) and wiki.bedrock.dev. Some rules come from in-game tests that the authors of the
`mochafloats` project published. These rules have the label `documentation` and the note
"community test".

Parity of this document is with Bedrock Edition, not with the Java client. Molang has no
counterpart in the Java client.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `MOL-LOAD` | sources of expressions, versions, load errors and load warnings |
| `MOL-LEX` | tokens |
| `MOL-SYN` | grammar and operator precedence |
| `MOL-VAL` | value types and conversions |
| `MOL-EVAL` | evaluation of operators |
| `MOL-STMT` | statements, blocks, `return`, loops |
| `MOL-VAR` | namespaces, variables, structs, entity references |
| `MOL-RES` | resource references and arrays |
| `MOL-NUM` | number format, non-finite values, optimisation limits |
| `MOL-MATH` | the functions of `math.` |
| `MOL-RAND` | random numbers |
| `MOL-DEV` | intentional deviations |

### 0.4 Scope

In scope: the syntax, the value types, the operators, the statements, the namespaces, the
lifetime of variables, the `math.` functions, random numbers and the limits on optimisation.

Out of scope: the meaning of each query (`molang-queries.md`), the moment at which a consumer
runs an expression, and the meaning of `this` in each consumer. The consumer specifications
state these.

---

## 1. Sources and loading

### MOL-LOAD-001: Where an expression comes from

- **Inputs:** a JSON value in a field that the consumer specification marks as Molang.
- **Behavior:**
  1. A JSON number is a constant expression with that value, converted to f32 (MOL-NUM-001).
  2. A JSON string is the text of an expression.
  3. A JSON `true` or `false` is the constant `1.0` or `0.0`.
  4. A JSON object with the member `expression` is a versioned value (MOL-LOAD-006).
  5. Each other JSON type is a load error `type`. A JSON object without the member
     `expression` is also a load error `type`.
  6. An empty string, or a string that contains only whitespace, is the constant `0.0`.
- **Postconditions:** each Molang field gives one expression.

Provenance: `documentation`; item 4: `documentation` (particle reference, "Legacy Molang
Expression"). Kind: semantic. Confidence: medium. Milestone: 1.

### MOL-LOAD-002: Script arrays are one expression

- **Inputs:** a JSON array of strings in a field that takes a script array, for example
  `scripts.initialize` and `scripts.pre_animation` of a client entity file.
- **Behavior:**
  1. Join the entries in array order, with no text between them.
  2. Compile the joined text as one expression.
  3. The joined expression has one set of temporary variables (MOL-VAR-004).
- **Postconditions:** a block can open in one entry and close in a later entry. A temporary
  variable that one entry writes is visible to the entries after it.
- **Example:** the vanilla zombie `pre_animation` opens `(v.melee_spear_equipped) ? {` in one
  entry and closes `};` in a later entry.

Provenance: `documentation` (bedrock-samples, `entity/zombie.entity.json`, `entity/pig.entity.json`).
Kind: semantic. Confidence: medium. Milestone: 1. Experiment: in Bedrock, put
`"t.a = 1;"` and `"v.b = t.a;"` in two entries of `pre_animation`. Show `v.b` through a
render-controller colour. The colour shows `v.b = 1`.

### MOL-LOAD-003: One set of language rules

- **Inputs:** the `min_engine_version` of the pack that contains the expression.
- **Behavior:**
  1. The compiler applies the rules of this document to each expression, for each value of
     `min_engine_version`.
  2. When `min_engine_version` is less than `1.19.60`, the loader reports one load warning
     `old_engine_version` for the pack.
  3. When the pack has no `min_engine_version`, the loader applies these rules and reports no
     warning.
- **Postconditions:** the result of an expression does not depend on the version of its pack.

Provenance: `documentation` (versioned changes table); one set of rules for each version:
`design` (MOL-DEV-001). Kind: semantic. Confidence: high. Milestone: 1.

### MOL-LOAD-004: Load errors

- **Inputs:** an expression.
- **Behavior:** the compiler reports a load error for each of these defects. The error has a
  kind, a byte range in the expression text and a message.

| Kind | Defect |
|------|--------|
| `syntax` | the text does not match the grammar (MOL-SYN-001) |
| `type` | the JSON value has a type that MOL-LOAD-001 or MOL-LOAD-006 rejects |
| `unknown_function` | a call of a `math.` name that MOL-MATH-001 does not list |
| `unknown_query` | a query name that the host does not declare, in a context of use where `molang-queries.md`, MOLQ-HOST-002, makes it an error |
| `unknown_resource` | a resource short name that the client entity definition does not have (MOL-RES-001) |
| `arity` | a call of a function or a query with a number of arguments that its definition does not allow |
| `read_only` | an assignment to `query.`, `context.`, `math.`, a resource or `this` |
| `reserved` | an identifier with no namespace that is not a keyword (MOL-LEX-003) |
| `temp_struct` | a path of more than one name after `temp.` (MOL-VAR-004) |
| `misplaced` | `break` or `continue` outside a loop, or a resource name outside a resource context |
| `static_type` | an operator with an operand of a type that MOL-VAL-002 rejects, when the compiler knows the type at load; or a literal argument of a query that the declaration of the query does not accept |
| `unsupported` | a construct or a function of a later milestone (section 0.1) |

- **Postconditions:** the consumer specification states what replaces an expression with a
  load error. When it states nothing, the expression is the constant `0.0`, and the loader
  logs the error with the file and the JSON path.

Provenance: `documentation` (content errors of 1.17.40). Kind: semantic. Confidence: medium.
Milestone: 1.

### MOL-LOAD-005: Load warnings

- **Inputs:** an expression.
- **Behavior:** the compiler reports a load warning for each of these cases:
  1. A query name that the host of the consumer does not declare (`molang-queries.md`,
     MOLQ-HOST-002). The query reads as missing. In the context of use `block`, the name is a
     load error `unknown_query` instead.
  2. A pack with an old `min_engine_version` (MOL-LOAD-003).
  3. A versioned value with an old `version` (MOL-LOAD-006).
- **Postconditions:**
  1. The expression runs.
  2. The loader reports a warning of item 1 one time for each query name and each consumer in
     one load, not one time for each reference. The warning names the query and one file that
     uses it.
  3. The loader reports a warning of item 3 one time for each pack.

Provenance: `documentation`; postconditions 2 and 3: `design`, so that a pack with many
references to one unsupported query gives one line in the log. Kind: semantic. Confidence:
medium. Milestone: 1.

### MOL-LOAD-006: Versioned values

- **Inputs:** a JSON object in a Molang field, with the members `expression` and `version`.
- **Behavior:**
  1. The member `expression` is a JSON number, a JSON string or a JSON Boolean. MOL-LOAD-001,
     items 1 to 3 and 6, apply to it. Another type is a load error `type`.
  2. The member `version` is an integer. When it is absent, the value is `0`. A value that is
     not an integer is a load error `type`.
  3. The compiler applies the rules of this document to the expression, for each value of
     `version` (MOL-LOAD-003).
  4. When `version` is less than `7`, the loader reports the load warning
     `old_molang_version` (MOL-LOAD-005).
  5. The compiler ignores each other member of the object.
- **Postconditions:** the result of a versioned value does not depend on `version`.
- **Example:** `{"expression": "v.a * 2", "version": 12}` is the expression `v.a * 2`.

Provenance: `documentation` for the form (particle reference, "Legacy Molang Expression");
items 3 and 4: `design`, from the decision of one set of language rules (MOL-DEV-001). The
documentation does not state the meaning of the number. Item 4 counts the rows of the table
of versioned changes from `1`, and `7` is the row of `1.19.60` (section 13). Kind: semantic.
Confidence: low for item 4; medium for the other items. Milestone: 1.

---

## 2. Tokens

### MOL-LEX-001: Characters and whitespace

- **Inputs:** the expression text, UTF-8.
- **Behavior:**
  1. Outside a string literal, space, tab, carriage return and line feed separate tokens.
     They have no other meaning there. Inside a string literal, they are characters of the
     string: `'a b'` and `'ab'` are not equal.
  2. Outside a string, the text must contain only ASCII characters. Another character is a
     load error `syntax`.
  3. Outside a string, letters have no case. `Query.Is_Baby` and `query.is_baby` are the same
     token sequence. Inside a string literal, letters keep their case (MOL-LEX-002, item 4).
  4. Molang has no comments.
- **Postconditions:** the compiler folds letters outside strings to lower case before it
  compares names.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### MOL-LEX-002: Literals

- **Inputs:** the expression text.
- **Behavior:**
  1. A number literal has this form: `digits [ "." digits ] [ "f" ]`, or
     `"." digits [ "f" ]`. `digits` is one or more of `0`–`9`.
  2. The suffix `f` has no meaning. `0.2f` and `0.2` are equal.
  3. A number literal has no sign and no exponent. A minus sign is the unary operator.
  4. A string literal starts and ends with `'`. It contains each character except `'`. It has
     no escape sequence. Its case does not change.
  5. `true` is the number `1.0`. `false` is the number `0.0`.
- **Example:** `1`, `1.25`, `.5`, `0.2f`, `'minecraft:pig'`, `''`.

Provenance: `documentation`; the `f` suffix: bedrock-samples. The form `"." digits`: an mcrs
choice. Kind: semantic. Confidence: medium. Milestone: 1.

### MOL-LEX-003: Names and keywords

- **Inputs:** the expression text.
- **Behavior:**
  1. A name has the form `[a-z_][a-z0-9_]*` after the fold to lower case.
  2. These names are keywords: `return`, `loop`, `for_each`, `break`, `continue`, `this`,
     `true`, `false`.
  3. These names are namespaces, with their short forms:

| Namespace | Short form |
|-----------|------------|
| `query` | `q` |
| `variable` | `v` |
| `temp` | `t` |
| `context` | `c` |
| `math` | none |
| `geometry` | none |
| `material` | none |
| `texture` | none |
| `array` | none |

  4. A short form and its namespace are the same. `q.is_baby` and `query.is_baby` refer to the
     same query.
  5. A name that is not a keyword and that a `.` and a namespace do not follow is a load
     error `reserved`.
- **Postconditions:** each reference starts with a namespace, or is a keyword.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### MOL-LEX-004: Operator tokens

The tokens are `!`, `-`, `+`, `*`, `/`, `<`, `<=`, `>`, `>=`, `==`, `!=`, `&&`, `||`, `?`,
`:`, `??`, `=`, `->`, `(`, `)`, `{`, `}`, `[`, `]`, `,`, `;` and `.`. The lexer takes the
longest token at each position. Thus `??` is one token, and `<=` is one token.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

---

## 3. Grammar

### MOL-SYN-001: The grammar

- **Inputs:** the token sequence of an expression.
- **Behavior:** the expression must match `expression` in this grammar. A mismatch is a load
  error `syntax`.

```ebnf
expression   = complex | simple ;
simple       = operand ;
complex      = { statement ";" } [ statement ] ;
               (* at least one top-level ";", or one statement that is not an operand *)
statement    = "return" operand
             | "break" | "continue"
             | lvalue "=" operand
             | "loop" "(" operand "," block ")"
             | "for_each" "(" variable_ref "," operand "," block ")"
             | block
             | operand
             | (* empty *) ;
block        = "{" { statement ";" } [ statement ] "}" ;

operand      = coalesce ;
coalesce     = conditional [ "??" ( block | coalesce ) ] ;
conditional  = or [ "?" branch [ ":" branch ] ] ;
branch       = block | assign_group | conditional ;
assign_group = "(" lvalue "=" operand ")" ;
or           = and { "||" and } ;
and          = equality { "&&" equality } ;
equality     = relation { ( "==" | "!=" ) relation } ;
relation     = additive { ( "<" | "<=" | ">" | ">=" ) additive } ;
additive     = term { ( "+" | "-" ) term } ;
term         = unary { ( "*" | "/" ) unary } ;
unary        = ( "!" | "-" ) unary | postfix ;
postfix      = primary { "[" operand "]" | "->" reference } ;
primary      = number | string | "true" | "false" | "this"
             | "(" operand ")"
             | reference ;
reference    = query_ref | variable_ref | context_ref | math_ref | resource_ref | array_ref ;
query_ref    = ( "query" | "q" ) "." name [ "(" [ operand { "," operand } ] ")" ]
               { "." name } ;
math_ref     = "math" "." name [ "(" [ operand { "," operand } ] ")" ] ;
variable_ref = ( "variable" | "v" | "temp" | "t" ) "." name { "." name } ;
context_ref  = ( "context" | "c" ) "." name { "." name } ;
resource_ref = ( "geometry" | "material" | "texture" ) "." name ;
array_ref    = "array" "." name ;
lvalue       = variable_ref | postfix "->" variable_ref ;
```

- **Postconditions:**
  1. In `conditional`, the `":"` part is optional. `c ? a` is the binary conditional
     (MOL-EVAL-004).
  2. `conditional` and `coalesce` associate to the right: `a ? b : c ? d : e` is
     `a ? b : (c ? d : e)`.
  3. An argument list must not end with `,`.
  4. A top-level `;` makes the expression complex. The last statement can have no `;`. An
     expression that is one assignment, one `return` or one loop with no `;` is also complex:
     `v.x = 1` stores `1` and gives `0.0`, and `return 2` gives `2.0`.
  5. Empty statements are allowed: `;;` is valid.
  6. A `conditional` can be the first branch of a conditional:
     `a ? b ? c : d : e` is `a ? (b ? c : d) : e`.
  7. `assign_group` is valid only as a branch of `?`. After `(`, the parser takes an
     `assign_group` when a `variable_ref`, or a `postfix` with `->` and a `variable_ref`,
     comes next and the token after it is `=`. Otherwise the branch is a `conditional` with a
     parenthesised operand. Elsewhere, `(v.a = 1)` is a load error `syntax`.
  8. The names after the argument list, or after the name, of a `query_ref` are a member path
     of the result of the query (MOL-VAL-001). `q.spellcolor.r` reads the member `r`.
  9. A `block` is valid on the right side of `??`: `v.d ?? { v.d.x = 0; };` is one
     statement (MOL-EVAL-005).

Provenance: `documentation`; empty statements and trailing commas: community test; items 7
to 9: bedrock-samples, the particle files use these forms (`docs/molang-corpus.md`,
section 10). Kind: semantic. Confidence: medium. Milestone: 1, except `loop`, `for_each`,
`break`, `continue` and `->`, which are `later`. A member path of a query result is not a
load error `unsupported` in milestone 1. No query of milestones 1 and 2 has a `struct`
result, so the path reads as missing (`molang-queries.md`, MOLQ-KNOWN-001).

### MOL-SYN-002: Operator precedence

The grammar of MOL-SYN-001 gives this precedence, from the highest to the lowest:

| Level | Operators | Associativity |
|-------|-----------|---------------|
| 1 | call `f(…)`, index `[…]`, `->` | left |
| 2 | unary `!`, unary `-` | right |
| 3 | `*`, `/` | left |
| 4 | `+`, `-` | left |
| 5 | `<`, `<=`, `>`, `>=` | left |
| 6 | `==`, `!=` | left |
| 7 | `&&` | left |
| 8 | `\|\|` | left |
| 9 | `? :`, binary `?` | right |
| 10 | `??` | right |
| 11 | `=` (statement level only) | right |

- **Postconditions:**
  1. `a && b || c` is `(a && b) || c`.
  2. `a < b == c > d` is `(a < b) == (c > d)`.
  3. `v.x = 1 ? 5 : 6` stores `5` in `v.x`.

Provenance: `documentation` (rules of 1.18.10 and 1.18.20); item 3: community test. Kind:
semantic. Confidence: high. Milestone: 1.

---

## 4. Values

### MOL-VAL-001: Value types

A value has one of these types:

| Type | Meaning |
|------|---------|
| number | an IEEE 754 binary32 value (MOL-NUM-001) |
| string | a sequence of characters, with case |
| struct | a set of named members, each a value; a `variable.` path and the result of a query hold a struct |
| entity reference | a reference to an entity, or an invalid reference |
| resource | a reference to a geometry, a material or a texture of a render controller |

Molang has no Boolean type. A comparison and a logical operator give `1.0` or `0.0`. A test
of a condition uses truthy numbers.

- **Postconditions:**
  1. The declaration of a query states when its result is a struct, and its members
     (`molang-queries.md`, MOLQ-HOST-001).
  2. A read of a member that the struct does not have is missing.
  3. A member path on a query result that is not a struct is missing. When the declaration
     of the query is known at load and its result is not a struct, the path is a load error
     `static_type`.
  4. A query result that is a struct can be stored in a variable. `v.o = q.f('x');` copies
     each member of the result into `v.o` (MOL-STMT-002, item 3).

Provenance: `documentation`; the struct results of queries: documentation of
`query.bone_origin`, `query.bone_orientation_trs` and `query.spellcolor`; postconditions 2 and
3: `design`. Kind: semantic. Confidence: high for the types; medium for the postconditions.
Milestone: 1, except entity reference and struct, which are `later`.

### MOL-VAL-002: Operators and types

- **Inputs:** an operator and the types of its operands.
- **Behavior:**
  1. The arithmetic operators, the relational operators, unary `-` and the `math.` functions
     take numbers only.
  2. `==` and `!=` take two numbers or two strings. Two strings are equal when they contain
     the same characters in the same order. The comparison has case.
  3. `==` between a string and a number gives `0.0`. `!=` between a string and a number gives
     `1.0`.
  4. `!`, `&&`, `||` and the condition of `?` take numbers.
  5. A resource is valid only as a result of an operand, as a branch of `?`, and as the
     element of an array. Each other use is a load error `static_type`.
  6. Each other combination is an error at run time (MOL-EVAL-006). When the compiler knows
     the types at load, the combination is a load error `static_type`.
  7. These forms are also a load error `static_type`, because one operand cannot have two
     types:
     - the two branches of `?` have different types, for example `c ? 'a' : 1`;
     - the binary form `c ? a` where `a` is a string, because the other branch is `0.0`;
     - a variable that the expressions of one variable layout assign both a string and a
       number;
     - a `return` of a string in a complex expression, because a complex expression with no
       `return` gives `0.0` (MOL-STMT-001, item 4).
- **Postconditions:** `'a' == 'A'` is `0.0`. `'text' + 1` is a load error. A simple expression
  can give a string: `c ? 'a' : 'b'` is valid.

Provenance: `documentation`; item 3: `design`; item 7: `design`, so that each operand has one
type at load and the compiler needs no type tag at run time. Kind: semantic. Confidence:
medium. Milestone: 1.

---

## 5. Evaluation

### MOL-EVAL-001: Order of evaluation

- **Inputs:** an operand.
- **Behavior:**
  1. The operands of a binary operator are evaluated from left to right.
  2. The arguments of a `math.` call are evaluated from left to right, before the call.
  3. The arguments of a query are evaluated as the query definition states
     (`molang-queries.md`, MOLQ-HOST-003).
  4. Statements run in source order.
- **Postconditions:** each side effect occurs in source order. A side effect is an assignment
  or a draw of a random number.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Milestone: 1.

### MOL-EVAL-002: Arithmetic, relational and unary operators

| Operator | Result |
|----------|--------|
| `a + b`, `a - b`, `a * b` | the IEEE 754 binary32 result |
| `a / b` | `0.0` when `b` is `0.0` or `-0.0`; otherwise the IEEE 754 binary32 quotient |
| `-a` | `a` with the sign bit inverted |
| `a < b`, `a <= b`, `a > b`, `a >= b` | `1.0` when the IEEE 754 comparison is true, otherwise `0.0` |
| `a == b` | `1.0` when the IEEE 754 comparison is true, otherwise `0.0` |
| `a != b` | `0.0` when `a == b` gives `1.0`, otherwise `1.0` |
| `!a` | `1.0` when `a` is `0.0` or `-0.0`, otherwise `0.0` |

- **Postconditions:**
  1. `1 / 0` is `0.0`. `0 / 0` is `0.0`.
  2. `5 / -1` is `-5.0`.
  3. A comparison with NaN gives `0.0`, except `!=`, which gives `1.0`.
  4. `!` of NaN is `0.0`.

Provenance: `documentation`; division by zero: community test; the sign of division: rule of
1.19.60. Kind: semantic. Confidence: high. Milestone: 1.

### MOL-EVAL-003: Logical operators

- **Inputs:** `a && b` or `a || b`.
- **Behavior:**
  1. `a && b`: evaluate `a`. When `a` is not truthy, the result is `0.0`, and `b` is not
     evaluated. Otherwise the result is `1.0` when `b` is truthy, and `0.0` when it is not.
  2. `a || b`: evaluate `a`. When `a` is truthy, the result is `1.0`, and `b` is not
     evaluated. Otherwise the result is `1.0` when `b` is truthy, and `0.0` when it is not.
- **Postconditions:** the result is always `1.0` or `0.0`. An operand that is not evaluated
  has no side effect.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### MOL-EVAL-004: Conditional operators

- **Inputs:** `c ? a : b` or `c ? a`.
- **Behavior:**
  1. Evaluate `c`.
  2. When `c` is truthy, evaluate `a`. The result is the value of `a`.
  3. When `c` is not truthy, evaluate `b`. The result is the value of `b`. In the binary form
     `c ? a`, the result is `0.0`.
  4. When a branch is a block, the block runs (MOL-STMT-003). The value of a block is `0.0`.
  5. When a branch is an `assign_group`, the assignment runs (MOL-STMT-002). The value of the
     branch is `0.0`.
- **Postconditions:** only one branch is evaluated. A branch that is not evaluated has no
  side effect.
- **Example:** `(v.y > 20) ? { v.y = 0; };` sets `v.y` to `0` only when `v.y > 20`.
  `v.c ? (v.a = 3);` sets `v.a` to `3` only when `v.c` is truthy.

Provenance: `documentation`; item 5: the form comes from bedrock-samples, the value `0.0` is
`design`, equal to the value of a block. Kind: semantic. Confidence: high for items 1 to 4;
medium for item 5. Milestone: 1.

### MOL-EVAL-005: Null coalescing

- **Inputs:** `a ?? b`.
- **Behavior:**
  1. Evaluate `a`.
  2. When the evaluation of `a` reads a missing value, or causes an error (MOL-EVAL-006), the
     result is the value of `b`.
  3. Otherwise the result is the value of `a`, and `b` is not evaluated.
  4. When `b` is a block and item 2 applies, the block runs (MOL-STMT-003). The result is
     `0.0`.
  5. A struct with one or more members is not missing. A struct path with no member is
     missing.
  6. A query call that gives no value for the subject is missing (MOL-EVAL-006, item 2), also
     when the call has arguments.
  7. An inner `??` consumes the missing values of its own left operand. A missing value that
     an inner `??` replaces does not select the `b` of an outer `??`.
- **Postconditions:**
  1. A stored `0.0` is not missing. `v.x = 0; v.x ?? 5` gives `0.0`.
  2. A missing value at any depth of `a` selects `b`. `(v.unset + 1) ?? 5` gives `5.0`.
  3. The side effects of `a` before the missing value remain.
  4. `??` does not apply to resources.
  5. `v.d ?? { v.d.x = 0; };` writes the default member only when `v.d` has no member.
  6. `q.unknown_name(1) ?? 5` gives `5.0`.
  7. `((t.x ?? 0) + 1) ?? 5` gives `1.0` when `t.x` is missing. `(t.x + 1) ?? 5` gives `5.0`.

Provenance: `documentation`; item 2: community implementations agree; item 4: the form comes
from bedrock-samples, the value `0.0` is `design`, equal to the value of a block; items 5
and 6: `design`; item 7: `design`, because the value of an inner `??` is defined, and a reader
sees the scope of each `??` in the text (molangjs keeps one flag for the whole run and gives
`5.0`, see `tools/molang-diff/README.md`). Kind: semantic. Confidence: medium. Milestone: 1;
item 5: `later`, with struct variables (MOL-VAR-005). Experiments: in Bedrock, evaluate
`(v.unset + 1) ?? 5` in a render-controller colour; the channel shows `5`. Evaluate
`((v.unset ?? 0) + 1) ?? 5`; the channel shows `1` when item 7 agrees with Bedrock.

### MOL-EVAL-006: Errors at run time

- **Inputs:** a run.
- **Behavior:** each of these events is an error at run time:
  1. a read of a variable that has no assignment;
  2. a query that gives no value for the subject;
  3. an operator with operands of a type that MOL-VAL-002 rejects;
  4. a read of a struct as a number;
  5. an invalid entity reference on the left side of `->` (MOL-VAR-006).
- **Postconditions:** the value of the operand that causes the error is missing and reads as
  `0.0`. The run continues, except for item 5. An error at run time is not logged for each
  run.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Milestone: 1.

---

## 6. Statements

### MOL-STMT-001: Simple and complex expressions

- **Inputs:** an expression.
- **Behavior:**
  1. A simple expression gives the value of its operand.
  2. A complex expression runs its statements in order.
  3. A `return` statement ends the run. The result is the value of its operand.
  4. When the last statement runs and no `return` ran, the result is `0.0`.
  5. A statement that is an operand evaluates the operand and discards its value.
- **Postconditions:** `v.x = 1; v.x * 2` gives `0.0`. `v.x = 1; return v.x * 2;` gives `2.0`.
  `1;` is complex and gives `0.0`. `1` is simple and gives `1.0`.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### MOL-STMT-002: Assignment

- **Inputs:** `lvalue = operand`.
- **Behavior:**
  1. Evaluate the operand.
  2. Store its value in the variable that `lvalue` names (MOL-VAR-002, MOL-VAR-004).
  3. When the value is a struct, store a copy of each member (MOL-VAR-005).
  4. When the value is missing, the variable keeps the state "missing".
  5. The value of the operand is missing only when the operand is a read of a missing value:
     a variable read or a query call, also inside parentheses, or as the result that `?` or
     `??` selects. The result of an operator or of a `math.` function is never missing. A
     missing operand of that operator reads as `0.0` (MOL-EVAL-006).
- **Postconditions:**
  1. A later read of the variable in the same run gives the stored value.
  2. `v.x = v.unset + 1;` stores `1.0`. `v.x = v.unset;` leaves `v.x` missing.
  3. `v.t = v.t + q.delta_time;` counts up from the first run.
  4. Item 5 applies only to assignment. For `??`, a missing value at any depth of the left
     operand counts (MOL-EVAL-005, item 2).

Provenance: `documentation`; items 4 and 5: `design`; item 5 lets a counter start from a
variable that has no assignment, which is the common form in packs. Kind: semantic. Confidence: medium.
Milestone: 1; an `lvalue` with `->` is `later`.

### MOL-STMT-003: Blocks

- **Inputs:** `{ statement; … }`.
- **Behavior:**
  1. The statements of the block run in order.
  2. A `return` in a block ends the whole run (MOL-STMT-001).
  3. A block does not start a new scope. A temporary variable keeps its value after the block
     ends.
- **Postconditions:** the value of a block is `0.0`.

Provenance: `documentation`; item 3: documentation states "C rules", community tests show
one scope for the whole run. Kind: semantic. Confidence: medium. Milestone: 1.

### MOL-STMT-004: `loop`

- **Inputs:** `loop(n, block)`.
- **Behavior:**
  1. Evaluate `n` one time.
  2. The count is `trunc(n)`, limited to the range 0 to 1024. A NaN count is 0.
  3. Run the block the count number of times.
  4. `break` ends the innermost loop. `continue` starts the next pass of the innermost loop.
- **Postconditions:** a loop never runs its block more than 1024 times. Loops can be nested.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Milestone: later.

### MOL-STMT-005: `for_each`

- **Inputs:** `for_each(variable, operand, block)`.
- **Behavior:**
  1. Evaluate the operand one time. The result must be an array of entity references.
  2. For each element, in order, store the element in the variable, and run the block.
  3. When the result is not an array of entity references, the block does not run.
  4. `break` and `continue` act as in MOL-STMT-004.
- **Postconditions:** the variable holds the last element that the loop stored.

Provenance: `documentation`. Kind: semantic. Confidence: low. Milestone: later.

---

## 7. Variables

### MOL-VAR-001: Namespaces

| Namespace | Read | Write | Lifetime |
|-----------|------|-------|----------|
| `query.` | yes | no | supplied by the host for each read (`molang-queries.md`) |
| `variable.` | yes | yes | one store for each subject (MOL-VAR-002) |
| `temp.` | yes | yes | one run (MOL-VAR-004) |
| `context.` | yes | no | supplied by the consumer for each run (MOL-VAR-003) |
| `math.` | yes | no | constant |
| `geometry.`, `material.`, `texture.`, `array.` | yes | no | the resource set of a render controller (MOL-RES-001) |

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### MOL-VAR-002: Entity variables

- **Inputs:** a reference `variable.p`, where `p` is a path of one or more names.
- **Behavior:**
  1. Each subject has its own store of entity variables.
  2. The store starts empty when the client creates the subject. Each variable is missing at
     that time.
  3. A value that a run stores remains for the next runs of all expressions on the same
     subject.
  4. The store ends when the client removes the subject. The client does not save the store.
  5. Names compare without case: `v.Foo` and `v.foo` are the same variable.
- **Postconditions:** a run of one expression can read a variable that a run of another
  expression wrote on the same subject.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### MOL-VAR-003: Context values

- **Inputs:** a reference `context.p`.
- **Behavior:**
  1. The consumer supplies the context values of a run. The consumer specification lists
     them.
  2. A context name that the consumer does not supply is missing.
- **Postconditions:** a run cannot change a context value.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Milestone: 1.

### MOL-VAR-004: Temporary variables

- **Inputs:** a reference `temp.n`, where `n` is one name.
- **Behavior:**
  1. Each run starts with all temporary variables missing.
  2. A temporary variable keeps its value until the run ends, through all blocks of the run.
  3. A path of more than one name after `temp.` is a load error `temp_struct`.
- **Postconditions:** two runs never share a temporary variable, also when they run on the
  same subject.

Provenance: `documentation`; item 2: community test. Kind: semantic. Confidence: medium.
Milestone: 1.

### MOL-VAR-005: Struct variables

- **Inputs:** a reference `variable.a.b…` with more than one name.
- **Behavior:**
  1. An assignment to `variable.a.b` makes `variable.a` a struct with the member `b`. Paths
     can have each depth.
  2. A read of `variable.a.b` gives the value of the member. When the member has no
     assignment, the value is missing.
  3. `variable.s = variable.t`, where `variable.t` is a struct, copies each member of
     `variable.t` into `variable.s`. A later change to `variable.t` does not change
     `variable.s`.
  4. A read of a struct where the operator needs a number is an error (MOL-EVAL-006).
- **Postconditions:** an implementation can store each path as one variable, because each
  path is known at load.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Milestone: later.

### MOL-VAR-006: Entity references and `->`

- **Inputs:** `e->r`, where `e` gives an entity reference and `r` is a reference.
- **Behavior:**
  1. Evaluate `e`.
  2. When `e` is missing, an invalid reference, or not an entity reference, the run ends.
     The result of the run is `0.0`. `r` is not evaluated.
  3. Otherwise evaluate `r` with the entity of `e` as the subject. A query gives the value
     for that entity.
  4. A read of `variable.` through `->` gives a value only for a variable that the client
     entity definition of that entity marks `public`. Each other variable is missing.
  5. A read of a public variable through `->` gives the value that the variable had at the
     start of the current frame.
  6. A write through `->` changes the variable of that entity at the end of the current
     frame. Writes to the same variable apply in the order of the runs that wrote them.
- **Postconditions:** the result of a read through `->` does not depend on the order in which
  the client evaluates the subjects of one frame.

Provenance: `documentation` (items 1–4); items 5 and 6: `design`, see section 13. Kind:
semantic. Confidence: low. Milestone: later.

### MOL-VAR-007: `this`

- **Inputs:** the keyword `this`.
- **Behavior:**
  1. `this` reads a number that the consumer supplies for the run.
  2. When the consumer supplies no value, `this` is `0.0`.
- **Postconditions:** the consumer specification states the meaning of `this`. For a bone
  channel of an animation, `entity-animation.md` states it. For a colour channel of a render
  controller, `client-entity-definition.md`, CENT-RC-006, states it.

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### MOL-VAR-008: Variables that the engine supplies

- **Inputs:** a read of `variable.p` on a subject.
- **Behavior:**
  1. The consumer specification can declare engine variables. An engine variable is a
     `variable.` name that the host writes for the subject before the scripts of a frame run.
     The declaration states the name, the value and the rate class, as for a query
     (`molang-queries.md`, MOLQ-HOST-001).
  2. A variable that no expression writes and that no declaration supplies stays missing. A
     read gives `0.0`, and `??` selects its right side.
  3. A script can write an engine variable. The written value applies until the host writes
     the variable again.
  4. The compiler reports no load warning for a variable that no expression writes.
- **Postconditions:**
  1. A pack that reads an engine variable of Bedrock, for example `v.attack_time`, loads.
     The variable reads as `0.0` until a consumer specification declares it.
  2. The consumers of milestones 1 and 2 declare no engine variable.

Provenance: `documentation` (variables are missing until a write); items 1, 3 and 4:
`design`. bedrock-samples reads 242 variables that no expression of the pack writes
(`docs/molang-corpus.md`, section 7). Kind: semantic. Confidence: medium. Milestone: 1.

---

## 8. Resources and arrays

### MOL-RES-001: Resource references

- **Inputs:** `geometry.n`, `material.n` or `texture.n` in a render controller.
- **Behavior:**
  1. The client compiles a render controller one time for each pair of a client entity and a
     render controller that the client entity uses. The reference gives the resource with the
     short name `n` of the client entity definition of that pair
     (`client-entity-definition.md`).
  2. A name that the definition does not have is a load error `unknown_resource` of that pair.
     The pair draws nothing for the layer of the expression. The loader logs the error one
     time for each pair.
  3. A resource reference outside a render controller is a load error `misplaced`.
- **Postconditions:** a resource value can only be the result of the expression, a branch of
  `?`, or an array element.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Milestone: 1.

### MOL-RES-002: Arrays

- **Inputs:** `array.n[i]` in a render controller.
- **Behavior:**
  1. The render controller defines the array `n` as a list of resources or numbers.
  2. Evaluate `i`. When `i` is NaN, `+∞` or `-∞`, let `k = 0`. Otherwise let
     `k = max(0, trunc(i))`.
  3. The result is the element at index `k mod len`, where `len` is the length of the array.
  4. An array name that the render controller does not define is a load error `misplaced`.
- **Postconditions:** an index never selects an element outside the array.

Provenance: `documentation`. Kind: semantic. Confidence: medium. Milestone: 1.

---

## 9. Numbers

### MOL-NUM-001: The number format

- **Inputs:** each number value.
- **Behavior:**
  1. A number is an IEEE 754 binary32 value.
  2. Each operation rounds its result to binary32, in source order.
  3. A number literal converts to the nearest binary32 value.
  4. An integer result, for example of `math.floor`, is a binary32 value.
- **Postconditions:** two CPU implementations that obey this document give the same bits for
  `+`, `-`, `*`, `/`, the comparisons and the functions that MOL-MATH-002 marks exact. A GPU
  evaluation can give other bits (MOL-NUM-005).

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### MOL-NUM-002: Non-finite values

- **Inputs:** an operation with a result that is NaN, `+∞` or `-∞`.
- **Behavior:**
  1. Molang keeps the non-finite value. `math.sqrt(-1)` is NaN. `math.ln(0)` is `-∞`.
  2. A consumer that writes a number into a transform, a colour or a weight must replace a
     non-finite value. The consumer specification states the replacement.
- **Postconditions:** a non-finite value from Molang never reaches a vertex position. On a GPU
  this holds through the clamp of MOL-NUM-005, item 7.

Provenance: `documentation`; item 1: community test. Kind: semantic. Confidence: medium.
Milestone: 1.

### MOL-NUM-003: Limits on optimisation

- **Inputs:** a compiled expression.
- **Behavior:**
  1. An optimisation must not change the result bits of an expression, for each input. The
     inputs include NaN, `+∞`, `-∞` and `-0.0`. The payload bits of a NaN are free.
  2. An optimisation must not change the side effects or their order.
  3. An optimisation must not remove or merge a draw of a random number (MOL-RAND-001).
  4. A constant fold is allowed only when the folded result has the same bits as the run.
- **Postconditions:**
  1. `x * 0` is not `0` for each `x`, because `x` can be NaN or `∞`. `x - x` is not `0`.
  2. `x + 0` is not `x`, because `-0.0 + 0.0` is `+0.0`.
  3. `(a + b) + c` is not `a + (b + c)`.
  4. A batch evaluation and a scalar evaluation of the same lane give the same bits.

Provenance: `design`. Kind: semantic. Confidence: high. Milestone: 1.

### MOL-NUM-004: Accuracy of transcendental functions

- **Inputs:** a call of `math.sin`, `math.cos`, `math.asin`, `math.acos`, `math.atan`,
  `math.atan2`, `math.exp`, `math.ln` or `math.pow`.
- **Behavior:**
  1. The result of these functions is not exact. Their bits are unspecified.
  2. Before `math.sin` and `math.cos` convert degrees to radians, they reduce the argument to
     the range `[-180, 180]` with an exact remainder by `360`.
  3. The absolute error of `math.sin` and `math.cos` must be at most `2^-11` for each finite
     argument.
  4. Two runs of the same function on the same machine, with the same inputs, give the same
     bits. This applies to the batch evaluation and the scalar evaluation on the CPU.
- **Postconditions:** a GPU evaluation can give other bits than the CPU evaluation. A parity
  test compares these functions within the error of item 3.

Provenance: `design`; the error bound is that of `sin` and `cos` in WGSL. Kind:
unspecified. Confidence: high. Milestone: 1.

### MOL-NUM-005: Evaluation on a GPU

- **Inputs:** an expression that an implementation evaluates on a GPU.
- **Behavior:**
  1. A division by zero gives `0.0`, as MOL-EVAL-002 states. The GPU code must test the
     divisor explicitly.
  2. The bits of `/` are unspecified. The relative error must be at most 2.5 ULP.
  3. A result that is NaN, `+∞` or `-∞` is unspecified.
  4. The CPU must not write a value that is NaN, `+∞` or `-∞` as an input of a GPU evaluation.
     It writes `0.0` instead. The CPU writes a subnormal input as it is; the GPU can flush it
     to zero.
  5. When the compiler cannot show at load that the argument of `math.sqrt`, `math.ln`,
     `math.pow`, `math.exp`, `math.asin`, `math.acos`, or of an easing (MOL-MATH-003) with the
     shape `circ`, `expo` or `elastic`, stays in the range where the result is finite, the
     expression must not run on a GPU. It runs on the CPU. These easings call the functions of
     the list.
  6. An overflow of `+`, `-` or `*` with finite inputs is not part of the test of item 5.
  7. The pose pass clamps each value that it samples from a channel to `[−1e6, 1e6]`. A NaN
     gives an unspecified finite value in that range.
- **Postconditions:** a GPU evaluation gives a finite result for each finite input, except
  after an overflow. A sampled pose value is always finite. A GPU evaluation and a CPU evaluation agree within the tolerance of the
  consumer.

Provenance: `design`; the error bound of item 2 is that of `/` in WGSL. Kind: unspecified for
the bits; semantic for items 1, 4 and 5. Confidence: medium. Milestone: 1.

---

## 10. Math functions

### MOL-MATH-001: Calls and constants

- **Inputs:** a `math_ref` (MOL-SYN-001).
- **Behavior:**
  1. `math.pi` is a constant. It has no argument list. Its value is π, rounded to binary32.
  2. Each other `math.` name is a function. MOL-MATH-002 lists them, with their arguments.
  3. A function name with no argument list is a load error `arity`.
  4. A name that MOL-MATH-002 does not list is a load error `unknown_function`.
- **Postconditions:** each function is pure, except the random functions (MOL-RAND-001).

Provenance: `documentation`. Kind: semantic. Confidence: high. Milestone: 1.

### MOL-MATH-002: Function definitions

Angles are in degrees. `rad(x) = x · π / 180`, and `deg(x) = x · 180 / π`. "Exact" means that
the result has the bits of the formula of the table: each operation of the formula, in the
order that the parentheses give, rounds to binary32 (MOL-NUM-001). `sqrt` and `mod` are one
operation each, with a correctly rounded result.

| Function | Result | Exact |
|----------|--------|-------|
| `abs(x)` | `|x|` | yes |
| `ceil(x)` | the smallest integer ≥ `x` | yes |
| `floor(x)` | the largest integer ≤ `x` | yes |
| `trunc(x)` | `x` rounded toward zero | yes |
| `round(x)` | `x` rounded to the nearest integer; a half rounds away from zero: `round(-2.5) = -3`, `round(2.5) = 3` | yes |
| `sqrt(x)` | `√x`; NaN when `x < 0` | yes |
| `exp(x)` | `e^x` | no |
| `ln(x)` | the natural logarithm; `-∞` when `x = 0`; NaN when `x < 0` | no |
| `pow(b, e)` | `b^e`, as the C function `powf` | no |
| `sin(x)` | `sin(rad(x))` | no |
| `cos(x)` | `cos(rad(x))` | no |
| `asin(x)` | `deg(asin(x))`; NaN when `|x| > 1` | no |
| `acos(x)` | `deg(acos(x))`; NaN when `|x| > 1` | no |
| `atan(x)` | `deg(atan(x))` | no |
| `atan2(y, x)` | `deg(atan2(y, x))`; note the order of the arguments | no |
| `min(a, b)` | the smaller value; when one operand is NaN, the other operand; when `a == b`, `a` (so `min(-0, 0)` is `-0` and `min(0, -0)` is `0`) | yes |
| `max(a, b)` | the larger value; when one operand is NaN, the other operand; when `a == b`, `a` | yes |
| `clamp(v, lo, hi)` | `min(max(v, lo), hi)` | yes |
| `mod(a, b)` | the C function `fmodf(a, b)`: the exact remainder of `a / b` with the sign of `a`, with no intermediate rounding; `0.0` when `b = 0` | yes |
| `sign(x)` | `1.0` when `x > 0`; `-1.0` otherwise | yes |
| `copy_sign(a, b)` | `|a|` with the sign bit of `b` | yes |
| `lerp(a, b, t)` | `a + ((b − a) · t)`; `t` is not limited | yes |
| `inverse_lerp(a, b, v)` | `(v − a) / (b − a)`; `0.0` when `b = a` (MOL-EVAL-002) | yes |
| `min_angle(x)` | `x − (360 · floor((x + 180) / 360))`; the result is in `[-180, 180)` | yes |
| `lerprotate(a, b, t)` | `min_angle(a + (min_angle(b − a) · t))` | yes |
| `hermite_blend(t)` | `(3 · (t · t)) − (2 · ((t · t) · t))` | yes |
| `random(lo, hi)` | MOL-RAND-002 | — |
| `random_integer(lo, hi)` | MOL-RAND-002 | — |
| `die_roll(n, lo, hi)` | MOL-RAND-002 | — |
| `die_roll_integer(n, lo, hi)` | MOL-RAND-002 | — |
| `ease_<kind>(start, end, t)` | MOL-MATH-003 | no |

Each listed function takes exactly the listed number of arguments. Another number is a load
error `arity`.

Provenance: `documentation`; `round`: community test; `sign(0)`: the documentation text,
section 13; `min_angle` and `lerprotate`: the formula is `design` for the documented
meaning; the `-0`/`+0` result of `min` and `max`: `design`, so that the result has one
defined sign bit. Kind: semantic. Confidence: medium. Milestone: 1 for `cos`, `sin`, `clamp`
and `lerp`; 2 for each other function except the random functions, which are later
(MOL-RAND-002).

### MOL-MATH-003: Easing functions

- **Inputs:** `math.ease_<dir>_<shape>(start, end, t)`. `<dir>` is `in`, `out` or `in_out`.
  `<shape>` is `quad`, `cubic`, `quart`, `quint`, `sine`, `expo`, `circ`, `back`, `elastic`
  or `bounce`.
- **Behavior:**
  1. The result is `lerp(start, end, f(t))`. `t` is not limited.
  2. `f` for `in` is `E(t)` from this table:

| `<shape>` | `E(t)` |
|-----------|--------|
| `quad` | `t²` |
| `cubic` | `t³` |
| `quart` | `t⁴` |
| `quint` | `t⁵` |
| `sine` | `1 − cos(90 · t)` (degrees) |
| `expo` | `0` when `t = 0`; otherwise `2^(10t − 10)` |
| `circ` | `1 − √(1 − t²)` |
| `back` | `2.70158 · t³ − 1.70158 · t²` |
| `elastic` | `0` when `t = 0`; `1` when `t = 1`; otherwise `−2^(10t − 10) · sin((10t − 10.75) · 120)` (degrees) |
| `bounce` | `1 − B(1 − t)`, with `B` below |

  3. `f` for `out` is `1 − E(1 − t)`.
  4. `f` for `in_out` is `E(2t) / 2` when `t < 0.5`, and `1 − E(2 − 2t) / 2` otherwise.
  5. `B(u)`, with `n = 7.5625` and `d = 2.75`:
     - `n · u²` when `u < 1 / d`;
     - `n · (u − 1.5 / d)² + 0.75` when `u < 2 / d`;
     - `n · (u − 2.25 / d)² + 0.9375` when `u < 2.5 / d`;
     - `n · (u − 2.625 / d)² + 0.984375` otherwise.
- **Postconditions:** `f(0) = 0` and `f(1) = 1` for each function.

Provenance: `documentation` (names); the formulas are the common easing formulas, `design`. Kind: semantic. Confidence: low for `back`, `elastic` and `bounce` with `in_out`;
medium for the other functions. Milestone: 2; the functions are pure and the scalar and batch
paths compute them at no extra cost. Experiment: in Bedrock, show
`math.ease_in_out_back(0, 1, 0.25)` through a render-controller colour. Compare the channel
value with the formula.

---

## 11. Random numbers

### MOL-RAND-001: Random streams

- **Inputs:** a call of a random function.
- **Behavior:**
  1. Each subject has its own random stream. The client seeds the stream from the subject.
     For an entity, the seed is the network id of the entity.
  2. Each call takes new values from the stream of the subject of the run.
  3. Two calls are two draws, also when their text is equal.
  4. The algorithm of the stream is unspecified. It must give the same sequence for the same
     seed.
- **Postconditions:**
  1. The draws of one subject do not depend on the order in which the client evaluates
     subjects, nor on the size of a batch.
  2. A random call is never folded and never shared with another call (MOL-NUM-003).
  3. A parity test must not compare the values of random draws.
  4. A random function runs only on the CPU. An expression with a random call never runs on a
     GPU.

Provenance: `documentation` (items 2 and 3); items 1 and 4: `design`. Kind:
unspecified. Confidence: high. Milestone: later.

### MOL-RAND-002: Random functions

| Function | Result |
|----------|--------|
| `random(lo, hi)` | a uniform value between `min(lo, hi)` and `max(lo, hi)`, with both limits included |
| `random_integer(lo, hi)` | let `a = trunc(min(lo, hi))` and `b = trunc(max(lo, hi))`; a uniform integer from `a` to `b`, with both limits included |
| `die_roll(n, lo, hi)` | the sum of `trunc(n)` draws of `random(lo, hi)`; `0.0` when `trunc(n) ≤ 0` |
| `die_roll_integer(n, lo, hi)` | the sum of `trunc(n)` draws of `random_integer(lo, hi)`; `0.0` when `trunc(n) ≤ 0` |

Provenance: `documentation`; equal and reversed limits: community test; the conversion of
`random_integer` limits: `design`. Kind: semantic. Confidence: medium. Milestone: later.

---

## 12. Intentional deviations

### MOL-DEV-001: Old language rules

- **Rule:** MOL-LOAD-003.
- **Reference:** Bedrock applies the rules of the `min_engine_version` of the pack. Before
  1.18.10, `? :` associates to the left. Before 1.18.20, `a && b || c` is `a && (b || c)`,
  and the relational and equality operators have one level. Before 1.19.60, a division by a
  negative divisor that is not a literal uses the absolute value of the divisor.
- **Visible to the client:** yes, for a pack with an old `min_engine_version` and an
  expression that the old rules evaluate differently. An animation or a selection of a
  texture can differ.
- **Test:** a differential test uses the rules of 1.19.60 for each expression. It does not
  compare an expression from a pack with an older version.

---

## 13. Open questions

- `sign(0)`: the documentation text gives `-1.0`. No in-game test is known. Experiment: in
  Bedrock, show `math.sign(0)` through a render-controller colour.
- Script arrays: Bedrock can join entries with a separator. With no separator, two names in
  adjacent entries can become one name. No vanilla file has such a pair.
- Number literals of the form `.5`: the documentation does not state them.
- `->` reads and writes: Bedrock does not document when a read sees the value of another
  entity, nor when a write applies. MOL-VAR-006 states a `design` choice that gives a result
  that does not depend on the order of evaluation.
- Assignment of a missing value (MOL-STMT-002, items 4 and 5): Bedrock can store `0.0`.
- Nested `??` (MOL-EVAL-005, item 7): Bedrock can keep one missing flag for the whole run,
  as molangjs does. The experiment of MOL-EVAL-005 decides.
- One statement with no `;` (MOL-SYN-001, postcondition 4): Bedrock can reject `v.x = 1` as a
  simple expression. mcrs accepts it, so that a pack with this text loads.
- The meaning of `context.` names for each consumer: each consumer specification lists them.
- The `version` member of a versioned value (MOL-LOAD-006): the documentation gives no map
  from the number to a row of the table of versioned changes. The value `12` in the vanilla
  particles is consistent with a count of the rows from `1`. Experiment: none is necessary,
  because mcrs applies one set of rules; only the warning of item 4 depends on the map.
