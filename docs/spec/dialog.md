# Server dialogs: behavioural specification

This document states the dialogs that a server shows to a client. It states the registry
`minecraft:dialog` and its tags, the packets that show and clear a dialog, the packet of a
custom click action, and the dialog screen of the client with its controls and its follow-up
screens. An implementer can build the data model, the wire, the server message and the client
screen from it. The last section states the widget kinds that a dialog needs. The
document does not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **vanilla client**, **clientbound**,
  **serverbound**: as `item-stack.md`, section 0.1, defines them.
- **Network NBT**, **item template**: as `item-stack.md`, section 0.1, defines them.
- **Text**: a text component as `text-component.md` states it.
- **Click event**, **hover event**: as `text-component.md`, section 1.3, defines them.
- **Screen**, **widget**, **focused**, **activation**, **activation key**, **caption**,
  **tab order group**, **last input device**, **layout**, **cell**, **gap**: as
  `client-widget.md`, section 0.1, defines them.
- **Scroll panel**, **wheel step**: as `client-list.md`, section 0.1, defines them.
- **GUI unit**, **W**, **H**: as `client-chat.md`, section 0.1, defines them.
- **Dialog**: one value of the registry `minecraft:dialog`, or one inline value of the same
  form. A dialog has a type and the common fields (DLG-DATA-003).
- **Inline dialog**: a dialog that a packet or a text holds as a value, not as a registry id.
- **Dialog screen**: the screen of the client that shows one dialog.
- **Previous screen**: the screen, or no screen, that a dialog screen shows when it closes. It
  is the parent screen of `client-widget.md` for a dialog screen.
- **Action button**: a button of a dialog with a label, a width, an optional tooltip and an
  optional action (DLG-DATA-006).
- **Action**: a value that gives a click event when a player activates its button
  (DLG-ACT-001).
- **Cancel action**: the action that a dialog runs when the player closes it with Escape
  (DLG-DATA-013).
- **After action**: the field `after_action`. It selects the screen that shows after an action
  (DLG-FLOW-001).
- **Target screen**: the screen that the after action selects. A click event shows it at its end.
- **Input**: one entry of `inputs`. It has a key and a control.
- **Control**: the widget that shows an input and holds its value (section 9).
- **Template**: the string of a `dynamic/run_command` action, with variables (DLG-ACT-002).
- **Variable**: the part `$(name)` of a template.
- **Template value**: the string that an input gives to a variable (DLG-ACT-005).
- **Tag value**: the NBT tag that an input gives to a custom payload (DLG-ACT-005).
- **Button-list type**: the dialog types `multi_action`, `server_links` and `dialog_list`.
- **Simple type**: the dialog types `notice` and `confirmation`.
- **Server link**: one entry of the packet `server_links` (DLG-LINK-001).
- **Waiting screen**: the screen that shows while the client waits for the server
  (DLG-WAIT-001).
- **Warning screen**: the screen that explains custom screens (DLG-WARN-001).
- **Confirm screen**: a screen with a title, a message and a row of answer buttons
  (DLG-CONF-001).
- **Local world**: a world of singleplayer that is not open to LAN.
- **Configuration phase**, **play phase**: the protocol states `configuration` and `play`.
- **Code unit**: one UTF-16 code unit. A length of a string is a number of code units.

### 0.2 Provenance and confidence

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The ids, the JSON keys, the translation keys, the English
texts and the sprite names come from the reports of the game and from the client jar. They are
artefacts. The behaviour comes from a read of the reference. A rule that names an experiment
of section 16 has that experiment as its test. A rule with the label `source_suggested` cites
no artefact.

A fixture of a load holds the error cases only. The feedback for a valid dialog is "No player
was found", and that feedback holds no field. Such a fixture shows no default and no valid
value. A rule with the label `source_suggested_black_box_confirmed` is confirmed for the claims
that its fixtures show. Each claim that no fixture shows is `source_suggested`, and item 25 of
section 17 lists these claims.

### 0.3 Text and arithmetic

- The font, the width of a text, the line height 9 and the drop shadow are as `client-chat.md`,
  section 0.3, states. To wrap a text at a width is as `client-tooltip.md`, section 0.3, states.
- A colour is a 32-bit ARGB value in hexadecimal.
- `trunc`, `floor`, `round`, `clamp` and `lerp` are as `client-widget.md`, section 0.3, states.
- `pos(v, p, q)` is `(v − p) / (q − p)`. It is the inverse of `lerp`.
- A "32-bit float" is an IEEE 754 binary32 value. Each real value of section 4 is a 32-bit
  float.
- A key is a physical key position, as `client-chat.md` states for key bindings.
- A sprite name with no namespace has the namespace `minecraft`.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `DLG-DATA` | the registry, the dialog types, the common fields, the action button |
| `DLG-BODY` | the body elements |
| `DLG-IN` | the inputs and their fields |
| `DLG-ACT` | the actions, the templates and the values of the inputs |
| `DLG-TAG` | the dialog tags, the pause menu button and the key "Quick Actions" |
| `DLG-WIRE` | the packets `show_dialog`, `clear_dialog` and `custom_click_action` |
| `DLG-LINK` | the server links and the dialog `server_links` |
| `DLG-SCR` | the layout of a dialog screen and its body widgets |
| `DLG-CTL` | the controls of the inputs |
| `DLG-KEY` | the focus, the keys and Escape |
| `DLG-FLOW` | open, close, nested dialogs, clear, disconnect, configuration phase |
| `DLG-CLK` | the click events of a dialog and their confirm screens |
| `DLG-WAIT` | the waiting screen |
| `DLG-WARN` | the warning screen |
| `DLG-CONF` | the confirm screen |
| `DLG-SRV` | what the server does with a custom click action |
| `DLG-DEV` | intentional deviations |
| `DLG-EXP` | experiments |

### 0.5 Scope

In scope: the data and the behaviour that a server, a data pack and a client see. Out of scope:

- the write forms of the fields, and the fields that a writer leaves out. `registry-fields.md`,
  REG-FLD-400 to REG-FLD-418, states them;
- the fields of each click event and the codec of a text. `text-component.md` states them;
- the generic widgets: button, cycle button, slider, checkbox, one-line edit box, text widgets,
  layouts and scroll panels. `client-widget.md`, `client-list.md` and `client-edit-box.md`
  state them;
- the narration of a screen, except where a rule names a narration key.

---

## 1. The registry and the dialog types

### DLG-DATA-001: The registry `minecraft:dialog`

- **Behavior:**
  1. `minecraft:dialog` is a registry of a data pack. Its entries are in
     `data/<namespace>/dialog/<path>.json`. Its tags are in `data/<namespace>/tags/dialog/`.
  2. The registry is synced to the client in the configuration phase (`registry.md`,
     REG-NET-004).
  3. An unknown field of an entry is ignored (`registry.md`, REG-ENTRY-003).
  4. The field `type` selects the dialog type by its id. The id has the namespace
     `minecraft` when it has none.
- **Postconditions:** the types are, with their numeric ids in the report of registries:

| Type | Id |
|------|----|
| `minecraft:notice` | 0 |
| `minecraft:server_links` | 1 |
| `minecraft:dialog_list` | 2 |
| `minecraft:multi_action` | 3 |
| `minecraft:confirmation` | 4 |

- **Edge cases:** a dialog in NBT or JSON names its type by the id string, never by the number.
  An unknown type is a load error.

Provenance: source_suggested_black_box_confirmed: `assets/mcrs/reports/registries.json`,
registry `minecraft:dialog_type`, and `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`. Kind:
wire. Confidence: medium. Experiment: DLG-EXP-001.

### DLG-DATA-002: Width

- **Behavior:** a field of the kind "width" is an integer from 1 to 1024. A value out of that
  range is a load error. A field of another range states its range.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001.

### DLG-DATA-003: Common fields

- **Inputs:** a dialog object. These fields are next to `type` in the same object.

| Field | Value | Default |
|-------|-------|---------|
| `title` | text | required |
| `external_title` | text | none |
| `body` | one body element, or a list of body elements (section 2) | `[]` |
| `inputs` | a list of inputs (section 3) | `[]` |
| `can_close_with_escape` | Boolean | `true` |
| `pause` | Boolean | `true` |
| `after_action` | `close`, `none` or `wait_for_response` | `close` |

- **Behavior:**
  1. The title shows in the header of the dialog screen (DLG-SCR-002).
  2. The order of `body` and of `inputs` is the order on the screen (DLG-SCR-003).
  3. `can_close_with_escape` is DLG-KEY-002. `pause` is DLG-KEY-003. `after_action` is
     DLG-FLOW-001.
- **Edge cases:** an unknown value of `after_action` is a load error.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001.

### DLG-DATA-004: External title

- **Behavior:** the external title of a dialog is its `external_title`, or its `title` when
  `external_title` is absent.
- **Postconditions:** the external title is the caption of a button that opens the dialog from
  another screen: the pause menu button (DLG-TAG-001) and a button of a `dialog_list`
  (DLG-DATA-011).

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-002.

### DLG-DATA-005: A pause needs an after action that closes

- **Behavior:** a dialog with `pause` = `true` and `after_action` = `none` is a load error.
- **Postconditions:** the error text is "Dialogs that pause the game must use after_action
  values that unpause it after user action!". A pack with this dialog does not load
  (`registry.md`).
- **Edge cases:** `pause` = `true` with `close` or `wait_for_response` is valid. `pause` =
  `false` with each value is valid.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001.

### DLG-DATA-006: Action button

- **Inputs:**

| Field | Value | Default |
|-------|-------|---------|
| `label` | text | required |
| `tooltip` | text | none |
| `width` | width (DLG-DATA-002) | 150 |
| `action` | an action (section 4) | none |

- **Behavior:**
  1. The button is `width` × 20 GUI units. Its caption is `label`.
  2. When `tooltip` is present, it is the tooltip of the button (`client-widget.md`,
     WID-TIP-001).
  3. An activation of the button runs DLG-FLOW-001 with the action.
- **Edge cases:** a button with no `action` runs only the after action.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-002.

### DLG-DATA-007: Type `notice`

