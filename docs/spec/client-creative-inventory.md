# The creative inventory screen of the client: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states the inventory screen that the vanilla client shows to a player with the
creative flag. It states the tabs and their content, the scroll, the search and the destroy slot.
It states how each click in the screen changes the stacks and the cursor, and which
`set_creative_mode_slot` packets the client sends. It also states the layout of the screen as
data. `inventory.md`, section 5, states what the server does with the packets. The document does
not prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Stack**, **empty stack**, **count**, **maximum stack size**, **component patch**, **saved
  form**: as `item-stack.md`, section 0.1, defines them.
- **Cursor**, **drag**: as `inventory-transactions.md`, section 0.1, defines them. The click
  kinds are those of ITX-JAV-006. The drag kind is that of ITX-JAV-015.
- **Menu**, **inventory menu**, **player inventory**, **infinite materials**: as `inventory.md`,
  section 0.1, defines them. Each element of a menu is an **entry**, with its menu index. `M[i]`
  is the entry with the menu index `i` of the inventory menu (INV-MNU-001). `P[i]` is the slot
  `i` of the player inventory.
- **Static registry**, **data-pack registry**, **tag**: as `registry.md`, section 0.1, defines
  them.
- **Screen**, **in a world**: as `client-widget.md`, section 0.1, defines them.
- **Local player**: as `client-camera.md`, section 0.1, defines it.
- **Tick**: as `client-entities.md`, section 0.1, defines it. A tick of the client is a **client
  tick**.
- **GUI unit**, **W**, **H**: as `client-chat.md`, section 0.1, defines them.
- **Mouse position**: as `client-tooltip.md`, section 0.1, defines it.
- **Wheel event**, **notch**: as `entity-physics.md`, section 0.1, defines them.
- **Creative flag**: as `client-block-breaking.md`, section 0.1, defines it.
- **Hotbar keys**, **toolbar activators**: as `client-hotbar.md`, section 0.1, defines them. A
  **hotbar key** is one of the hotbar keys.
- **Off-hand key**: the key binding `key.swapOffhand`.
- **Window area**, **entry area**, **empty area**, **target entry**, **pointer index**, **cursor
  stack**: as `client-inventory.md`, section 0.1, defines them. They apply to each menu screen,
  as `client-container-screen.md`, section 0.1, states.
- **Menu screen**, **item tooltip**, **creative tooltip**, **part**: as
  `client-container-screen.md`, section 0.1, defines them. CCI-TIP-001 states the parts that a
  creative tooltip adds.
- **Creative screen**: the screen of this document.
- **Figure**: as `client-player-render.md`, section 0.1, defines it. The figure rectangle, the
  feet point and the eye point are those of `client-player-render.md`, CPR-FIG-001 and
  CPR-FIG-002.
- **Enabled features**: the set of feature flag ids that the last `update_enabled_features`
  packet gave (`registry.md`, REG-NET-001).
- **Operator condition**: true when each of these is true:
  - the creative flag is on;
  - the last permission level that the client got is `gamemasters` or higher
    (`command-permissions.md`, PERM-LEVEL-003);
  - the option "Operator Items Tab" (`client-controls.md`) is on.
- **Tab**: one entry of the list of CCI-TAB-001. A tab has a kind: items, saved hotbars, search
  or survival inventory.
- **Items tab**: a tab of the kind items.
- **Selected tab**: the one tab whose content the screen shows.
- **Visible tab**: a tab that a click can select, and that the screen draws also when it is not
  the selected tab (CCI-TAB-002).
- **Creative report**: the generated report `assets/mcrs/reports/creative_tabs.json`
  (CCI-CNT-001).
- **Tab list**: the ordered list of stacks that an items tab shows (CCI-CNT-002).
- **Search list**: the ordered list of stacks that an items tab gives to the search tab
  (CCI-CNT-002).
- **Group**: one row of the table of CCI-CNT-003. A group gives a stack for each element of a
  registry.
- **Span**: a sequence of one stack or more that are adjacent in a list of the creative report.
  The span of a group is the span of the stacks of that group (CCI-CNT-003).
- **Same stack**: two stacks with the same item and the same component patch. The count has no
  effect.
- **Content**: the ordered list of stacks that the selected tab shows in the grid (CCI-TAB-004).
- **Grid**: the 5 rows of 9 slots of the creative screen that show a part of the content.
- **Grid slot**: one slot of the grid.
- **Hotbar row**: the 9 slots below the grid in each tab except the survival inventory tab. The
  slot `c` of the hotbar row shows `P[c]`.
- **Destroy slot**: the slot of the survival inventory tab that deletes stacks (CCI-CLK-007).
- **Row count**: `R = ceil(n / 9) − 5`. `n` is the number of stacks of the content.
- **Scroll offset**: a real number `s` from 0 to 1 that selects the first row of the content
  that the grid shows (CCI-SCR-001).
- **Search text**: the text of the search box (CCI-SRH-001).
- **Search parts**: the parts of the creative tooltip of a stack that the name match reads, as
  plain text (CCI-SRH-002).
- **Saved hotbar**: one of the 9 lists of 9 stacks that the client stores outside a world.
- **Creative drop**: a `set_creative_mode_slot` packet with the slot number −1 (CCI-SYN-003).
- **Entry change packet**: a `set_creative_mode_slot` packet with a slot number in `0..45`
  (CCI-SYN-002).

### 0.2 Provenance, kind and observer

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents". The language keys and their English texts are in
`assets/minecraft/lang/en_us.json` of the client jar.

The observer of a rule of the kind wire is the server that reads the serverbound packets. The
observer of each other rule is a player in normal play who sees the screen.

Each rule with the label `source_suggested` names its experiment in section 12, "Unresolved".
When a rule has a step of the kind deviation, its provenance line names that step. That step has
the label `design`, and section 11 records the reference behaviour.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `CCI-OPEN` | the open, the close and the change to the survival inventory screen |
| `CCI-TAB` | the tabs, their order, which tabs show, and the selection of a tab |
| `CCI-CNT` | the content of each tab, the creative report and the rebuild of the content |
| `CCI-SCR` | the scroll: the wheel and the scroll bar |
| `CCI-SRH` | the search box, the search text and the match |
| `CCI-CLK` | the result of each click kind and key on the stacks and on the cursor |
| `CCI-SYN` | the `set_creative_mode_slot` packets and the clientbound packets during the screen |
| `CCI-HOT` | the saved hotbars tab and the load of a saved hotbar |
| `CCI-TIP` | the parts that the creative screen adds to a tooltip |
| `CCI-LAY` | the layout: sizes, positions, textures and sprites |
| `CCI-EXP` | the experiments that have not run (section 12) |
| `CCI-DEV` | intentional deviations |

### 0.4 Scope

In scope:

- the creative screen in a world, for a local player with the creative flag;
- the tabs, the content of each tab and the creative report;
- the scroll, the search and the destroy slot;
- the result of each click in the creative screen, and the packets that the client sends;
- the load and the save of a saved hotbar with the toolbar activators of `client-hotbar.md`;
- the layout of the creative screen as data;
- where the figure shows in the survival inventory tab, its size and its draw order.

Out of scope:

- the server side of `set_creative_mode_slot`: `inventory.md`, INV-CRE-001 to INV-CRE-006;
- the change of mouse buttons and keys into clicks: `client-inventory.md`;
- the slot highlight, the drag, the double click and the item tooltip:
  `client-container-screen.md`;
- the survival inventory screen and the clientbound packets that change the stacks of the
  inventory menu: `client-inventory.md`;
- the draw of a tooltip: `client-tooltip.md`;
- the edit rules of the search box: `client-edit-box.md`;
- the model, the skin, the state, the turn to the mouse and the light of the figure:
  `client-player-render.md`, section 5;
- the file of the saved hotbars and its format;
- the status effects that the screen draws beside it.

### 0.5 Constants

| Value | Data | Rule |
|-------|------|------|
| Size of the creative screen | 195 × 136 GUI units | CCI-LAY-001 |
| Grid | 5 rows, 9 columns, 18 GUI units apart | CCI-LAY-003 |
| Count of a stack of a tab list | 1 | CCI-CNT-002 |
| Maximum length of the search text | 50 characters (EDIT-OPT-002) | CCI-SRH-001 |
| Slot number of a creative drop | −1 | CCI-SYN-003 |
| Container id of the `container_close` at the close | 0 | CCI-OPEN-002 |
| Drop counter of the client: increment, limit | 20 for each creative drop, 1280 | CCI-SYN-003 |
| First tab | `minecraft:building_blocks` | CCI-TAB-003 |
| Figure rectangle | (73, 6) to (105, 49), 32 × 43 GUI units | CCI-LAY-007 |
| Size of the figure | 20 GUI units for each block | CCI-LAY-007 |

---

## 1. Open and close

### CCI-OPEN-001: The open of the creative screen

- **Inputs:** a press of the key binding `key.inventory` while the client is in a world and no
  screen is open.
- **Behavior:**
  1. When the creative flag is on, the client opens the creative screen. Else it opens the
     survival inventory screen of `client-inventory.md`.
  2. The creative screen builds the content of the tabs again when an input of CCI-CNT-005
     changed since the last build.
  3. The creative screen selects the tab of CCI-TAB-003.
  4. The search text is empty.
  5. The scroll offset is 0.
  6. The client sends no packet (CCI-DEV-001).
