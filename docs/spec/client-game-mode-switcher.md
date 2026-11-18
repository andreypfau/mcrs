# The game mode switcher of the client: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states the entries of the game mode switcher, the item icon of each entry, and the
layout of the switcher: the background, the two lines of text, the slots, the selection and the
icons. The document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **GUI unit**, **W**, **H**: as `client-chat.md`, section 0.1, defines them.
- **Screen**, **in-game screen**: as `client-widget.md`, section 0.1, defines them.
- **Stack**, **item**, **count**: as `item-stack.md`, section 0.1, defines them.
- **GUI sprite**: as `client-gui-sprite.md`, section 0.1, defines it.
- **Debug modifier**: the key binding `key.debug.modifier`. Its default is F3.
- **Switch key**: the key binding `key.debug.switchGameMode`. Its default is F4.
- **Switcher**: the screen that the debug modifier and the switch key open together, for a player
  who can change the game mode. The language key of its name is `key.debug.switchGameMode`.
- **Entry**: one of the four game modes that the switcher shows (CGMS-ENT-001).
- **Selected entry**: the one entry that the switcher marks. When the switcher closes, the
  client asks the server for the game mode of the selected entry.
- **Icon stack**: the stack that an entry shows (CGMS-ENT-001).
- **Centre**: the point (`cx`, `cy`) with `cx = W / 2` and `cy = H / 2`. The divisions are
  integer divisions that round down.
- **Centred line**: a line of text with the width `w` and an anchor (`x`, `y`). The line starts
  at the left edge `x − w / 2`, with an integer division that rounds down. Its top edge is `y`.

### 0.2 Provenance, kind and observer

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The language keys and their English texts are in
`assets/minecraft/lang/en_us.json` of the client jar. The textures and the GUI sprites are in
the resource pack of the client jar.

