# The client chat: behavioural specification

| Field | Value |
|-------|-------|
| Game version | `26.4-snapshot-4` (Minecraft Java Edition, 26.4 Snapshot 4) |
| Protocol version | `1073742166` |

Source of these values: `assets/minecraft/version.json`.

This document states what the vanilla client does to let a player type chat and commands and
to show received messages. The subjects are the chat screen, the input box, the history, the
packets that the client sends, the chat log, the overlay message and the options that change
them. `client-commands.md` states the parse of a command input, the suggestions, and the error
and usage lines. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **GUI unit**: one pixel of the GUI at the current GUI scale. All sizes in this document are
  in GUI units.
- **W**, **H**: the width and the height of the GUI, in GUI units. The origin is the top left
  corner, and y increases downwards.
- **Tick**: one client tick, 50 ms. The GUI tick counter adds 1 each tick while the game is
  not paused.
- **Chat screen**: the screen with the input box at the bottom and the chat log above it.
- **Input box**: the one-line text field of the chat screen.
- **Input**: the text in the input box.
- **Caret**: the position in the input where the next typed code unit goes, counted in UTF-16
  code units.
- **Command input**, **client parse**, **hint line**, **suggestion list**: as
  `client-commands.md`, section 0.1, defines them.
- **Draft**: an input that the client keeps after the chat screen closes without a send.
- **History**: the list of earlier inputs that Up and Down recall.
- **Message**: one entry of the chat log, with its text component, its source, its tag and the
  GUI tick when it arrived.
- **Line**: one row of a message after the wrap to the chat width.
- **Chat log**: the messages and lines that the client keeps.
- **Tag**: a mark on a message, with an indicator colour, an optional icon and a tooltip
  (CHAT-LOG-005).
- **Overlay message**: the one text that shows above the hotbar, also called the action bar.
- **Open**, **closed**: the chat is open while the chat screen is the current screen.
  Otherwise it is closed.
- **Unsigned session**: a connection where the client has no profile key pair. An offline
  server always gives an unsigned session.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents".

### 0.3 Colours and text

A colour is a 32-bit ARGB value in hexadecimal. The font has a line height of 9 GUI units. GUI
text draws with a drop shadow unless a rule says otherwise. The width of a text is the font
width in GUI units. The font and the wrap of a text component do not have a specification yet
(section 7).

### 0.4 Wire types

The wire types of `commands-packet.md`, section 0.2, apply. These types are also used:

| Type | Form |
|------|------|
| UUID | 16 bytes, big-endian |
| Signature | 256 bytes |
| Optional X | a Boolean; X follows when it is true |
| Chat type | a VarInt `v`. When `v` > 0, the entry `v − 1` of the synced registry `minecraft:chat_type`. When `v` = 0, an inline chat type follows |

### 0.5 Packets

| State, direction | Name | Id |
|------------------|------|----|
| PLAY, serverbound | `chat` | 9 |
| PLAY, serverbound | `chat_command` | 7 |
| PLAY, serverbound | `chat_command_signed` | 8 |
| PLAY, clientbound | `system_chat` | 124 |
| PLAY, clientbound | `player_chat` | 66 |
| PLAY, clientbound | `disguised_chat` | 33 |
| PLAY, clientbound | `set_action_bar_text` | 89 |
| PLAY, clientbound | `custom_chat_completions` | 23 |

Provenance: `black_box_confirmed` (`assets/mcrs/reports/packets.json`).

### 0.6 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CHAT-OPEN` | how the chat screen opens and closes, and drafts |
| `CHAT-BOX` | the input box |
| `CHAT-HIST` | the history |
| `CHAT-KEY` | keys and the mouse wheel in the chat screen |
| `CHAT-SEND` | the text that the client sends, and the packet that carries it |
| `CHAT-RECV` | received packets that add a message or an overlay message |
| `CHAT-LOG` | the chat log, its layout, fade and scroll |
| `CHAT-OPT` | options that change the chat |
| `CHAT-DEV` | intentional deviations |

### 0.7 Scope

In scope: the chat screen, the input box, drafts, the history, the send of chat and commands
in an unsigned session, received chat messages, the chat log and the overlay message.