- **Postconditions:**
  - The open does not change the cursor stack. After a close of the creative screen, the cursor
    holds the empty stack (CCI-OPEN-002, step 2).
  - After a close of the survival inventory screen, the cursor can hold the stack that
    `client-inventory.md`, CINV-CLS-002, keeps. This occurs when the `set_cursor_item` of the
    server did not arrive before the open. CCI-SYN-004, step 1, then ignores that packet. The
    stack stays on the cursor until the close of the creative screen.

Provenance: `source_suggested`; `design` for step 6. Kind: semantic; deviation for step 6.
Confidence: medium. Experiment: CCI-EXP-001.

### CCI-OPEN-002: The close of the creative screen

- **Inputs:** one of these events while the creative screen shows:
  - a press of the key Escape;
  - a press of the key binding `key.inventory` while the search tab is not selected;
  - each other event of `client-inventory.md`, CINV-CLS-001, with the creative screen in the
    place of the inventory screen.
- **Behavior:**
  1. The client sends `container_close` with the container id 0.
  2. The cursor of the client becomes the empty stack. The client sends no packet for the cursor.
  3. The client shows no screen.
- **Postconditions:**
  - The stack that the cursor held is gone. It does not go to the player inventory, and it does
    not become an item entity (CCI-DEV-002).
  - The server does the steps of INV-CLS-001 for the inventory menu.
  - A clientbound `container_close` closes the creative screen, as `client-inventory.md`,
    CINV-CLS-003, states. Step 2 also applies then.
- **Edge cases:** while the search tab is selected, `key.inventory` types its character into the
  search box (CCI-SRH-001). A key closes the screen then only with Escape.

Provenance: `source_suggested`; `design` for step 2. Kind: semantic; deviation for step 2.
Confidence: medium. Experiment: CCI-EXP-001.

### CCI-OPEN-003: A change of the creative flag while a screen shows

- **Inputs:** a change of the creative flag while the creative screen or the survival inventory
  screen shows.
- **Behavior:**
  1. The client checks the rows of the table below at each client tick. The first true row gives
     the result.
  2. The change sends no `container_close`.
  3. In row 1, the cursor of the creative screen becomes the empty stack, as in CCI-OPEN-002,
     step 2 (CCI-DEV-002).

| # | Condition | Result |
|---|-----------|--------|
| 1 | The creative screen shows, and the creative flag is off. | The survival inventory screen replaces the creative screen. |
| 2 | The survival inventory screen shows, and the creative flag is on. | The creative screen replaces the survival inventory screen, as CCI-OPEN-001 opens it. |

- **Postconditions:** in row 2, the cursor of the creative screen keeps the cursor stack of the
  survival inventory screen.

Provenance: `source_suggested`; `design` for step 3. Kind: semantic; deviation for step 3.
Confidence: medium. Experiment: CCI-EXP-001.

---

## 2. Tabs

### CCI-TAB-001: The tabs and their order

- **Behavior:** the creative screen has the 14 tabs of this table. The order of the tabs is the
  order of the rows. It is the order of the protocol ids of the static registry
  `minecraft:creative_mode_tab` in `assets/mcrs/reports/registries.json`. The data of each tab
  is the data of its row. The creative report does not repeat it (CCI-CNT-001).

| Tab id | Kind | Row | Column | Right | Title key | Icon | Background |
|--------|------|-----|--------|-------|-----------|------|------------|
| `minecraft:building_blocks` | items | top | 0 | no | `itemGroup.buildingBlocks` | `minecraft:bricks` | items |
| `minecraft:colored_blocks` | items | top | 1 | no | `itemGroup.coloredBlocks` | `minecraft:cyan_wool` | items |
| `minecraft:natural_blocks` | items | top | 2 | no | `itemGroup.natural` | `minecraft:grass_block` | items |
| `minecraft:functional_blocks` | items | top | 3 | no | `itemGroup.functional` | `minecraft:oak_sign` | items |
| `minecraft:redstone_blocks` | items | top | 4 | no | `itemGroup.redstone` | `minecraft:redstone` | items |
| `minecraft:hotbar` | saved hotbars | top | 5 | yes | `itemGroup.hotbar` | `minecraft:bookshelf` | items |
| `minecraft:search` | search | top | 6 | yes | `itemGroup.search` | `minecraft:compass` | item search |
| `minecraft:tools_and_utilities` | items | bottom | 0 | no | `itemGroup.tools` | `minecraft:diamond_pickaxe` | items |
| `minecraft:combat` | items | bottom | 1 | no | `itemGroup.combat` | `minecraft:netherite_sword` | items |
| `minecraft:food_and_drinks` | items | bottom | 2 | no | `itemGroup.foodAndDrink` | `minecraft:golden_apple` | items |
| `minecraft:ingredients` | items | bottom | 3 | no | `itemGroup.ingredients` | `minecraft:iron_ingot` | items |
| `minecraft:spawn_eggs` | items | bottom | 4 | no | `itemGroup.spawnEggs` | `minecraft:creeper_spawn_egg` | items |
| `minecraft:op_blocks` | items | bottom | 5 | yes | `itemGroup.op` | `minecraft:command_block` | items |
| `minecraft:inventory` | survival inventory | bottom | 6 | yes | `itemGroup.inventory` | `minecraft:chest` | inventory |

- **Notes on the columns:**
  - "Right" is the right alignment of CCI-LAY-002.
  - The icon is a stack of count 1 with no component patch.
  - CCI-LAY-001 gives the texture of each background.
  - The survival inventory tab has no title in the screen and no scroll bar. Each other tab has
    both.
- **Postconditions:** the title of a tab is the text of its title key in the language of the
  client.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: CCI-EXP-002.

### CCI-TAB-002: Which tabs are visible

- **Behavior:**
  1. The saved hotbars tab, the search tab and the survival inventory tab are always visible.
  2. An items tab is visible when its tab list has one stack or more (CCI-CNT-002).
- **Postconditions:**
  - The tab `minecraft:op_blocks` is visible only when the operator condition is true, because
    its tab list is empty in the other case (CCI-CNT-002, step 4).
  - An items tab whose stacks the enabled features all disable is not visible.
  - A tab that is not visible leaves an empty place. The other tabs keep their positions.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-002.

### CCI-TAB-003: The selected tab at the open

- **Inputs:** the tab that was selected when the last creative screen closed, during the same run
  of the client. Another world or another server does not reset it.
- **Behavior:**
  1. At the first open in a run of the client, the selected tab is `minecraft:building_blocks`.
  2. At a later open, the selected tab is the tab of the last creative screen.
  3. When that tab is not visible, the selected tab is `minecraft:building_blocks`.
  4. When `minecraft:building_blocks` is not visible, it still becomes the selected tab in steps
     1 and 3. The content is then empty. The screen draws the selected tab also when it is not
     visible (CCI-LAY-002, step 2).
- **Postconditions:** the search text is empty at each open, also when the search tab is
  selected.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-002.

### CCI-TAB-004: The selection of a tab

- **Inputs:** a tab `T`, and one of these events:
  - the release of the left mouse button while the mouse position is in the click rectangle of
    the visible tab `T` (CCI-LAY-002);
  - an open of the creative screen with `T` from CCI-TAB-003;
  - a selection by CCI-TAB-005 or CCI-CNT-005.
- **Behavior:**
  1. `T` becomes the selected tab.
  2. A drag that is not complete ends with no result.
  3. The content becomes the list of the table below.
  4. The search text becomes empty. The exception is a release on the search tab while the search
     tab is already selected: the search text does not change.
  5. The scroll offset becomes 0.

| Kind of `T` | Content |
|-------------|---------|
| items | the tab list of `T` |
| saved hotbars | the 81 stacks of CCI-HOT-001 |
| search | the result of CCI-SRH-002 for the search text |
| survival inventory | no content; the screen shows the slots of CCI-LAY-004 in place of the grid |

- **Postconditions:**
  - The press of the left button on a tab has no other result.
  - The selection sends no packet.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-002.

### CCI-TAB-005: The chat key selects the search tab

- **Inputs:** a press of the key binding `key.chat` while the creative screen shows and the
  selected tab is not the search tab.
- **Behavior:**
  1. The search tab becomes the selected tab, as CCI-TAB-004 states.
  2. The character of that key press does not go into the search box.
- **Postconditions:** with the default keys, the key T selects the search tab, and the search
  text stays empty.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-005.

---

## 3. The content of the tabs

### CCI-CNT-001: The creative report

The tab lists are not in the data pack. The client reads them from the creative report.

- **Shape:** the creative report is one JSON object with the fields of the first table below.
  Each field comes from a run of the game with the vanilla data pack. In that run, the enabled
  features hold each feature flag, and the operator condition is true.

| Field | Type | Meaning |
|-------|------|---------|
| `item_features` | object | For each item that needs a feature flag other than `minecraft:vanilla`: the item id, and the list of the feature flag ids that the item needs. An item that is not in the object needs only `minecraft:vanilla`. |
| `tabs` | object | For each items tab of CCI-TAB-001: the tab id, and an object with the fields of the second table. |