The observer of each rule is a player in normal play who sees the switcher. A screenshot at a
known GUI scale tests each rule of the kind layout.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CGMS-ENT` | the entries, their order and their icon stacks |
| `CGMS-LAY` | the layout of the switcher |

### 0.4 Scope

In scope:

- the four entries, their order, their names and their icon stacks;
- the background, the text lines, the slots, the selection and the position of each icon.

Out of scope:

- the keys that open, cycle and close the switcher, and the selected entry when the switcher
  opens;
- the mouse over a slot;
- the packet that the client sends when the switcher closes. `command-gamemode.md`, CMD-GM-003,
  states the change on the server;
- the permission that the switcher needs: `command-permissions.md`, PERM-LEVEL-003;
- the draw of an item model.

### 0.5 Constants

| Value | Data | Rule |
|-------|------|------|
| Number of entries | 4 | CGMS-ENT-001 |
| Count of an icon stack | 1 | CGMS-ENT-002 |
| Size of the background | 125 × 75, from a texture of 128 × 128 | CGMS-LAY-001 |
| Size of a slot | 26 × 26 | CGMS-LAY-003 |
| Distance from the left edge of a slot to the left edge of the next slot | 31 | CGMS-LAY-003 |
| Position of the icon in its slot | (5, 5) | CGMS-LAY-003 |
| Size of the icon area | 16 × 16 | CGMS-LAY-003 |
| Colour of the two text lines | `FFFFFFFF`, with the drop shadow | CGMS-LAY-002 |
| Colour of the key name in the hint line | `FF55FFFF` | CGMS-LAY-002 |

---

## 1. The entries

### CGMS-ENT-001: The four entries

- **Behavior:** the switcher shows these four entries, in this order:

| Index | Game mode | Name key | English name | Item of the icon stack |
|-------|-----------|----------|--------------|------------------------|
| 0 | creative | `gameMode.creative` | Creative Mode | `minecraft:grass_block` |
| 1 | survival | `gameMode.survival` | Survival Mode | `minecraft:iron_sword` |
| 2 | adventure | `gameMode.adventure` | Adventure Mode | `minecraft:buried_treasure_map` |
| 3 | spectator | `gameMode.spectator` | Spectator Mode | `minecraft:ender_eye` |

- **Postconditions:**
  - The index is the position of the slot from left to right (CGMS-LAY-003).
  - The switch key moves the selected entry to the entry with the next index. The entry after
    the index 3 is the index 0.
  - The switcher shows each of the four entries, also the entry of the current game mode of the
    player.
  - The entries and their items do not depend on the data pack.

Provenance: `source_suggested`. Kind: layout. Confidence: high. Experiment: as an operator, press
F3 + F4, then press F4 three times. Each press moves the selection one slot to the right, and the
icons are a grass block, an iron sword, a map and an eye of ender.

### CGMS-ENT-002: The icon stack

- **Behavior:** the icon stack of an entry is a stack of the item of CGMS-ENT-001, with the count
  1 and no component patch. Its components are the default components of the item.
- **Postconditions:**
  - The icon shows no count and no damage bar.
  - The icon stack is not in an inventory. The player cannot take it or move it.

Provenance: `source_suggested`. Kind: layout. Confidence: high.

---

## 2. The layout

### CGMS-LAY-001: The background

- **Behavior:**
  1. The switcher is not an in-game screen. It draws its own background in place of the
     background of `client-widget.md`, WID-SCR-004. It draws no gradient, no blur and no menu
     background texture.
  2. The background copies the source rectangle (0, 0, 125, 75) of the texture
     `minecraft:textures/gui/container/gamemode_switcher.png` to the destination
     (`cx − 62`, `cy − 58`). One texel is one GUI unit, and the texture is 128 × 128 texels.
- **Postconditions:** the world stays visible around the background.

Provenance: `source_suggested_black_box_confirmed` (the texture of the client jar is 128 × 128
pixels). Kind: layout. Confidence: high.

### CGMS-LAY-002: The text lines

- **Behavior:** the switcher draws two centred lines in `FFFFFFFF` with the drop shadow:

| Line | Text | Anchor |
|------|------|--------|
| name line | the English name of the selected entry (CGMS-ENT-001) | (`cx`, `cy − 51`) |
| hint line | `debug.gamemodes.select_next` ("%s Next"). The argument is the name of the key of the switch key, in `FF55FFFF`. | (`cx`, `cy + 5`) |

- **Postconditions:** with the default switch key, the argument is `key.keyboard.f4` ("F4"),
  and the hint line is "F4 Next".

Provenance: `source_suggested`. Kind: layout. Confidence: high.

### CGMS-LAY-003: The slots, the selection and the icons

- **Behavior:**
  1. The entry with the index `i` has a slot at (`cx − 59 + 31·i`, `cy − 31`), with the size
     26 × 26.
  2. Each slot draws the GUI sprite `minecraft:gamemode_switcher/slot` over the slot.
  3. The slot of the selected entry then draws the GUI sprite
     `minecraft:gamemode_switcher/selection` over the same rectangle.
  4. The icon area of the entry is 16 × 16. Its top left corner is 5 GUI units right of the left
     edge of the slot and 5 GUI units below the top edge of the slot.
  5. The client draws the icon stack of the entry in its icon area, as it draws a stack in a slot
     of a menu screen.
  6. The order of draw is the background, the two text lines, and then each entry in the order of
     its index. For each entry, the order is the slot sprite, the selection sprite and the icon.
- **Postconditions:**
  - The left edges of the four slots are `cx − 59`, `cx − 28`, `cx + 3` and `cx + 34`. The
    gap between two slots is 5 GUI units.
  - The four slots cover 119 GUI units, from `cx − 59` to `cx + 60`. They are inside the
    background.
  - The icon is in the middle of its slot: 5 + 16 + 5 = 26.
  - The icon of the selected entry draws over the selection sprite.

Provenance: `source_suggested_black_box_confirmed` (each of the two GUI sprites of the client
jar is 26 × 26 pixels). Kind: layout. Confidence: high. Experiment: at the GUI scale 2, open the
switcher and take a screenshot. Measure the slot edges and the icon edges against the centre of
the window.

---

## 3. Known vanilla quirks

None.

---

## 4. Intentional deviations

None.

---

## 5. Unresolved

None.