Out of scope: the signature of messages and the chain of a signed session, chat reports, the
social interactions screen, narration, what each click event in the chat log does, the
`delete_chat` packet, titles, and the screen in a bed.

---

## 1. The chat screen

### CHAT-OPEN-001: Keys that open the chat screen

- **Behavior:** the client tests these keys each tick, only when no screen and no overlay
  screen is open:

| Key binding | Translation key | Default key | Initial input |
|-------------|-----------------|-------------|---------------|
| Open Chat | `key.chat` | the key at the position of `T` | the draft, or the empty text |
| Open Command | `key.command` | the key at the position of `/` | the draft if it is a command input, or `/` |

  - A key binding names a physical key position, not a character.
  - "Open Chat" restores each draft. "Open Command" restores only a draft that starts with
    `/`.
  - A restored draft shows in gray italic. The first change of the input ends the draft
    style. If the first key is Backspace, it clears the full input.
- **Postconditions:** the character of the key that opens the screen is not typed into the
  input. The caret is at the end of the initial input.
- **Edge cases:** the chat screen opens also when the chat visibility hides the chat
  (CHAT-OPT-002).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on a keyboard
with the AZERTY layout, press the key that types `t` on a QWERTY keyboard. The chat screen
opens.

### CHAT-OPEN-002: Drafts

- **Behavior:** when the chat screen closes:
  1. Reset the scroll of the chat log to 0.
  2. If the input is blank, discard the draft.
  3. Else, if the screen closed after a send, discard the draft.
  4. Else, if the player closed the screen with Escape, keep the input as the draft only when
     the option "Save Unsent Chats" is on. Otherwise discard the draft.
  5. Else the screen closed for another reason, for example another screen opened. Keep the
     input as the draft.
- **Postconditions:** the client keeps at most one draft. The draft remembers if it is a
  command input.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: type `abc`,
press Escape, and press `T`. The input is empty. Turn on "Save Unsent Chats" and do it again.
The input is `abc` in gray italic.

### CHAT-BOX-001: The input box

- **Behavior:**
  - The input box holds at most 256 code units. A paste or a typed text that is longer is cut
    at the limit. The cut does not split a surrogate pair.
  - The box rejects the code units U+00A7, U+007F and each code unit below U+0020. A paste
    loses them, and the rest of the paste stays.
  - The box rectangle has the left edge 4, the top `H − 12`, the width `W − 4` and the height
    12. It has no border. The text starts at x = 4 and y = `H − 12`.
  - Before the box, the screen fills the rectangle from (2, `H − 14`) to (`W − 2`, `H − 2`).
    The fill is `80000000`, or CHAT-OPT-001 "Text Background" gives another fill.
  - The default text colour is `FFE0E0E0`. `client-commands.md`, CCMD-HL-001, colours a
    command input.
  - The caret blinks: it shows for 300 ms, then hides for 300 ms, from the moment of focus.
    At the end of a shorter input, the caret is the glyph `_`. Otherwise it is a bar of
    1 GUI unit, from y − 1 to y + 10 of the text.
  - When the text is wider than the box, the box shows the part around the caret.
- **Postconditions:** the input never holds a code unit that the server rejects
  (`commands-packet.md`, CMDP-EXEC-002).

Provenance: `source_suggested`. Kind: layout for the rectangles and colours, semantic for the
rest. Confidence: medium. Experiment: paste a text of 300 `a` into the input box. The input has
256 code units.

### CHAT-BOX-002: Edit keys

The input box has the edit keys of a one-line text field:

| Key | Effect |
|-----|--------|
| Backspace, Delete | remove the code unit before or after the caret; with Ctrl, remove a word |
| Left, Right | move the caret by one code unit; with Ctrl, by one word |
| Home, End | move the caret to the start or to the end |
| Shift with a move key | select text |
| Ctrl+A, Ctrl+C, Ctrl+V, Ctrl+X | select all, copy, paste, cut |

On macOS, Cmd replaces Ctrl. A generic text field specification can replace this table.

Provenance: `source_suggested`. Kind: semantic. Confidence: low. Experiment: type `a b`, press
Ctrl+Backspace. The input is `a `.

### CHAT-HIST-001: The history

