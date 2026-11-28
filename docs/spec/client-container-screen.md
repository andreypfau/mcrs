# Container screens of the client: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states the generic menus `generic_9x1` to `generic_9x6`, `generic_3x3` and `hopper`,
and the screen that the vanilla client shows for each of them. It states what the client does
with the clientbound `open_screen` packet. It also states the drag, the double click and the
parts of an item tooltip, for each menu screen. The document does not
prescribe an architecture.

`inventory-transactions.md` states the clicks on the server. `inventory.md` states the slots of
the player and the inventory menu. `client-inventory.md` states how the client makes a click from
a press, how it predicts the click and sends it, and how the player closes a menu. This document
refers to the rules of these documents by id.

## 0. Conventions

### 0.1 Terms

- **Stack**, **empty stack**, **count**, **maximum stack size**, **item**, **component type**,
  **effective component**, **prototype**: as `item-stack.md`, section 0.1, defines them.
- **Slot**, **cursor**, **slot limit**, **capacity**, **place filter**, **take filter**, **clicked
  slot**, **drag**, **holder**: as `inventory-transactions.md`, section 0.1, defines them.
- **Player inventory**, **menu**, **inventory menu**, **open menu**: as `inventory.md`, section
  0.1, defines them.
- **Creative flag**: as `client-block-breaking.md`, section 0.1, defines it.
- **Window area**, **entry area**, **empty area**, **target entry**, **pointer index**, **mouse
  press**, **mouse release**, **shift**, **button number**, **cursor stack**, **click**: as
  `client-inventory.md`, section 0.1, defines them. They apply to each menu screen, not only to
  the inventory screen. Section 3 states the window area and the entry areas of a container
  screen.
- **GUI unit**, **W**, **H**: as `client-chat.md`, section 0.1, defines them.
- **Mouse position**, **tooltip**, **tooltip request**, **line**: as `client-tooltip.md`,
  section 0.1, defines them.
- **Screen**, **widget**, **last input device**: as `client-widget.md`, section 0.1, defines
  them.
- **Generic menu**: a menu of one of the eight menu types of CCS-MNU-001.
- **Container slot**: an entry of a generic menu that is a slot of the block or the entity that
  the server opened. It is not a slot of the player.
- **Container part**: the container slots of a generic menu.
- **Player part**: the entries of a generic menu that are slots of the player inventory.
- **Menu screen**: the screen that the client shows for the open menu. A container screen is
  one kind of menu screen.
- **Container screen**: the menu screen of a generic menu.
- **Drag set**: the set of entries that a drag of the client fills.
- **Drag button**: the button of the press that started a drag of the client.
- **Remembered stack**: a copy of the stack that the last quick move click of the screen took
  from its slot (CCS-DBL-003).
- **Item tooltip**: the text that the client shows for a stack (section 6). Each element of it
  is a **part**. A part can give more than one line (CCS-TIP-001).
- **Creative tooltip**: an item tooltip that the creative screen requests for a slot of
  its grid (`client-creative-inventory.md`).
- **Advanced tooltips**: the option that F3 + H changes. It is off by default.
- **Pick key**: the key binding `key.pickItem`. Its default is the middle mouse button.

### 0.2 Provenance, kind and observer

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The language keys and their English texts are in
`assets/minecraft/lang/en_us.json` of the client jar.

The observer of the rules of section 1 is a player who clicks in a container screen and sees the
stacks move. The observer of a rule of the kind client agreement is the server, which reads the
clicks of the client. The observer of the other rules is a player in normal play who sees the
screen, unless the rule names another observer.

A rule that comes from the reference keeps the label `source_suggested` until an experiment of
section 9 confirms it.

### 0.3 Artefacts cited

- `assets/mcrs/reports/registries.json`: the registry `minecraft:menu` gives the numeric id of
  each menu type.
- `assets/mcrs/reports/packets.json`: the names `open_screen`, `container_close`,
  `container_click`, `container_set_content` and `container_set_slot`.