| Field | Type | Meaning |
|-------|------|---------|
| `needs_operator` | boolean | `true` when the game gives an empty tab list and an empty search list for the tab in a second run, with the operator condition false. |
| `tab_list` | list | The tab list that the game gives for the tab, in order. |
| `search_list` | list | The search list that the game gives for the tab, in order. |

- **Values:** each element of `tab_list` and `search_list` is a stack of count 1, in the saved
  form of `item-stack.md`.
- **Constraints:** the client rejects a creative report in each of these cases. It then shows no
  items tab.
  - The report has a field that these tables do not name.
  - The report has no field that these tables name.
  - A key of `tabs` is not an items tab of CCI-TAB-001, or an items tab is not a key of `tabs`.
  - A list of the report does not hold the stacks of each group of that list as one span
    (CCI-CNT-003).
- **Example:** the start of a creative report.

```json
{
  "item_features": {},
  "tabs": {
    "minecraft:building_blocks": {
      "needs_operator": false,
      "tab_list": [
        { "id": "minecraft:oak_log" },
        { "id": "minecraft:oak_wood" }
      ],
      "search_list": [
        { "id": "minecraft:oak_log" },
        { "id": "minecraft:oak_wood" }
      ]
    }
  }
}
```

Provenance: `design`. Kind: semantic. Confidence: high. Reason: the game builds the tab lists in
its code. A run of the game gives the lists, the feature flags of each item and `needs_operator`.
No output of the game gives the groups, so CCI-CNT-003 states them as a rule. The data of the tabs
is fixed for the version, and CCI-TAB-001 states it.

### CCI-CNT-002: The tab list and the search list of an items tab

- **Inputs:**
  - the `tab_list` and the `search_list` of the tab in the creative report;
  - the enabled features and the operator condition;
  - the registries and the tags that the server sent.
- **Behavior:** the client builds the tab list of an items tab from its `tab_list`, and the
  search list from its `search_list`. For each of the two lists, it does these steps:
  1. The client replaces the span of each group of the list by the stacks of that group for the
     world (CCI-CNT-003). A group with no span in the list gives no stack (CCI-CNT-003, step 6).
  2. The client decodes each other stack with the registries of the world. When the decode fails,
     the client removes the stack (CCI-DEV-004).
  3. The client removes each stack when the enabled features do not contain all the feature flags
     of the stack (CCI-CNT-004).
  4. When `needs_operator` is true and the operator condition is false, the list is empty.
  5. The client removes each stack that is the same stack as an earlier stack of the list
     (CCI-DEV-003).
- **Postconditions:** the order of each list is the order of the report. The stacks of a group
  are at the place of its span, in the order of CCI-CNT-003.

Provenance: `source_suggested`; `design` for steps 2 and 5. Kind: semantic; deviation for steps
2 and 5. Confidence: medium. Experiment: CCI-EXP-003.

### CCI-CNT-003: The groups

- **Behavior:**
  1. A stack of a list of the report is in a group when these conditions are true:
     - the group is for the tab and for the list of the stack, in the table below;
     - the stack has the item of the group;
     - the saved form of the stack has the component of the group.
  2. The client does not decode the stacks of a group. The span of the group in the report only
     gives the place of the group in the list.
  3. The stacks of a group for the world are one stack or more for each element `e` of the
     registry `Q` of the group. The column "Elements" selects the elements. The column "Order"
     gives their order.
  4. Each stack of a group has the count 1 and the item of the group. It also has the component
     of the group, with the value of the second table below.
  5. When the registry `Q` or the tag does not exist in the world, the group gives no stack.
  6. When a list of the report has no stack of a group for that list, the group gives no stack
     in that list. The span is the only place of a group (step 2).

| # | Tab | List | Item | `Q` | Elements | Order |
|---|-----|------|------|-----|----------|-------|
| 1 | `minecraft:functional_blocks` | both | `minecraft:painting` | `minecraft:painting_variant` | in the tag `minecraft:placeable` | size |
| 2 | `minecraft:tools_and_utilities` | both | `minecraft:goat_horn` | `minecraft:instrument` | in the tag `minecraft:goat_horns` | tag |
| 3 | `minecraft:combat` | both | `minecraft:tipped_arrow` | `minecraft:potion` | each | registry |
| 4 | `minecraft:food_and_drinks` | both | `minecraft:potion` | `minecraft:potion` | each | registry |
| 5 | `minecraft:food_and_drinks` | both | `minecraft:splash_potion` | `minecraft:potion` | each | registry |
| 6 | `minecraft:food_and_drinks` | both | `minecraft:lingering_potion` | `minecraft:potion` | each | registry |
| 7 | `minecraft:ingredients` | tab list | `minecraft:enchanted_book` | `minecraft:enchantment` | each | registry |
| 8 | `minecraft:ingredients` | search list | `minecraft:enchanted_book` | `minecraft:enchantment` | each | registry |
| 9 | `minecraft:op_blocks` | both | `minecraft:painting` | `minecraft:painting_variant` | not in the tag `minecraft:placeable` | size |

| Groups | Component | Value for `e` | Stacks for one element |
|--------|-----------|---------------|------------------------|
| 1, 9 | `minecraft:painting/variant` | `e` | 1 |
| 2 | `minecraft:instrument` | `e` | 1 |
| 3 to 6 | `minecraft:potion_contents` | `{"potion": e}` | 1 |
| 7 | `minecraft:stored_enchantments` | `{e: L}` | one, with `L` the `max_level` of `e` |
| 8 | `minecraft:stored_enchantments` | `{e: L}` | one for each `L` from 1 to the `max_level` of `e`, in the order from the smallest `L` |

The level `L` of a stored enchantment has the range `1..255` (`item-stack.md`, STK-SHP-015). A value of `L` out of this range gives no stack.

- **Definitions:** the values of the column "Order":
  - registry: the order of the numeric ids of `Q`. For a data-pack registry, this is the order of
    the `registry_data` packet. For a static registry, this is the order of the protocol ids in
    `registries.json`.
  - tag: the order of the entries of the tag in the last `update_tags` packet.
  - size: the order from the smallest area `width × height` of the painting variant. Two variants
    with the same area are in the order from the smallest `width`. Two variants with the same
    area and the same `width` keep the order registry.
- **Postconditions:**
  - A data pack that adds a painting variant to the tag `minecraft:placeable` adds a painting to
    the tab `minecraft:functional_blocks`, at the place of its size.
  - A data pack that adds an enchantment adds its enchanted books to the tab
    `minecraft:ingredients` and to the search tab.
  - The creative report needs no change for such a data pack.
  - A creative report from a run with an empty tag `minecraft:goat_horns` gives no goat horn. This
    is also true in a world in which that tag has elements (CCI-DEV-010).
  - A stack of the report that is in no group is a fixed stack, for example the ominous banner of
    `minecraft:functional_blocks`. Its banner patterns are ids of `minecraft:banner_pattern`, so
    its decode can fail (CCI-CNT-002, step 2).

Provenance: `source_suggested`; `design` for steps 1, 2 and 6. Kind: semantic; deviation for
step 6. Confidence: medium. Experiment: CCI-EXP-003.

### CCI-CNT-004: The feature flags of a stack

- **Behavior:** the feature flags of a stack are the flags of its item in `item_features`, or
  `minecraft:vanilla` when the item is not there. No element of the registry `Q` of a group needs
  a feature flag.
- **Postconditions:** a stack shows only when the enabled features contain each of its flags.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-003.

### CCI-CNT-005: The rebuild of the content

- **Inputs:** these inputs of the lists:
  1. the enabled features;
  2. the operator condition;
  3. the registries that a configuration phase gave (`registry.md`, REG-NET-001);
  4. the tags that the last `update_tags` packet gave;
  5. the language of the client, for the search parts of CCI-SRH-002 only.
- **Behavior:** when one of these inputs changes, the client builds the tab lists, the search
  lists and the search parts of each stack again. While the creative screen shows, the new lists
  show at the latest in the next client tick. Then:
  1. When the selected tab is an items tab with an empty tab list, `minecraft:building_blocks`
     becomes the selected tab (CCI-TAB-004). It becomes the selected tab also when it is not
     visible. The content is then empty, as in CCI-TAB-003, step 4.
  2. Else the content becomes the new list of CCI-TAB-004 for the selected tab. The first visible
     row stays the same, as CCI-SCR-002 states.
- **Postconditions:**
  - An operator that the server deops while the creative screen shows loses the tab
    `minecraft:op_blocks` in the next client tick.
  - A `/reload` that changes the tag `minecraft:placeable` of painting variants changes the
    paintings of the tabs (CCI-DEV-005).

Provenance: `source_suggested`; `design` for input 4. Kind: semantic; deviation for input 4.
Confidence: medium. Experiment: CCI-EXP-004.

### CCI-CNT-006: The content of the search tab

- **Behavior:** the search list of the search tab is the concatenation of the search lists of the
  items tabs, in the order of CCI-TAB-001. The client does not add a stack that is the same stack
  as an earlier stack of the concatenation.
- **Postconditions:**
  - The search tab with an empty search text shows each stack of this list.
  - The search tab holds the enchanted books of each level. The tab `minecraft:ingredients` holds
    only the books of the highest level.
  - The search tab holds the stacks of `minecraft:op_blocks` only when the operator condition is
    true.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-003.

---

## 4. Scroll

### CCI-SCR-001: The grid shows the content from the first row