- **Behavior:**
  - The history holds at most 100 inputs. The oldest input goes first when the history is
    full.
  - A send adds the normalized text of CHAT-SEND-001 to the end of the history. The client
    does not add a text that equals the last entry.
  - The client also keeps a command history of at most 50 command inputs, with the `/`. The
    client writes it to the file `command_history.txt` in the game directory, in UTF-8, one
    input on each line. A command input that equals the last command is not added.
  - When the client starts, and after a disconnect, the history holds the command history.
- **Postconditions:** chat text that the player sent is lost at a disconnect. Commands stay.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: send `hi` and
`/help`, disconnect, join again, and press Up. The input is `/help`. Press Up again. The input
does not change.

### CHAT-HIST-002: Navigation

- **Behavior:**
  1. When the chat screen opens, the history position `p` is the size of the history.
     Position `size` is the line that the player types.
  2. Up adds −1 to `p`, and Down adds 1. Clamp `p` to `[0, size]`. If `p` does not change,
     nothing happens.
  3. When `p` leaves position `size`, the client saves the input.
  4. When `p` comes back to `size`, the input becomes the saved input.
  5. At another position, the input becomes the history entry at `p`.
  6. After a recall, `client-commands.md`, CCMD-LIST-001, does not show the suggestion list
     automatically.
- **Postconditions:** the caret is at the end of the new input. Up and Down recall the history
  only when the suggestion list is hidden (`client-commands.md`, CCMD-LIST-004).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: send `a` and
`b`. Open the chat, type `c`, press Up twice, then Down twice. The input is `b`, `a`, `b`, then
`c`.

### CHAT-KEY-001: Keys and the mouse in the chat screen

- **Behavior:** the screen handles a key in this order. The first step that uses the key
  is the last step.
  1. The suggestion list (`client-commands.md`, CCMD-LIST-004).
  2. Backspace on a draft clears the input (CHAT-OPEN-001).
  3. Escape closes the chat screen. The input box handles its edit keys (CHAT-BOX-002).
  4. Enter or keypad Enter sends the input (CHAT-SEND-002).
  5. Page Up scrolls the chat log up by `perPage − 1` lines. Page Down scrolls it down by
     `perPage − 1` lines (CHAT-LOG-004).
  6. Up and Down move in the history (CHAT-HIST-002).
- **Mouse wheel:**
  1. Clamp the wheel step to `[−1, 1]`.
  2. If the mouse is over the suggestion list, scroll the list (`client-commands.md`,
     CCMD-LIST-006).
  3. Otherwise scroll the chat log by the step times 7 lines. With Shift, scroll by the step.
     Upwards is positive.
- **Mouse click:** a left click goes to the suggestion list first, then to the chat log
  (CHAT-LOG-006), then to the input box.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: with 40 lines in
the chat log, open the chat and press Page Up. The log moves by 19 lines.

---

## 2. Send

### CHAT-SEND-001: Normalization

- **Inputs:** the input.
- **Behavior:**
  1. Remove each code unit up to U+0020 at the start and at the end.
  2. Replace U+00A0 with U+0020.
  3. Replace each run of whitespace code units with one U+0020. A whitespace code unit is
     U+0009 to U+000D, U+001C to U+001F, U+0020, U+1680, U+2000 to U+2006, U+2008 to U+200A,
     U+2028, U+2029, U+205F and U+3000. Remove a run at the start or at the end.
  4. If the result is longer than 256 code units, keep the first 256.
- **Postconditions:** the result has no whitespace at the ends and no two adjacent spaces.
  A U+00A0 next to a space stays as a second space.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: type `  a   b  `
and press Enter. Capture the `chat` packet. Its text is `a b`.

### CHAT-SEND-002: Enter

- **Behavior:** when the player presses Enter or keypad Enter, and the suggestion list does not
  use the key:
  1. If the input is blocked (CHAT-OPT-002), do nothing. The screen stays open.
  2. Normalize the input (CHAT-SEND-001). If the result is empty, send nothing and go to
     step 6.
  3. Add the result to the history (CHAT-HIST-001).
  4. If the result starts with `/`, the command line is the result without the first `/`.
     Send it as CHAT-SEND-004 states.
  5. Otherwise send the result in a `chat` packet (CHAT-SEND-003).
  6. Close the chat screen.