- **Inputs:** the field `action`: an action button. The default is a button with the label
  `gui.ok` ("Ok"), the width 150 and no action.
- **Behavior:** the screen shows the button in the footer (DLG-SCR-004).
- **Postconditions:** the cancel action is the action of this button.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-009.

### DLG-DATA-008: Type `confirmation`

- **Inputs:** the fields `yes` and `no`. Each is a required action button.
- **Behavior:** the screen shows `yes`, then `no`, in the footer (DLG-SCR-004).
- **Postconditions:** the cancel action is the action of `no`.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-009.

### DLG-DATA-009: Type `multi_action`

- **Inputs:**

| Field | Value | Default |
|-------|-------|---------|
| `actions` | a list of action buttons, with at least one element | required |
| `exit_action` | an action button | none |
| `columns` | an integer ≥ 1 | 2 |

- **Behavior:**
  1. The buttons of `actions` go into the button grid (DLG-SCR-005) in list order.
  2. `exit_action`, when present, is a button in the footer.
- **Postconditions:** the cancel action is the action of `exit_action`. With no `exit_action`,
  there is no cancel action.
- **Edge cases:** an empty `actions` is a load error. `columns` = 0 is a load error.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001, DLG-EXP-009.

### DLG-DATA-010: Type `server_links`

- **Inputs:**

| Field | Value | Default |
|-------|-------|---------|
| `exit_action` | an action button | none |
| `columns` | an integer ≥ 1 | 2 |
| `button_width` | width (DLG-DATA-002) | 150 |

- **Behavior:**
  1. The client makes one button for each server link that it holds (DLG-LINK-001), in the
     order of the links.
  2. Each button is `button_width` × 20. Its caption is the name of the link (DLG-LINK-002).
     It has no tooltip.
  3. The action of each button is an `open_url` click event with the URL of the link.
  4. The buttons go into the button grid (DLG-SCR-005). `exit_action` is as for
     `multi_action`.
- **Postconditions:** the cancel action is the action of `exit_action`.
- **Edge cases:** with no server link, the grid is empty and the body can be empty.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001,
DLG-EXP-003.

### DLG-DATA-011: Type `dialog_list`

- **Inputs:**

| Field | Value | Default |
|-------|-------|---------|
| `dialogs` | a set of dialogs: a tag `#id`, one id, a list of ids, or inline dialogs | required |
| `exit_action` | an action button | none |
| `columns` | an integer ≥ 1 | 2 |
| `button_width` | width (DLG-DATA-002) | 150 |

- **Behavior:**
  1. The client makes one button for each dialog of `dialogs`, in the order of the set.
  2. Each button is `button_width` × 20. Its caption is the external title of that dialog
     (DLG-DATA-004). It has no tooltip.
  3. The action of each button is a `show_dialog` click event with that dialog.
  4. The buttons go into the button grid. `exit_action` is as for `multi_action`.
- **Postconditions:** the cancel action is the action of `exit_action`.
- **Edge cases:**
  - A tag resolves on the client against the tags that the server synced. An empty tag gives
    an empty grid.
  - An empty list of ids is valid. An id that is not in the registry is a load error.
  - `registry-fields.md`, REG-FLD-405, states the forms that a reader accepts.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001, DLG-EXP-002.

### DLG-DATA-012: Built-in dialogs and tags

- **Behavior:** the vanilla data pack holds these entries. The server must load them as data,
  with each other entry.

| Entry | Type | Fields |
|-------|------|--------|
| `minecraft:server_links` | `server_links` | `title` `menu.server_links.title`, `external_title` `menu.server_links`, `columns` 1, `button_width` 310, `exit_action` with `label` `gui.back` and `width` 200 |
| `minecraft:custom_options` | `dialog_list` | `dialogs` `#minecraft:pause_screen_additions`, `title` `menu.custom_options.title`, `external_title` `menu.custom_options`, layout as `minecraft:server_links` |
| `minecraft:quick_actions` | `dialog_list` | `dialogs` `#minecraft:quick_actions`, `title` `menu.quick_actions.title`, `external_title` `menu.quick_actions`, layout as `minecraft:server_links` |

- **Postconditions:**
  - The tags `minecraft:pause_screen_additions` and `minecraft:quick_actions` exist and are
    empty.
  - The English texts are: "Server Links", "Server Links...", "Custom Options",
    "Custom Options...", "Quick Actions", "Quick Actions...", "Back".

Provenance: black_box_confirmed: `data/minecraft/dialog/server_links.json`,
`data/minecraft/dialog/custom_options.json`, `data/minecraft/dialog/quick_actions.json`,
`data/minecraft/tags/dialog/pause_screen_additions.json`,
`data/minecraft/tags/dialog/quick_actions.json` and `assets/minecraft/lang/en_us.json` of the
client jar, and `assets/mcrs/reports/names.json`. Kind: semantic. Confidence: high.

### DLG-DATA-013: Cancel action

- **Behavior:** the cancel action of each type is:

| Type | Cancel action |
|------|---------------|
| `notice` | the action of `action` |
| `confirmation` | the action of `no` |
| `multi_action`, `server_links`, `dialog_list` | the action of `exit_action`, when present |

- **Edge cases:** a button with no action, or an absent `exit_action`, gives no cancel action.
  Escape then only closes the dialog (DLG-KEY-002).

Provenance: source_suggested. Kind: semantic. Confidence: medium.

---

## 2. Body elements

### DLG-BODY-001: Element `plain_message`

- **Inputs:**

| Field | Value | Default |
|-------|-------|---------|
| `contents` | text | required |
| `width` | width (DLG-DATA-002) | 200 |

- **Behavior:** the element shows the text, wrapped (DLG-SCR-010).
- **Edge cases:** where a `plain_message` is the description of an `item` element, a bare text
  is also accepted. It is a `plain_message` with that text and the width 200.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001.

### DLG-BODY-002: Element `item`

- **Inputs:**

| Field | Value | Default |
|-------|-------|---------|
| `item` | an item template (`item-stack.md`) | required |
| `description` | a `plain_message` object, or a bare text | none |
| `show_decorations` | Boolean | `true` |
| `show_tooltip` | Boolean | `true` |
| `width` | an integer from 1 to 256 | 16 |
| `height` | an integer from 1 to 256 | 16 |

- **Behavior:** the element shows one stack that the client makes from the template
  (DLG-SCR-011). The description, when present, shows right of the stack.
- **Edge cases:** a `width` or a `height` out of its range is a load error.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001.

---

## 3. Inputs

### DLG-IN-001: Input key

- **Inputs:** each input object has the field `key`, a string, and the fields of its control
  type next to `type`.
- **Behavior:**
  1. A key is valid when each code unit is a Unicode letter, a Unicode digit or `_`.
  2. An invalid key is a load error with the text "<key> is not a valid input name".
- **Postconditions:** the control types are, with their numeric ids in the report:

| Type | Id |
|------|----|
| `minecraft:boolean` | 0 |
| `minecraft:number_range` | 1 |
| `minecraft:single_option` | 2 |
| `minecraft:text` | 3 |

- **Postconditions:** the keys `abc_1`, `1a` and `ключ` are valid. The keys `a-b`, `a b` and `a.b`
  are not valid.
- **Edge cases:**
  - The empty key is valid. The variable `$()` reads it (DLG-ACT-002).
  - Two inputs can have the same key (DLG-IN-006).