- **Inputs:** the content of `n` stacks, the row count `R`, and the scroll offset `s`.
- **Behavior:**
  1. The first row is `r0 = max(floor(s · R + 0.5), 0)`.
  2. The grid slot in the row `y` and the column `x` shows the stack with the index
     `x + 9 · (y + r0)` of the content. When that index is `n` or more, the grid slot shows the
     empty stack.
  3. The content can scroll when the tab has a scroll bar and `n > 45`.
- **Postconditions:** when the content cannot scroll, `s` stays 0, and the grid shows the first 45
  stacks.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-006.

### CCI-SCR-002: The wheel and the change of the content

- **Behavior:**
  1. A wheel event with the vertical value `v` changes `s` to `clamp(s − v / R, 0, 1)` when the
     content can scroll. A part of the screen that takes the wheel event first prevents this step.
  2. When the content changes and the selected tab does not change (CCI-CNT-005), `s` becomes
     `clamp(r0 / R, 0, 1)`. `r0` is the first row before the change, and `R` is the row count
     after the change. This step applies only when `R` is 1 or more.
  3. When the row count `R` after such a change is 0 or less, `s` becomes 0 (CCI-DEV-009). No
     scroll occurs until a change of the content gives a row count of 1 or more.
  4. A change of the search text sets `s` to 0 (CCI-SRH-001, step 6).
- **Postconditions:** one notch of the wheel moves the grid by one row.

Provenance: `source_suggested`; `design` for step 3. Kind: semantic; deviation for step 3.
Confidence: medium. Experiment: CCI-EXP-006.

### CCI-SCR-003: The scroll bar

- **Inputs:** the left mouse button, and the scroll bar rectangle of CCI-LAY-006.
- **Behavior:**
  1. A press of the left button in the scroll bar rectangle starts a scroll drag when the content
     can scroll. The press has no other result, also when the content cannot scroll.
  2. During a scroll drag, each move of the mouse sets `s = clamp((my − top − 25.5) / 97, 0, 1)`.
     `my` is the y value of the mouse position, and `top` is the top edge of the creative screen.
  3. The release of the left button ends the scroll drag.
- **Postconditions:** the press itself does not move the thumb. The survival inventory tab has no
  scroll bar rectangle.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-006.

---

## 5. Search

### CCI-SRH-001: The search box

- **Behavior:**
  1. The search box shows only while the search tab is selected. It has the options of the row
     "search of the creative inventory" of `client-edit-box.md`, EDIT-OPT-002.
  2. While the search tab is selected, the search box has the focus and keeps it.
  3. A key goes to the search box, except Escape and the keys of step 4.
  4. A hotbar key or the off-hand key does the swap click of CCI-CLK-005 only when its key is a
     digit key. The digit keys are 0 to 9 of the main keys and of the numeric keypad. The
     conditions of `client-inventory.md`, CINV-GES-003, also apply: the cursor stack is empty,
     and a target entry exists. A grid slot with the empty stack is the exception: the key then
     goes to the search box.
  5. A click on a grid slot selects all the search text. The next typed character thus replaces
     it.
  6. When a key or a character changes the search text, the content becomes the result of
     CCI-SRH-002.
- **Postconditions:**
  - While the search tab is selected, the drop key, `key.inventory` and the other key bindings of
    the screen type their characters.
  - With the default keys, the keys 1 to 9 do the swap click over a slot. The off-hand key F types
    `f` into the search box.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-005.

### CCI-SRH-002: The match of the search text

- **Inputs:** the search text `q`, and the search list of the search tab (CCI-CNT-006).
- **Behavior:**
  1. When `q` is empty, the content is the search list of the search tab.
  2. When `q` starts with `#`, the client removes the `#` and does the tag match of step 4. Else
     it does the name match of step 3.
  3. The name match. `q` becomes lower case. When `q` has no `:`, a stack matches when one of
     its search parts contains `q`. When `q` has a `:`, `a` is the text before the first `:`, and
     `b` is the text after it. The client removes the spaces at the ends of `a` and of `b`. A
     stack matches when the namespace of its item id contains `a`, and its item id path or one of
     its search parts contains `b`.
  4. The tag match. `q` becomes lower case. When `q` has no `:`, a stack matches when the path of
     an item tag of its item contains `q`. When `q` has a `:`, `a` and `b` are as in step 3. A
     stack matches when one item tag of its item has a namespace that contains `a` and a path
     that contains `b` (CCI-DEV-006).
  5. The content is the stacks of the search list that match, in the order of the search list.
- **Definitions:**
  - "Contains" is a substring test. Each text contains the empty text.
  - "Lower case" is the Unicode lower case map of each character. It does not depend on the
    language of the client.
  - The search parts of a stack are the parts of its creative tooltip, with the advanced
    tooltips off. They include the parts of CCI-TIP-001, steps 2 and 3, and not the parts of its
    step 6. A part is not divided into wrapped lines. The client takes each part as plain text and
    removes each format code (`§` and the next character). It removes the spaces at the ends and
    drops each empty part. Then each part becomes lower case.
  - The client uses the language at the time of the build (CCI-CNT-005).
  - The item tags are those of the last `update_tags` packet.
- **Postconditions:**
  - `diamond` finds "Diamond Sword". `diamond_sword` does not find it, because the name match
    does not read the item id when `q` has no `:`.
  - `minecraft:sword` finds each sword.
  - `#swords` finds each item in the tag `minecraft:swords`.
  - The match ignores the case of `q`. The name match reads the item names in the language of the
    client.

Provenance: `source_suggested`; `design` for step 4. Kind: semantic; deviation for step 4.
Confidence: medium. Experiment: CCI-EXP-005.

---

## 6. Clicks, keys and the cursor

`client-inventory.md` and `client-container-screen.md` state how the mouse buttons and the keys
become clicks of the click kinds of ITX-JAV-006, with a slot index and a button. The creative screen does not send these
clicks. It computes each result in the client and sends the packets of section 7. This section
states the result of each click kind in the creative screen.

### CCI-CLK-001: A click on a grid slot

- **Inputs:** a click on a grid slot `G` of an items tab, of the search tab or of the saved
  hotbars tab. `S` is the stack that `G` shows. `C` is the cursor stack.
- **Behavior:** the first true condition gives the result.
  1. The enabled features do not contain all the feature flags of `S`, or `S` has the component
     `minecraft:creative_slot_lock`: no result.
  2. The click kind is swap: CCI-CLK-005.
  3. The click kind is clone: when `C` is empty and `S` is not empty, `C` becomes `S` with the
     count of its maximum stack size. Else no result.
  4. The click kind is throw: when `S` is not empty, the client makes a creative drop of `S`. The
     count of the drop is 1 for button 0, and the maximum stack size of `S` for each other button.
     `C` does not change.
  5. The click kind is quick craft: no result. A grid slot is never a slot of a drag.
  6. The click kind is pick up all: no result. The creative screen does not permit pick up all
     from a grid slot (CCI-LAY-003, step 8). Thus it never makes this click on a grid slot.
  7. The click kind is pick up or quick move: the first true row of this table gives the result.

| # | Condition | Result on `C` |
|---|-----------|---------------|
| a | `C` and `S` are not empty and are the same stack; button 0; quick move | `C` gets the count of its maximum stack size. |
| b | `C` and `S` are not empty and are the same stack; button 0 | `C` gets one more item, when its count is less than its maximum stack size. |
| c | `C` and `S` are not empty and are the same stack; each other button | `C` loses one item. |
| d | `S` is not empty, and `C` is empty | `C` becomes `S` with the count 1, or with its maximum stack size for quick move. |
| e | button 0 | `C` becomes the empty stack. |
| f | `C` is not empty | `C` loses one item. |

- **Postconditions:**
  - A grid slot never changes. A click does not consume the stacks of the content.
  - A left click takes one item. A left click with the shift key and a click of the middle button
    take a full stack.
  - A left click on a grid slot with another stack, or on an empty grid slot, deletes the cursor
    stack. A right click there deletes one item of it.
  - A click on a grid slot sends no packet, except the creative drop of step 4 and the packets of
    step 2.
  - A double click of the left button on a grid slot gathers no stacks. The release gives a pick
    up click, or a quick move click with Shift (CCS-DBL-002, step 1, and CCS-DBL-003, step 2).
    With an empty cursor, the press gives row d, and the release gives row b or row a.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-007.

### CCI-CLK-002: A click on the hotbar row

- **Inputs:** a click on the slot `c` of the hotbar row, in each tab except the survival inventory
  tab.
- **Behavior:**
  1. The click kind is quick move: `P[c]` becomes the empty stack.
  2. The click kind is throw: the client removes 1 item (button 0), or the full stack (each other
     button), from `P[c]`. The removed stack is a creative drop.
  3. Each other click kind: the client applies the click to the entries `M[36]` to `M[44]`. The
     rules of `inventory-transactions.md`, section 6, apply, for a player with infinite
     materials. Pick up all and the drag use only these entries.
  4. The client sends each entry that the click changes (CCI-SYN-002). A swap with the off-hand
     key also changes `M[45]`.
- **Postconditions:**
  - Shift and a left click on a stack of the hotbar row deletes that stack.
  - A drag of drag kind 2 over the hotbar row makes full stacks (ITX-JAV-020).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-007.

### CCI-CLK-003: A click outside the screen