- **Postconditions:**
  - The client sends a command line also when the client parse fails or does not end at an
    executable node.
  - The input `/` sends a `chat_command` with the empty command line.
- **Edge cases:** a draft that the player did not edit is sent as it is.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: type `/` and
press Enter. Capture a `chat_command` with the empty text.

### CHAT-SEND-003: `chat` in an unsigned session

| Field | Type | Value in an unsigned session |
|-------|------|------------------------------|
| message | String (256) | the normalized text |
| timestamp | Long | the current time, in milliseconds since 1970-01-01T00:00:00Z |
| salt | Long | a random value |
| signature | Optional Signature | absent |
| offset | VarInt | CHAT-SEND-005 |
| acknowledged | 3 bytes | a bit set of 20 bits, CHAT-SEND-005 |
| checksum | Byte | CHAT-SEND-005 |

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: on an offline
server, send `hi`. Decode the packet. The signature flag is false.

### CHAT-SEND-004: Choice of the command packet

- **Inputs:** the command line `L` of CHAT-SEND-002, step 4.
- **Behavior:**
  1. Parse `L` against the client tree with the client source, from position 0
     (`client-commands.md`, CCMD-TREE-001).
  2. If the parse has no parsed value of the type `minecraft:message`, send `chat_command`
     with `L` (`commands-packet.md`, CMDP-EXEC-001).
  3. Otherwise send `chat_command_signed`:

| Field | Type | Value in an unsigned session |
|-------|------|------------------------------|
| command | String (32767) | `L` |
| timestamp | Long | the current time, in milliseconds since 1970-01-01T00:00:00Z |
| salt | Long | a random value |
| argument signatures | Array of entry, at most 8 | empty |
| offset | VarInt | CHAT-SEND-005 |
| acknowledged | 3 bytes | CHAT-SEND-005 |
| checksum | Byte | CHAT-SEND-005 |

  An entry is a String (16) with the name of the argument, then a Signature. In a signed
  session, each message argument has one entry. In an unsigned session the array is empty.
- **Postconditions:** in an unsigned session, `/say hi`, `/me hi` and `/msg Steve hi` arrive
  as `chat_command_signed` with no signature. A server that accepts unsigned players must run
  such a packet as a `chat_command` with the same command line.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: on an offline
server, send `/say hi`. Capture a `chat_command_signed` with the command `say hi` and an empty
array.

### CHAT-SEND-005: Last seen messages in an unsigned session

- **Behavior:**
  - The client tracks only `player_chat` packets that have a signature. In an unsigned
    session where no packet has a signature, each send carries:
    - `offset` = 0;
    - `acknowledged` = three zero bytes;
    - `checksum` = 1.
  - The client never sends `chat_ack` and `chat_session_update` in an unsigned session.
- **Postconditions:** the checksum 1 is the checksum of an empty list of last seen messages.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: on an offline
server, send `hi` twice. Both packets end with the bytes `00 00 00 00 01`.

---

## 3. Received messages

### CHAT-RECV-001: `system_chat`

| Field | Type |
|-------|------|
| content | Text component |
| overlay | Boolean |

- **Behavior:**
  - When `overlay` is true, the content becomes the overlay message (CHAT-RECV-005). The
    chat log does not change.
  - When `overlay` is false, the client adds a message with the source "server system" and
    the tag "system, single player" (CHAT-LOG-005). The client adds it only when the player
    can receive system messages (CHAT-OPT-002).
- **Postconditions:** the content shows as it is, with no decoration.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: send
`system_chat` with `{"text":"a"}` and `overlay` false. The chat log shows `a`.

### CHAT-RECV-002: `player_chat`

| Field | Type |
|-------|------|
| global index | VarInt |
| sender | UUID |
| index | VarInt |
| signature | Optional Signature |
| body: content | String (256) |
| body: timestamp | Long |
| body: salt | Long |
| body: last seen | Array of packed signature, at most 20 |
| unsigned content | Optional Text component |
| filter type | VarInt: 0 pass through, 1 fully filtered, 2 partly filtered |
| filter mask | present for type 2 only: a VarInt count, then that number of bytes; bit `i` is bit `i mod 8` of byte `i / 8` |
| chat type | Chat type |
| sender name | Text component |
| target name | Optional Text component |