- The resource pack of the client jar: the textures and the GUI sprites that section 3 names.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CCS-MNU` | the entries and the quick move of the generic menus |
| `CCS-OPN` | the `open_screen` packet in the client |
| `CCS-LAY` | the layout of a container screen |
| `CCS-DRG` | the drag in a menu screen |
| `CCS-DBL` | the double click in a menu screen |
| `CCS-TIP` | the parts of an item tooltip |

### 0.5 Scope

In scope:

- the generic menus: the entries, the container part, the player part and the quick move;
- the `open_screen` packet in the client;
- the layout of a container screen: the textures, the sizes, the slot positions and the labels;
- the drag, the double click and Shift with each of them, in each menu screen;
- the parts of an item tooltip for the component types that `item-stack.md` defines.

Out of scope:

- the press, the release and the keys that give one click, the prediction of a click, the
  state id, the close by the player (CINV-CLS-004) and the clientbound `container_close`
  (CINV-CLS-003): `client-inventory.md`;
- the keyboard navigation between slots, and the narration of slots;
- the draw of an item, of its count and of its damage bar in a slot;
- the image of an item tooltip, for example the contents of a bundle, and the wheel on a bundle;
- the tooltip style of an item tooltip (`minecraft:tooltip_style`);
- the parts that the component types of section 6.2 add when this document marks them as out of
  scope;
- the menu types other than the eight of CCS-MNU-001, and their screens.

### 0.6 Constants

| Value | Data | Rule |
|-------|------|------|
| Time of a double click | less than 250 ms between two presses | CCS-DBL-001 |
| Width of the window area | 176 | CCS-LAY-001 |
| Colour of the labels | `FF404040`, no drop shadow | CCS-LAY-004 |
| Colour over a slot of the drag preview | `80FFFFFF` | CCS-DRG-005 |
| Size of the entry area | 16 × 16 | CCS-LAY-005 |
| Wrap width of an item tooltip | 170 | CCS-TIP-001 |

---

## 1. The generic menus

### CCS-MNU-001: The entries of a generic menu

- **Inputs:** a menu type of the table, and the container id of `open_screen`.
- **Behavior:** the menu has the entries of the table, in the order of the menu index. `n` is
  the number of container slots. The container part is the menu indices `0..n−1`. The player part
  is the menu indices `n..n+35`.

| Menu type | Numeric id | `n` | Container slots |
|-----------|------------|-----|-----------------|
| `minecraft:generic_9x1` | 0 | 9 | 1 row of 9 |
| `minecraft:generic_9x2` | 1 | 18 | 2 rows of 9 |
| `minecraft:generic_9x3` | 2 | 27 | 3 rows of 9 |
| `minecraft:generic_9x4` | 3 | 36 | 4 rows of 9 |
| `minecraft:generic_9x5` | 4 | 45 | 5 rows of 9 |
| `minecraft:generic_9x6` | 5 | 54 | 6 rows of 9 |
| `minecraft:generic_3x3` | 6 | 9 | 3 rows of 3 |
| `minecraft:hopper` | 17 | 5 | 1 row of 5 |

| Menu index | Slot |
|------------|------|
| `0..n−1` | the slot `i` of the container, for the menu index `i`, in rows from the top left |
| `n..n+26` | `P[9]` through `P[35]` |
| `n+27..n+35` | `P[0]` through `P[8]` |

  `P[i]` is as `inventory.md`, section 0.5, states. A generic menu has `n + 36` entries and no
  data values. The slot limit of each container slot is 99. A container slot has no take filter
  and no place filter.
- **Postconditions:** `generic_9x3` has 63 entries. Its menu index 27 is `P[9]`, and its menu
  index 54 is `P[0]`.

Provenance: `source_suggested`; `black_box_confirmed` for the numeric ids. Kind: wire.
Confidence: medium.

### CCS-MNU-002: The quick move in a generic menu

- **Inputs:** a quick move click on the menu index `s` of a generic menu with `n` container
  slots (`inventory-transactions.md`, ITX-JAV-009).
- **Behavior:** the destination list is the first row of the table whose condition is true.

| # | Condition | Destinations, in order |
|---|-----------|------------------------|
| 1 | `s` is in the container part. | `n+35` down to `n` |
| 2 | `s` is in the player part. | `0` up to `n−1` |

- **Postconditions:**
  - Each pass of ITX-JAV-009 examines the hotbar first, from `P[8]` to `P[0]`, and then the
    slots `P[35]` down to `P[9]`. Thus a stack from the container part merges into a partial stack
    of `P[9]` to `P[35]` before it goes to an empty slot of the hotbar.
  - A stack from the player part never goes to another slot of the player part. When the
    container part has no room, the click moves no item.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CCS-MNU-003: The repeat of a quick move in a generic menu

- **Inputs:** a quick move click on the menu index `s` of a generic menu. `S` is the slot of `s`.
- **Behavior:** the repeat of a quick move of `inventory-transactions.md`, ITX-JAV-026, applies.
  Each pass uses the destination list of CCS-MNU-002.
- **Postconditions:** in normal play, a stack has a count not larger than its maximum stack size,
  and the first pass moves all items for which room exists. Thus the repeat changes the result
  only for a stack whose count is larger than its capacity in the destination.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CCS-MNU-004: The conditions of a generic menu

- **Behavior:**
  1. Pick up all can take from each entry (ITX-JAV-011).
  2. A drag can go to each entry (ITX-JAV-017).
  3. A double click can gather from each entry (CCS-DBL-002).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

---

## 2. The open

### CCS-OPN-001: The client gets `open_screen`

- **Inputs:** a clientbound `open_screen` packet. It has a VarInt container id, a VarInt numeric
  id of the registry `minecraft:menu` and a text component, the title.
- **Behavior:**
  1. The client makes a new menu of the menu type, with the container id of the packet.
  2. Each container slot of the new menu holds the empty stack. The cursor stack becomes the
     empty stack. The player part shows the stacks that the client holds for the player
     inventory.
  3. The new menu becomes the open menu. The client does not send `container_close` for the menu
     that was open before.
  4. The client opens the menu screen of the new menu in place of the current screen
     (`client-widget.md`, WID-SCR-001). It does this also when another menu screen, the chat
     screen or another screen is open.
  5. The title of the packet is the title of the screen (CCS-LAY-004).
- **Postconditions:**
  - The container slots show stacks only after the next `container_set_content` or
    `container_set_slot` packet for the container id.
  - The client discards the stacks that it showed for the container part of the menu that was
    open before.
- **Edge cases:** a menu type that has no menu screen in the client gives no screen. The open menu
  then does not change.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

---

## 3. The layout of a container screen

### CCS-LAY-001: The window area

- **Inputs:** the menu type, W and H.
- **Behavior:** the window area is the rectangle with the left edge `left`, the top edge `top`,
  the width `iw` = 176 and the height `ih` of the table. `left` is `floor((W − iw) / 2)`, and
  `top` is `floor((H − ih) / 2)`. A container screen has no other part in its window area.

| Menu type | `ih` |
|-----------|------|
| `generic_9xR`, with `R` rows | `114 + 18·R` |
| `generic_3x3` | 166 |
| `hopper` | 133 |

- **Postconditions:**
  - `generic_9x3` has `ih` = 168, and `generic_9x6` has `ih` = 222.
  - The window area stays at the centre of the screen after a resize (`client-widget.md`,
    WID-SCR-003).
  - For `generic_9xR`, the background texture covers the window area except its bottom row of
    GUI units (CCS-LAY-002). That row is still a part of the window area.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CCS-LAY-002: The background texture

- **Inputs:** the window area of CCS-LAY-001.
- **Behavior:**
  1. The screen draws the background of an in-game screen (`client-widget.md`, WID-SCR-004).
  2. It then draws the parts of the table. A part copies the source rectangle of the texture to
     the same size at the destination. One texel is one GUI unit, and the texture is 256 × 256
     texels.

| Menu type | Texture | Source (x, y, w, h) | Destination (x, y) |
|-----------|---------|---------------------|--------------------|
| `generic_9xR` | `minecraft:textures/gui/container/generic_54.png` | (0, 0, 176, `18·R + 17`) | (`left`, `top`) |
| `generic_9xR` | the same texture | (0, 126, 176, 96) | (`left`, `top + 18·R + 17`) |
| `generic_3x3` | `minecraft:textures/gui/container/dispenser.png` | (0, 0, 176, 166) | (`left`, `top`) |
| `hopper` | `minecraft:textures/gui/container/hopper.png` | (0, 0, 176, 133) | (`left`, `top`) |

- **Postconditions:** the two parts of `generic_9xR` touch. The second part holds the label of
  the player inventory and the player part.

Provenance: `source_suggested`; `black_box_confirmed` for the size of the three textures in the
client jar. Kind: layout. Confidence: medium.

### CCS-LAY-003: The positions of the slots

- **Inputs:** the menu type.
- **Behavior:** each entry has an entry area of 16 × 16 GUI units. The table gives the top left
  corner of the entry area, relative to (`left`, `top`). `c` is the column and `r` is the row,
  from 0, in the order of CCS-MNU-001.

| Menu type | Container slot | `P[9]` to `P[35]` | Hotbar `P[0]` to `P[8]` |
|-----------|----------------|------------------------------|-------------------------|
| `generic_9xR` | (`8 + 18·c`, `18 + 18·r`) | (`8 + 18·c`, `31 + 18·R + 18·r`) | (`8 + 18·c`, `89 + 18·R`) |
| `generic_3x3` | (`62 + 18·c`, `17 + 18·r`) | (`8 + 18·c`, `84 + 18·r`) | (`8 + 18·c`, 142) |
| `hopper` | (`44 + 18·c`, 20) | (`8 + 18·c`, `51 + 18·r`) | (`8 + 18·c`, 109) |

  The slots `P[9]` to `P[35]` have the rows `r` = 0, 1 and 2. `P[9 + 9·r + c]` is in the column
  `c` and the row `r`. The hotbar slot `P[c]` is in the column `c`.
- **Postconditions:** in `generic_9x3`, the entry area of the container slot 0 has the corner
  (`left + 8`, `top + 18`), and `P[0]` has the corner (`left + 8`, `top + 143`).

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CCS-LAY-004: The labels

- **Behavior:** the screen draws two labels in `FF404040`, with no drop shadow. Each position is
  relative to (`left`, `top`) and is the top left corner of the text.

| Label | Text | Position |
|-------|------|----------|
| title | the title of `open_screen`, with its style | (8, 6); for `generic_3x3`: (`floor((176 − w) / 2)`, 6), where `w` is the font width of the title |
| player inventory | the translation `container.inventory` | (8, `ih − 94`) |

  A part of the title with a colour in its style draws in that colour. The labels do not wrap,
  and the screen does not cut them.
- **Postconditions:** with the `en_us` language, the second label is `Inventory`. In
  `generic_9x3` it is at (8, 74).

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CCS-LAY-005: The target entry and its highlight

- **Inputs:** the mouse position and the last input device, in each menu screen. The positions
  of CCS-LAY-003 are those of a container screen.
- **Behavior:**
  1. The entry area of an entry is `x ≤ mx < x + 16` and `y ≤ my < y + 16`.
  2. When the last input device is mouse, the screen draws a highlight for the target entry. It
     draws the GUI sprite `minecraft:container/slot_highlight_back` before the stack of the
     slot. It draws `minecraft:container/slot_highlight_front` after the
     stack. Each sprite covers (`x − 4`, `y − 4`) with the size 24 × 24.
  3. The sprites draw by the rules of `client-gui-sprite.md`.
- **Postconditions:** the gap of 2 GUI units between two entry areas is a part of the empty area.
  There, the pointer index is -1.

Provenance: `source_suggested`; `black_box_confirmed` for the two sprites of the client jar
(24 × 24, `nine_slice` with the border 4). Kind: layout. Confidence: medium.

### CCS-LAY-006: The order of the draw and the cursor stack

- **Behavior:** a container screen draws these layers in this order:
  1. the background (CCS-LAY-002);
  2. the labels (CCS-LAY-004);
  3. the entries, each with its highlight (CCS-LAY-005) and its drag preview (CCS-DRG-005);
  4. the stack of the cursor, at (`mx − 8`, `my − 8`), with its count. CCS-DRG-006 states the
     count during a drag;
  5. the tooltip of the frame (`client-tooltip.md`, TIP-REQ-001).
- **Postconditions:** the stack of the cursor is above each entry. When the cursor holds the
  empty stack, nothing draws at the mouse.
- **Edge cases:** the mouse position is outside the window area when `mx < left`, `my < top`,
  `mx ≥ left + iw` or `my ≥ top + ih`. The pointer index is then -999.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

---

## 4. The drag

The rules of this section apply in each menu screen. A menu states which entries permit a drag
(`inventory-transactions.md`, ITX-JAV-017, last condition). Each entry of a generic menu permits
it (CCS-MNU-004).

### CCS-DRG-001: The start of a drag

- **Inputs:** a mouse press while the cursor stack is not the empty stack and no drag is open.
- **Behavior:** the press opens a drag with an empty drag set when two conditions are true:
  - the button is in the table;
  - the pointer is not in the empty area (`client-inventory.md`, CINV-GES-002, step 1).

  The press sends no click.

| Button | Drag kind | Drag button |
|--------|-----------|-------------|
| left | 0, an equal part for each slot | 0 |
| right | 1, one item for each slot | 1 |
| the pick key, and the creative flag is on | 2, a full stack for each slot | 2 |

  A press of another button does not open a drag. `client-inventory.md` states what it does.
- **Postconditions:** a press of the left button on an entry or outside the window area opens a
  drag, also when the release later gives one click (CCS-DRG-003).
- **Edge cases:** a press of each button while a drag is open has no effect on the drag.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CCS-DRG-002: A slot joins the drag set

- **Inputs:** a move of the mouse with a button down while a drag is open. The target entry after
  the move is `S`. `C` is the cursor.
- **Behavior:** `S` joins the drag set when each of these conditions is true:
  1. `C` holds a stack;
  2. the drag kind is 2, or `count(C)` is larger than the number of entries in the drag set;
  3. `S` holds the empty stack, or `same(S, C)` is true and `count(S)` is not larger than
     `max(C)`;
  4. `place(S, C)` is true;
  5. the menu permits a drag to `S`;
  6. the press that opened the drag was not a double click (CCS-DBL-001);
  7. `S` is not in the drag set.

  The notation is that of `inventory-transactions.md`, section 0.5. The stacks are those that
  the client shows.
- **Postconditions:**
  - A press and a release with no move between them add no entry. The slot of the press joins
    the drag set only at the first move, when the mouse is still on it.
  - With drag kind 0 or 1, the drag set has at most `count(C)` entries.
  - The conditions are those of ITX-JAV-017. When the stacks of the client equal those of the
    server, each entry of the drag set also enters the set of the server.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CCS-DRG-003: The end of a drag

- **Inputs:** a release of the drag button while a drag is open, with the drag set `D`, when
  the checks 1 to 3 of CCS-DRG-004 are false.
- **Behavior:**
  1. When `D` has one or more entries, the client sends these clicks of the click kind quick
     craft (`inventory-transactions.md`, ITX-JAV-014 and ITX-JAV-015), in this order:
     1. the start: slot index -999, stage 0, the drag kind;
     2. for each entry of `D`: the menu index of the entry, stage 1, the drag kind;
     3. the end: slot index -999, stage 2, the drag kind.
  2. When `D` is empty, the table of `client-inventory.md`, CINV-GES-002, step 5, gives the click
     of the release.
  3. The drag closes.
- **Postconditions:**
  - The client sends all clicks of a drag at the release, in one frame.
  - A drag of kind 0 or 1 with one entry gives the result of a pick up click on that entry. A
    drag of kind 2 with one entry gives no result on the server (ITX-JAV-018).
  - The client predicts the result of each click as `client-inventory.md` states.
- **Edge cases:** the order of the stage 1 clicks is unspecified. The server gives the same result
  for each order (ITX-JAV-019). The order is not an observable result.

Provenance: `source_suggested`. Kind: wire; unspecified for the order of the stage 1 clicks.
Confidence: medium.

### CCS-DRG-004: The cancel of a drag

- **Inputs:** a mouse release of the button `b` in a menu screen.
- **Behavior:** the client does these checks in this order. The first check that is true gives
  the result of the release:
  1. The double click: the last mouse press was a double click, and `b` is the left button.
     CCS-DBL-002 or CCS-DBL-003 then gives the result, unless its step 1 or step 2 continues
     the release. The open drag closes.
  2. The cancel: a drag is open, and `b` is not the drag button. The drag closes with an empty
     drag set. The client sends no click.
  3. The skip: the release is the first release after one of these events, with no mouse press
     between them. The client sends no click.
     - a cancel of check 2;
     - a press that sent a click (`client-inventory.md`, CINV-GES-001);
     - the open of the screen (`client-inventory.md`, CINV-GES-002, postconditions).
  4. Else CCS-DRG-003 gives the result when a drag is open. With no open drag,
     `client-inventory.md`, CINV-GES-002, step 5, gives it.
- **Postconditions:**
  - A cancel does not stop a double click, because check 1 comes before checks 2 and 3. Only the
    next mouse press changes the double click (CCS-DBL-001).
  - Example: a player double clicks a stone stack with the left button. The first press picks up
    the stack, and the second press opens a drag. The player presses the right button, releases
    it, and releases the left button. These three events send no click. The press of the right
    button is not a double click, so check 1 is false at the last release.
  - Example: a player presses the right button in the empty area and holds it down. The player
    then double clicks a stone stack with the left button. The player releases the right button,
    and the drag closes. The release of the left button on the stack sends pick up all.
  - A release of the right button never gives a pick up all.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CCS-DRG-005: The preview in an entry of the drag set

- **Inputs:** a drag of kind `k` with a drag set `D` of `z` entries, `z` ≥ 2. `C` is the cursor.
- **Behavior:** in each frame, for each entry `X` of `D`:
  1. When `C` holds the empty stack, or the condition 3 or 5 of CCS-DRG-002 is false for `X`,
     `X` leaves `D`. `X` then draws its own stack.
  2. Else `a` is `floor(count(C) / z)` for `k` = 0, 1 for `k` = 1, and `max(C)` for `k` = 2.
     `m` is `cap(X, C)`. The preview count `p` is `count(X) + a`, but not more than `m`.
  3. The screen fills the entry area of `X` with `80FFFFFF`. It then draws a stack with the item
     and the components of `C` and the count `p`, in place of the stack of `X`.
  4. When `count(X) + a` is larger than `m`, the count text of the stack is `m` in yellow
     (`FFFFFF55`). The count text shows also when `m` is 1.
- **Postconditions:**
  - With `z` = 1, no entry shows a preview.
  - The preview changes no slot. The slots change at the end (CCS-DRG-003).
  - Example: the cursor holds 10 stone, kind 0, and three empty entries. Each entry shows 3
    stone.

Provenance: `source_suggested`. Kind: layout. Confidence: medium.

### CCS-DRG-006: The count of the cursor during a drag

- **Inputs:** a drag of kind `k` with a drag set `D` of `z` entries. `C` is the cursor, and it
  holds a stack.
- **Behavior:** the stack at the mouse (CCS-LAY-006) has the item and the components of `C`. Its
  count `r` is the first of these that applies:
  1. When `z` is 0 or 1, `r` is `count(C)`. This is true for each kind.
  2. When `k` is 2, `r` is `max(C)`. The entries of `D` and their stacks have no effect on `r`.
  3. When `k` is 0 or 1, `r` is `count(C)` minus the sum of `p(X) − count(X)` over the entries
     `X` of `D`. `p(X)` is the preview count of CCS-DRG-005.

  When `r` is 0 or less, nothing draws at the mouse: no item and no count text. Else the count
  text shows `r` when `r` is not 1. The count text has the usual form of a slot (section 0.5). The
  yellow text of CCS-DRG-005 does not apply to the stack at the mouse.
- **Postconditions:**
  - The cursor does not change before the release. Only the stack at the mouse shows `r`.
  - For `k` = 0 or 1, `r` is the count of the cursor after the release, when the stacks of the
    client equal those of the server (ITX-JAV-018).
  - For `k` = 2, `r` does not show the result of the release. ITX-JAV-020 states that result. With
    two empty entries, the cursor holds the empty stack after the release.
  - Example: the cursor holds 10 stone, kind 0, and three empty entries. The mouse shows 1 stone.
  - Example: the cursor holds 4 stone, kind 0, and two empty entries. Nothing draws at the mouse.
  - Example: the creative flag is on, and the cursor holds 10 stone, kind 2, two empty entries.
    The mouse shows 64 stone. With one entry, the mouse shows 10 stone.
  - Example: the cursor holds 1 diamond sword, kind 2, and two empty entries. The mouse shows the
    sword with no count text, because `max(C)` is 1.
- **Edge cases:** the vanilla client computes `r` only when an entry joins `D` or leaves it. When
  the server changes `C` and `D` does not change, the vanilla client keeps the old `r`. This rule
  does not specify `r` in that case.

Provenance: `source_suggested`. Kind: layout; unspecified for a change of `C` with no change of
`D`. Confidence: medium.

### CCS-DRG-007: Shift and a drag

- **Behavior:**
  1. Shift has no effect on the start of a drag, on the drag set or on the clicks of
     CCS-DRG-003, step 1.
  2. When the drag set is empty at the release, CCS-DRG-003, step 2, gives one click. With Shift
     down and the mouse position in the window area, that click is a quick move at the pointer
     index. The pick key is the exception: its release gives a clone (CINV-GES-002, row 1).
     Outside the window area, Shift has no effect.
- **Postconditions:** a player who holds a stack, presses Shift and moves over three slots
  paints the three slots. The quick move occurs only when the player does not move to a second
  slot.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

---

## 5. The double click

### CCS-DBL-001: A double click

- **Inputs:** a press of a mouse button in a menu screen.
- **Behavior:** the press is a double click when each of these conditions is true:
  1. less than 250 ms passed since the last press that the screen took;
  2. that press had the same button;
  3. that press was in the same screen;
  4. the target entry of that press is the target entry of this press, or neither press had a
     target entry.
- **Postconditions:**
  - A third press within 250 ms of the second press is also a double click.
  - Each mouse press in the screen sets the double click again. A press of another button is not
    a double click (condition 2). A release does not change the double click, and a cancel of
    CCS-DRG-004 does not change it.
  - The double click acts at a release of the left button, in check 1 of CCS-DRG-004 (CCS-DBL-002,
    CCS-DBL-003).
  - `client-widget.md`, WID-PTR-002, states the double click of the other screens.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CCS-DBL-002: Pick up all with a double click

- **Inputs:** a release of the left button after a double click, with Shift up. `S` is the
  target entry at the release.
- **Behavior:**
  1. When `S` does not exist, or the menu does not permit pick up all from `S`, the release
     continues as a release without a double click.
  2. Else the client sends one click on `S` with the click kind pick up all and the button 0.
     The click has no other condition in the client. ITX-JAV-011 states its result.
  3. The open drag then closes with no click. CCS-DRG-003 does not apply to this release.

  CCS-DRG-004 states the order of the checks. This check is the first, so a cancel before the
  release does not stop it.
- **Postconditions:**
  - In normal play, the first press picks up the stack of `S`. The second press opens a drag
    with that cursor stack. The release then gathers the stacks with the same item and
    components of the menu into the cursor.
  - The first press with an empty cursor also sends its own click (`client-inventory.md`).

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

### CCS-DBL-003: Shift with a double click

- **Inputs:** a release of the left button after a double click, with Shift down. `S` is
  the target entry at the release. The remembered stack is `T`.
- **Behavior:**
  1. The remembered stack changes at each quick move click that the screen sends on an entry.
     It becomes a copy of the stack of that entry before the click, or the empty stack when the
     entry holds the empty stack.
  2. When `S` does not exist, the release continues as a release without a double click. The
     same occurs when the menu does not permit pick up all from `S`.
  3. When `T` is the empty stack, the client sends no click.
  4. Else the client examines each entry `X` of the menu, in the order of the menu index. It
     sends one quick move click on `X` with the button 0 when each of these conditions is true:
     - `take(X)` is true;
     - `X` and `S` are slots of the same holder;
     - `X` holds a stack with the item and the components of `T`, and `count(X)` is not larger
       than `max(T)`.
  5. Each quick move click of step 4 also sets the remembered stack (step 1).
- **Postconditions:**
  - In a generic menu, the holder of `S` is the container or the player inventory. When `T` is
    not empty, a Shift double click on a slot of the player part moves the stacks with the item
    and the components of `T` from the player part to the container part.
  - With an empty cursor, the first press is a quick move of `S`. The second press is also a
    quick move of `S`, and it sets `T` to the stack that `S` then holds. When the first quick move
    emptied `S`, `T` is empty and step 3 applies.
- **Edge cases:** the conditions of step 4 read the stacks that the client shows before the first
  click of step 4. Each click then has its result on the server as ITX-JAV-009 states.

Provenance: `source_suggested`. Kind: client agreement. Confidence: medium.

---

## 6. The item tooltip

### 6.1 The request

### CCS-TIP-001: When an entry shows an item tooltip

- **Inputs:** the target entry `S` of a menu screen and the cursor `C`.
- **Behavior:**
  1. The entry makes a tooltip request when `S` holds a stack and `C` holds the empty stack.
  2. The request is the request of a widget with the delay 0 (`client-widget.md`, WID-TIP-001).
     The bounds of the widget are the entry area of `S`.
  3. The text of the request is the parts of CCS-TIP-002, with a line break between two parts.
     The text wraps at the width 170 (WID-TIP-002). Each wrapped row is a line of
     `client-tooltip.md`.
  4. The request has the gap after the first line (`client-tooltip.md`, TIP-BOX-001). This is an
     exception to WID-TIP-002, step 3.
  5. The position is that of WID-TIP-003, with `bh` = 16.
  6. When CCS-TIP-002 gives no part, the entry makes no request.
- **Postconditions:**
  - A slot with the empty stack makes no request in a generic menu.
  - While the cursor holds a stack, the target entry shows no item tooltip.
  - A long lore part wraps at 170 GUI units. Each wrapped row is a line of TIP-BOX-001.

Provenance: `source_suggested`. Kind: semantic; layout for the wrap and the position.
Confidence: medium.

### 6.2 The parts

### CCS-TIP-002: The order of the parts

- **Inputs:** a stack, the option advanced tooltips, the local player, and the enabled features of
  the world. `TD` is the effective component `minecraft:tooltip_display` of the stack, or the
  default of STK-SHP-017 when the stack has none.
- **Behavior:**
  1. When the `hide_tooltip` of `TD` is true, the stack has no part. A creative tooltip is the
     exception: it ignores `hide_tooltip`.
  2. Else the parts come from the groups of the table, in this order. A group of a component type
     gives parts only when the stack has that effective component and `hidden_components` of `TD`
     does not hold the component type.

| # | Group | Rule |
|---|------|------|
| 1 | the name | CCS-TIP-003 |
| 2 | the parts that the item adds for itself | out of scope |
| 3 | `tropical_fish/pattern`, `instrument` | out of scope |
| 4 | `map_id` | CCS-TIP-007 |
| 5 | `bees`, `container_loot`, `container`, `banner_patterns`, `pot_decorations`, `written_book_content`, `charged_projectiles`, `fireworks`, `firework_explosion`, `potion_contents`, `jukebox_playable`, `trim`, `stored_enchantments` | out of scope |
| 6 | `enchantments` | CCS-TIP-004 |
| 7 | `dyed_color` | out of scope |
| 8 | `profile` | CCS-TIP-007 |
| 9 | `lore` | CCS-TIP-005 |
| 10 | `sulfur_cube_content` | out of scope |
| 11 | `attribute_modifiers` | CCS-TIP-006 |
| 12 | `intangible_projectile` | out of scope |
| 13 | `unbreakable` | CCS-TIP-007 |
| 14 | `sign_text_front`, `sign_text_back`, `ominous_bottle_amplifier`, `suspicious_stew_effects`, `block_state`, `entity_data`, the spawn data of `block_entity_data`, `can_break`, `can_place_on` | out of scope |
| 15 | the parts of the advanced tooltips | CCS-TIP-008 |
| 16 | the part of a disabled item | CCS-TIP-008 |
| 17 | the warning for an operator item | out of scope |

  Each identifier in the table has the namespace `minecraft`.
- **Postconditions:** a stack of `minecraft:stone` with no patch has one part, the name, when
  the advanced tooltips are off.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CCS-TIP-003: The name

- **Behavior:**
  1. The text of the name is the first value of this list:
     - the effective component `minecraft:custom_name`;
     - for the item `minecraft:player_head` with an effective `minecraft:profile` that has a player
       name: the translation `block.minecraft.player_head.named` with that name as its argument;
     - the effective component `minecraft:item_name`;
     - the empty text.
  2. The rarity is the effective `minecraft:rarity` (STK-SHP-014), or `common` when the stack has
     none. When the effective `minecraft:enchantments` has one or more entries, `common` and
     `uncommon` become `rare`, and `rare` becomes `epic`.
  3. The part is a text with the colour of the rarity and the name as its one child. With a
     `minecraft:custom_name`, the part also has the italic style.

| Rarity | Colour |
|--------|--------|
| `common` | white, `FFFFFFFF` |
| `uncommon` | yellow, `FFFFFF55` |
| `rare` | aqua, `FF55FFFF` |
| `epic` | light purple, `FFFF55FF` |

- **Postconditions:**
  - A style of the name itself wins over the colour and the italic style of the part.
  - An enchanted stone shows its name in aqua.
- **Edge cases:** the name of a stack with `minecraft:written_book_content` is out of scope.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CCS-TIP-004: The enchantments

- **Inputs:** the effective `minecraft:enchantments`, the registry `minecraft:enchantment` and its
  tags, as the server sent them.
- **Behavior:**
  1. First, for each enchantment of the tag `minecraft:tooltip_order`, in the order of the tag:
     one part when the stack has that enchantment with a level above 0.
  2. Then one part for each other enchantment of the component. The order of these parts is
     unspecified.
  3. A part is the `description` of the enchantment. When the enchantment is in the tag
     `minecraft:curse`, the part has the colour red, `FFFF5555`. Else it has gray, `FFAAAAAA`.
     A colour of the description itself wins.
  4. When the level is not 1, or the `max_level` of the enchantment is not 1, the part continues
     with one space and the translation `enchantment.level.<level>`.
- **Postconditions:** Sharpness at level 5 shows `Sharpness V` in `en_us`. Mending at level 1
  shows `Mending`, because its maximum level is 1. A level with no translation shows the
  translation key.
- **Edge cases:** when the client has no tag `minecraft:tooltip_order`, step 2 gives all parts.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CCS-TIP-005: The lore

- **Behavior:** each text of the effective `minecraft:lore` is one part, in the order of the list.
  The default style of a part is dark purple, `FFAA00AA`, and italic. A style of the text itself
  wins over the default style.
- **Postconditions:** a lore text with a line break, or a lore text wider than 170, gives more
  than one line (CCS-TIP-001).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CCS-TIP-006: The attribute modifiers

- **Inputs:** the entries of the effective `minecraft:attribute_modifiers` (STK-SHP-016), and the
  base values of the attributes of the local player (`attributes.md`).
- **Behavior:**
  1. For each slot group in the order of the list "slot group" of STK-SHP-003, from `any` to
     `saddle`, take the entries whose `slot` is that slot group. Skip each entry with the display
     type `hidden`.
  2. Before the first such entry of a slot group, add an empty part and the translation
     `item.modifiers.<slot group>` in gray.
  3. An entry with the display type `override` adds its `value` text.
  4. An entry with the display type `default` adds the part of the table below, or no part. `v`
     is the `amount` of the entry. For the `id` `minecraft:base_attack_damage`, `v` is the amount
     plus the base value of `minecraft:attack_damage` of the player. For the `id`
     `minecraft:base_attack_speed`, it is the amount plus the base value of
     `minecraft:attack_speed`.
  5. The shown number `d` is `100·v` for the operations `add_multiplied_base` and
     `add_multiplied_total`. It is `10·v` for `minecraft:knockback_resistance` with `add_value`.
     It is `v` in each other case. `d` has at most two digits after the point, with the point
     `.` and no group separator.

| Case | Part | Colour |
|------|------|--------|
| the `id` is one of the two base ids | one space, then `attribute.modifier.equals.<op>` with `d` and the attribute name | dark green, `FF00AA00` |
| `v` > 0 | `attribute.modifier.plus.<op>` with `d` and the attribute name | the colour of an increase |
| `v` < 0 | `attribute.modifier.take.<op>` with `−d` and the attribute name | the colour of a decrease |
| `v` = 0 | no part | |

  `<op>` is the numeric id of the operation (STK-SHP-003): 0 for `add_value`, 1 for
  `add_multiplied_base` and 2 for `add_multiplied_total`. The attribute name is the translation
  `attribute.name.<name>`, where `<name>` is the path of the attribute entry.

| Attributes | Colour of an increase | Colour of a decrease |
|------------|-----------------------|----------------------|
| `minecraft:burning_time`, `minecraft:fall_damage_multiplier` | red, `FFFF5555` | blue, `FF5555FF` |
| `minecraft:gravity`, `minecraft:scale`, `minecraft:waypoint_transmit_range`, `minecraft:waypoint_receive_range` | gray, `FFAAAAAA` | gray, `FFAAAAAA` |
| each other attribute | blue, `FF5555FF` | red, `FFFF5555` |

- **Postconditions:** an iron sword shows an empty part, `When in Main Hand:`, ` 6 Attack Damage`
  and ` 1.6 Attack Speed` in `en_us`. The base values of the player are 1 and 4.
- **Edge cases:** the modifiers that an enchantment adds through its effects are out of scope.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CCS-TIP-007: Map, profile and unbreakable

- **Behavior:**
  - `minecraft:map_id`: when the client holds no map data for the id, the part is the
    translation `filled_map.unknown` in gray. Else the group gives these parts, in this order:
    1. `filled_map.id` with the id, in gray, only when the stack has no `minecraft:custom_name`;
    2. `filled_map.locked` in gray, only when the map data is locked;
    3. with the advanced tooltips only: `filled_map.scale` with `2^s`, and `filled_map.level`
       with `s` and 4, both in gray. `s` is the scale of the map data.
  - `minecraft:profile`: one part, the translation `component.profile.dynamic` in gray, for a
    dynamic profile (`item-stack.md`, section 8.2). Each other profile adds no part.
  - `minecraft:unbreakable`: one part, the translation `item.unbreakable` in blue, `FF5555FF`.
- **Edge cases:** the map data comes from the `map_item_data` packet. Its rules, and the effect of
  `minecraft:map_post_processing`, are out of scope.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

### CCS-TIP-008: The advanced parts and a disabled item

- **Behavior:**
  1. When the advanced tooltips are on, the stack adds these parts, in this order:
     1. `item.durability` with `max_damage − damage` and `max_damage`, with no colour, only when
        the stack is damaged (STK-DMG-004) and `hidden_components` does not hold
        `minecraft:damage`;
     2. the identifier of the item as literal text, for example `minecraft:stone`, in dark gray
        (`FF555555`);
     3. `item.components` with the number of effective components of the stack, in dark gray, only
        when that number is above 0.
  2. When the enabled features of the world do not enable the item, the stack adds the translation
     `item.disabled` in red, `FFFF5555`.
- **Postconditions:** the durability part is white. `minecraft:stone` with no patch shows
  `minecraft:stone` and `N component(s)`, where `N` is the
  number of components of its prototype.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium.

---

## 7. Known vanilla quirks

None.

---

## 8. Intentional deviations

None.

---

## 9. Unresolved

These experiments have not run. Each uses a vanilla client of the version of
`assets/minecraft/version.json`, recorded by `tools/mcrs_minecraft_client_recorder` against
`tools/vanilla-server`. The recorder logs each packet. A screenshot is one that the screenshot
key of the client takes at the GUI scale 2.

- **E1.** Open a chest, a double chest, a dispenser and a hopper. Take a screenshot of each
  screen. Read the stacks of `container_set_content`. Expect CCS-MNU-001, CCS-MNU-004 and
  CCS-LAY-001 to CCS-LAY-004.
- **E2.** Quick move a stack from each part of each generic menu, with a full container part and
  with an empty one. Expect CCS-MNU-002 and CCS-MNU-003.
- **E3** (proxy). Send `open_screen` while the chat screen is open, and while a chest screen is
  open. Expect CCS-OPN-001.
- **E4.** Hold 10 stone. Drag with the left button over three empty slots. Take a screenshot
  before the release. Read the clicks after the release. Repeat with 4 stone over two slots. In
  creative mode, repeat with the pick key and 10 stone over one slot and over two slots. Expect
  CCS-DRG-001 to CCS-DRG-003, CCS-DRG-005, CCS-DRG-006, CCS-LAY-005 and CCS-LAY-006.
- **E5.** Hold 64 stone. Drag with the right button over a slot with 63 stone and an empty slot.
  Take a screenshot before the release. Expect CCS-DRG-002 and CCS-DRG-005.
- **E6.** Start a drag with the left button. Press and release the right button, then release the
  left button. Then hold the right button down in the empty area, double click a stone stack with
  the left button, and release the right button and then the left button. Expect CCS-DRG-004,
  CCS-DBL-001 and CCS-DBL-002.
- **E7.** Hold a stack, press Shift and drag over one slot, then over three slots. Expect
  CCS-DRG-007.
- **E8.** Double click a stack of 20 stone with three other stone stacks in the menu, at 200 ms
  and at 300 ms. Expect CCS-DBL-001 and CCS-DBL-002.
- **E9.** With an empty cursor, and then with a stone on the cursor, Shift double click a stone
  stack in the player part. Expect CCS-DBL-003.
- **E10.** Hover stacks with a custom name, lore, enchantments, attribute modifiers,
  `unbreakable`, a filled map and a player head, with the advanced tooltips off and on. Take a
  screenshot of each tooltip. Expect CCS-TIP-001 to CCS-TIP-008.
- The input agent of `tools/vanilla-client` sets only the movement keys, jump, sneak, sprint,
  use and the rotation. It cannot press a mouse button or a key in a screen, and it cannot take
  a screenshot. Until it can, a person plays the recorded client for E1 to E10.