- **Inputs:** a click with the slot index −999. The mouse position is outside the rectangle of
  the creative screen and outside the click rectangle of each visible tab (CCI-LAY-002). `C` is
  not empty.
- **Behavior:**
  1. Button 0: the client makes a creative drop of `C`. `C` becomes the empty stack.
  2. Button 1: the client makes a creative drop of one item of `C`. `C` loses one item.
  3. The same steps apply in each tab, the survival inventory tab included (CCI-DEV-007).
- **Postconditions:** the item entity comes from the server (`item-entity.md`, IEN-SPN-007). The
  local player starts the swing of the main hand. The swing sends no packet.

Provenance: `source_suggested`; `design` for step 3. Kind: semantic; deviation for step 3.
Confidence: medium. Experiment: CCI-EXP-008.

### CCI-CLK-004: A click in the survival inventory tab

- **Inputs:** a click on a slot of the survival inventory tab that is not the destroy slot. The
  slot shows the entry `M[i]` with `i` in `5..45` (CCI-LAY-004).
- **Behavior:**
  1. The client applies the click with the menu index `i` to its inventory menu. The rules of
     `inventory-transactions.md`, section 6, and of `inventory.md`, section 2, apply. The player
     has infinite materials.
  2. The stack that a throw of step 1 removes is a creative drop. Step 2 removes no more items.
  3. The client sends the changed entries (CCI-SYN-002).
- **Postconditions:** a clone gives `C` a full stack (ITX-JAV-012). A drag of drag kind 2 makes
  items (ITX-JAV-020).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-007.

### CCI-CLK-005: The hotbar keys and the off-hand key over a slot

- **Inputs:** the swap click with the button `k` (INV-MNU-003) over a slot of the creative screen.
- **Behavior:**
  1. Over a grid slot with the stack `S` that is not empty: `P[k]` becomes `S` with the count of
     its maximum stack size. The stack that `P[k]` held is gone.
  2. Over a grid slot with the empty stack: no result.
  3. Over the hotbar row: CCI-CLK-002, step 3.
  4. Over a slot of the survival inventory tab that is not the destroy slot: CCI-CLK-004.
  5. Over the destroy slot: CCI-CLK-007.
  6. The client sends the changed entries (CCI-SYN-002).
- **Postconditions:** with the default keys, the key 3 over a stone of a tab puts 64 stone into
  `P[2]`, and the client sends the entry `M[38]`. Outside the search tab, the off-hand key F puts
  the stack into `P[40]`, and the client sends `M[45]`. In the search tab, CCI-SRH-001, step 4,
  states which keys give the swap click.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-007.

### CCI-CLK-006: The drag

- **Behavior:** a drag in the creative screen uses only the slots that are not grid slots and not
  the destroy slot. At the end of the drag, the client applies the drag to the stacks, as
  ITX-JAV-014 to ITX-JAV-020 state. It then sends the changed entries (CCI-SYN-002).
- **Postconditions:** a drag of drag kind 2 makes items in each tab, because the player has
  infinite materials.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-007.

### CCI-CLK-007: The destroy slot

- **Inputs:** a click on the destroy slot of the survival inventory tab.
- **Behavior:**
  1. The click kind is quick move: each entry `M[0]` to `M[45]` becomes the empty stack. `C` does
     not change.
  2. After a quick move, the client sends the changed entries (CCI-SYN-002). Thus it sends one
     entry change packet with the empty stack for each entry that held a stack (CCI-DEV-001).
  3. Each other click kind: `C` becomes the empty stack. The client sends no packet.
- **Postconditions:**
  - The destroy slot always shows the empty stack. When the cursor is empty, its tooltip is the
    text of the key `inventory.binSlot`, "Destroy Item".
  - When `M[0]` held a stack, a vanilla server refuses the packet with the slot number 0
    (INV-CRE-003, check 4). It sets each other entry that the client sends, the craft grid
    included.

Provenance: `source_suggested`; `design` for step 2. Kind: semantic; deviation for step 2.
Confidence: medium. Experiment: CCI-EXP-007.

---

## 7. Packets

### CCI-SYN-001: The conditions of a `set_creative_mode_slot` packet

- **Behavior:** the client sends a `set_creative_mode_slot` packet only when each of these is
  true:
  - the creative flag is on;
  - the enabled features contain each feature flag of the item of the stack (CCI-CNT-004);
  - for a creative drop: the stack is not empty, and no menu screen other than the creative screen
    shows.
- **Postconditions:** when a condition is false for a packet of a click, the click has no result.
  The stacks and the cursor of the client do not change, and the client sends no packet of that
  click (CCI-DEV-008).

Provenance: `source_suggested`; `design` for the postcondition. Kind: wire; deviation for the
postcondition. Confidence: medium. Experiment: CCI-EXP-007.

### CCI-SYN-002: Entry change packets after a click

- **Inputs:** a click or a key of section 6 that changes stacks of the inventory menu.
- **Behavior:**
  1. After the click, the client compares each entry `M[i]` with its stack before the click.
  2. For each entry that differs, the client sends one `set_creative_mode_slot` packet. The slot
     number is `i`, and the stack is the new stack of `M[i]`.
  3. The packets go in the order of `i`, from 0 to 45.
  4. A creative drop of the same click goes before these packets.
- **Postconditions:**
  - The slot number of the slot `c` of the hotbar row is `36 + c`. The slot number of `P[40]` is
    45. The slot numbers of the survival inventory tab are the menu indices of CCI-LAY-004.
  - The client never sends `container_click` while the creative screen shows.
  - A click that changes only the cursor sends no packet.

Provenance: `source_suggested`; `design` for step 1. Kind: wire; deviation for step 1.
Confidence: medium. Experiment: CCI-EXP-007.

### CCI-SYN-003: The creative drop

- **Behavior:**
  1. A creative drop is a `set_creative_mode_slot` packet with the slot number −1 and the dropped
     stack.
  2. The client has a drop counter. It is 0 when the client joins a world. Each creative drop adds
     20. Each client tick subtracts 1 while the counter is above 0.
  3. A click that drops needs a counter below 1280. Else the click has no result.
- **Postconditions:** a burst can have 64 creative drops. The server has its own counter
  (`item-entity.md`, IEN-SPN-007).

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: CCI-EXP-008.

### CCI-SYN-004: Clientbound packets while the creative screen shows

- **Behavior:**
  1. The client ignores `set_cursor_item`. The cursor keeps its stack.
  2. A `container_set_slot` or a `container_set_content` for the container id 0 changes the
     entries of the inventory menu, as `client-inventory.md` states. The client sends no packet as
     an answer (CCI-DEV-001).
  3. After such a change, the stack before the next click (CCI-SYN-002, step 1) is the new stack.
- **Postconditions:** an item that the player picks up from the ground shows in the hotbar row.

Provenance: `source_suggested`; `design` for step 2. Kind: semantic; deviation for step 2.
Confidence: medium. Experiment: CCI-EXP-009.

---

## 8. Saved hotbars

### CCI-HOT-001: The content of the saved hotbars tab

- **Inputs:** the 9 saved hotbars, at the selection of the tab.
- **Behavior:** the content is 81 stacks, 9 for each saved hotbar. The stacks of the saved hotbar
  `k` have the indices `9k` to `9k + 8` of the content.
  1. The client decodes each stack with the registries of the world. A stack that does not decode
     is the empty stack.
  2. A saved hotbar `k` with only empty stacks gives a row of empty stacks, with one exception.
     The stack with the index `k` in the row is a `minecraft:paper` with these components:
     - `minecraft:creative_slot_lock`;
     - `minecraft:item_name`: the key `inventory.hotbarInfo`, with the key name of
       `key.saveToolbarActivator` and the key name of `key.hotbar.(k+1)` as its arguments.
- **Postconditions:** with the default keys, the English name of the paper of row 0 is "Save
  hotbar with C + 1". A click on it has no result (CCI-CLK-001, step 1).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-010.

### CCI-HOT-002: The load and the save of a saved hotbar

- **Inputs:** a press of the hotbar key `k` while a toolbar activator is down, as
  `client-hotbar.md`, CHB-KEY-001, case 2, states. No screen is open, and the creative flag is
  on.
- **Behavior:**
  1. When `key.loadToolbarActivator` is down, the client loads. Else, when
     `key.saveToolbarActivator` is down, the client saves. The load wins when both are down.
  2. A load does these steps for `j` from 0 to 8:
     - The client decodes the stack `j` of the saved hotbar `k`.
     - A stack that is not empty can need a feature flag that the enabled features do not
       contain. Then `P[j]` does not change, and no packet goes for `j` (CCI-DEV-008).
     - Else `P[j]` becomes the stack, and the client sends one `set_creative_mode_slot` with the
       slot number `36 + j` and the stack.
  3. A save stores `P[0]` to `P[8]` in the saved hotbar `k`. It sends no packet. The HUD shows the
     text of the key `inventory.hotbarSaved` as an overlay message.
- **Postconditions:** a load sends at most 9 packets, in the order of `j`. It also sends a packet
  for a stack that does not change and for an empty stack.

Provenance: `source_suggested`; `design` for step 2. Kind: wire; deviation for step 2.
Confidence: medium. Experiment: CCI-EXP-010.

---

## 9. Tooltips and layout