A packed signature is a VarInt `v`. When `v` = 0, a Signature follows. When `v` > 0, it refers
to the entry `v − 1` of the signature cache.

- **Behavior:**
  1. The client counts `player_chat` packets from 0 for each connection. If the global index
     is not equal to the count, the client disconnects with
     `multiplayer.disconnect.bad_chat_index`.
  2. If the sender is not in the player list, the client adds the decoration of the chat type
     around `chat.validation_error` in red italic (CHAT-RECV-004). The tag is "error". Stop.
  3. If the sender has no chat session and the server enforces secure chat, do the same as
     step 2. Stop.
  4. The content is the unsigned content when it is present. Otherwise it is the literal text
     of the body content. When the option "Only Show Secure Chat" is on, the unsigned content
     is ignored.
  5. A message with no signature is "not secure". When the option "Only Show Secure Chat" is
     on, the client does not show it.
  6. A fully filtered message does not show. For a partly filtered message, the content is
     the body content, with each code unit whose mask bit is 1 replaced by `#`. Each run of
     `#` is dark gray (`555555`), with the hover text `chat.filtered`.
  7. Add a message with the source "player": the decoration of the chat type around the
     content, and the tag "not secure" (CHAT-LOG-005).
- **Postconditions:**
  - Steps 4 to 7 apply only when the player can receive player messages (CHAT-OPT-002).
  - In an unsigned session, each message from another player has the tag "not secure".
- **Edge cases:** a message with a valid signature has no tag, or the tag "modified". The
  signature check is out of scope.

Provenance: `source_suggested`. Kind: semantic; wire for the table and step 1. Confidence:
medium. Experiment: on an offline server with two players, one sends `hi`. The other client
shows `<Name> hi` with a gray indicator, and the tooltip of the indicator is
`chat.tag.not_secure`.

### CHAT-RECV-003: `disguised_chat`

| Field | Type |
|-------|------|
| message | Text component |
| chat type | Chat type |
| sender name | Text component |
| target name | Optional Text component |

- **Behavior:** when the player can receive player messages, the client adds a message with
  the source "player". The text is the decoration of the chat type around the message. The
  tag is "system".
- **Postconditions:** the chat delay of CHAT-OPT-003 applies.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: on a vanilla
server in offline mode, run `/say hi` from the console. The chat shows `[Server] hi` with a gray
indicator.

### CHAT-RECV-004: Decoration of a chat type

- **Inputs:** a chat type, a sender name, an optional target name and a content.
- **Behavior:** the `chat` decoration of the chat type gives the text:
  1. Make the list of arguments. For each entry of `parameters`, in order: `sender` gives the
     sender name, `target` gives the target name or the empty text, `content` gives the
     content.
  2. The text is the translation of `translation_key` with these arguments.
  3. The style of the decoration applies to the translation. The arguments keep their own
     styles.
- **Postconditions:** the vanilla chat type `minecraft:chat` gives `<%s> %s` (`chat.type.text`)
  with the sender name and the content.

The fields of a chat type are in `registry-fields.md`, REG-FLD-102. The client reads them from
the synced registry, so a data pack can add a chat type with no change to the client.

Provenance: `source_suggested` (data pack file `data/minecraft/chat_type/chat.json`). Kind:
semantic. Confidence: medium. Experiment: add a chat type with `"translation_key": "%s: %s"` to
a pack, and send a `disguised_chat` with it.

### CHAT-RECV-005: The overlay message

- **Inputs:** a `system_chat` with `overlay` true, or a `set_action_bar_text` with one Text
  component.
- **Behavior:**
  1. The text replaces the overlay message. The time left becomes 60 ticks.
  2. Each tick, the time left decreases by 1, while the game is not paused.
  3. The overlay shows while the time left is above 0. With `t` = the time left minus the
     partial tick, the alpha is `min(255, floor(t · 255 / 20))`. It does not show when the
     alpha is 0 or less.
  4. The text is centred on x = `W / 2`, with its top at `H − 72`. Its colour is white with
     that alpha.
  5. When "Text Background" is "Everywhere", a backdrop is behind the text. It extends 2 GUI
     units on each side. Its colour is black with the text background opacity, times the
     text colour. With "Chat", there is no backdrop.