Provenance: source_suggested_black_box_confirmed: `assets/mcrs/reports/registries.json`,
registry `minecraft:input_control_type`, and `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001.

### DLG-IN-002: Type `boolean`

| Field | Value | Default |
|-------|-------|---------|
| `label` | text | required |
| `initial` | Boolean | `false` |
| `on_true` | string | `"true"` |
| `on_false` | string | `"false"` |

- **Behavior:** the control is a checkbox (DLG-CTL-001). `on_true` and `on_false` are the
  template values (DLG-ACT-005).

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001,
DLG-EXP-005.

### DLG-IN-003: Type `number_range`

| Field | Value | Default |
|-------|-------|---------|
| `label` | text | required |
| `label_format` | a translation key | `options.generic_value` |
| `width` | width (DLG-DATA-002) | 200 |
| `start` | 32-bit float | required |
| `end` | 32-bit float | required |
| `initial` | 32-bit float | none |
| `step` | 32-bit float > 0 | none |

- **Behavior:**
  1. The control is a slider (DLG-CTL-002).
  2. `start` is the value at the left end, and `end` is the value at the right end.
  3. Let `lo = min(start, end)` and `hi = max(start, end)`. An `initial` that is not in
     [`lo`, `hi`] is a load error with the text "Initial value <v> is outside of range
     [<lo>, <hi>]". Each number in the text has a decimal point, for example `11.0`.
- **Edge cases:**
  - `start` > `end` is valid. The slider then goes down from left to right.
  - `start` = `end` is valid.
  - A `step` ≤ 0 is a load error with the text "Value must be positive: <step>".
  - A `start`, an `end` or an `initial` that is not finite is a load error with the text
    "Value must be finite: <v>" (DLG-DEV-007). The check runs before the range check of `initial`.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
The finite check is `design`: see DLG-DEV-007. Kind: semantic, and deviation for the finite
check. Confidence: medium. Experiment: DLG-EXP-001.

### DLG-IN-004: Type `single_option`

| Field | Value | Default |
|-------|-------|---------|
| `label` | text | required |
| `label_visible` | Boolean | `true` |
| `width` | width (DLG-DATA-002) | 200 |
| `options` | a list of options, with at least one element | required |

- **Inputs:** an option is an object `{id, display, initial}`, or a bare string that is its
  `id`. `id` is a string and is required. `display` is a text and is optional. `initial` is a
  Boolean with the default `false`.
- **Behavior:**
  1. The control is a cycle button (DLG-CTL-003). Its values are the options in list order.
  2. The text of an option is its `display`. When `display` is absent, the text is the value of
     the field `id` of the option.
  3. The initial option is the option with `initial` = `true`, or the first option when no
     option has it.
- **Edge cases:** more than one option with `initial` = `true` is a load error with the text
  "Multiple initial values". An empty `options` is a load error. Two options with the same `id`
  are valid. An option with no `id` is a load error.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001, DLG-EXP-005.

### DLG-IN-005: Type `text`

| Field | Value | Default |
|-------|-------|---------|
| `label` | text | required |
| `label_visible` | Boolean | `true` |
| `width` | width (DLG-DATA-002) | 200 |
| `initial` | string | `""` |
| `max_length` | an integer ≥ 1 | 32 |
| `multiline` | `{max_lines, height}` | none |

- **Inputs:** in `multiline`, `max_lines` is an optional integer ≥ 1, and `height` is an
  optional integer from 1 to 512.
- **Behavior:**
  1. With no `multiline`, the control is a one-line edit box (DLG-CTL-004).
  2. With `multiline`, the control is a multi-line edit box (DLG-CTL-005 to DLG-CTL-008).
  3. `max_length` is the limit of code units of the value.
- **Edge cases:** an `initial` longer than `max_length` is a load error with the text "Default
  text length exceeds allowed size".

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001.

### DLG-IN-006: Duplicate keys

- **Behavior:**
  1. Two inputs of one dialog can have the same key.
  2. The screen shows the control of each of the two inputs.
  3. For a template value and for a tag value, the later input in `inputs` gives the value
     (DLG-ACT-005).
  4. The server reports a warning at load. The warning names the dialog and the key
     (DLG-DEV-005).
- **Postconditions:** the loaded dialog and its wire form keep both inputs, each with its own
  key, in the order of `inputs`. The load and the wire do not merge or remove an input.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: "Unresolved",
item 23. The warning of step 4 is a design choice (DLG-DEV-005).

---

## 4. Actions and templates

### DLG-ACT-001: Action types

- **Inputs:** an action object has the field `type` and the fields of its type next to it.

| Type | Id | Fields | Click event |
|------|----|--------|-------------|
| `minecraft:open_url` | 0 | as the click event `open_url` | that event |
| `minecraft:run_command` | 1 | as the click event `run_command` | that event |
| `minecraft:suggest_command` | 2 | as the click event `suggest_command` | that event |
| `minecraft:show_dialog` | 3 | as the click event `show_dialog` | that event |
| `minecraft:change_page` | 4 | as the click event `change_page` | that event |
| `minecraft:copy_to_clipboard` | 5 | as the click event `copy_to_clipboard` | that event |
| `minecraft:custom` | 6 | as the click event `custom` | that event |
| `minecraft:dynamic/run_command` | 7 | `template` (DLG-ACT-002) | `run_command` (DLG-ACT-003) |
| `minecraft:dynamic/custom` | 8 | `id`, `additions` (DLG-ACT-004) | `custom` (DLG-ACT-004) |

- **Behavior:** a static type (ids 0 to 6) gives the same click event each time. A dynamic type
  reads the inputs of the dialog at the time of the action (DLG-ACT-006).
- **Edge cases:**
  - `open_file` is not an action type. An unknown type is a load error.
  - A type id with no namespace has the namespace `minecraft`.
  - An `open_url` action with a URL scheme other than `http` or `https` is a load error with the
    text "Unsupported protocol in URI".
  - The `command` of a `run_command` action can start with `/`.

Provenance: source_suggested_black_box_confirmed: `assets/mcrs/reports/registries.json`,
registry `minecraft:dialog_action_type`, and `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001.

### DLG-ACT-002: Template

- **Inputs:** the string `template`.
- **Behavior:**
  1. A variable starts with `$(` and ends at the next `)`. The name is the text between them.
  2. A `$` that is not followed by `(` is literal text.
  3. A `$(` with no `)` after it is a load error with the text "Unterminated macro variable".
  4. A name that is not a valid input key (DLG-IN-001) is a load error.
  5. A template with no variable is a load error with the text "No variables in macro".
- **Postconditions:** `say $(a) and $(b)` has two variables, `a` and `b`. `cost $5` alone is a
  load error, because it has no variable.
- **Edge cases:**
  - `$()` is a variable with the empty name.
  - `$$(a)`, and a `$` at the end of the template, are valid. A name with a space or `$` is a
    load error with the text "Invalid macro variable name '<name>'". The template `say $(a $(b))`
    is such a case.
  - The template can start with `/`.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001.

### DLG-ACT-003: Action `dynamic/run_command`

- **Behavior:**
  1. The client replaces each variable by the template value of the input with that key.
  2. A variable with no input of that key becomes the empty string.
  3. The result is the command of a `run_command` click event.
- **Postconditions:** with the template `say $(msg)` and a text input `msg` with the value
  `hi "you"`, the command is `say hi \"you\"` (DLG-ACT-008).

Provenance: source_suggested. Kind: semantic. Confidence: medium.

### DLG-ACT-004: Action `dynamic/custom`

- **Inputs:** `id`, a required identifier. `additions`, an optional NBT compound.
- **Behavior:**
  1. The payload starts as a copy of `additions`, or as an empty compound.
  2. For each input, the client puts its tag value into the payload under its key.
  3. An input replaces an entry of `additions` with the same name.
  4. The result is a `custom` click event with `id` and the payload.
- **Postconditions:** with `additions` `{a:1b, n:"x"}` and one text input `n` with the value
  `y`, the payload is `{a:1b, n:"y"}`.
- **Edge cases:** the order of the keys of the payload is not specified.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-005.

### DLG-ACT-005: Values of an input

- **Behavior:**

| Control | Template value | Tag value |
|---------|----------------|-----------|
| `boolean` | `on_true` when checked, else `on_false` | Byte 1 when checked, else Byte 0 |
| `number_range` | the number string of the value (DLG-ACT-007) | Float of the value (DLG-ACT-006) |
| `single_option` | the `id` of the selected option | String of that `id` |
| `text` | the value, escaped (DLG-ACT-008) | String of the value, not escaped |

- **Edge cases:** two inputs with the same key give one value (DLG-IN-006).

Provenance: the column "Tag value" is source_suggested_black_box_confirmed:
`docs/spec/fixtures/dialog/dlg-exp-005-values.txt`. The column "Template value" is
source_suggested, because that fixture holds no command ("Unresolved", item 10). Kind:
semantic. Confidence: medium. Experiment: DLG-EXP-005.

### DLG-ACT-006: Value of a `number_range`

- **Inputs:** the slider position `t` in [0, 1], and the fields of DLG-IN-003.
- **Behavior:**
  1. The initial position is `pos(i, start, end)`, with `i` = `initial`, or `start/2 + end/2`
     when `initial` is absent. When `start` = `end`, the initial position is 0.5.
  2. The raw value is `v = lerp(t, start, end)`.
  3. With no `step`, the value is `v`.
  4. With a `step` `s`, let `n = round((v − i) / s)`, where `round` yields a 32-bit signed
     integer, saturated at −2147483648 and 2147483647. Let `r = i + n·s`.
  5. When `pos(r, start, end)` is in [0, 1], the value is `r`.
  6. Else, the value is `i + (n − sign(n))·s`, clamped to [lo, hi]. `sign(n)` is −1, 0 or 1.
- **Postconditions:**
  - The steps start at `initial`, not at `start`. With `start` 0, `end` 10, `initial` 3 and
    `step` 4, the values are 3 and 7. The position 0 gives 3, because −1 is out of the range.
    The position 1 gives 7, because 11 is out of the range.
  - The thumb of the slider does not move to the value. Only the value and the caption snap.
  - The midpoint `start/2 + end/2` is finite for each finite `start` and `end`. It is equal to
    `(start + end) / 2` when that sum is finite and each half is a normal number
    (DLG-DEV-006). With `start` 2¹²⁷ and `end` 3·2¹²⁶, the sum overflows, but the midpoint is
    5·2¹²⁵ and the initial position is 0.5.
- **Edge cases:**
  - When `end − start` is finite, all arithmetic is in 32-bit float.
  - Else, steps 1 to 6 use 64-bit float, and the value rounds to a 32-bit float
    (DLG-DEV-006). Example: `start` −3·10³⁸ and `end` 3·10³⁸ give the initial position 0.5.

Provenance: source_suggested. Kind: deviation: see DLG-DEV-006. Confidence: medium.
Experiment: DLG-EXP-006.

### DLG-ACT-007: Number string

- **Inputs:** the value `v` of DLG-ACT-006.
- **Behavior:**
  1. When `v` is a whole number in the range of a 32-bit signed integer, the string is that
     integer in decimal, with a `-` when it is negative.
  2. The value 2³¹ (2147483648) is the one whole number outside that range that also follows
     step 1. Its string is `2147483647`.
  3. Else, the string is the shortest decimal that reads back as the same 32-bit float, in
     one of two forms.
  4. When 10⁻³ ≤ |`v`| < 10⁷, the form is plain: digits, a point and at least one digit.
  5. Else, the form is `d.dddE<n>`: one digit, a point, at least one digit, `E` and the
     exponent in decimal.
- **Postconditions:** 5 gives `5`. −2.5 gives `-2.5`. 0.1 gives `0.1`. 0.0001 gives `1.0E-4`.
  3·10⁹ gives `3.0E9`. 2147483648 gives `2147483647`. −2147483648 gives `-2147483648`.
- **Edge cases:** the caption of the slider uses the same string (DLG-CTL-002).

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-006.

### DLG-ACT-008: Escape of a text value

- **Behavior:** the template value of a text input is the value with these changes:

| Code unit | Becomes |
|-----------|---------|
| `"` | `\"` |
| `'` | `\'` |
| `\` | `\\` |
| U+0008 | `\b` |
| U+0009 | `\t` |
| U+000A | `\n` |
| U+000C | `\f` |
| U+000D | `\r` |
| another code unit below U+0020 | `\x` and two upper-case hexadecimal digits |

- **Postconditions:** each other code unit stays. A line break of a multi-line edit box becomes
  `\n`. U+001B becomes `\x1B`.
- **Edge cases:** the template text outside the variables does not change.

Provenance: source_suggested. Kind: semantic. Confidence: medium.

### DLG-ACT-009: When the values are read

- **Behavior:**
  1. The client reads the values of the inputs when the player activates a button.
  2. For the cancel action, the client reads them when the player presses Escape.
- **Postconditions:** a value that the player changes, and then a button activation, sends the
  changed value.

Provenance: source_suggested. Kind: semantic. Confidence: medium.

---

## 5. Dialog tags

### DLG-TAG-001: The pause menu button

- **Inputs:** the pause menu of a world (`client-menu.md`, MENU-PAUSE-003), the synced tag
  `minecraft:pause_screen_additions`, the synced registry and the server links.
- **Behavior:** the pause menu chooses one dialog for its button:
  1. With exactly one dialog in the tag, it is that dialog.
  2. With more than one dialog in the tag, it is `minecraft:custom_options`.
  3. With no dialog in the tag and at least one server link, it is `minecraft:server_links`.
  4. Else, there is no button.
- **Postconditions:**
  - The button is 204 × 20 and spans two columns, as MENU-PAUSE-003, child 5, states.
  - Its caption is the external title of the chosen dialog (DLG-DATA-004).
  - Its tooltip is `menu.custom_options.tooltip`: "Note: Custom options are provided by
    third-party servers and/or content.\nHandle with care!".
  - An activation opens the dialog (DLG-FLOW-002). The pause menu is the current screen.
- **Edge cases:** when the chosen dialog is not in the synced registry, there is no button.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-002,
DLG-EXP-003.

### DLG-TAG-002: The key "Quick Actions"

- **Inputs:** the key binding `key.quickActions` ("Quick Actions"). Its default key is the
  position of G. Its category is the miscellaneous category.
- **Behavior:** a press of the key, in a world, with no open screen and no overlay:
  1. With exactly one dialog in the tag `minecraft:quick_actions`, opens that dialog.
  2. With more than one dialog in the tag, opens `minecraft:quick_actions`.
  3. With no dialog in the tag, does nothing.
- **Postconditions:** the dialog opens with no previous screen (DLG-FLOW-002).
- **Edge cases:** the key has no fallback to the server links.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-003.

---

## 6. Wire

### DLG-WIRE-001: Packet ids

| Packet | Direction | Configuration id | Play id |
|--------|-----------|------------------|---------|
| `server_links` | clientbound | 17 | 140 |
| `clear_dialog` | clientbound | 18 | 142 |
| `show_dialog` | clientbound | 19 | 143 |
| `custom_click_action` | serverbound | 8 | 68 |

Provenance: black_box_confirmed: `assets/mcrs/reports/packets.json`. Kind: wire. Confidence:
high.

### DLG-WIRE-002: `show_dialog` in the configuration phase

- **Behavior:**
  1. The packet holds one network NBT. It is an inline dialog.
  2. The client decodes the dialog with no registries of the data pack.
  3. The decode has no NBT quota.
- **Postconditions:** the client opens the dialog (DLG-FLOW-002).
- **Edge cases:** a field that names an entry of a synced registry of the data pack cannot
  decode. For example, a `show_dialog` click event with a dialog id. Section 17 lists the
  reaction of the client.

Provenance: source_suggested. Kind: wire. Confidence: medium.

### DLG-WIRE-003: `show_dialog` in the play phase

- **Behavior:**
  1. The packet starts with a VarInt `n`.
  2. When `n` = 0, one network NBT follows. It is an inline dialog. The decode can use the
     synced registries. It has no NBT quota.
  3. When `n` ≥ 1, no more data follows. The dialog is the entry `n − 1` of the synced
     registry `minecraft:dialog`, in the order of the registry data packet.
- **Postconditions:** the server can send a dialog of its data pack by number.
- **Edge cases:** an `n − 1` past the last entry is a decode error.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-007-wire.txt`.
Kind: wire. Confidence: medium. Experiment: DLG-EXP-007.

### DLG-WIRE-004: `clear_dialog`

- **Behavior:** the packet has no fields. It is valid in the configuration phase and in the play
  phase. The client handles it as DLG-FLOW-003 states.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-007-wire.txt`.
Kind: wire. Confidence: medium. Experiment: DLG-EXP-007.

### DLG-WIRE-005: `custom_click_action`

- **Behavior:** the packet has these fields, in this order:
  1. the id: an identifier string;
  2. the payload length: a VarInt `L`;
  3. `L` bytes that hold one network NBT.
- **Postconditions:** a payload whose root tag type is 0 (End) means "no payload". The client
  sends a `custom` action with no payload as `L` = 1 and one byte 0.
- **Edge cases:** the server ignores bytes after the NBT inside the `L` bytes. This input is
  unspecified.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-004-after-actions.txt`
and `docs/spec/fixtures/dialog/dlg-exp-005-values.txt`. Kind: wire. Confidence: medium.
Experiment: DLG-EXP-004, DLG-EXP-005.

### DLG-WIRE-006: Limits of `custom_click_action`

- **Behavior:**
  1. The id has at most 32767 code units.
  2. The payload length `L` is at most 65536. An `L` greater than 65536 is a decode error of
     the packet, and the server disconnects the client. An `L` of 65536 decodes.
  3. An `L` of 0 is a decode error, and the server disconnects the client. The `L` bytes must
     hold one network NBT (DLG-WIRE-005), and the shortest one is the one byte 0.
  4. The NBT budget of the server is `nbt-quota.md` (NBQ-UNIT-001). A quota error is a decode
     error (NBQ-REF-002).
- **Postconditions:** the client sends the packet in the configuration phase and in the play
  phase.

Provenance: source_suggested. Kind: wire. Confidence: medium. Experiment: DLG-EXP-008. The
fixture of that experiment does not show the `L` or the NBT of each case ("Unresolved", item 24).

---

## 7. Server links

### DLG-LINK-001: Packet `server_links`

- **Behavior:**
  1. The packet holds a VarInt count, then each entry.
  2. An entry is a Boolean. When it is `true`, a VarInt of a known type follows. When it is
     `false`, a text as network NBT follows.
  3. Then the URL follows as a string.
  4. The client keeps the list until the next `server_links` packet of the connection.
  5. The client drops an entry whose URL has no scheme, or a scheme other than `http` or
     `https`. The comparison of the scheme ignores ASCII case.
- **Postconditions:** the server links of the dialog `server_links` and of the pause menu
  (DLG-TAG-001) are the kept list.

Provenance: source_suggested. Kind: wire. Confidence: medium. Experiment: DLG-EXP-003.

### DLG-LINK-002: Name of a server link

- **Behavior:**
  1. The name of a link of a known type is the text `known_server_link.<name>`.
  2. The name of a link with a text is that text.

| Id | Name | Id | Name |
|----|------|----|------|
| 0 | `report_bug` | 5 | `community` |
| 1 | `community_guidelines` | 6 | `website` |
| 2 | `support` | 7 | `forums` |
| 3 | `status` | 8 | `news` |
| 4 | `feedback` | 9 | `announcements` |

- **Postconditions:** the type 0 shows "Report Server Bug" in English.

Provenance: source_suggested. Kind: wire. Confidence: medium. Experiment: DLG-EXP-003.

---

## 8. Layout of a dialog screen

### DLG-SCR-001: Parts of the screen

- **Behavior:**
  1. A dialog screen is a header, content and footer layout (`client-widget.md`,
     WID-LAY-011).
  2. The header height is 33. The footer height is 33.
  3. For a button-list type with no `exit_action`, the footer height is 5.
  4. The content holds the body panel (DLG-SCR-003).
  5. The screen arranges its parts again when W or H changes. The values of the controls stay.

Provenance: source_suggested. Kind: layout. Confidence: medium. Experiment: DLG-EXP-009.

### DLG-SCR-002: Header and warning button

- **Behavior:**
  1. The header holds a row with the gap 10. Each cell has the alignment (0.5, 0.5).
  2. The first child is a one-line text widget with the title. It has no maximum width.
  3. The second child is the warning button.
  4. The warning button is an image button of 20 × 20 (`client-widget.md`, WID-ICON-002). Its
     sprites are `dialog/warning_button`, `dialog/warning_button_disabled` and
     `dialog/warning_button_highlighted`.
  5. Its tooltip is `menu.custom_screen_info.tooltip`: "This is a custom screen. Click here to
     learn more.". Its narration is `menu.custom_screen_info.button_narration`.
  6. Its tab order group is −10.
  7. An activation opens the warning screen (DLG-WARN-001).
  8. After each arrangement, a warning button with `x` < 0, `y` < 0, `x` > W − 20 or
     `y` > H − 20 moves to (`max(0, W − 40)`, `min(5, H)`).
- **Postconditions:** a title wider than W pushes the button out of the screen. Step 8 then
  puts it at the top right.

Provenance: source_suggested. Kind: layout. Confidence: medium. Experiment: DLG-EXP-009.

### DLG-SCR-003: Body column and body panel

- **Behavior:**
  1. The body is a column with the gap 10. Each cell has the horizontal alignment 0.5.
  2. The children are, in this order: the body elements, the controls of the inputs, and the
     button grid of a button-list type (DLG-SCR-005).
  3. The column is in a scroll panel (`client-list.md`, SCRL-PANEL-001). Its wheel step is 10.
  4. The maximum height of the panel is the content height of the screen, `H − hh − fh`. The
     height of the panel is `min(C, H − hh − fh)`, with `C` the height of the column.
  5. The width of the panel is the width of the column plus 20.
- **Postconditions:** a body that is higher than the content scrolls. A body that fits does not
  show a scrollbar.

Provenance: source_suggested. Kind: layout. Confidence: medium. Experiment: DLG-EXP-009.

### DLG-SCR-004: Footer of a simple type

- **Behavior:**
  1. The footer holds a row with the gap 8.
  2. For `notice`, the row holds the one button.
  3. For `confirmation`, the row holds `yes`, then `no`.
- **Postconditions:** at W = 427 and H = 240, the default button of a `notice` is at
  (138, 214).

Provenance: source_suggested. Kind: layout. Confidence: medium. Experiment: DLG-EXP-009.

### DLG-SCR-005: Button grid and footer of a button-list type