### CCI-TIP-001: The parts that the creative screen adds to an item tooltip

- **Inputs:** the stack `S` of a slot under the mouse position, and its item tooltip of
  `client-container-screen.md`.
- **Behavior:**
  1. The tooltip of a grid slot is a creative tooltip. The tooltip of each other slot is an item
     tooltip that is not a creative tooltip.
  2. In a creative tooltip, a stack of `minecraft:painting` with no component
     `minecraft:painting/variant` has one more part: the text of the key `painting.random`, with
     the colour `FFAAAAAA`. It is a part that the item adds for itself, in the order of
     `client-container-screen.md`. The part is absent when the `hidden_components` of the
     `tooltip_display` of the stack hold `minecraft:painting/variant`.
  3. In a creative tooltip, the component `minecraft:suspicious_stew_effects` gives the parts of
     its effects, at the place of that component in the order of `client-container-screen.md`.
     These parts are the effect parts that the component `minecraft:potion_contents` gives for the
     same effects, with the duration factor 1. An item tooltip that is not a creative tooltip has
     no such part.
  4. A tooltip with no part gets no added part.
  5. On a grid slot of an items tab, the client adds no part.
  6. On each other slot, the client adds these parts after the first part of the tooltip and
     before its second part, in this order:
     - the tab parts. There is one tab part for each visible items tab whose search list holds
       the same stack as `S`. A tab part is the title of that tab, with the colour `FF5555FF`.
       The tab parts are in the order of CCI-TAB-001;
     - the tag parts, only on a grid slot of the search tab while the search text starts with
       `#`. There is one tag part for each item tag of `S` that the tag match of CCI-SRH-002
       accepts. A tag part is the text `#` and the tag id, with the colour `FFAA00AA`. The order
       of the tag parts is unspecified.
- **Postconditions:** in the search tab, the tooltip of an oak log shows "Building Blocks" in the
  colour `FF5555FF` below its name.

Provenance: `source_suggested`; `design` for the tag parts of step 6. Kind: layout; semantic for
steps 2 and 3; deviation for the tag parts of step 6. Confidence: medium. Experiment: CCI-EXP-011
for steps 2 and 3, CCI-EXP-005 for the other steps.

### CCI-LAY-001: The rectangle and the background

- **Behavior:**
  1. The creative screen is a rectangle of 195 × 136 GUI units. Its top left corner is at
     `left = floor((W − 195) / 2)` and `top = floor((H − 136) / 2)`.
  2. The client draws the region (0, 0) to (195, 136) of the background texture of the selected
     tab at (`left`, `top`). The texture size is 256 × 256.
  3. The window area of the creative screen (`client-container-screen.md`) is this rectangle and
     the click rectangle of each visible tab (CCI-LAY-002). Outside the window area, the pointer
     index is −999 (CCI-CLK-003).

| Background | Texture |
|------------|---------|
| items | `minecraft:textures/gui/container/creative_inventory/tab_items.png` |
| item search | `minecraft:textures/gui/container/creative_inventory/tab_item_search.png` |
| inventory | `minecraft:textures/gui/container/creative_inventory/tab_inventory.png` |

- **Postconditions:**
  - Each position in the rules below is relative to (`left`, `top`), in GUI units.
  - A click rectangle of a tab is not in an entry area, so it is a part of the empty area. The
    pointer index there is −1, and a press there opens no drag (CCS-DRG-001).

Provenance: `source_suggested`; `design` for the tabs in step 3. Kind: layout; deviation for the
tabs in step 3. Confidence: medium. Experiment: CCI-EXP-002.

### CCI-LAY-002: The tabs

- **Behavior:** a tab with the column `c` has the x position `tx`:
  - `tx = 27 · c` when the tab is not aligned right;
  - `tx = 195 − 27 · (7 − c) + 1` when the tab is aligned right.

| Part | Top row | Bottom row |
|------|---------|------------|
| sprite position | (`tx`, −28) | (`tx`, 132) |
| sprite size | 26 × 32 | 26 × 32 |
| unselected sprite | `minecraft:container/creative_inventory/tab_top_unselected_N` | `minecraft:container/creative_inventory/tab_bottom_unselected_N` |
| selected sprite | `minecraft:container/creative_inventory/tab_top_selected_N` | `minecraft:container/creative_inventory/tab_bottom_selected_N` |
| icon position (16 × 16) | (`tx + 5`, −19) | (`tx + 5`, 139) |
| click rectangle, edges included | `tx ≤ x ≤ tx + 26`, `−32 ≤ y ≤ 0` | `tx ≤ x ≤ tx + 26`, `136 ≤ y ≤ 168` |
| hover rectangle | `tx + 2 ≤ x < tx + 25`, `−30 ≤ y < −1` | `tx + 2 ≤ x < tx + 25`, `138 ≤ y < 167` |

  1. `N` is `c + 1`. The sprites are in `textures/gui/sprites/` of the resource pack.
  2. The order of draw: the unselected visible tabs, the background, the search box, the thumb of
     the scroll bar, then the selected tab with its icon.
  3. While the mouse position is in the hover rectangle of a visible tab, the screen shows a
     tooltip with the title of that tab. The first such tab in the order of CCI-TAB-001 gives the
     tooltip.
  4. The click rectangle of a visible tab and the rectangle of the creative screen overlap, for
     example in the row y = 0 of a top tab. There, the tab gets the left button: the press has no
     other result, and the release selects the tab (CCI-TAB-004).
- **Postconditions:** the x positions are 0, 27, 54, 81 and 108 for the columns 0 to 4, 142 for
  the column 5 and 169 for the column 6.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: CCI-EXP-002.

### CCI-LAY-003: The grid, the hotbar row and the search box

- **Behavior:**
  1. The grid slot in the row `y` (`0..4`) and the column `x` (`0..8`) has its item at
     (`9 + 18x`, `18 + 18y`).
  2. The slot `c` of the hotbar row has its item at (`9 + 18c`, 112).
  3. The search box is at (82, 6), with the size 80 × 9.
  4. Each slot of the creative screen has an entry area of 16 × 16 GUI units. Its top left corner
     is the item position of the slot. These slots are the grid slots, the slots of the hotbar
     row, and the slots of CCI-LAY-004 with the destroy slot.
  5. The rules of `client-container-screen.md` take each of these slots as an entry. These rules
     are the target entry, the drag, the double click and the item tooltip.
  6. A grid slot is not an entry of the inventory menu, and it has no menu index. A click on a
     grid slot names that grid slot (CCI-CLK-001). The creative screen sends no click with it
     (CCI-SYN-002).
  7. The menu index of the slot `c` of the hotbar row is `36 + c`, the menu index of `P[c]`.
  8. A grid slot and the destroy slot permit no drag (CCI-CLK-006). They permit no pick up all
     (`client-container-screen.md`, CCS-DBL-002).
- **Postconditions:** the survival inventory tab shows no grid and no hotbar row.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: CCI-EXP-002.

### CCI-LAY-004: The slots of the survival inventory tab

- **Behavior:** the survival inventory tab shows the 41 entries `M[5]` to `M[45]`, and the
  destroy slot. It does not show `M[0]` to `M[4]`, the craft result and the craft grid.

| Entries | Item position |
|---------|---------------|
| `M[5]`, head | (54, 6) |
| `M[6]`, chest | (54, 33) |
| `M[7]`, legs | (108, 6) |
| `M[8]`, feet | (108, 33) |
| `M[9 + 9r + c]`, `r` in `0..2`, `c` in `0..8` | (`9 + 18c`, `54 + 18r`) |
| `M[36 + c]`, `c` in `0..8` | (`9 + 18c`, 112) |
| `M[45]`, off hand | (35, 20) |
| destroy slot | (173, 112) |

- **Postconditions:** the figure of CCI-LAY-007 is between `M[5]` and `M[6]` on the left and
  `M[7]` and `M[8]` on the right. No entry area overlaps the figure rectangle.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: CCI-EXP-002.

### CCI-LAY-005: The title

- **Behavior:** when the selected tab shows its title, the client draws the title at (8, 6), with
  the colour `FF404040` and no shadow.
- **Postconditions:** the survival inventory tab shows no title. In the search tab, the title
  "Search Items" and the search box are on the same row.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: CCI-EXP-002.

### CCI-LAY-006: The scroll bar

- **Behavior:**
  1. In a tab with a scroll bar, the scroll bar rectangle is `175 ≤ x < 189`, `18 ≤ y < 130`.
  2. The thumb is a sprite of 12 × 15 at (175, `18 + floor(95 · s)`).
  3. The sprite is `minecraft:container/creative_inventory/scroller` when the content can scroll.
     Else it is `minecraft:container/creative_inventory/scroller_disabled`.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: CCI-EXP-006.

### CCI-LAY-007: The figure in the survival inventory tab

- **Inputs:** the selected tab; the mouse position; the frame values of the local player.
- **Behavior:**
  1. The creative screen draws the figure only while the survival inventory tab is the selected
     tab. Each other tab shows no figure.
  2. The figure rectangle has the left edge `left + 73` and the top edge `top + 6`. It is 32 GUI
     units wide and 43 GUI units high. CPR-FIG-001, items 2 and 3, apply to this rectangle.
  3. CPR-FIG-002 applies with two changes:
     - The centre (`cx`, `cy`) is (`left + 89`, `top + 27.5`), the centre of the figure
       rectangle of step 2.
     - One block is 20 GUI units. The factor 20 replaces the factor 30 in items 2 to 4.
  4. CPR-FIG-003 applies with the eye point of step 3. The divisor 40 of CPR-FIG-003, item 2,
     does not change.
  5. CPR-FIG-004 and CPR-FIG-005 apply with no change.
  6. CPR-FIG-006 applies, with the background of CCI-LAY-001 in the place of the inventory image.