- **Postconditions:** the overlay shows for 3 s. It fades out during the last 1 s. The chat
  visibility option does not hide it. It is hidden while the HUD is hidden.

Provenance: `source_suggested`. Kind: layout for steps 3 to 5, semantic for the rest.
Confidence: medium. Experiment: send `set_action_bar_text` with `a`. Record the screen. The text
stays opaque for 2 s, then fades for 1 s.

---

## 4. The chat log

### CHAT-LOG-001: Messages and lines

- **Behavior:**
  1. A message gets the GUI tick of its arrival. The client tests the message against the
     visibility filter (CHAT-OPT-002). A message that fails is dropped.
  2. The client keeps the 100 newest messages.
  3. The client wraps the message to the wrap width (CHAT-LOG-002). When the tag of the
     message has an icon, the wrap width is 15 GUI units smaller.
  4. A line that continues a wrapped row starts with one U+0020 with no style. A message that
     gives no row gives one empty line.
  5. The client keeps the 100 newest lines.
  6. When the chat is open and the scroll is above 0, each new line adds 1 to the scroll
     (CHAT-LOG-004). The flag "new since scroll" becomes true.
  7. When the option "Colors" is off, the client removes each `§` code and the character after
     it from the text of each message before the wrap.
- **Postconditions:** a change of the chat scale, the width, a height, the text opacity or the
  text background opacity wraps the kept messages again. It also resets the scroll to 0.
- **Edge cases:** a disconnect discards all messages and lines.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: send 150
one-line system messages. Open the chat and scroll to the top. The first line is message 51.

### CHAT-LOG-002: Layout of the lines

- **Inputs:** the chat scale `s`, the chat width `cw` (CHAT-OPT-001), the line spacing `ls`,
  the number of lines `perPage` (CHAT-LOG-004), and an alpha `a` for each line (CHAT-LOG-003).
- **Behavior:** the log uses a local frame. A point (x, y) of the frame is at (`s·(x + 4)`,
  `s·y`) on the GUI.
  - The wrap width is `floor(cw / s)`. The draw width `dw` is `ceil(cw / s)`.
  - The chat bottom `B` is `floor((H − 40) / s)`.
  - The entry height `e` is `floor(9 · (1 + ls))`. The text offset `d` is
    `round(8 · (1 + ls) − 4 · ls)`.
  - Slot `i`, from 0, shows line `scroll + i`. Line 0 is the newest line. The slots are
    `i = 0` to `min(lines − scroll, perPage) − 1`.
  - Slot `i` covers y from `B − (i + 1)·e` to `B − i·e`.
  - A slot with `a` ≤ 0.00001 is not drawn.
  - The background of a slot is the rectangle from x = −4 to `dw + 8`. Its colour is black
    with the alpha `a · tbo`. `tbo` is the text background opacity (CHAT-OPT-001).
  - The text of a slot starts at x = 0, with its top at `B − i·e − d`. Its opacity is
    `a · (0.1 + 0.9 · co)`. `co` is the chat text opacity.
  - When the line has a tag, a bar from x = −4 to −2 covers the slot. Its colour is the
    indicator colour of the tag, with the opacity of the text.
- **Postconditions:** with the default options, a slot is 9 GUI units high, and the newest
  line ends 40 GUI units above the bottom of the GUI.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: at the default
options, send one message and take a screenshot. The background covers x 0 to 332, and y
`H − 49` to `H − 40`.

### CHAT-LOG-003: Fade when closed

- **Behavior:**
  - When the chat is open, `a` = 1 for each line.
  - When the chat is closed, `t` is the GUI tick now minus the GUI tick of the message. Then
    `a` = `clamp(10 · (1 − t / 200), 0, 1)²`.
  - The closed chat draws only while no chat screen is open and the HUD is not hidden.
- **Postconditions:** a message shows at full opacity for 9 s after it arrives. It fades for
  1 s, then is not drawn. A line that is not drawn has no background.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: send a message
and record the screen. The line is opaque for 9 s and gone after 10 s.