- **Inputs:** `n` buttons and `c` = `columns`.
- **Behavior:**
  1. The grid has the column gap 2 and the row gap 2. Each cell has the alignment (0.5, 0.5).
  2. Let `f = floor(n / c)·c`. The first `f` buttons fill the grid in rows of `c`, left to right
     and top to bottom (`client-widget.md`, WID-LAY-004, WID-LAY-005).
  3. When `n − f` > 0, the other buttons are in a row with the gap 2. That row is one cell of
     the grid below the full rows. It spans all `c` columns and has the alignment (0.5, 0.5).
  4. The grid is the last child of the body column.
  5. `exit_action`, when present, is the only child of the footer.
- **Postconditions:** 5 buttons of 150 in 2 columns give 2 full rows of 302, and a third row
  with one centred button.
- **Edge cases:** with `n` = 0, the grid is empty and has the size 0 × 0.

Provenance: source_suggested. Kind: layout. Confidence: medium. Experiment: DLG-EXP-009.

### DLG-SCR-006: Example of a notice

- **Inputs:** a `notice` with a short title, one `plain_message` of width 200 with one line,
  and the default button. W = 427 and H = 240.
- **Postconditions:**
  - The row of the header is at `y` = 7.
  - The body panel is 220 × 17 at (103, 63).
  - The text widget of the message is at `x` = 113. The text line is at `y` = 67.
  - The button is at (138, 214).

Provenance: source_suggested. Kind: layout. Confidence: medium. Experiment: DLG-EXP-009.

### DLG-SCR-010: Message widget

- **Inputs:** a text `t` and a width `w`. A `plain_message`, and the description of an `item`
  element, use this widget.
- **Behavior:**
  1. The widget is `w` wide. Its padding is 4.
  2. The text wraps at `w − 8`. Let `k` be the number of lines. The widget is `9·k + 8` high.
  3. The first line is at `y + 4`. Each line is centred on `x + floor(w / 2)`.
  4. The colour of the text is that of a multi-line text widget (`client-widget.md`,
     WID-TXT-002).
  5. The widget draws no background. When it is focused, it draws an outline of 1 GUI unit in
     white on its bounds.
  6. The widget is active and can take the focus. A press plays no click sound.
  7. A hover over a part of the text shows the hover event of that part.
  8. A press on a part with a click event runs that click event (DLG-CLK-007).
  9. An activation key, while the widget is focused, runs the first click event of the text,
     in the order of its parts.
- **Postconditions:** the pointer shape is a hand over a part with a click event
  (`client-widget.md`, WID-STATE-004).

Provenance: source_suggested. Kind: layout. Confidence: medium.

### DLG-SCR-011: Item widget

- **Inputs:** an `item` element (DLG-BODY-002).
- **Behavior:**
  1. The item widget is `width` × `height`.
  2. The stack draws at 16 × 16 at the top left of the widget. It does not scale with
     `width` and `height`.
  3. When `show_decorations` is `true`, the count, the durability bar and the cooldown draw.
  4. When `show_tooltip` is `true`, the tooltip of the stack shows while the widget is hovered
     or focused.
  5. The widget can take the focus. A press plays the click sound (`client-widget.md`,
     WID-SND-001) and does nothing else.
  6. With no description, the narration is `narration.item` ("Item: %s") with the name of the
     stack.
  7. With a description, the element is a row with the gap 2 and the vertical alignment 0.5.
     The row holds the item widget, then a message widget (DLG-SCR-010) with the description.
- **Postconditions:** an item element of 64 × 64 has the stack at its top left and an empty area
  of 48 GUI units right of it and below it.

Provenance: source_suggested. Kind: layout. Confidence: medium.

---

## 9. Controls of the inputs

### DLG-CTL-001: Checkbox

- **Behavior:** the control of a `boolean` input is a checkbox (`client-widget.md`,
  WID-CHK-001 to WID-CHK-003). Its label is `label`. It is checked when `initial` is `true`.
- **Postconditions:** the control reports checked or not checked.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: DLG-EXP-011.

### DLG-CTL-002: Slider

- **Behavior:**
  1. The control of a `number_range` input is a slider of `width` × 20 (`client-widget.md`,
     WID-SLD-001 to WID-SLD-004).
  2. Its initial position is that of DLG-ACT-006, step 1.
  3. Its caption is the translation `label_format` with two arguments: `label` and the number
     string of the value (DLG-ACT-007).
- **Postconditions:** the control reports its position. DLG-ACT-006 makes the value from it. In
  English, the default caption is `<label>: <value>`.

Provenance: source_suggested. Kind: layout. Confidence: medium. Experiment: DLG-EXP-011.

### DLG-CTL-003: Cycle button

- **Behavior:**
  1. The control of a `single_option` input is a cycle button of `width` × 20
     (`client-widget.md`, WID-CYC-001, WID-CYC-002).
  2. Its name is `label`. Its values are the options, with the texts of DLG-IN-004.
  3. The caption form is "name and value" when `label_visible` is `true`, else "value".
- **Postconditions:** the control reports the selected option.

Provenance: source_suggested. Kind: layout. Confidence: medium. Experiment: DLG-EXP-011.

### DLG-CTL-004: One-line edit box

- **Behavior:**
  1. The control of a `text` input with no `multiline` is a one-line edit box of `width` × 20
     (`client-edit-box.md`).
  2. Its limit is `max_length`. Its value starts as `initial`.
  3. When `label_visible` is `true`, the control is an element with a label
     (`client-widget.md`, WID-LAY-010), with `label`.
  4. When `label_visible` is `false`, the label is not shown. It is the narration of the box.
- **Postconditions:** the control reports its value.

Provenance: source_suggested. Kind: layout. Confidence: medium. Experiment: DLG-EXP-011.

The rules DLG-CTL-005 to DLG-CTL-008 state only what a dialog needs from a multi-line edit box.
Section 17 asks which specification states the box itself.

### DLG-CTL-005: Multi-line edit box: size and limits

- **Inputs:** a `text` input with `multiline`.
- **Behavior:**
  1. The box is `width` wide.
  2. Its height is `height` when present. Else it is `min(9·m + 8, 512)`, with `m` =
     `max_lines`, or 4 when `max_lines` is absent.
  3. The limit of code units is `max_length`.
  4. When `max_lines` is present, it is the limit of lines. The lines are counted after a wrap
     at `width − 8`. A value that ends with a line break counts one more line.
  5. An edit that passes `max_lines` does not apply (DLG-CTL-006).
  6. When `label_visible` is `true`, the control is an element with a label, as DLG-CTL-004.
- **Postconditions:** with no `height` and no `max_lines`, the box is 44 high.
- **Edge cases:** an `initial` with more lines than `max_lines` does not go into the box. The
  box starts empty, and its value is the empty string. The box does not cut the text. The load
  is not an error.

Provenance: source_suggested. Kind: layout. Confidence: medium. Experiment: DLG-EXP-012.

### DLG-CTL-006: Multi-line edit box: input

- **Behavior:**
  1. Enter inserts a line break.
  2. Typed or pasted text drops each character that chat does not allow (`client-chat.md`).
     It keeps the line breaks.
  3. Text that would pass `max_length` is cut to the free code units.
  4. An insert that would pass `max_lines` does not apply.
  5. The cursor, the selection, the clipboard keys and the cursor keys are as for the one-line
     edit box (`client-edit-box.md`), over lines.
  6. Up and Down move the cursor one line, and the box scrolls to show the cursor.
- **Postconditions:** the control reports its value, with each line break as U+000A.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-012.

### DLG-CTL-007: Multi-line edit box: draw

- **Behavior:**
  1. The background sprite is `widget/text_field`, or `widget/text_field_highlighted` while
     the box is highlighted (`client-widget.md`, WID-STATE-002).
  2. The inner padding is 4. The text wraps at `width − 8`.
  3. The text colour is `FFE0E0E0` with a drop shadow. The cursor colour is `FFD0D0D0`.
  4. When the lines are higher than the box, a scrollbar draws right of the box, outside its
     bounds.
  5. A counter draws below the box: `gui.multiLineEditBox.character_limit` ("%s/%s") with
     the length of the value and `max_length`. It is right-aligned to the right edge of the box,
     at `y + height + 4`, in the colour `FFA0A0A0`.
- **Postconditions:** the counter is inside the gap of 10 below the box in the body column.

Provenance: source_suggested. Kind: layout. Confidence: medium.

### DLG-CTL-008: Multi-line edit box: wheel

- **Behavior:**
  1. A wheel event over a multi-line edit box that can scroll scrolls the box. Its wheel step is
     4.
  2. A wheel event over a box that cannot scroll in that direction goes to the body panel.
  3. A wheel event over each other widget of the body goes to the body panel.
- **Postconditions:** a player can scroll a long text in the box with the wheel.

Provenance: `design`: see DLG-DEV-003. Kind: deviation. Confidence: medium.

---

## 10. Focus and keys

### DLG-KEY-001: Focus

- **Behavior:**
  1. The tab order is the warning button (group −10), then the widgets of the body in column
     order, then the buttons of the footer in row order.
  2. The initial focus is that of `client-widget.md`, WID-FOC-002. The dialog names no initial
     widget.
  3. A widget of the body that gets the focus by a key scrolls into the panel
     (`client-list.md`, SCRL-PANEL-002).
- **Postconditions:** a dialog that opens by an activation key focuses the warning button first.
  A dialog that the server opens has no focused widget.

Provenance: source_suggested. Kind: semantic. Confidence: medium.

### DLG-KEY-002: Escape

- **Behavior:**
  1. When `can_close_with_escape` is `true`, Escape runs the cancel action with the after
     action `close` (DLG-FLOW-001). The `after_action` of the dialog does not apply.
  2. With no cancel action, Escape shows the previous screen.
  3. When `can_close_with_escape` is `false`, Escape does nothing.
- **Postconditions:**
  - A dialog with `wait_for_response` closes on Escape to the previous screen, not to the
    waiting screen.
  - A dialog that a player cannot close with Escape still has the warning button. That button
    lets the player leave the server (DLG-WARN-001).

Provenance: source_suggested. Kind: semantic. Confidence: medium.

### DLG-KEY-003: Pause

- **Behavior:**
  1. A dialog screen with `pause` = `true` pauses the game in a local world, as a pause screen
     does.
  2. In multiplayer and in a world open to LAN, `pause` has no effect.
  3. The waiting screen does not pause the game.

Provenance: source_suggested. Kind: semantic. Confidence: medium.