- **Postconditions:**
  - The figure rectangle is the black rectangle of the background `tab_inventory.png`
    (CCI-LAY-001).
  - The points of the figure have these `y` values:

| Pose | Feet point | Eye point |
|------|------------|-----------|
| `standing` | `top + 46.75` | `top + 14.35` |
| `crouching` | `top + 43.75` | `top + 18.35` |

  - With the mouse at (`left + 89`, `top + 14`), a standing figure has `a` = 0 and `t` = 0.175
    degrees. With the mouse at (`left + 200`, `top + 14`), `a` is 24.50 degrees.
  - No tab sprite, no entry area and no visible pixel of a highlight (CCS-LAY-005) is in the
    figure rectangle. Only the cursor stack and a tooltip cover the figure.
  - A change from the survival inventory tab to another tab removes the figure in the next
    frame.
- **Edge cases:** when the creative flag goes off, the survival inventory screen replaces the
  creative screen (CCI-OPEN-003). That screen draws the figure by CPR-FIG-001 to CPR-FIG-006.

Provenance: `source_suggested_black_box_confirmed` for the figure rectangle: the black rectangle of
`assets/minecraft/textures/gui/container/creative_inventory/tab_inventory.png` in the client jar
is (73, 6) to (105, 49). `source_suggested` for each other step. Kind: layout; semantic for step
1; deviation for the parts of steps 3 and 5 that `client-player-render.md`, CPR-DEV-005 to
CPR-DEV-007, name. Confidence: medium. Experiment: CCI-EXP-012.

---

## 10. Known vanilla quirks

None.

---

## 11. Intentional deviations

### CCI-DEV-001: No packet that repeats a stack back to the server

- **Rule:** CCI-OPEN-001, step 6; CCI-SYN-002, step 1; CCI-SYN-004, step 2; CCI-HOT-002, step 2;
  CCI-CLK-007, step 2.
- **Reference:** the vanilla client sends entry change packets at the open of the creative screen
  and after each click. It also sends them after each `container_set_slot` while the screen shows.
  Each such packet repeats a stack that the server changed since the last send of the client, for
  example a picked up item. After the load of a saved hotbar, it sends a second packet for each
  slot that changed. After a quick move on the destroy slot, it sends 46 entry change packets
  with the empty stack, for the slot numbers 0 to 45 in this order.
- **Reason:** these packets repeat a stack that the server holds already. When the server changes a
  slot two times in a short time, the repeat of the first stack can come after the second change.
- **Visible to the client:** no. The stacks on the server are the same after the packets.
- **Bug reference:** none.
- **Test:** a differential test ignores an entry change packet whose stack is the stack that the
  server holds for that entry.

### CCI-DEV-002: The close deletes the cursor stack

- **Rule:** CCI-OPEN-002, step 2, and CCI-OPEN-003, step 3.
- **Reference:** the vanilla client keeps the cursor stack after the close. The server does not
  know that stack. The next creative screen or survival inventory screen shows it on the cursor
  again, until a packet of the server replaces the cursor.
- **Reason:** the creative screen changes the cursor with no packet (CCI-SYN-002). Thus the server
  does not hold the cursor stack of the client. The survival inventory screen sends each click to
  the server. A cursor stack that the server does not hold gives that screen a local result that
  the server does not give (`client-inventory.md`, CINV-PRD-001).
- **Visible to the client:** yes. In the reference, the stack comes back on the cursor at the next
  open.
- **Bug reference:** none.
- **Test:** a differential test does not compare the cursor after a close of the creative screen.

### CCI-DEV-003: A stack that a list gives two times

- **Rule:** CCI-CNT-002, step 5.
- **Reference:** the vanilla client stops with an error when a tab list or a search list gets a
  stack that the list holds already.
- **Reason:** a data pack can give such a stack, for example with a tag that names an element two
  times. The client must not stop because of data.
- **Visible to the client:** yes, only with such a data pack.
- **Bug reference:** none.
- **Test:** a differential test does not use a data pack that gives a stack two times in one list.

### CCI-DEV-004: A stack of the report that does not decode

- **Rule:** CCI-CNT-002, step 2.
- **Reference:** the vanilla client stops with an error when the ominous banner names a banner
  pattern that the world does not have.
- **Reason:** the client must not stop because of data. The other stacks of the tab stay usable.
- **Visible to the client:** yes, only in a world that does not have such a banner pattern.
- **Bug reference:** none.
- **Test:** a differential test does not use a world in which a stack of the creative report does
  not decode.

### CCI-DEV-005: A new `update_tags` builds the tab lists again

- **Rule:** CCI-CNT-005, input 4.
- **Reference:** after `update_tags` in the play state, the vanilla client builds again only the
  data of the tag match. The tab lists keep the tags of the last configuration phase, for example
  the paintings of `minecraft:placeable` and the goat horns of `minecraft:goat_horns`.
- **Reason:** the tab lists must agree with the tags that the server uses. A data pack that a
  `/reload` changes must show in the creative screen.
- **Visible to the client:** yes, after a `/reload` that changes one of these tags.
- **Bug reference:** none.
- **Test:** a differential test does not use a `/reload` that changes a tag of a group.

### CCI-DEV-006: One item tag matches both parts of a tag search

- **Rule:** CCI-SRH-002, step 4, and the tag parts of CCI-TIP-001, step 6.
- **Reference:** for `#a:b`, a stack matches when one of its tags has a namespace that contains
  `a`. One of its tags must also have a path that contains `b`, and this tag can be another tag.
  For the tag parts of the tooltip, the reference compares the text before the lower case map.
  Thus an upper case letter after `#` gives no tag part.
- **Reason:** a player who types `#a:b` asks for one tag. The tooltip must show the tags that the
  match found.
- **Visible to the client:** yes, with tags in two namespaces, and with upper case letters after
  `#`.
- **Bug reference:** none.
- **Test:** a differential test uses only tag searches with no `:` and with lower case letters.

### CCI-DEV-007: A click outside the screen

- **Rule:** CCI-CLK-003, step 3, and CCI-LAY-001, step 3.
- **Reference:** in the survival inventory tab, a click outside the screen drops the full cursor
  stack with each button. In the other tabs, a right click on a tab that is not selected drops one
  item of the cursor stack.
- **Reason:** a right click outside a menu drops one item in each other screen (ITX-JAV-007). A
  click on a tab must not drop.
- **Visible to the client:** yes.
- **Bug reference:** none.
- **Test:** a differential test does not right click outside the screen or on a tab with a cursor
  stack.

### CCI-DEV-008: A stack that the client cannot send

- **Rule:** CCI-SYN-001, postcondition, and CCI-HOT-002, step 2.
- **Reference:** when a condition of CCI-SYN-001 is false, the vanilla client changes its own
  stacks and sends no packet. For example, a load puts a stack with a disabled item into `P[j]`.
  The player then sees an item that the server does not hold, until the server sends the slot.
- **Reason:** the client must not show a stack that the server does not hold.
- **Visible to the client:** yes, with a saved hotbar from a world with more enabled features.
- **Bug reference:** none.
- **Test:** a differential test does not load a saved hotbar with a disabled item. It does not
  click while a condition of CCI-SYN-001 is false.

### CCI-DEV-009: The scroll offset of a content that cannot scroll

- **Rule:** CCI-SCR-002, step 3.
- **Reference:** when a change of the content gives the row count 0, and the first row before the
  change was above 0, the vanilla client sets `s` to 1. The disabled thumb then shows at the
  bottom of the scroll bar.
- **Reason:** a content that cannot scroll has no scroll offset. The thumb must show at the top.
- **Visible to the client:** yes, the place of the disabled thumb.
- **Bug reference:** none.
- **Test:** a differential test does not compare the thumb after a change of the content to 37
  to 45 stacks.

### CCI-DEV-010: A group with no span in the creative report

- **Rule:** CCI-CNT-003, step 6.
- **Reference:** the vanilla client gives each group a fixed place in each list. The data does not
  change that place. Thus the stacks of a group show at that place when its registry `Q` or its
  tag has elements in the world.
- **Reason:** the creative report is the only source of the order of a list (CCI-CNT-001). Without
  a span, the report gives no place for the group. The client must not select a place that the
  report does not give.
- **Visible to the client:** yes, only with a creative report from a run in which the group gave
  no stack in that list, for example with an empty tag `minecraft:goat_horns`.
- **Bug reference:** none.
- **Test:** a differential test uses only a creative report that has a span for each group of each
  list.

---

## 12. Unresolved

- The text and the duration format of an effect part of `minecraft:potion_contents` and
  `minecraft:suspicious_stew_effects` have no rule yet. `client-container-screen.md` puts these
  component types out of its scope. CCI-TIP-001, step 3, depends on that rule.

These experiments have not run.