### CHAT-LOG-004: Scroll when open

- **Behavior:**
  - The visible height is `floor(v · 160 + 20)`. `v` is the option "Focused Height" when the
    chat is open, and "Unfocused Height" when it is closed.
  - `perPage` is `floor(height / e)`. The chat scale does not change it.
  - The scroll is the number of the newest lines that are below the view. To scroll by `k`,
    add `k` to the scroll. Then:
    - if the scroll is above `lines − perPage`, it becomes `lines − perPage`;
    - if the scroll is 0 or less, it becomes 0, and "new since scroll" becomes false.
  - When the chat screen closes, or after a send, the scroll becomes 0.
- **Scroll bar:** when the chat is open and the log has more lines than slots that show:
  1. `ch` = (the number of slots that show) · `e`. `vh` = `lines · e`.
  2. `q` = `floor(scroll · ch / lines)`. `h` = `floor(ch · ch / vh)`.
  3. The bar covers y from `B − q − h` to `B − q`, in the local frame. Column x = `dw + 4`
     has the colour `3333AA`, or `CC3333` when "new since scroll" is true. Column `dw + 5`
     has the colour `CCCCCC`.
  4. The alpha of both columns is 170 when `q > B`. Otherwise it is 96.
- **Postconditions:** with the default options, the open chat shows 20 lines and the closed
  chat shows 10.

Provenance: `source_suggested`. Kind: semantic; layout for the scroll bar. Confidence: medium.
Experiment: send 30 messages. The closed chat shows the 10 newest. Open the chat. It shows the
20 newest and a scroll bar.

### CHAT-LOG-005: Tags

| Tag | Indicator | Icon | Tooltip key | Given to |
|-----|-----------|------|-------------|----------|
| system, single player | `D0D0D0` | none | `chat.tag.system_single_player` | `system_chat` |
| system | `D0D0D0` | none | `chat.tag.system` | `disguised_chat` |
| not secure | `D0D0D0` | none | `chat.tag.not_secure` | `player_chat` with no signature |
| modified | `606060` | `icon/chat_modified`, 9 × 9 | `chat.tag.modified`, a new line, and the original text in gray | `player_chat` that the server changed |
| error | `FF5555` | none | `chat.tag.error` | `player_chat` that fails its check |

- **Behavior:** when the chat is open and the mouse is over the bar of a tag, the tooltip
  shows. It wraps at 210 GUI units. The icon shows 4 GUI units after the end of the last line
  of its message, while the mouse is over that message or over the icon.

Provenance: `source_suggested`. Kind: layout for the colours, semantic for the rest. Confidence:
medium. Experiment: send a `system_chat`, open the chat and put the mouse on the left edge of
the line. The tooltip `Server message.` shows.

### CHAT-LOG-006: Clicks in the open chat

- **Behavior:** a left click on the text of a line in the open chat finds the style under the
  mouse.
  - With Shift, if the style has an insertion text, insert it at the caret.
  - Without Shift, if the style has a click event, run it. The event `suggest_command`
    replaces the input with its text.
  - A hover event of the style shows its tooltip while the mouse is over the text.
- **Postconditions:** the other click events are out of scope.

Provenance: `source_suggested`. Kind: semantic. Confidence: low. Experiment: send a
`system_chat` whose text has the click event `suggest_command` with `/help`. Open the chat and
click it. The input is `/help`.

---

## 5. Options

### CHAT-OPT-001: Options of the chat

