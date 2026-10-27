# Commands in the client chat screen: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states what the vanilla client does with a command while the player types it in
the chat screen. The client parses the input against the tree that the server sent. It colours
the input, shows suggestions, shows usage and error lines, and asks the server for suggestions
where the tree tells it to. `client-chat.md` states the chat screen, the history and the packets
that send the input. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Command tree**, **node**, **root**, **literal**, **argument**, **child**, **redirect**,
  **requirement**, **source**, **executable**, **context**, **error**, **suggestion**: as
  `command-parser.md`, section 0.1, defines them.
- **Suggestion type**, **restricted**: as `commands-packet.md`, section 0.1, defines them.
- **Chat screen**, **input box**, **input**, **caret**, **GUI unit**, **W**, **H**: as
  `client-chat.md`, section 0.1, defines them.
- **Command input**: an input whose first code unit is `/`.
- **Position**: an offset into the input, counted in code units from the start of the input.
  The `/` is at position 0. Every position in this document includes the `/`.
- **Client tree**: the command tree of the last `commands` packet that the client received.
- **Client source**: the source of the client parse (CCMD-TREE-001).
- **Client parse**: the parse of a command input against the client tree (CCMD-PARSE-001).
- **Suggestion parent**: the node whose children make the suggestions at the caret
  (`command-parser.md`, CMD-SUG-002).
- **Server request**: a `command_suggestion` packet (`commands-packet.md`, CMDP-SUG-001).
- **Open request**: the last server request that has no reply and that the client did not
  cancel.
- **Suggestion list**: the list of suggestions that the chat screen shows above the input box.
- **Selected suggestion**: the one entry of the suggestion list that Tab inserts.
- **Ghost text**: the grey text after the caret that shows how the selected suggestion
  completes the input.
- **Hint line**: one line of text above the input box that is not a suggestion. A hint line is
  an error line, a usage line or a restriction line.
- **Suggestion state**: the suggestions at the caret, the range of each, and the flag "ready".
  The client computes it again after each change of the input.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply, and `library_source` has the meaning of
`command-parser.md`, section 0.2. The kinds of rule are those of `README.md`, section
"Semantics, not accidents".

### 0.3 Colours

A colour is a 32-bit ARGB value in hexadecimal. The named text colours are:

| Name | RGB |
|------|-----|
| gray | `AAAAAA` |
| aqua | `55FFFF` |
| yellow | `FFFF55` |
| green | `55FF55` |
| light purple | `FF55FF` |
| gold | `FFAA00` |
| red | `FF5555` |

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CCMD-TREE` | the client tree and the client source |
| `CCMD-PARSE` | when the client parses, and what the parse decides |
| `CCMD-SUG` | the suggestion state |
| `CCMD-REQ` | server requests and their replies |
| `CCMD-LIST` | the suggestion list on the screen |
| `CCMD-HINT` | error, usage and restriction lines |
| `CCMD-HL` | the colours of the command input |
| `CCMD-DEV` | intentional deviations |

### 0.5 Scope

In scope: the client parse in the chat screen, the suggestion state, server requests, the
suggestion list, hint lines and the colours of the input.

Out of scope: the grammar of each argument type (`command-parser.md` and `argument-*.md`), the
packet layouts of `commands-packet.md`, the narration of the screen, and the command block
editor. The editor uses the same mechanisms with other parameters.

---

## 1. The client tree

### CCMD-TREE-001: Client tree and client source

- **Behavior:**
  1. When a `commands` packet arrives, the client tree becomes the tree of that packet. The
     earlier tree is discarded.
  2. Before the first `commands` packet, the client tree has only a root.
  3. The client source holds the permission set of the player (`command-permissions.md`,
     PERM-SET-001). Its level is the last level of PERM-LEVEL-003.
  4. The client source also holds the atom `minecraft:client/commands/restricted`. Thus every
     restricted node passes its requirement in the client parse.
- **Postconditions:** every node of the client tree is usable by the client source. The
  permission level has an effect only inside argument types, for example on entity
  selectors that start with `@` (`command-permissions.md`, PERM-SET-001).
- **Edge cases:** a change of the level does not change the client tree. The server sends a
  new tree when the level changes (`commands-packet.md`, CMDP-TREE-002).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium. Experiment: as a
player at level `gamemasters`, type `/tell @`. The suggestion list shows the selectors. At level
`all`, type the same text. The list shows no selector, and the `@` is red (CCMD-HL-001).

### CCMD-TREE-002: Argument types

- **Behavior:**
  - The client parses each argument with the grammar of its parser id
    (`commands-packet.md`, CMDP-ARG-001).
  - The client parse has an effect only on the colours, the hint lines and the suggestions.
    The client sends a command input when the client parse fails
    (`client-chat.md`, CHAT-SEND-002).
- **Edge cases:** an implementation can lack the grammar of a parser id. Then the client
  must read the argument as `brigadier:string` in the mode `greedy`, with the suggestion
  type `minecraft:ask_server`. The client must still send the input.

Provenance: `source_suggested`. Kind: unspecified for the edge case; client agreement for the
rest. Confidence: medium. Experiment: on a vanilla server, type `/gamemode x` and press Enter.
The client sends `chat_command` with `gamemode x`, and the server replies with an error.

### CCMD-TREE-003: Redirects in the client parse

- **Behavior:** the client parse follows a redirect as `command-parser.md`, CMD-TREE-004,
  states. After a redirect, the parse continues in a child context at the children of the
  target.
- **Postconditions:** a redirect to the root, for example `execute ... run`, starts a new
  child context. CCMD-HL-001 colours only the arguments of the last child context.

Provenance: `library_source`. Kind: client agreement. Confidence: high.

---

## 2. The client parse

### CCMD-PARSE-001: When the client parses

- **Inputs:** the input and the caret.
- **Behavior:** after each change of the input, and when the chat screen opens:
  1. If the input does not start with `/`, the input is chat text. Go to CCMD-SUG-004.
  2. If the text of the last client parse is not equal to the input, parse the input again.
     The parse starts at position 1 and reads to the end of the input. It uses the client tree
     and the client source (`command-parser.md`, section 4).
  3. Clear the suggestion state and the hint lines. Clear the suggestion list, unless
     CCMD-LIST-005 keeps it.
  4. If the caret is at position 1 or after it, compute the suggestion state (CCMD-SUG-001).
- **Postconditions:** a move of the caret without a change of the input does not start this
  rule. The suggestion state then stays for the caret position of the last change.
- **Edge cases:** the input `/` gives a parse with no parsed node.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: type
`/gamemode `, then move the caret to position 3 with the Left key. The suggestion list does not
change.

### CCMD-PARSE-002: Message arguments

- **Behavior:** the client parse "contains a message argument" when the chain of contexts has a
  parsed value of the argument type `minecraft:message`. CCMD-HINT-001 and `client-chat.md`,
  CHAT-SEND-002 and CHAT-OPT-002, use this flag.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

---

## 3. The suggestion state

### CCMD-SUG-001: Suggestions of a command input

- **Inputs:** the client parse, and the caret `c`.
- **Behavior:**
  1. Find the suggestion parent and the start for `c` (`command-parser.md`, CMD-SUG-002).
  2. The truncated input is the input from position 0 to `c`, with the `/`.
  3. Collect the suggestions of each child of the suggestion parent, in the order of the
     children (`command-parser.md`, CMD-SUG-003 to CMD-SUG-005). A child that needs the
     server uses the reply of a server request (CCMD-REQ-001).
  4. When the suggestions of all children are known, merge them into one list
     (`command-parser.md`, CMD-SUG-006 and CMD-SUG-007).
  5. Mark the suggestion state ready. Then apply CCMD-HINT-001 and CCMD-LIST-001.
- **Postconditions:**
  - Until step 5, the screen shows no suggestion list and no hint line.
  - When the suggestion parent has exactly one child, the list is the list of that child,
    with its own order. A reply of the server then keeps the order of the server.
  - The suggestion types `minecraft:available_sounds`, `minecraft:summonable_entities` and
    `minecraft:post_effects` take their candidates from client data
    (`commands-packet.md`, CMDP-ARG-002).
- **Edge cases:** if a child never gets its reply, the suggestion state never becomes ready.
  The screen then shows no list and no hint line for that input.

Provenance: `library_source`; `source_suggested`. Kind: semantic. Confidence: medium.
Experiment: on a modified server that drops every server request, type `/x ` where `x` has one
child with the suggestion type `minecraft:ask_server`. No list and no usage line show.

### CCMD-SUG-002: Children that need the server

A child needs the server in each of these cases:

1. it is an argument with the suggestion type `minecraft:ask_server`, or with a suggestion type
   that the client does not know;
2. it is an argument that suggests the entries of a registry that the client does not have;
3. it is an argument of type `minecraft:objective`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: as in
`commands-packet.md`, CMDP-SUG-001.

### CCMD-SUG-003: Display order

- **Inputs:** the list of CCMD-SUG-001 or CCMD-SUG-004, the input and the caret `c`.
- **Behavior:**
  1. The last word is the input from the end of the last whitespace run before `c` to `c`. A
     whitespace run is one or more of U+0009 to U+000D and U+0020. With no whitespace before
     `c`, the last word starts at position 0.
  2. Map the last word to lower case with the locale-independent mapping.
  3. The first group holds each suggestion whose text starts with the last word. It also holds
     each suggestion whose text starts with `minecraft:` and then the last word. The
     comparison is exact and case-sensitive on the suggestion text.
  4. The second group holds the other suggestions.
  5. The display order is the first group, then the second group. Each group keeps the order
     of the list.
- **Edge cases:** for the first word of a command input, the last word starts with `/`. No
  literal starts with `/`, so the list keeps its order.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: type
`/give @s log`. The list shows the items that start with `log` first, then items such as
`minecraft:oak_log`.

### CCMD-SUG-004: Suggestions of chat text

- **Inputs:** an input that does not start with `/`, and the caret `c`.
- **Behavior:**
  1. If the input has only whitespace, there is no suggestion state.
  2. Otherwise the candidates are the names of the players in the player list. They are also
     the entries of the custom completion set.
  3. Filter the candidates with `command-parser.md`, CMD-SUG-008, against the last word
     before `c` (CCMD-SUG-003, step 1). Each candidate is a text.
  4. The range of each suggestion is the last word. The order is that of `command-parser.md`,
     CMD-SUG-006, step 5.
- **Postconditions:** chat text has no client parse, no usage line and no error line. The
  postconditions of CCMD-HINT-001 state its one restriction line.

The `custom_chat_completions` packet changes the custom completion set. Its action `add`
adds the entries, `remove` removes them and `set` replaces the set. The set starts empty for
each connection.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with a second
player `Alex` online, type `hi al`. No list shows. Press Tab: the list shows `Alex`. Press Tab
again: the input becomes `hi Alex`.

---

## 4. Server requests

### CCMD-REQ-001: When the client sends a request

- **Behavior:**
  1. CCMD-SUG-001, step 3, sends one server request when a child of the suggestion parent
     needs the server (CCMD-SUG-002).
  2. The text of the request is the truncated input, with the `/`.
  3. The id of the request follows CCMD-REQ-002.
  4. The client sends the request at once. There is no delay and no merge of fast edits.
  5. The client sends one request for one computation, also when more than one child needs
     the server. Each such child takes the suggestions of that one reply (CCMD-DEV-002).
- **Postconditions:** a new request cancels the open request. The new request becomes the
  open request.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: on a server with an
`ask_server` argument after the literal `x`, type `/x a` one key at a time. Capture one request
for each key after the space, with the texts `/x `, then `/x a`.

### CCMD-REQ-002: Request ids

- **Behavior:**
  - The client keeps a counter. Its first value is −1.
  - A new request adds 1 to the counter. The new value is the id of the request.
  - When a reply with the id of the open request arrives, the counter becomes −1 again.
- **Postconditions:** the first request after a reply has the id 0. Ids increase while
  requests have no reply.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: on a server that
never replies, type three keys after an `ask_server` argument. The ids are 0, 1 and 2. On a
server that replies, the ids are 0, 0 and 0.

### CCMD-REQ-003: Replies

- **Inputs:** a `command_suggestions` packet.
- **Behavior:**
  1. If its id is not the id of the open request, ignore the packet.
  2. Otherwise the suggestions of the packet are the suggestions of the child that needs the
     server. The range of each suggestion is the range of the packet. The tooltip of each
     suggestion is the tooltip of the packet.
  3. The request is no longer open.
- **Postconditions:**
  - A reply to a cancelled request has no effect.
  - When the input changes and the new computation needs no server, the client cancels the
    open request. Its reply then has no effect (CCMD-DEV-001).
  - An empty reply gives no suggestion for that child.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on a modified
server that delays each reply by 2 s, type `/x a`, then `b` within 2 s. Only the reply for
`/x ab` changes the list.

---

## 5. The suggestion list

### CCMD-LIST-001: When the list shows

- **Behavior:**
  - The chat screen has a flag "auto". The flag is false when the screen opens. Each change
    of the input that the player makes sets the flag to true. A recall from the history sets
    it to false (`client-chat.md`, CHAT-HIST-002).
  - When the suggestion state of a command input becomes ready, the screen hides the list.
    Then, if "auto" is true and the option "Command Suggestions" is on, the screen shows the
    list.
  - Tab shows the list when the list is hidden (CCMD-LIST-004).
  - To show the list, the suggestion state must be ready and must hold at least one
    suggestion. Otherwise nothing shows.
- **Postconditions:**
  - For chat text, the list shows only after Tab.
  - When the screen opens with `/`, no list shows until the player types or presses Tab.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: press `/`. No
list shows. Press Tab. The list of commands shows.

### CCMD-LIST-002: Layout

- **Inputs:** the list in display order (CCMD-SUG-003), with `n` entries; the range start `s`
  of the suggestions.
- **Behavior:**
  1. `w` is the largest font width of the suggestion texts.
  2. `r` = `min(n, 10)` is the number of rows.
  3. `x0` = `4 + width(input[0..s])`. If `s` is past the end of the input, `x0` = 4.
  4. `x` = the value of `x0`, clamped to `[0, max(W − w, 0)]`.
  5. The list rectangle has the left edge `x − 1` and the width `w + 1`. Its top is
     `H − 15 − 12·r`, and its height is `12·r`.
  6. Row `i`, from 0, covers the rectangle from `top + 12·i` to `top + 12·i + 12`. Its fill
     is `D0000000`.
  7. Row `i` shows entry `offset + i`. The text starts at `x`, with its top at
     `top + 12·i + 2`. The colour is `FFFFFF00` for the selected suggestion and `FFAAAAAA`
     for the others.
  8. When `n > 10`, a line of 1 GUI unit is above the rectangle and one is below it. Each
     line has the fill `D0000000`. When `offset > 0`, the line above has a white pixel at each
     even column of the rectangle. When `offset + 10 < n`, the line below has the same dots.
- **Postconditions:** the last row ends 3 GUI units above the top of the input text. The list
  grows upwards.

`offset` is the index of the first row. It starts at 0.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: at a GUI scale of
2 on a 1920×1080 window, type `/`. Take a screenshot. The list has 10 rows, and its last row
ends 15 GUI units above the bottom of the screen.

### CCMD-LIST-003: Selection and ghost text

- **Behavior:**
  1. When the list shows, the selected suggestion is entry 0.
  2. To select entry `k`, take `k` modulo `n`, so that −1 selects the last entry.
  3. After each selection, compute the result of the selected suggestion. It is the input of
     the moment when the list showed, with the range of the suggestion replaced by its text.
  4. If the result starts with the current input, the ghost text is the rest of the result.
     Otherwise there is no ghost text.
  5. The ghost text shows only when the caret is at the end of the input, and the input is
     shorter than 256 code units. It starts 1 GUI unit left of the caret, in `FF808080`.
- **Postconditions:** the ghost text never changes the input.

Provenance: `source_suggested`. Kind: semantic for steps 1 to 4, layout for step 5. Confidence:
medium. Experiment: type `/gam`. The ghost text is `emode`.

### CCMD-LIST-004: Keys

The screen gives each key to the suggestion list first. The list uses a flag "cycle", false
when the list shows.

| Key | List shown | Effect |
|-----|-----------|--------|
| Up | yes | select the previous entry (CCMD-LIST-006), set "cycle" to false |
| Down | yes | select the next entry, set "cycle" to false |
| Tab | yes | if "cycle" is true, select the next entry; then insert the selected entry (CCMD-LIST-005) |
| Shift+Tab | yes | if "cycle" is true, select the previous entry; then insert the selected entry |
| Escape | yes | hide the list and clear the ghost text |
| Tab or Shift+Tab | no | show the list (CCMD-LIST-001) |

A key that the table does not name goes to the chat screen (`client-chat.md`, CHAT-KEY-001).
Up and Down with no list recall the history. Escape with no list closes the chat screen.

- **Postconditions:** the first Tab after the list shows inserts entry 0. Each later Tab
  inserts the next entry. Up or Down, then Tab, inserts the entry that Up or Down selected.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: type
`/gamemode `, press Tab three times. The input shows the first, the second, then the third game
mode.

### CCMD-LIST-005: Insert a suggestion

- **Behavior:**
  1. Replace the input with the result of the selected suggestion (CCMD-LIST-003, step 3).
  2. Put the caret at the start of the suggestion range plus the length of its text. Clear
     the text selection.
  3. Apply CCMD-PARSE-001 for the new input, but keep the suggestion list and the ghost text.
     Do not compute a new suggestion state.
  4. Select the same entry again, to update the ghost text. Set "cycle" to true.
- **Postconditions:**
  - The list stays with the same entries and the same range. Each insert replaces the same
    range of the input of the moment when the list showed. Thus Tab cycles through the
    entries in place.
  - Step 3 clears the hint lines. They stay empty until the next change of the input.
- **Edge cases:** the next change that the player makes computes a new suggestion state, and
  the list closes (CCMD-PARSE-001, step 3).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: type
`/gamemode s`, press Tab twice. The input is `/gamemode survival`, then `/gamemode spectator`.

### CCMD-LIST-006: Scroll of the list

- **Behavior:**
  - After a move of the selection by one entry to entry `k`:
    - if `k < offset`, `offset` becomes `k`;
    - if `k > offset + 9`, `offset` becomes `k − 9`.
  - Then clamp `offset` to `[0, max(n − 10, 0)]`.
  - A mouse wheel step over the list rectangle subtracts the step from `offset`, then clamps
    it. Upwards is +1. The selection does not change.
- **Postconditions:** a wrap from the last entry to entry 0 sets `offset` to 0. A wrap from
  entry 0 to the last entry sets `offset` to `n − 10`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: type `/`, press
Tab, then press Up once. The last command is selected, and the list shows the last 10 commands.

### CCMD-LIST-007: Mouse and tooltip

- **Inputs:** the list rectangle with the left edge `lx`, the top `top` and the width `lw`
  (CCMD-LIST-002); the mouse position (`mx`, `my`) of `client-tooltip.md`, section 0.1.
- **Behavior:** each frame that shows the list does these steps:
  1. The mouse is over row `i` when all of these are true:
     - `lx < mx < lx + lw`;
     - `top + 12·i < my < top + 12·i + 12`.
  2. When the mouse is over row `i`, and the mouse position is not the same as in the frame
     before, the screen selects entry `offset + i` (CCMD-LIST-003).
  3. When the mouse is over a row, and the selected suggestion has a tooltip, the screen makes
     a tooltip request (`client-tooltip.md`, TIP-REQ-001) at the mouse position. The request
     has one line, the tooltip of the selected suggestion. It has no tooltip style.
  4. When `lx ≤ mx ≤ lx + lw` and `top ≤ my ≤ top + 12·r`, the mouse pointer is a hand.
- **Text of the tooltip:**
  - A suggestion of the client parse has a translation key as its tooltip. The line is the
    translation of the key in the current language.
  - A suggestion from a server reply has a text component as its tooltip
    (`commands-packet.md`, CMDP-SUG-002). The line is that text component.
  - The line does not wrap, whatever its width.
- **Clicks:** a left click at (`mx`, `my`) with `lx ≤ mx ≤ lx + lw` and `top ≤ my ≤ top + 12·r`
  selects entry `offset + floor((my − top) / 12)` and inserts it (CCMD-LIST-005), if that
  entry exists.
- **Postconditions:**
  - Only the mouse shows a tooltip. While the mouse is not over a row, a selection with the
    keyboard shows no tooltip.
  - While the mouse stays still over a row, Up and Down change the selected suggestion and not
    the row. The tooltip shows the selected suggestion, not the suggestion of the row under the
    mouse.
  - The edge pixels of a row select nothing and show no tooltip.
  - The tooltip uses the position, the background and the order of `client-tooltip.md`.

Provenance: `source_suggested`. Kind: semantic; layout for the rectangles. Confidence: medium.
Experiment: type `/execute as @`. Put the mouse on the row `@p`. The tooltip
`Nearest player` shows at the mouse. Press Down without a move of the mouse. The tooltip of `@r`
shows.

---

## 6. Hint lines

### CCMD-HINT-001: Which lines show

- **Inputs:** a ready suggestion state of a command input, the client parse, the caret and the
  input.
- **Behavior:** the screen builds the hint lines in this order:
  1. If the caret is at the end of the input:
     - If the suggestion state has no suggestion, and the client parse has errors: add one
       error line for each error that is not `literal_incorrect`, in the order of the parse.
       If one or more errors are `literal_incorrect`, add one more error line for
       `dispatcher_unknown_argument` at the final cursor of the parse.
     - Otherwise, if the parse did not read to the end, note "trailing".
  2. If step 1 added no line:
     - Make the usage lines of the suggestion parent (CCMD-HINT-003).
     - If there is no usage line and "trailing" is noted, add one error line. Its error is the
       reported error of `command-parser.md`, CMD-PARSE-007, cases 1 to 3.
     - Add the usage lines.
  3. If the player cannot send commands, add the restriction line
     `chat_screen.commands_not_allowed` in red.
  4. If the parse contains a message argument (CCMD-PARSE-002), and the player cannot send
     chat messages, add the restriction line `chat_screen.messages_not_allowed` in red.
- **Postconditions:**
  - For chat text, the only possible hint line is `chat_screen.messages_not_allowed` in red,
    when the player cannot send chat messages. It shows as soon as the input is not blank.
  - The hint lines show only while the suggestion list is hidden.
  - `client-chat.md`, CHAT-OPT-002, states when the player cannot send.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on a tree with
only the executable literal `a`, type `/b`. One error line shows (CCMD-HINT-002). Type `/a`. No
line shows.

### CCMD-HINT-002: Text of an error line

- **Inputs:** an error with a position `q`, and the input.
- **Behavior:** the line is the translation `command.context.parse_error`. It has three
  arguments:
  1. the text of the error (`command-parser.md`, CMD-ERR-002, or the argument document);
  2. the number `q`, which counts the `/`;
  3. the context text of `command-parser.md`, CMD-ERR-003, over the input, followed by the
     literal text `<--[HERE]`.
- **Postconditions:** an error with no position is the text of the error only. The line has
  no colour of its own, so it shows in `FFFFFFFF`, unless the text of the error has a style.
- **Example:** on a tree with only the executable literal `a`, the input `/b` gives this
  line in `en_us`:
  `Unknown or incomplete command. See below for error at position 1: /<--[HERE]`.

| Key | `en_us` text |
|-----|--------------|
| `command.context.parse_error` | `%s at position %s: %s` |
| `command.unknown.command` | `Unknown or incomplete command. See below for error` |
| `command.unknown.argument` | `Incorrect argument for command` |
| `command.expected.separator` | `Expected whitespace to end one argument, but found trailing data` |
| `chat_screen.commands_not_allowed` | `Sending commands is not allowed` |
| `chat_screen.messages_not_allowed` | `Sending chat messages is not allowed` |

Provenance: `source_suggested` (language file `assets/minecraft/lang/en_us.json` of the client
jar); `library_source`. Kind: semantic. Confidence: medium. Experiment: type `/b` as above, and
read the line and its colour on a screenshot. A red line contradicts this rule.

### CCMD-HINT-003: Usage lines

- **Inputs:** the suggestion parent `P` at the caret.
- **Behavior:**
  1. For each child of `P`, in the order of the children, compute the short usage of that
     child (`command-parser.md`, CMD-USAGE-002, postconditions). `optional` is true when `P`
     is executable.
  2. Drop the usage of each child that is a literal.
  3. Each usage that remains is one usage line, in gray.
- **Postconditions:** the number of usage lines has no limit.
- **Example:** `tp` is a literal that is not executable. It has two argument children.
  `<destination>` is executable. `<location>` is executable, with the children `<rotation>`
  (executable) and the literal `facing`. The input `/tp ` gives two usage lines, in this
  order: `<destination>` and `<location> [<rotation>|facing]`.

Provenance: `source_suggested`; `library_source`. Kind: semantic. Confidence: medium.
Experiment: on a modified server, send the tree of the example and type `/tp `.

### CCMD-HINT-004: Layout of hint lines

- **Inputs:** the hint lines, with `m` lines; the start `s` of the suggestion state.
- **Behavior:**
  1. `w` is the largest font width of the lines.
  2. If the lines come from chat text, `x` = 0. Otherwise `x` is the value of
     CCMD-LIST-002, step 4, for the start `s` and the width `w`.
  3. Line `j`, from 0, has its top at `H − 27 − 12·j`. Line 0 is the lowest line.
  4. Each line has a fill from `x − 1` to `x + w + 1`, and from its top to its top + 12. The
     fill is `D0000000`.
  5. The text starts at `x`, with its top at the top of the line + 2. The default colour is
     `FFFFFFFF`.
- **Postconditions:** the first hint line is the lowest. The lines go upwards.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: type `/tp ` on the
tree of CCMD-HINT-003. Take a screenshot. The line `<destination>` has its top 27 GUI units
above the bottom of the screen.

---

## 7. Colours of the input

### CCMD-HL-001: Colours of a command input

- **Inputs:** the client parse of a command input.
- **Behavior:**
  1. Take the last context in the chain of child contexts.
  2. Take its parsed arguments in the order in which the parse added them. Give argument `k`,
     from 0, the colour `k mod 5` of this cycle: aqua, yellow, green, light purple, gold.
  3. Each argument range has its colour.
  4. If the parse did not read to the end, the text from the final cursor of the parse to
     the end of the input is red.
  5. All other text is gray. This includes the `/`, the literals, the separators and the
     arguments of earlier contexts.
- **Postconditions:**
  - Chat text keeps the default colour of the input box (`client-chat.md`, CHAT-BOX-001).
  - A draft shows in gray italic, and this rule does not apply (`client-chat.md`,
    CHAT-OPEN-002).
- **Edge cases:** when the box shows only a part of the input, the colours stay with their
  positions in the input.
- **Example:** the input `/give @s stone 5`, with three arguments, shows `/give ` in gray,
  `@s` in aqua, ` ` in gray, `stone` in yellow, ` ` in gray and `5` in green.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: type
`/execute as @s run give @s stone 5`. Only `@s`, `stone` and `5` after `run` have colours. `@s`
after `as` is gray.

---

## 8. Intentional deviations

### CCMD-DEV-001: A reply for an older input

- **Rule:** CCMD-REQ-003.
- **Reference:** the client does not cancel the open request when a new computation needs no
  server. A late reply with the id of that request then rebuilds the hint lines with the old
  suggestions. The hint lines of the current input show twice, and the list can show again
  after Escape.
- **Visible to the client:** yes, only when a reply comes after the input changed.
- **Test:** a differential test ignores hint lines that repeat after a late reply.

### CCMD-DEV-002: Two children that need the server

- **Rule:** CCMD-REQ-001, step 5.
- **Reference:** the client sends one request for each child that needs the server. Each
  request cancels the one before it. The cancelled child never gets suggestions, so the
  suggestion state never becomes ready, and no list shows.
- **Visible to the client:** yes, only for a suggestion parent with two such children. No
  vanilla tree has two.
- **Test:** a differential test reports such a tree as this deviation.

---

## 9. Open points

- The command block editor uses the same mechanisms with other values: no auto flag, the list
  below the box, and 7 rows. A separate document can state it.
- The narration of the suggestion list (`narration.suggestion` and its variants) is not
  stated.
- CCMD-LIST-007: a tooltip text with a line break. The line does not wrap and does not split
  at the line break. What the font draws for the line break is not stated. Experiment: on a
  server whose reply has the tooltip text `a\nb`, put the mouse on the row. Record what the
  tooltip shows.

- CCMD-HINT-002: the colour of an error line. The source gives white. Run the experiment of
  CCMD-HINT-002 to confirm.