- **CCI-EXP-001** (`tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`). The open,
  the close and a change of the game mode. Rules: CCI-OPEN-001 to CCI-OPEN-003.
  1. Start the server. Join with the recorded client in creative mode.
  2. Press `key.inventory`. Wait 5 client ticks.
  3. Take a stone from a grid slot. Press Escape.
  4. Press `key.inventory` again. Run `/gamemode survival` for the player. Wait 2 client ticks.

  Expected result, without the packets that the test of CCI-DEV-001 ignores: step 2 sends no
  packet. Step 3 sends `container_close` with the id 0 and no `set_creative_mode_slot`. In step
  4, the survival inventory screen replaces the creative screen, and no `container_close` goes.
- **CCI-EXP-002** (`tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`). The tabs
  and the layout. Rules: CCI-TAB-001 to CCI-TAB-004, CCI-LAY-001 to CCI-LAY-005.
  1. Start the server. Join with the recorded client in creative mode, as a player that is not an
     operator. Use the GUI scale 1, with W = 427 and H = 240.
  2. Open the creative screen. Take a frame for each visible tab.
  3. Close the screen. Run `/op` for the player from the server console.
  4. Set the option "Operator Items Tab" to on. Open the creative screen. Take a frame again.

  Expected result: in step 2, 13 tabs show at the positions of CCI-LAY-002. In step 4, the tab
  `minecraft:op_blocks` shows at x = 142 of the bottom row. The slots and the title are at the
  positions of CCI-LAY-003 to CCI-LAY-005.
- **CCI-EXP-003** (`tools/vanilla-oracle`). The tab lists and a data pack. Rules: CCI-CNT-002,
  CCI-CNT-003, CCI-CNT-004 and CCI-CNT-006.
  1. Build the lists of each items tab with the vanilla data pack and with the enabled features
     of a new world.
  2. Build the same lists from the creative report by CCI-CNT-002 to CCI-CNT-004. Compare them
     with the lists of step 1.
  3. Add a data pack with one new painting variant of 1 × 1 in `minecraft:placeable`, and one new
     enchantment with `max_level` 3. Do steps 1 and 2 again.

  Expected result: the lists are equal in step 2 and in step 3. In step 3, the new painting is
  among the paintings of the area 1 in `minecraft:functional_blocks`. The book of level 3 is in
  `minecraft:ingredients`, and the books of the levels 1 to 3 are in the search list.
- **CCI-EXP-004** (`tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`). The
  rebuild. Rule: CCI-CNT-005.
  1. Join as an operator in creative mode, with the option "Operator Items Tab" on.
  2. Open the creative screen on the tab `minecraft:op_blocks`.
  3. Run `/deop` for the player from the server console. Wait 2 client ticks.

  Expected result: the tab `minecraft:op_blocks` disappears, and the selected tab is
  `minecraft:building_blocks`.
- **CCI-EXP-005** (`tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`). The
  search. Rules: CCI-TAB-005, CCI-SRH-001, CCI-SRH-002 and CCI-TIP-001.
  1. Open the creative screen in a world with the language `en_us`. Press T.
  2. Type `diamond`. Clear the text and type `diamond_sword`.
  3. Clear the text and type `#swords`. Move the mouse over a diamond sword.
  4. Clear the text and type `minecraft:sword`.

  Expected result: step 1 selects the search tab with an empty text. `diamond` finds the diamond
  sword, and `diamond_sword` finds nothing. Step 3 finds each sword, and the tooltip shows
  "Combat" in the colour `FF5555FF` and `#minecraft:swords` in the colour `FFAA00AA`. Step 4
  finds each sword.
- **CCI-EXP-006** (the client frame capture of `tools/vanilla-oracle`). The scroll. Rules:
  CCI-SCR-001 to CCI-SCR-003 and CCI-LAY-006.
  1. Open the tab `minecraft:building_blocks`. Turn the wheel by one notch toward the player.
     Take a frame.
  2. Drag the thumb to the bottom of the scroll bar. Take a frame.

  Expected result: step 1 moves the grid by one row. In step 2, the last row of the grid holds
  the last stacks of the tab list, and the thumb is at y = 113.
- **CCI-EXP-007** (`tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`). The
  clicks and the entry change packets. Rules: CCI-CLK-001, CCI-CLK-002, CCI-CLK-004 to
  CCI-CLK-007, CCI-SYN-001 and CCI-SYN-002.
  1. Open the creative screen with an empty player inventory. Record the serverbound packets.
  2. Left click a stone in a grid slot. Left click the slot 0 of the hotbar row.
  3. Shift and left click a stone. Left click the slot 1 of the hotbar row.
  4. Press the key 3 over a stone.
  5. Shift and left click the slot 1 of the hotbar row.
  6. Select the survival inventory tab. Shift and left click the destroy slot.
  7. Select an items tab. Double click a stone in a grid slot with the left button. Left click
     the slot 2 of the hotbar row.

  Expected result, without the packets that the test of CCI-DEV-001 ignores: step 2 sends
  `set_creative_mode_slot` 36 with 1 stone. Step 3 sends 37 with 64 stone. Step 4 sends 38 with
  64 stone. Step 5 sends 37 with the empty stack. Step 6 sends 36 and 38, each with the empty
  stack. Step 7 sends 38 with 2 stone. No step sends `container_click`.
- **CCI-EXP-008** (`tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`). The
  creative drop. Rules: CCI-CLK-003 and CCI-SYN-003.
  1. Shift and left-click a stone. Right-click outside the screen. Left click outside the screen.
  2. Wait 40 client ticks, so that the drop counter is 0. Then press the drop key over a stone of
     a grid slot 70 times in one client tick.

  Expected result: step 1 sends `set_creative_mode_slot` −1 with 1 stone, then −1 with 63
  stone. Step 2 sends 64 creative drops.
- **CCI-EXP-009** (`tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`). The
  clientbound packets while the screen shows. Rule: CCI-SYN-004.
  1. Open the creative screen with an empty hotbar, and take a stone on the cursor.
  2. Run `/item replace entity @s player.cursor with minecraft:dirt`.
  3. Run `/summon minecraft:item ~ ~ ~ {Item:{id:"minecraft:stone"}}`. Wait 20 client ticks.

  Expected result: after step 2, the cursor holds the stone. After step 3, the stone shows in the
  slot 0 of the hotbar row.
- **CCI-EXP-010** (`tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`). The saved
  hotbars. Rules: CCI-HOT-001 and CCI-HOT-002.
  1. In creative mode, put a stone into `P[0]`. Hold C and press 1.
  2. Clear `P[0]`. Hold X and press 1.
  3. Open the creative screen on the tab `minecraft:hotbar`.

  Expected result: step 1 sends no packet. Step 2 sends `set_creative_mode_slot` for the slot
  numbers 36 to 44, and the packet 36 holds the stone. Step 3 shows the stone in row 0, and the
  paper "Save hotbar with C + 2" in row 1 at the column 1.
- **CCI-EXP-011** (`tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`). The parts
  of a creative tooltip. Rule: CCI-TIP-001, steps 2 and 3.
  1. Open the creative screen on the tab `minecraft:functional_blocks`. Move the mouse over the
     painting with no variant.
  2. Select the tab `minecraft:food_and_drinks`. Move the mouse over a suspicious stew.
  3. Put that suspicious stew into `P[0]`. Select the survival inventory tab. Move the mouse over
     `P[0]`.

  Expected result: in step 1, the tooltip shows the text of the key `painting.random` in the
  colour `FFAAAAAA` below the name. In step 2, the tooltip shows the effect parts of the stew. In
  step 3, the tooltip shows no effect part.
- **CCI-EXP-012** (the client frame capture of `tools/vanilla-oracle`). The figure in the survival
  inventory tab. Rule: CCI-LAY-007.
  1. Put a stone into `P[9]`. Open the creative screen as a local player that stands still and
     is not invisible. Use the GUI scale 2.
  2. Select the survival inventory tab. Put the mouse at (`left + 89`, `top + 14`). Take a frame.
  3. Put the mouse at (`left + 200`, `top + 14`). Take a frame.
  4. Left click `M[9]` to take the stone. Put the mouse at (`left + 89`, `top + 30`). Take a
     frame.
  5. Select the tab `minecraft:building_blocks`. Take a frame.

  Expected result: in step 2, the figure faces the viewer, and its feet are at `top + 46.75`. No
  pixel of the figure is outside the figure rectangle. In step 3, the body turns by 24.50 degrees
  and the head by 49.0 degrees, to the right of the screen. In step 4, the stone is in front of
  the figure. In step 5, no figure shows.

The tools cannot run some of these experiments yet.

- The input agent of `tools/vanilla-client` sets only the movement keys, the use key and the
  rotation. It cannot press another key, turn the wheel, or click or type in a screen.
  CCI-EXP-001, CCI-EXP-002 and CCI-EXP-004 to CCI-EXP-012 need such input. CCI-EXP-006 needs
  the wheel and a drag of the thumb of the scroll bar.
- The client frame capture of `tools/vanilla-oracle` runs the scenes of `tools/scenes/`. A scene
  cannot open a screen, select a tab or move the mouse. CCI-EXP-006 and CCI-EXP-012 need these
  actions.
- `tools/mcrs_minecraft_client_recorder` records packets. It does not take a frame of the client
  window, and CCI-EXP-002 needs frames.