| Option | Translation key | Default | Effect |
|--------|-----------------|---------|--------|
| Chat | `options.chat.visibility` | Shown | CHAT-OPT-002 |
| Colors | `options.chat.color` | on | CHAT-LOG-001, step 7 |
| Chat Text Opacity | `options.chat.opacity` | 1.0 | `co` of CHAT-LOG-002 |
| Text Background Opacity | `options.accessibility.text_background_opacity` | 0.5 | `tbo` of CHAT-LOG-002 |
| Text Background | `options.accessibility.text_background` | Chat | with Everywhere, the input box fill is black with `tbo`, and the overlay message has a backdrop |
| Chat Text Size | `options.chat.scale` | 1.0 | `s` of CHAT-LOG-002 |
| Width | `options.chat.width` | 1.0 | `cw` = `floor(v · 280 + 40)`; the default is 320 |
| Focused Height | `options.chat.height.focused` | 1.0 | 180 GUI units (CHAT-LOG-004) |
| Unfocused Height | `options.chat.height.unfocused` | 0.4375 | 90 GUI units |
| Line Spacing | `options.chat.line_spacing` | 0.0 | `ls` of CHAT-LOG-002 |
| Chat Delay | `options.chat.delay_instant` | 0.0 s | CHAT-OPT-003 |
| Command Suggestions | `options.autoSuggestCommands` | on | `client-commands.md`, CCMD-LIST-001 |
| Save Unsent Chats | `options.chat.drafts` | off | CHAT-OPEN-002 |
| Only Show Secure Chat | `options.onlyShowSecureChat` | off | CHAT-RECV-002 |

Each value with no unit is in the range 0 to 1. Chat Delay is in steps of 0.1 s from 0 to 6 s.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: in a new game
directory, open the chat options and read each value.

### CHAT-OPT-002: Chat visibility and restrictions

- **Behavior:** the client computes four abilities from the option "Chat" and the account:

| "Chat" | Send messages | Send commands | Receive player messages | Receive system messages |
|--------|---------------|---------------|-------------------------|-------------------------|
| Shown | yes | yes | yes | yes |
| Commands Only | no | yes | no | yes |
| Hidden | no | no | no | no |

  - On a multiplayer server, the launcher or the account can also forbid chat. Then the
    client cannot send messages and does not receive player messages.
  - A message from the server with the source "server system" needs "receive system
    messages". A message with the source "player" needs "receive player messages".
  - The visibility filter drops each message that fails its test (CHAT-LOG-001).
- **Input that is blocked:**
  - A command input is blocked when the client cannot send commands.
  - Chat text that is not blank is blocked when the client cannot send messages.
  - A command input whose client parse contains a message argument is blocked when the
    client cannot send messages (`client-commands.md`, CCMD-PARSE-002).
  - `client-commands.md`, CCMD-HINT-001, shows the restriction line. Enter does nothing.
- **Restricted prompt:** with a restriction, the open chat shows the line
  `chat_screen.restricted` above the newest slot. It is red and underlined, with a black
  background of opacity `tbo`. A click on it opens the screen of restrictions.
- **Postconditions:** a change of the option filters the kept messages again. The client
  reports the option to the server in `client_information`.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: set "Chat" to
"Commands Only", type `hi`. The line `Sending chat messages is not allowed` shows in red, and
Enter keeps the screen open.

### CHAT-OPT-003: Chat delay

- **Behavior:**
  - When the delay is 0, the client shows each message at once.
  - When the delay is above 0, `player_chat` and `disguised_chat` messages go into a queue.
    `system_chat` does not use the queue.
  - Each tick, if the time since the last shown message is at least the delay, the client
    shows the next message of the queue.
  - While messages wait, the chat shows the line `chat.queue` with their number, under the
    newest slot at half the text opacity. A click on it in the open chat shows the next
    message.
  - When the delay changes to 0, the client shows all queued messages.
- **Postconditions:** a disconnect shows all queued messages and then discards the log.

Provenance: `source_suggested`. Kind: semantic. Confidence: low. Experiment: set the delay to 2
s. Two players send one message each in the same second. The second message shows 2 s after the
first.

---

## 6. Intentional deviations

No deviation is recorded.

---

## 7. Open points

- The font, the wrap of a text component and the text of a translation with arguments
  have no specification. CHAT-LOG-001 and CHAT-RECV-004 depend on them.
- CHAT-LOG-002: the chat scale 0 is allowed, and the option shows "OFF". The layout then
  divides by 0. Experiment: set "Chat Text Size" to 0, send a message and check that no line
  shows.
- CHAT-LOG-005: the gray indicator of `system_chat` messages. The source draws it for each
  message with a tag. Experiment: send a `system_chat` and look at the left edge of the closed
  chat.
- What each click event in the chat log does, with the confirmation screens for
  `run_command`, needs a separate specification.
- The server side of an unsigned `chat` and of `chat_command_signed` with no signature has no
  specification. CHAT-SEND-004 states what the server receives.