---

## 11. Open, close and nested dialogs

### DLG-FLOW-001: Activation and after action

- **Inputs:** an action button, or the cancel action, and an after action `a`.
- **Behavior:**
  1. The client makes the click event from the action (DLG-ACT-001), with the current values
     of the inputs.
  2. The target screen follows from `a`:

| `a` | Target screen |
|-----|---------------|
| `none` | this dialog screen, with its controls and their values |
| `close` | the previous screen |
| `wait_for_response` | a new waiting screen with the previous screen of this dialog |

  3. With a click event, the client runs it with the target screen (section 12).
  4. With no click event, the client shows the target screen.
- **Postconditions:** with `none`, the values of the controls stay after the action.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-004.

### DLG-FLOW-002: Open a dialog

- **Inputs:** a dialog `d`, and the current screen `s` (or no screen).
- **Behavior:** the client shows a dialog screen of `d`. Its previous screen is:

| `s` | Previous screen of `d` |
|-----|------------------------|
| a dialog screen | the previous screen of `s` |
| the waiting screen | the previous screen of the waiting screen |
| the warning screen | the previous screen of the dialog that the warning screen returns to |
| another screen, or no screen | `s` |

- **Postconditions:**
  - Dialogs do not stack. A dialog that replaces a dialog does not run its cancel action.
  - When `s` is the warning screen, the warning screen stays. Its return screen becomes the
    dialog screen of `d`.
  - A `show_dialog` from a button runs this rule with the target screen as `s`.
- **Edge cases:** a dialog that opens from the pause menu returns to the pause menu.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-002.

### DLG-FLOW-003: Clear a dialog

- **Behavior:** on `clear_dialog`:
  1. When the current screen is a dialog screen, the client shows its previous screen.
  2. When the current screen is the warning screen and it returns to a dialog screen, its return
     screen becomes the previous screen of that dialog.
  3. Else, nothing changes.
- **Postconditions:** no action runs. No packet goes to the server.
- **Edge cases:** the waiting screen stays. Only a new dialog (DLG-FLOW-002), or the button of
  the waiting screen after 5 seconds (DLG-WAIT-001), ends it.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-007.

### DLG-FLOW-004: Disconnect and other screens

- **Behavior:**
  1. On a disconnect, the disconnect screen replaces each dialog screen, waiting screen and
     confirm screen. No action runs.
  2. A screen that the game shows over a dialog, for example the screen of a dimension change
     or the screen that loads a level, replaces the dialog. No action runs.

Provenance: source_suggested. Kind: semantic. Confidence: low.

### DLG-FLOW-005: The configuration phase

- **Behavior:**
  1. In the configuration phase, the client shows no world. The current screen is the screen
     of the connection. A dialog has that screen as its previous screen.
  2. A `custom` click event sends `custom_click_action` in the configuration phase.
  3. A `run_command` click event sends no command. The client shows the target screen.
  4. At the start of the play phase, the screen that loads the level replaces an open dialog
     (DLG-FLOW-004).
- **Postconditions:** a server can ask for input before the play phase with a dialog and a
  `custom` action.

Provenance: source_suggested. Kind: deviation: see DLG-DEV-002. Confidence: medium.

---

## 12. Click events

### DLG-CLK-001: Click events of a dialog

- **Inputs:** a click event `e` and a target screen `g`.
- **Behavior:**

| `e` | Result |
|-----|--------|
| `run_command` | DLG-CLK-002 |
| `show_dialog` | the dialog opens with `g` as the current screen (DLG-FLOW-002) |
| `custom` | the client sends `custom_click_action` with the id and the payload, then shows `g` |
| `open_url` | DLG-CLK-004 |
| `copy_to_clipboard` | the client copies the value to the clipboard, then shows `g` |
| `suggest_command` | when `g` is the chat screen, the text replaces the text of its input; then the client shows `g` |
| `change_page` | no effect; the client shows `g` |

- **Postconditions:** "shows `g`" with `g` = this dialog screen keeps the screen and its values.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-004.

### DLG-CLK-002: `run_command` in the play phase

- **Inputs:** the command `c` without one leading `/`, and the target screen `g`.
- **Behavior:** the client parses `c` with its command tree.
  1. When `c` parses with the current permissions, has no argument that needs a signature, and
     parses with no permissions: the client sends `chat_command` with `c`, then shows `g`.
  2. When `c` does not parse: the confirm screen of DLG-CLK-003 with the message
     `multiplayer.confirm_command.parse_errors`.
  3. When `c` has an argument that needs a signature: the confirm screen with the message
     `multiplayer.confirm_command.signature_required`.
  4. When `c` needs a permission: the confirm screen with the message
     `multiplayer.confirm_command.permissions_required`.

Provenance: source_suggested. Kind: client agreement. Confidence: medium. Experiment:
DLG-EXP-005.

### DLG-CLK-003: Confirm screen of a command

- **Behavior:** a confirm screen (DLG-CONF-001) with:
  1. the title `multiplayer.confirm_command.title`;
  2. the message of DLG-CLK-002, with `c` in yellow as its argument;
  3. the first button "Run Command" (`multiplayer.confirm_command.run_command`). It sends
     `chat_command` with `c`, then shows `g`;
  4. the second button "Back" (`gui.back`) when a screen was open, else "Cancel"
     (`gui.cancel`). It shows the screen that was open, with its values.
- **Edge cases:** with a signed argument, the first button changes:
  1. When `g` is no screen and the player can send commands: "Copy to Chat Screen"
     (`multiplayer.confirm_command.suggest_command`). It opens the chat screen with `/` and
     `c` in its input.
  2. Else: "Copy to Clipboard" (`chat.copy`). It copies `/` and `c`, then shows `g`.

Provenance: source_suggested. Kind: semantic. Confidence: medium.

### DLG-CLK-004: `open_url`

- **Inputs:** the URL `u`, the target screen `g`, and the options "Web Links"
  (`options.chat.links`) and "Prompt on Links" (`options.chat.links.prompt`).
- **Behavior:**
  1. When "Web Links" is off, the client opens nothing and shows `g`.
  2. When "Prompt on Links" is on, the client shows the link confirm screen (DLG-CLK-005).
  3. Else, the client opens `u` in the system browser and shows `g`.
- **Postconditions:** the after action applies in each case.

Provenance: source_suggested. Kind: deviation: see DLG-DEV-001. Confidence: medium.

### DLG-CLK-005: Link confirm screen

- **Behavior:** a confirm screen with:
  1. the title `chat.link.confirm`: "Are you sure you want to open the following website?";
  2. the message `u`;
  3. the extra line `chat.link.warning`, in the colour `FFFFCCCC`;
  4. three buttons of 100 × 20 with the gap 4: "Yes" (`gui.yes`), "Copy to Clipboard"
     (`chat.copy`) and "No" (`gui.no`).
- **Postconditions:**
  - "Yes" opens `u`, then shows `g`.
  - "Copy to Clipboard" copies `u`, then shows `g`.
  - "No" and Escape show `g`.

Provenance: source_suggested. Kind: layout. Confidence: medium.

### DLG-CLK-006: Click events that a dialog cannot hold

- **Behavior:** `open_file` is not valid in a dialog, in an action or in a text that comes from
  a server (`text-component.md`). A dialog that holds it does not load.

Provenance: source_suggested_black_box_confirmed: `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
Kind: semantic. Confidence: medium. Experiment: DLG-EXP-001.

### DLG-CLK-007: Click events in the text of the body

- **Behavior:**
  1. A click event of a part of the text of a message widget runs as DLG-CLK-001 states.
  2. The after action is the `after_action` of the dialog (DLG-FLOW-001).
- **Postconditions:** a `run_command` in the text of a dialog with `close` runs the command and
  closes the dialog.

Provenance: source_suggested. Kind: semantic. Confidence: medium.

---

## 13. Follow-up screens

### DLG-CONF-001: Confirm screen

- **Behavior:**
  1. The screen is a column with the gap 8, centred in the screen.
  2. Its children are: the title as a one-line text widget; the message as a multi-line text
     widget with the maximum width W − 50, at most 15 rows, centred; an optional extra line; a
     row of buttons with the gap 4 and a top padding of 16.
  3. Each button is 150 × 20, unless the rule of the screen states another width.
  4. Escape gives the answer "no" of the screen.
- **Postconditions:** the confirm screen is not a dialog screen. A `clear_dialog` does not
  close it.

Provenance: source_suggested. Kind: layout. Confidence: medium.

### DLG-WAIT-001: Waiting screen

- **Behavior:**
  1. The title is `gui.waitingForResponse.title`: "Waiting for Server". The header height is
     33. The footer height is 0.
  2. The content holds one button of 200 × 20.
  3. Let `sec` be `floor(t / 20)`, with `t` the number of ticks since the screen opened.
  4. When `sec` = 0, the button is not visible.
  5. When `sec` is 1 to 4, the button is visible and not active. Its caption is
     `gui.waitingForResponse.button.inactive` ("Back (%ss)") with `5 − sec`.
  6. When `sec` ≥ 5, the caption is "Back" (`gui.back`) and the button is active.
  7. The button shows the previous screen. Escape does the same, only while the button is
     active.
- **Postconditions:** the captions are "Back (4s)", "Back (3s)", "Back (2s)", "Back (1s)", then
  "Back".

Provenance: source_suggested. Kind: layout. Confidence: medium. Experiment: DLG-EXP-017.

### DLG-WARN-001: Warning screen

- **Inputs:** the screen to return to. It is the dialog screen with the warning button.
- **Behavior:** a confirm screen with:
  1. the title `menu.custom_screen_info.title`: "Note about custom screens";
  2. the message `menu.custom_screen_info.contents`;
  3. the first button "Disconnect" (`menu.disconnect`), or "Save and Quit to Title"
     (`menu.returnToMenu`) in a local world. It disconnects with the reason
     `menu.custom_screen_info.disconnect`: "Custom screen rejected";
  4. the second button "Back" (`gui.back`). It shows the return screen.
- **Postconditions:** Escape shows the return screen. DLG-FLOW-002 and DLG-FLOW-003 can change
  the return screen while the warning screen is open.

Provenance: source_suggested. Kind: semantic. Confidence: medium. Experiment: DLG-EXP-018.

---

## 14. Server

### DLG-SRV-001: Custom click action message

- **Behavior:**
  1. For each `custom_click_action` that decodes, the server emits one public message. It holds
     the connection, the phase, the id and the payload, or "no payload".
  2. A user of the library can handle the message in each phase.
  3. With no handler, the server does nothing with the packet.
- **Postconditions:** the server sends no reply and changes no state by default.

Provenance: `design`. Kind: semantic. Confidence: medium.

### DLG-SRV-002: Show and clear from the server

- **Behavior:**
  1. A user of the library can send a dialog to one connection in the configuration phase and in
     the play phase, by a registry id or inline.
  2. In the configuration phase, the server writes the inline form only (DLG-WIRE-002). A
     registry id resolves on the server to its value.
  3. In the play phase, the server writes a registry id as its number (DLG-WIRE-003).
  4. A user of the library can send `clear_dialog` to one connection.
- **Edge cases:** the server resolves the outer id only. A `show_dialog` action inside the
  resolved value can name a registry entry, and the server does not replace it. Section 17 lists
  the reaction of the client.

Provenance: `design`. Kind: wire. Confidence: medium.

---

## 15. Intentional deviations

### DLG-DEV-001: An open link applies the after action

- **Rule:** DLG-CLK-004.
- **Reference:** when "Web Links" is off, nothing happens and the dialog stays. When "Prompt on
  Links" is off, the link opens and the dialog stays. In both cases the after action does not
  apply, also for `close` and `wait_for_response`. With the prompt on, the after action
  applies.
- **Reason:** the result of a button must not depend on an option of the client. The after
  action is what the dialog asks for.
- **Visible to the client:** yes. With `close`, the dialog closes after the link opens.
- **Bug reference:** `none`.
- **Test:** a differential test of the client ignores the screen after an `open_url` action.

### DLG-DEV-002: A command in the configuration phase applies the after action

- **Rule:** DLG-FLOW-005, step 3.
- **Reference:** the client sends no command, and the dialog stays.
- **Reason:** a button that does nothing and keeps the screen looks broken to the player.
- **Visible to the client:** yes. With `close`, the dialog closes.
- **Bug reference:** `none`.
- **Test:** a differential test ignores the screen after a `run_command` in the configuration
  phase.

### DLG-DEV-003: The wheel scrolls a multi-line edit box

- **Rule:** DLG-CTL-008.
- **Reference:** the body panel takes each wheel event, also over a multi-line edit box. A
  player can scroll a long text only with the cursor keys.
- **Reason:** a box with a scrollbar must scroll with the wheel.
- **Visible to the client:** yes. The text of the box scrolls, not the body.
- **Bug reference:** `none`.
- **Test:** a differential test ignores the scroll positions after a wheel event over a
  multi-line edit box.

### DLG-DEV-004: The budget of a custom payload

- **Rule:** DLG-WIRE-006.
- **Reference:** the server reads the payload with a fixed budget of 32768 bytes and a depth
  limit of 16. A payload that exceeds the budget gives a decode error and a disconnect. No
  experiment shows the exact edge of the budget or of the depth ("Unresolved", item 24).
- **Reason:** each serverbound network NBT of mcrs uses the one budget of `nbt-quota.md`.
- **Visible to the client:** only for a payload that one budget accepts and the other refuses.
- **Bug reference:** `none`.
- **Test:** a differential test does not send a payload between the two budgets.

### DLG-DEV-005: A duplicate input key gives a warning at load

- **Rule:** DLG-IN-006.
- **Reference:** a dialog with two inputs of one key loads with no diagnostic. Both controls
  show, and the later input gives the value. This is `source_suggested`, and no experiment
  shows it ("Unresolved", item 23).
- **Reason:** a duplicate key is nearly always an error of the pack author, and the later
  input hides the earlier one. The author must see it. The loaded result and the wire stay the
  same as in the reference.
- **Visible to the client:** no. The warning is a diagnostic of the server at load. It names the
  dialog and the key.
- **Bug reference:** `none`.
- **Test:** a differential test ignores the warnings of the server.

### DLG-DEV-006: A slider of a large range stays in its range

- **Rule:** DLG-ACT-006.
- **Reference:** the midpoint is `(start + end) / 2` in 32-bit float, and each step of the
  value is in 32-bit float. When the sum or `end − start` overflows, the initial position, the
  value and the caption can be wrong, infinite or not a number.
- **Reason:** a position of a slider must be in [0, 1], and a value must be in its range.
- **Visible to the client:** yes, only for a range whose sum or width is larger than the
  largest finite 32-bit float. The caption shows a number in the range.
- **Bug reference:** `none`.
- **Test:** a differential test does not use such a range.

### DLG-DEV-007: Non-finite slider edges and positions

- **Rule:** DLG-IN-003 and DLG-ACT-006.
- **Reference:** accepts non-finite values for `start`, `end` and `initial` with no validation
  error at load. The initial position in the slider widget is the unclamped value from the
  constructor. When a value is infinity or not a number, the caption shows the string
  `"Infinity"`, `"-Infinity"` or `"NaN"`.
- **Reason:** a slider position must be in [0, 1], and a value shown or sent must be finite
  and in the range [lo, hi]. DLG-IN-003 rejects non-finite edges at load. When all edges are
  finite, all arithmetic of steps stays in range.
- **Visible to the client:** no. The load error is a diagnostic of the server.
- **Bug reference:** `none`.
- **Test:** a differential test does not use non-finite edges.

---

## 16. Experiments

Each experiment runs the vanilla server of `tools/vanilla-server` with a data pack of dialogs, and
the vanilla client. Read the rectangle of each widget from the widget tree of the client at GUI
scale 2 (W = 427, H = 240). "Show" means a `show_dialog` from the server.

- **DLG-EXP-001, load.** Make a pack with one dialog for each field and each bound of sections 1 to
  4 and of DLG-CLK-006. Make one valid value, one value out of range, and each named error. Show
  each dialog with `dialog show`. Start the server with the dialog of DLG-DATA-005 in a pack.
  Expected: the server loads the valid dialogs and gives the stated text for each other dialog.
  The experiment checks DLG-DATA-001 to DLG-DATA-003, DLG-DATA-005, DLG-DATA-009 to DLG-DATA-011,
  DLG-BODY-001, DLG-BODY-002, DLG-IN-001 to DLG-IN-005, DLG-ACT-001, DLG-ACT-002 and DLG-CLK-006.
  The output is in `docs/spec/fixtures/dialog/dlg-exp-001-load.txt`.
- **DLG-EXP-002, open and nest.** Open the pause menu with one dialog in the tag
  `minecraft:pause_screen_additions`, and with two dialogs. Activate the button, then press Escape.
  Show a dialog with a `show_dialog` action. Activate its button, then press Escape. Expected: the
  caption of the button is the external title, Escape returns to the pause menu, and Escape after
  the nested dialog shows no screen. The experiment checks DLG-DATA-004, DLG-DATA-006,
  DLG-DATA-011 and DLG-FLOW-002.
- **DLG-EXP-003, tags and links.** Send `server_links` with the known types 0 and 1, one text
  entry, one `ftp:` URL, one URL with no scheme and one `HTTPS:` URL. Open the pause menu with 0, 1
  and 2 entries in the tag. Run the action of the key mapping "Quick Actions" with each tag.
  Expected: the buttons and dialogs of DLG-TAG-001 and DLG-TAG-002, and the entries of
  DLG-LINK-001 and DLG-LINK-002. The experiment also checks DLG-DATA-010.
- **DLG-EXP-004, after actions.** Show a dialog with a `custom` action on a button, once for each
  after action. Activate the button. Capture `custom_click_action`. Press Escape on the waiting
  screen. Expected: the screens of DLG-FLOW-001 and DLG-WAIT-001. The experiment checks the row
  `custom` of DLG-CLK-001. The output is in
  `docs/spec/fixtures/dialog/dlg-exp-004-after-actions.txt`.
- **DLG-EXP-005, values.** Show a dialog with one input of each type and a `dynamic/custom` action
  with `additions`. Set known values, with a line break. Activate the button. Capture
  `custom_click_action`. Show a `dynamic/run_command` action with the command `say`, and activate
  its button. Expected: DLG-ACT-004, the tag values of DLG-ACT-005, and the confirm screen of
  DLG-CLK-002. The experiment also checks the clicks of DLG-IN-002 and the captions of DLG-IN-004.
  The output is in `docs/spec/fixtures/dialog/dlg-exp-005-values.txt`.
- **DLG-EXP-006, numbers.** Show `number_range` inputs with the steps of DLG-ACT-006 and with the
  values 5, −2.5, 0.1, 0.0001, 3·10⁹, 2147483520 and 2147483648. Read the caption. Move the
  slider to the right end. Expected: the captions and the values of DLG-ACT-006 and DLG-ACT-007.
- **DLG-EXP-007, wire.** Show an inline dialog and a dialog of the registry in the play phase.
  Send `clear_dialog` over a dialog, over the warning screen and over the waiting screen.
  Expected: DLG-WIRE-003, DLG-WIRE-004 and DLG-FLOW-003. The output is in
  `docs/spec/fixtures/dialog/dlg-exp-007-wire.txt`.
- **DLG-EXP-008, payload length.** Open a raw connection to the vanilla server. Send
  `custom_click_action` in the configuration phase with `L` = 0, 1, 2, 65536 and 65537. Send
  payloads of sizes on each side of 32768 bytes, and compounds nested on each side of 16 levels.
  Expected: DLG-WIRE-006 and DLG-DEV-004. A value of `L` from 1 to 65536 with one network NBT
  decodes. The values 0 and 65537 disconnect. The output is in
  `docs/spec/fixtures/dialog/dlg-exp-008-payload-length.txt`.
- **DLG-EXP-009, layout.** Read the widget rectangles of DLG-SCR-006, of a `confirmation` and of a
  `multi_action` with 5 buttons in 2 and in 3 columns. Expected: DLG-SCR-001 to DLG-SCR-006,
  DLG-DATA-007 to DLG-DATA-009.
- **DLG-EXP-011, controls.** Read the rectangles of the controls of a `boolean`, a `number_range`,
  a `single_option` and a `text` input. Expected: DLG-CTL-001 to DLG-CTL-004.
- **DLG-EXP-012, multi-line.** Show a `text` input with `multiline` and `max_lines` 3. Type text
  and press Enter. Expected: the size of DLG-CTL-005 and step 1 of DLG-CTL-006.
- **DLG-EXP-017, waiting.** Activate a button of a dialog with `wait_for_response`. Read the
  waiting screen each second for 6 seconds. Press Escape in seconds 2 and 6. Expected:
  DLG-WAIT-001.
- **DLG-EXP-018, warning.** Activate the warning button of a dialog. Press Back, then Escape.
  Expected: step 4 and the Escape of DLG-WARN-001.

---

## 17. Unresolved

1. **A configuration dialog with a registry reference** (DLG-WIRE-002). What does the client do
   when the decode fails: disconnect, or ignore the packet? The same question applies to a
   `show_dialog` action that names a registry entry inside a dialog that DLG-SRV-002 resolves.
2. **The owner of the multi-line edit box** (DLG-CTL-005 to DLG-CTL-008). Which specification
   states the cursor keys over wrapped lines, the selection, the draw of the selection and the
   scroll to the cursor of the multi-line edit box?
3. **The output of the experiments.** The experiments DLG-EXP-002, DLG-EXP-003, DLG-EXP-006,
   DLG-EXP-009, DLG-EXP-011, DLG-EXP-012, DLG-EXP-017 and DLG-EXP-018 ran. No fixture holds their
   output. The saved logs hold no widget rectangle, screen or caption of the client. Until a
   fixture holds it, the rules that cite only these experiments keep the label `source_suggested`.
   The fixtures of DLG-EXP-001, DLG-EXP-004, DLG-EXP-005, DLG-EXP-007 and DLG-EXP-008 hold the
   load messages of the server and the packets, but not each command. The rules that these
   fixtures show have the label `source_suggested_black_box_confirmed`. The rules DLG-FLOW-001,
   DLG-FLOW-003, DLG-CLK-001 and DLG-WAIT-001 also cite these experiments and keep their labels,
   because the fixtures show no screen. The rule DLG-ACT-004 keeps its label, because the
   fixture of DLG-EXP-005 shows no entry of `additions` with the key of an input.
4. **The configuration phase** (DLG-WIRE-002, DLG-WIRE-004, DLG-FLOW-005). What does the client do
   with `show_dialog` and `clear_dialog` in the configuration phase? Do a `run_command` action
   and a `custom` action behave as DLG-FLOW-005 states?
5. **The captions of a dialog list** (DLG-DATA-004, DLG-DATA-011). Is the caption of the button of
   each dialog its `external_title`, and its `title` when `external_title` is absent?
6. **The action of a server link button** (DLG-DATA-010). Does an activation of the button open the
   URL of the link?
7. **The load of an item and of an inline dialog** (DLG-BODY-002, DLG-ACT-001). Does the `item`
   template of a body element load? Does a `show_dialog` action with an inline dialog load from a
   data pack?
8. **Two inputs with one key** (DLG-IN-006). Does the screen draw both controls?
9. **The click events of the static actions** (DLG-ACT-001). Does each static action type give its
   click event?
10. **The template values** (DLG-ACT-003, DLG-ACT-005, DLG-ACT-008, DLG-ACT-009). Does a
    `dynamic/run_command` action give the command that its rules state? Do the template values
    and the escape of a text value follow the rules? Does Escape read the values of the inputs?
11. **The values of a slider** (DLG-ACT-006, DLG-ACT-007). Do the slider positions other than the
    two ends give the stated values? Does the value −2147483648 give the string `-2147483648`?
12. **The pause menu tooltip and the key** (DLG-TAG-001, DLG-TAG-002). Is the tooltip of the pause
    menu button `menu.custom_options.tooltip`? Does a physical press of the key "Quick Actions"
    open the dialog?
13. **The cases of the layout** (DLG-SCR-001 to DLG-SCR-003, DLG-SCR-006). Is the footer height 5
    for a button-list type with no `exit_action`? Does step 8 of DLG-SCR-002 move a warning button
    that is outside the screen? Does the body panel scroll? At which `y` is the text line of
    DLG-SCR-006?
14. **The message widget and the item widget** (DLG-BODY-001, DLG-SCR-010, DLG-SCR-011,
    DLG-CLK-007). Does a `plain_message` wrap its text as DLG-SCR-010 states? Do the hover, the
    press, the focus and the activation key of the two widgets follow the rules?
15. **The multi-line edit box** (DLG-CTL-005 to DLG-CTL-008). Do the limits of code units and of
    lines, the scroll, the cursor keys and the draw follow the rules?
16. **Focus and pause** (DLG-KEY-001, DLG-KEY-003). Does the focus follow DLG-KEY-001 for a dialog
    that opens by an activation key and for a dialog that the server opens? Does `pause` pause a
    local world and no other world?
17. **Escape and the cancel action** (DLG-DATA-013, DLG-KEY-002). Does Escape in a dialog follow
    DLG-KEY-002 for each after action, for each type, and with `can_close_with_escape` `false`?
18. **The screen changes** (DLG-FLOW-004). Does the disconnect screen, the screen of a dimension
    change and the screen that loads a level replace a dialog, with no action?
19. **The commands** (DLG-CLK-002, DLG-CLK-003, DLG-CONF-001). Does a valid command, a command with
    a parse error and a command that needs a permission give the results of the rules? Does each
    button of the confirm screen follow DLG-CLK-003?
20. **The links** (DLG-CLK-004, DLG-CLK-005). Does an `open_url` action with "Prompt on Links" on
    show the screen of DLG-CLK-005, and does each of its buttons follow the rules?
21. **The button "Disconnect"** (DLG-WARN-001). Does it disconnect with the stated reason?
22. **A key of an input that is a key of `additions`** (DLG-ACT-004). Show a `dynamic/custom`
    action with `additions` that hold a key of an input, and capture `custom_click_action`. The
    fixture of DLG-EXP-005 shows no entry of `additions` with the key of an input. Does the value
    of the input replace the entry?
23. **Two inputs with one key** (DLG-IN-006, DLG-DEV-005). Show a `dynamic/custom` action of a
    dialog with two inputs that have the same key, and capture `custom_click_action`. Show a
    `dynamic/run_command` action of such a dialog. The fixture of DLG-EXP-005 has no such dialog.
    Does the later input give the value? Does the vanilla server report no diagnostic at load?
24. **The limits of `custom_click_action`** (DLG-WIRE-006, DLG-DEV-004). The fixture of
    DLG-EXP-008 names each raw connection with a label, for example `A32768` and `D16`. The saved
    logs do not hold the `L` and the NBT that each connection sent. Record `L`, the size of the
    payload and the nesting depth of each case with its result. Does `L` = 65536 decode and
    `L` = 65537 disconnect? Which size of payload and which depth does the budget accept? The
    line of `Raw0` shows no decode error, but a read of the reference gives a decode error for
    `L` = 0. Does `L` = 0 disconnect?
25. **The claims that no fixture shows** (section 0.2). The fixture of DLG-EXP-001 holds error
    cases only. Run each case again with a client that is online, and record the whole command
    and the decoded fields. Which of these claims are correct?
    - The defaults and the valid values of DLG-DATA-009, DLG-DATA-011, DLG-BODY-002,
      DLG-IN-001, DLG-IN-003 to DLG-IN-005, DLG-ACT-001 and DLG-ACT-002.
    - The valid combinations of `pause` and `after_action` (DLG-DATA-005), and the load of a
      pack with the dialog of DLG-DATA-005.
    - The template values of DLG-ACT-005. The fixture of DLG-EXP-005 shows the tag values only.
    - The decode of `show_dialog` with no quota, and a number past the last entry
      (DLG-WIRE-003). The packet `clear_dialog` in the configuration phase (DLG-WIRE-004).

---

## 18. Widget kinds

A dialog screen needs these widget kinds. Each kind reports its value by the key of its input,
when it has one.

1. **Button** with a caption, a fixed width, an optional tooltip and an action.
2. **Image button** of 20 × 20 with three sprites and a tooltip: the warning button.
3. **Checkbox** with a label of up to two rows. It reports checked or not checked.
4. **Slider** over a real range with an optional step. Its caption is a translation of the label
   and the value. It reports its thumb position, and DLG-ACT-006 makes the value from it. The
   thumb does not snap.
5. **Cycle button** over a list of options, with the caption forms "name and value" and
   "value". It reports the id of the selected option.
6. **One-line edit box** with a limit of characters. It reports its text.
7. **Multi-line edit box**. It has a fixed width and height, a limit of characters, an
   optional limit of wrapped lines, Enter for a line break, its own scrollbar, its own wheel
   scroll and a character counter below it. It reports its text with line breaks.
8. **Message widget**: wrapped text at a set width, centred lines, a padding of 4, and an
   outline only when focused. It can take the focus. It shows the hover events of the text,
   runs the click events of the text on a press and on an activation key, and sets the pointer
   to a hand over a click event.
9. **Item widget**. It shows one stack at 16 × 16 in a box of a set size, with optional
   decorations and an optional stack tooltip. It can take the focus.
10. **Element with a label**: a label above a control.
11. **Layouts**: a row and a column with a gap and alignment, a grid with a cell that spans all
    columns, and the header, content and footer layout.
12. **Scroll panel** for the body, with a wheel step of 10.

Scroll in scroll: the body panel holds the multi-line edit box, and both can scroll. A wheel event
goes to the innermost widget that can scroll in that direction, and then to the panel. The
panel scrolls to a widget that gets the focus by a key.

Text features: the body needs the wrap of a text at a set width, the hover events of the text
(`show_text`, `show_item`, `show_entity`), the click events of the text, the test of a click
event at a point of a line, and the first click event of a text in the order of its parts.
The header needs a one-line text with no maximum width.

Screen features: a screen with a previous screen; Escape that a screen can refuse; a screen
that the network can replace; a pause flag of a screen; a timer of ticks for a screen.
