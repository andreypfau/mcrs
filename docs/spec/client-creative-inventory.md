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
- **Inventory menu**, **player inventory**, **infinite materials**: as `inventory.md`, section
  0.1, defines them. `M[i]` is the entry with the menu index `i` of the inventory menu
  (INV-MNU-001). `P[i]` is the slot `i` of the player inventory.
- **Static registry**, **data-pack registry**, **tag**: as `registry.md`, section 0.1, defines
  them.
- **Screen**, **in a world**: as `client-widget.md`, section 0.1, defines them.
- **Local player**, **tick**: as `client-camera.md`, section 0.1, defines them. A tick of the
  client is a **client tick**.
- **GUI unit**, **W**, **H**: as `client-chat.md`, section 0.1, defines them.
- **Mouse position**: as `client-tooltip.md`, section 0.1, defines it.
- **Wheel event**, **notch**: as `entity-physics.md`, section 0.1, defines them.
- **Creative flag**: as `client-block-breaking.md`, section 0.1, defines it.
- **Creative screen**: the screen of this document.
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
- **Visible tab**: a tab that the screen draws and that a click can select (CCI-TAB-002).
- **Creative report**: the generated report `assets/mcrs/reports/creative_tabs.json`
  (CCI-CNT-001).
- **Tab list**: the ordered list of stacks that an items tab shows (CCI-CNT-002).
- **Search list**: the ordered list of stacks that an items tab gives to the search tab
  (CCI-CNT-002).
- **Same stack**: two stacks with the same item and the same component patch. The count has no
  effect.
- **Content**: the ordered list of stacks that the selected tab shows in the grid (CCI-TAB-004).
- **Grid**: the 5 rows of 9 slots of the creative screen that show a part of the content.
- **Grid slot**: one slot of the grid.
- **Hotbar row**: the 9 slots below the grid in each tab except the survival inventory tab. The
  slot `c` of the hotbar row shows `P[c]`.
- **Destroy slot**: the slot of the survival inventory tab that deletes stacks (CCI-CLK-007).
- **Row count**: `R = ceil(n / 9) − 5`. `n` is the number of stacks of the content.
- **Scroll offset**: a number `s` in `0..1` that selects the first row of the content that the
  grid shows (CCI-SCR-001).
- **Search text**: the text of the search box (CCI-SRH-001).
- **Creative tooltip**: the lines of the item tooltip of a stack, as `client-container-screen.md`
  states them, with the creative flag of that tooltip on. With this flag, a stack with
  `hide_tooltip` gives its lines. A painting stack with no variant gives the line of the key
  `painting.random`. A suspicious stew gives the lines of its effects.
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

Each rule with the label `source_suggested` names its experiment in section 10.

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
| `CCI-TIP` | the lines that the creative screen adds to a tooltip |
| `CCI-LAY` | the layout: sizes, positions, textures and sprites |
| `CCI-EXP` | experiments |
| `CCI-DEV` | intentional deviations |

### 0.4 Scope

In scope:

- the creative screen in a world, for a local player with the creative flag;
- the tabs, the content of each tab and the creative report;
- the scroll, the search and the destroy slot;
- the result of each click in the creative screen, and the packets that the client sends;
- the load and the save of a saved hotbar with the toolbar activators of `client-hotbar.md`;
- the layout of the creative screen as data.

Out of scope:

- the server side of `set_creative_mode_slot`: `inventory.md`, INV-CRE-001 to INV-CRE-006;
- the change of mouse buttons and keys into clicks: `client-inventory.md`;
- the slot highlight, the drag, the double click and the item tooltip:
  `client-container-screen.md`;
- the survival inventory screen and the clientbound packets that change the stacks of the
  inventory menu: `client-inventory.md`;
- the draw of a tooltip: `client-tooltip.md`;
- the edit rules of the search box: `client-edit-box.md`;
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
- **Postconditions:**
  - The open sends no packet (CCI-DEV-001).
  - The cursor holds the empty stack (CCI-OPEN-002).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-001.

### CCI-OPEN-002: The close of the creative screen

- **Inputs:** a press of the key Escape, or a press of the key binding `key.inventory` while the
  search tab is not selected.
- **Behavior:**
  1. The client sends `container_close` with the container id 0.
  2. The cursor of the client becomes the empty stack. The client sends no packet for the cursor.
  3. The client shows no screen.
- **Postconditions:**
  - The stack that the cursor held is gone. It does not go to the player inventory, and it does
    not become an item entity (CCI-DEV-002).
  - The server does the steps of INV-CLS-001 for the inventory menu.
- **Edge cases:** while the search tab is selected, `key.inventory` types its character into the
  search box (CCI-SRH-001). Only Escape closes the screen then.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-001.

### CCI-OPEN-003: A change of the creative flag while a screen shows

- **Inputs:** a change of the creative flag while the creative screen or the survival inventory
  screen shows.
- **Behavior:** the client checks these rows at each client tick. The first true row gives the
  result.

| # | Condition | Result |
|---|-----------|--------|
| 1 | The creative screen shows, and the creative flag is off. | The survival inventory screen replaces the creative screen. |
| 2 | The survival inventory screen shows, and the creative flag is on. | The creative screen replaces the survival inventory screen, as CCI-OPEN-001 opens it. |

- **Postconditions:** the change sends no `container_close`. The cursor of the creative screen
  becomes the empty stack, as in CCI-OPEN-002, step 2.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-001.

---

## 2. Tabs

### CCI-TAB-001: The tabs and their order

- **Behavior:** the creative screen has the 14 tabs of this table. The order of the tabs is the
  order of the rows. It is the order of the protocol ids of the static registry
  `minecraft:creative_mode_tab` in `assets/mcrs/reports/registries.json`. The creative report
  gives each column of the table (CCI-CNT-001).

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
- **Postconditions:** the English titles are "Building Blocks", "Colored Blocks", "Natural
  Blocks", "Functional Blocks", "Redstone Blocks", "Saved Hotbars", "Search Items", "Tools &
  Utilities", "Combat", "Food & Drinks", "Ingredients", "Spawn Eggs", "Operator Utilities" and
  "Survival Inventory".

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: CCI-EXP-002.

### CCI-TAB-002: Which tabs are visible

- **Behavior:**
  1. The saved hotbars tab, the search tab and the survival inventory tab are always visible.
  2. An items tab is visible when its tab list has one stack or more (CCI-CNT-002).
- **Postconditions:**
  - The tab `minecraft:op_blocks` is visible only when the operator condition is true, because
    its tab list is empty in the other case (CCI-CNT-002, step 3).
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

The tab lists are not in the data pack. The client reads them from the creative report. The
command `cargo run -p mcrs_minecraft_update` writes it from the game, as
`tools/mcrs_minecraft_update/README.md` states for the other reports.

- **Shape:** the creative report is one JSON object with these fields.

| Field | Type | Meaning |
|-------|------|---------|
| `item_features` | object | For each item that needs a feature flag other than `minecraft:vanilla`: the item id, and the list of the feature flag ids that the item needs. An item that is not in the object needs only `minecraft:vanilla`. |
| `tabs` | list | One object for each tab, in the order of CCI-TAB-001. |

Each object of `tabs` has these fields.

| Field | Type | Meaning |
|-------|------|---------|
| `id` | identifier | The id of the tab in `minecraft:creative_mode_tab`. |
| `kind` | string | `items`, `saved_hotbars`, `search` or `survival_inventory`. |
| `row` | string | `top` or `bottom`. |
| `column` | integer | `0..6`. |
| `aligned_right` | boolean | The right alignment of CCI-LAY-002. |
| `title` | text component | The title, for example `{"translate":"itemGroup.buildingBlocks"}`. |
| `icon` | stack | The icon, in the saved form of a stack (`item-stack.md`). |
| `background` | identifier | The texture of the background (CCI-LAY-001). |
| `show_title` | boolean | The screen draws the title (CCI-LAY-005). |
| `scroll_bar` | boolean | The tab has a scroll bar (CCI-SCR-001). |
| `needs_operator` | boolean | The tab list and the search list are empty when the operator condition is false. |
| `entries` | list | The entries of CCI-CNT-003, in order. Only a tab of the kind `items` has entries. |

- **Constraints:**
  - The client rejects a creative report with a field that these tables do not name, or with a
    missing field. It then shows no items tab.
  - Two tabs with the same `row` and the same `column` are an error of the report.
- **Example:** the start of a creative report.

```json
{
  "item_features": {},
  "tabs": [
    {
      "id": "minecraft:building_blocks",
      "kind": "items",
      "row": "top",
      "column": 0,
      "aligned_right": false,
      "title": { "translate": "itemGroup.buildingBlocks" },
      "icon": { "id": "minecraft:bricks" },
      "background": "minecraft:textures/gui/container/creative_inventory/tab_items.png",
      "show_title": true,
      "scroll_bar": true,
      "needs_operator": false,
      "entries": [
        { "stack": { "id": "minecraft:oak_log" } },
        { "stack": { "id": "minecraft:oak_wood" } }
      ]
    }
  ]
}
```

Provenance: `design`. Kind: semantic. Confidence: high. Reason: the game builds the tab lists in
its code. A generated report keeps the client data-driven. A data pack that adds an element of a
data-pack registry changes the tab lists with no change of the report (CCI-CNT-003).

### CCI-CNT-002: The tab list and the search list of an items tab

- **Inputs:** the entries of the tab in the creative report, the enabled features, the operator
  condition, and the registries and the tags that the server sent.
- **Behavior:** the client builds two lists for each items tab:
  1. The client expands each entry of the tab in order (CCI-CNT-003). Each entry gives a list of
     stacks of count 1, and a visibility.
  2. The client removes each stack whose feature flags the enabled features do not all contain
     (CCI-CNT-004).
  3. When `needs_operator` is true and the operator condition is false, both lists are empty.
  4. A stack with the visibility `tab_and_search` goes to the end of the tab list and of the
     search list. A stack with `tab` goes only to the tab list. A stack with `search` goes only to
     the search list.
  5. The client does not add a stack to a list that holds the same stack already.
- **Postconditions:** the order of each list is the order of the entries. In an entry, it is the
  order of CCI-CNT-003.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-003.
Step 5: see CCI-DEV-003.

### CCI-CNT-003: The entries of the creative report

- **Behavior:** an entry is a JSON object with one of the fields `stack` and `each`, and an
  optional field `visibility`. The values of `visibility` are `tab_and_search` (the default),
  `tab` and `search`.
  1. An entry with `stack` gives one stack. The value is the saved form of a stack with no count,
     for example
     `{"id":"minecraft:light","components":{"minecraft:block_state":{"level":"15"}}}`. The client
     decodes it with the registries of the world. When the decode fails, the entry gives no stack
     (CCI-DEV-004).
  2. An entry with `each` gives one stack or more for each element of a registry. The value of
     `each` is an object with the fields of the table below.

| Field | Type | Meaning |
|-------|------|---------|
| `registry` | identifier | The registry `Q` of the elements. |
| `tag` | identifier, optional | A tag of `Q` that filters the elements. |
| `in_tag` | boolean, default `true` | `true`: only the elements of `tag`. `false`: only the elements that are not in `tag`. |
| `order` | string | `registry`, `tag` or `painting_size`. |
| `item` | identifier | The item of each stack. |
| `component` | identifier | The component type that holds the element. |
| `levels` | string, optional | `max` or `all`. Only for the component `minecraft:stored_enchantments`. |
| `element_features` | object, optional | For a static registry only: the feature flag ids that an element needs, for each element that needs a flag other than `minecraft:vanilla`. |

  3. The order of the elements:
     - `registry`: the order of the numeric ids of `Q`. For a data-pack registry, this is the
       order of the `registry_data` packet. For a static registry, this is the order of the
       protocol ids in `registries.json`.
     - `tag`: the order of the entries of `tag` in the last `update_tags` packet. `in_tag` must be
       `true`.
     - `painting_size`: the order `registry`, then a stable sort by the area `width × height` of
       the painting variant, then a stable sort by its `width`. Both sorts go up. `Q` must be
       `minecraft:painting_variant`.
  4. The value of the component for the element `e`:

| `component` | Value | Stacks for one element |
|-------------|-------|------------------------|
| `minecraft:painting_variant` | `e` | 1 |
| `minecraft:instrument` | `e` | 1 |
| `minecraft:potion_contents` | `{"potion": e}` | 1 |
| `minecraft:stored_enchantments` | `{e: L}` | `levels` `max`: one, with `L` the `max_level` of `e`. `levels` `all`: one for each `L` from 1 to `max_level`, with `L` going up. |

  5. When the registry `Q` or the tag does not exist in the world, the entry gives no stack.
- **Postconditions:**
  - A data pack that adds a painting variant to the tag `minecraft:placeable` adds a painting to
    the tab `minecraft:functional_blocks`, at the place of its size.
  - A data pack that adds an enchantment adds its enchanted books to the tab
    `minecraft:ingredients` and to the search tab.
  - The creative report needs no change for such a data pack.

The creative report of this version has these entries of the form `each`:

| Tab | `registry` | `tag`, `in_tag` | `order` | `item` | `component` | `levels` | `visibility` |
|-----|------------|-----------------|---------|--------|-------------|----------|--------------|
| `functional_blocks` | `minecraft:painting_variant` | `minecraft:placeable`, `true` | `painting_size` | `minecraft:painting` | `minecraft:painting_variant` | | `tab_and_search` |
| `tools_and_utilities` | `minecraft:instrument` | `minecraft:goat_horns`, `true` | `tag` | `minecraft:goat_horn` | `minecraft:instrument` | | `tab_and_search` |
| `combat` | `minecraft:potion` | | `registry` | `minecraft:tipped_arrow` | `minecraft:potion_contents` | | `tab_and_search` |
| `food_and_drinks` | `minecraft:potion` | | `registry` | `minecraft:potion` | `minecraft:potion_contents` | | `tab_and_search` |
| `food_and_drinks` | `minecraft:potion` | | `registry` | `minecraft:splash_potion` | `minecraft:potion_contents` | | `tab_and_search` |
| `food_and_drinks` | `minecraft:potion` | | `registry` | `minecraft:lingering_potion` | `minecraft:potion_contents` | | `tab_and_search` |
| `ingredients` | `minecraft:enchantment` | | `registry` | `minecraft:enchanted_book` | `minecraft:stored_enchantments` | `max` | `tab` |
| `ingredients` | `minecraft:enchantment` | | `registry` | `minecraft:enchanted_book` | `minecraft:stored_enchantments` | `all` | `search` |
| `op_blocks` | `minecraft:painting_variant` | `minecraft:placeable`, `false` | `painting_size` | `minecraft:painting` | `minecraft:painting_variant` | | `tab_and_search` |

The three entries of `minecraft:potion` in `food_and_drinks` are in the order of the table. Each
other entry of a tab has the form `stack`. These include the ominous banner of
`functional_blocks`, whose banner patterns are ids of `minecraft:banner_pattern`. They also
include the fireworks of the flight durations 1, 2 and 3, the ominous bottles of the amplifiers 0
to 4, and the `minecraft:light` stacks of the levels 15 down to 0.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-003.

### CCI-CNT-004: The feature flags of a stack

- **Behavior:** the feature flags of a stack of an entry are the union of these sets:
  - the flags of its item in `item_features`, or `minecraft:vanilla` when the item is not there;
  - for an entry of the form `each`: the flags of the element in `element_features`, when the
    element is there.
- **Postconditions:** a stack shows only when the enabled features contain each of its flags.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-003.

### CCI-CNT-005: The rebuild of the content

- **Inputs:** these inputs of the lists:
  1. the enabled features;
  2. the operator condition;
  3. the registries that a configuration phase gave (`registry.md`, REG-NET-001);
  4. the tags that the last `update_tags` packet gave;
  5. the language of the client, for the search text of CCI-SRH-002 only.
- **Behavior:** when one of these inputs changes, the client builds the tab lists, the search
  lists and the search text of each stack again. While the creative screen shows, the new lists
  show at the latest in the next client tick. Then:
  1. When the selected tab is an items tab with an empty tab list, `minecraft:building_blocks`
     becomes the selected tab (CCI-TAB-004).
  2. Else the content becomes the new list of CCI-TAB-004 for the selected tab. The first visible
     row stays the same, as CCI-SCR-002 states.
- **Postconditions:**
  - An operator that the server deops while the creative screen shows loses the tab
    `minecraft:op_blocks` in the next client tick.
  - A `/reload` that changes the tag `minecraft:placeable` of painting variants changes the
    paintings of the tabs (CCI-DEV-005).

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-004.
Input 4: see CCI-DEV-005.

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
     after the change.
  3. A change of the search text sets `s` to 0 (CCI-SRH-002).
- **Postconditions:** one notch of the wheel moves the grid by one row.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-006.

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
  3. A key goes to the search box, except Escape and the swap key of step 4.
  4. A hotbar key does the swap click of CCI-CLK-005 when the mouse position is over a slot. A
     grid slot with the empty stack is the exception: the key then goes to the search box.
  5. A click on a grid slot selects all the search text. The next typed character thus replaces
     it.
  6. When a key or a character changes the search text, the content becomes the result of
     CCI-SRH-002.
- **Postconditions:** while the search tab is selected, the drop key, `key.inventory` and the
  other key bindings of the screen type their characters.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-005.

### CCI-SRH-002: The match of the search text

- **Inputs:** the search text `q`, and the search list of the search tab (CCI-CNT-006).
- **Behavior:**
  1. When `q` is empty, the content is the search list of the search tab.
  2. When `q` starts with `#`, the client removes the `#` and does the tag match of step 4. Else
     it does the name match of step 3.
  3. The name match. `q` becomes lower case. When `q` has no `:`, a stack matches when a line of
     its search text contains `q`. When `q` has a `:`, `a` is the text before the first `:`, and
     `b` is the text after it. The client removes the spaces at the ends of `a` and of `b`. A
     stack matches when the namespace of its item id contains `a`, and its item id path or a line
     of its search text contains `b`.
  4. The tag match. `q` becomes lower case. When `q` has no `:`, a stack matches when the path of
     an item tag of its item contains `q`. When `q` has a `:`, `a` and `b` are as in step 3. A
     stack matches when one item tag of its item has a namespace that contains `a` and a path
     that contains `b` (CCI-DEV-006).
  5. The content is the stacks of the search list that match, in the order of the search list.
  6. The scroll offset becomes 0.
- **Definitions:**
  - "Contains" is a substring test. Each text contains the empty text.
  - "Lower case" is the Unicode lower case map of each character. It does not depend on the
    language of the client.
  - The search text of a stack is a list of lines. They are the lines of its creative tooltip,
    with the option "Advanced tooltips" off. The client takes each line as plain text and removes
    each formatting code (`§` and the next character). It removes the spaces at the ends and drops
    each empty line. Then each line becomes lower case.
  - The client uses the language at the time of the build (CCI-CNT-005).
  - The item tags are those of the last `update_tags` packet.
- **Postconditions:**
  - `diamond` finds "Diamond Sword". `diamond_sword` does not find it, because the name match
    does not read the item id when `q` has no `:`.
  - `minecraft:sword` finds each sword.
  - `#swords` finds each item in the tag `minecraft:swords`.
  - The match ignores the case of `q`. The name match reads the item names in the language of the
    client.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-005.
Step 4: see CCI-DEV-006.

---

## 6. Clicks, keys and the cursor

`client-inventory.md` and `client-container-screen.md` state how the mouse buttons and the keys
become clicks of the click kinds of ITX-JAV-006, with a slot index and a button. The creative screen does not send these
clicks. It computes each result in the client and sends the packets of section 7. This section
states the result of each click kind in the creative screen. A click that makes a creative drop
also needs the drop counter of CCI-SYN-003 below its limit. Else the click has no result.

### CCI-CLK-001: A click on a grid slot

- **Inputs:** a click on a grid slot `G` of an items tab, of the search tab or of the saved
  hotbars tab. `S` is the stack that `G` shows. `C` is the cursor stack.
- **Behavior:** the first true condition gives the result.
  1. The enabled features do not contain each feature flag of `S`, or `S` has the component
     `minecraft:creative_slot_lock`: no result.
  2. The click kind is swap: CCI-CLK-005.
  3. The click kind is clone: when `C` is empty and `S` is not empty, `C` becomes `S` with the
     count of its maximum stack size. Else no result.
  4. The click kind is throw: when `S` is not empty, the client makes a creative drop of `S` with
     the count 1 (button 0), or with its maximum stack size (each other button). `C` does not
     change.
  5. The click kind is quick craft: no result. A grid slot is never a slot of a drag.
  6. The click kind is pick up, quick move or pick up all: the first true row of this table gives
     the result.

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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-007.

### CCI-CLK-002: A click on the hotbar row

- **Inputs:** a click on the slot `c` of the hotbar row, in each tab except the survival inventory
  tab.
- **Behavior:**
  1. The click kind is quick move: `P[c]` becomes the empty stack.
  2. The click kind is throw: the client removes 1 item (button 0), or the full stack (each other
     button), from `P[c]`. The removed stack is a creative drop.
  3. Each other click kind: the client applies the click to a menu of the 9 slots `P[0]` to
     `P[8]`. The rules of `inventory-transactions.md`, section 6, apply, for a player with
     infinite materials. Pick up all collects only from these 9 slots. A drag uses only these 9
     slots.
  4. The client sends the changed entries `M[36]` to `M[44]` (CCI-SYN-002).
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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-008.
Step 3: see CCI-DEV-007.

### CCI-CLK-004: A click in the survival inventory tab

- **Inputs:** a click on a slot of the survival inventory tab that is not the destroy slot. The
  slot shows the entry `M[i]` with `i` in `5..45` (CCI-LAY-004).
- **Behavior:**
  1. The client applies the click with the menu index `i` to its inventory menu. The rules of
     `inventory-transactions.md`, section 6, and of `inventory.md`, section 2, apply. The player
     has infinite materials.
  2. A throw removes 1 item (button 0), or the full stack (each other button), from `M[i]`. The
     removed stack is a creative drop.
  3. The client sends the changed entries (CCI-SYN-002).
- **Postconditions:**
  - A clone gives `C` a full stack (ITX-JAV-012). A drag of drag kind 2 makes items
    (ITX-JAV-020).
  - A change of the cursor alone sends no packet.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-007.

### CCI-CLK-005: A hotbar key over a slot

- **Inputs:** the swap click with the button `k` (INV-MNU-003) over a slot of the creative screen.
- **Behavior:**
  1. Over a grid slot with the stack `S` that is not empty: `P[k]` becomes `S` with the count of
     its maximum stack size. The stack that `P[k]` held is gone.
  2. Over a grid slot with the empty stack: no result.
  3. Over the hotbar row: CCI-CLK-002, step 3.
  4. Over a slot of the survival inventory tab: CCI-CLK-004.
  5. The client sends the changed entries (CCI-SYN-002).
- **Postconditions:** with the default keys, the key 3 over a stone of a tab puts 64 stone into
  `P[2]`, and the client sends the entry `M[38]`. The key F puts the stack into `P[40]`, and the
  client sends `M[45]`.

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
  1. The click kind is quick move: each entry `M[0]` to `M[45]` becomes the empty stack. The
     client sends 46 entry change packets with the empty stack, for the slot numbers 0 to 45 in
     this order. `C` does not change.
  2. Each other click kind: `C` becomes the empty stack. The client sends no packet.
- **Postconditions:**
  - The destroy slot always shows the empty stack. When the cursor is empty, its tooltip is the
    text of the key `inventory.binSlot`, "Destroy Item".
  - A vanilla server refuses the packet with the slot number 0 (INV-CRE-003, check 4). It sets the
    other 45 entries, the craft grid included.

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-007.

---

## 7. Packets

### CCI-SYN-001: The conditions of a `set_creative_mode_slot` packet

- **Behavior:** the client sends a `set_creative_mode_slot` packet only when each of these is
  true:
  - the creative flag is on;
  - the enabled features contain each feature flag of the item of the stack (CCI-CNT-004);
  - for a creative drop: the stack is not empty, and no menu screen other than the creative screen
    shows.
- **Postconditions:** when a condition is false, the click changes the stacks of the client and
  sends no packet.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: CCI-EXP-007.

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

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: CCI-EXP-007.
Step 1: see CCI-DEV-001.

### CCI-SYN-003: The creative drop

- **Behavior:**
  1. A creative drop is a `set_creative_mode_slot` packet with the slot number −1 and the dropped
     stack.
  2. The client has a drop counter. Each creative drop adds 20. Each client tick subtracts 1 while
     the counter is above 0.
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

Provenance: `source_suggested`. Kind: semantic. Confidence: medium. Experiment: CCI-EXP-009.
Step 2: see CCI-DEV-001.

---

## 8. Saved hotbars

### CCI-HOT-001: The content of the saved hotbars tab

- **Inputs:** the 9 saved hotbars, at the selection of the tab.
- **Behavior:** the content is 81 stacks: the 9 stacks of the saved hotbar 0, then the 9 stacks of
  the saved hotbar 1, up to the saved hotbar 8.
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
     - When the stack is not empty and the enabled features do not contain each of its feature
       flags, `P[j]` does not change, and no packet goes for `j` (CCI-DEV-008).
     - Else `P[j]` becomes the stack, and the client sends one `set_creative_mode_slot` with the
       slot number `36 + j` and the stack.
  3. A save stores `P[0]` to `P[8]` in the saved hotbar `k`. It sends no packet. The HUD shows the
     text of the key `inventory.hotbarSaved` as an overlay message.
- **Postconditions:** a load sends at most 9 packets, in the order of `j`. It also sends a packet
  for a stack that does not change and for an empty stack.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment: CCI-EXP-010.
Step 2: see CCI-DEV-001 and CCI-DEV-008.

---

## 9. Tooltips and layout

### CCI-TIP-001: The lines that the creative screen adds to an item tooltip

- **Inputs:** the stack `S` of a slot under the mouse position, and its item tooltip of
  `client-container-screen.md`.
- **Behavior:**
  1. The tooltip of a grid slot uses the creative tooltip. The tooltip of each other slot uses the
     item tooltip with the creative flag off.
  2. A tooltip with no line gets no added line.
  3. On a grid slot of an items tab, the client adds no line.
  4. On each other slot, the client adds these lines after the first line, in this order:
     - for each visible tab except the search tab whose search list holds the same stack as `S`,
       in the order of CCI-TAB-001: the title of that tab, with the colour `blue`;
     - on a grid slot of the search tab, while the search text starts with `#`: for each item tag
       of `S` that the tag match of CCI-SRH-002 accepts, the text `#` and the tag id, with the
       colour `dark_purple`. The order of these lines is unspecified.
- **Postconditions:** in the search tab, the tooltip of an oak log shows "Building Blocks" in blue
  below its name.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: CCI-EXP-005.
Step 4: see CCI-DEV-006.

### CCI-LAY-001: The rectangle and the background

- **Behavior:**
  1. The creative screen is a rectangle of 195 × 136 GUI units. Its top left corner is at
     `left = floor((W − 195) / 2)` and `top = floor((H − 136) / 2)`.
  2. The client draws the region (0, 0) to (195, 136) of the background texture of the selected
     tab at (`left`, `top`). The texture size is 256 × 256.

| Background | Texture |
|------------|---------|
| items | `minecraft:textures/gui/container/creative_inventory/tab_items.png` |
| item search | `minecraft:textures/gui/container/creative_inventory/tab_item_search.png` |
| inventory | `minecraft:textures/gui/container/creative_inventory/tab_inventory.png` |

- **Postconditions:** each position in the rules below is relative to (`left`, `top`), in GUI
  units.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: CCI-EXP-002.

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
- **Postconditions:** the x positions are 0, 27, 54, 81 and 108 for the columns 0 to 4, 142 for
  the column 5 and 169 for the column 6.

Provenance: `source_suggested`. Kind: layout. Confidence: medium. Experiment: CCI-EXP-002.

### CCI-LAY-003: The grid, the hotbar row and the search box

- **Behavior:**
  1. The grid slot in the row `y` (`0..4`) and the column `x` (`0..8`) has its item at
     (`9 + 18x`, `18 + 18y`).
  2. The slot `c` of the hotbar row has its item at (`9 + 18c`, 112).
  3. The search box is at (82, 6), with the size 80 × 9.
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

- **Postconditions:** the screen draws the local player in the box (73, 6) to (105, 49), with the
  scale 20 and the vertical offset 0.0625, as the survival inventory screen draws it. The model
  follows the mouse position.

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

---

## 10. Experiments

Each rule with the label `source_suggested` names one of these experiments. The rule keeps the
label until its experiment runs and gives the stated observation.

### CCI-EXP-001: Open, close and change of game mode

Tool: `tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`.

1. Start the server. Join with the recorded client in creative mode.
2. Press `key.inventory`. Wait 5 client ticks.
3. Take a stone from a grid slot. Press Escape.
4. Press `key.inventory` again. Run `/gamemode survival` for the player. Wait 2 client ticks.

Observation: step 2 sends no packet. Step 3 sends `container_close` with the id 0 and no
`set_creative_mode_slot`. In step 4, the survival inventory screen replaces the creative screen,
and no `container_close` goes.

### CCI-EXP-002: Tabs and layout

Tool: the client frame capture of `tools/vanilla-oracle`.

1. Open the creative screen at the GUI scale 1, with W = 427 and H = 240, as a player that is not
   an operator.
2. Take a frame for each visible tab.
3. Give the player the permission level `gamemasters`, and set the option "Operator Items Tab" to
   on. Take a frame again.

Observation: in step 2, 13 tabs show at the positions of CCI-LAY-002. In step 3, the tab
`minecraft:op_blocks` shows at x = 142 of the bottom row. The slots and the title are at the
positions of CCI-LAY-003 to CCI-LAY-005.

### CCI-EXP-003: The tab lists and a data pack

Tool: `tools/vanilla-oracle`.

1. Build the lists of each tab with the vanilla data pack and with the enabled features of a new
   world.
2. Compare them with the expansion of the creative report by CCI-CNT-002 and CCI-CNT-003.
3. Add a data pack with one new painting variant of 1 × 1 in `minecraft:placeable`, and one new
   enchantment with `max_level` 3. Build the lists again.

Observation: the lists of step 2 are equal. In step 3, the new painting is among the paintings of
the area 1 in `functional_blocks`. The book of level 3 is in `ingredients`, and the books of the
levels 1 to 3 are in the search list.

### CCI-EXP-004: Rebuild

Tool: `tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`.

1. Join as an operator in creative mode, with the option "Operator Items Tab" on.
2. Open the creative screen on the tab `minecraft:op_blocks`.
3. Run `/deop` for the player from the server console. Wait 2 client ticks.

Observation: the tab `minecraft:op_blocks` disappears, and the selected tab is
`minecraft:building_blocks`.

### CCI-EXP-005: Search

Tool: `tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`, with a new input of the
agent for the keys and the mouse of a screen.

1. Open the creative screen in a world with the language `en_us`. Press T.
2. Type `diamond`. Clear the text and type `diamond_sword`.
3. Clear the text and type `#swords`. Move the mouse over a diamond sword.
4. Clear the text and type `minecraft:sword`.

Observation: step 1 selects the search tab with an empty text. `diamond` finds the diamond sword,
and `diamond_sword` finds nothing. Step 3 finds each sword, and the tooltip shows "Combat" in blue
and `#minecraft:swords` in dark purple. Step 4 finds each sword.

### CCI-EXP-006: Scroll

Tool: the client frame capture of `tools/vanilla-oracle`.

1. Open the tab `minecraft:building_blocks`. Turn the wheel by one notch toward the player. Take a
   frame.
2. Drag the thumb to the bottom of the scroll bar. Take a frame.

Observation: step 1 moves the grid by one row. In step 2, the last row of the grid holds the last
stacks of the tab list, and the thumb is at y = 113.

### CCI-EXP-007: Clicks and entry change packets

Tool: `tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`, with a new input of the
agent for the keys and the mouse of a screen.

1. Open the creative screen with an empty player inventory. Record the serverbound packets.
2. Left click a stone in a grid slot. Left click the slot 0 of the hotbar row.
3. Shift and left click a stone. Left click the slot 1 of the hotbar row.
4. Press the key 3 over a stone.
5. Shift and left click the slot 1 of the hotbar row.
6. Select the survival inventory tab. Shift and left click the destroy slot.

Observation: step 2 sends `set_creative_mode_slot` 36 with 1 stone. Step 3 sends 37 with 64
stone. Step 4 sends 38 with 64 stone. Step 5 sends 37 with the empty stack. Step 6 sends 46
packets for the slot numbers 0 to 45, each with the empty stack. No step sends `container_click`.

### CCI-EXP-008: Creative drop

Tool: `tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`, with a new input of the
agent for the keys and the mouse of a screen.

1. Shift and left click a stone. Right click outside the screen. Left click outside the screen.
2. Press the drop key over a stone of a grid slot 70 times in one client tick.

Observation: step 1 sends `set_creative_mode_slot` −1 with 1 stone, then −1 with 63 stone. Step 2
sends 64 creative drops.

### CCI-EXP-009: Clientbound packets while the screen shows

Tool: `tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`.

1. Open the creative screen with an empty hotbar, and take a stone on the cursor.
2. Run `/item replace entity @s player.cursor with minecraft:dirt`.
3. Run `/summon minecraft:item ~ ~ ~ {Item:{id:"minecraft:stone"}}`. Wait 20 client ticks.

Observation: after step 2, the cursor holds the stone. After step 3, the stone shows in the slot 0
of the hotbar row.

### CCI-EXP-010: Saved hotbars

Tool: `tools/mcrs_minecraft_client_recorder` with `tools/vanilla-server`.

1. In creative mode, put a stone into `P[0]`. Hold C and press 1.
2. Clear `P[0]`. Hold X and press 1.
3. Open the creative screen on the tab `minecraft:hotbar`.

Observation: step 1 sends no packet. Step 2 sends `set_creative_mode_slot` for the slot numbers 36
to 44, and the packet 36 holds the stone. Step 3 shows the stone in row 0, and the paper "Save
hotbar with C + 2" in row 1 at the column 1.

---

## 11. Known vanilla quirks

None.

---

## 12. Intentional deviations

### CCI-DEV-001: No packet that repeats a stack back to the server

- **Rule:** CCI-OPEN-001, CCI-SYN-002, CCI-SYN-004 and CCI-HOT-002.
- **Reference:** the vanilla client keeps a copy of each entry of the inventory menu. It updates
  the copy only when it sends. At the open of the creative screen, after each
  `container_set_slot` while the screen shows, and after each click, it sends an entry change
  packet for each entry that differs from the copy. Thus it sends back the stacks that the server
  changed since the last send, for example a picked up item. After the load of a saved hotbar, it
  sends a second packet for each slot that changed.
- **Reason:** these packets repeat a stack that the server holds already. When the server changes a
  slot two times in a short time, the repeat of the first stack can come after the second change.
- **Visible to the client:** no. The stacks on the server are the same after the packets.
- **Bug reference:** none.
- **Test:** a differential test ignores an entry change packet whose stack is the stack that the
  server holds for that entry.

### CCI-DEV-002: The close deletes the cursor stack

- **Rule:** CCI-OPEN-002, step 2, and CCI-OPEN-003.
- **Reference:** the vanilla client keeps the cursor stack after the close. The server does not
  know that stack. The next creative screen or survival inventory screen shows it on the cursor
  again, until a packet of the server replaces the cursor.
- **Reason:** the cursor stack of the client must be a stack that the server knows, or the empty
  stack. A player in a survival inventory screen cannot use a cursor stack that the server does
  not hold.
- **Visible to the client:** yes. In the reference, the stack comes back on the cursor at the next
  open.
- **Bug reference:** none.
- **Test:** a differential test does not compare the cursor after a close of the creative screen.

### CCI-DEV-003: A stack that an entry gives two times

- **Rule:** CCI-CNT-002, step 5.
- **Reference:** the vanilla client stops with an error when an entry gives a stack that the tab
  list holds already.
- **Reason:** a data pack can give such a stack, for example with a tag that names an element two
  times. The client must not stop because of data.
- **Visible to the client:** only with such a data pack.
- **Bug reference:** none.
- **Test:** none.

### CCI-DEV-004: An entry that does not decode

- **Rule:** CCI-CNT-003, step 1.
- **Reference:** the vanilla client stops with an error when the ominous banner names a banner
  pattern that the world does not have.
- **Reason:** the client must not stop because of data. The other stacks of the tab stay usable.
- **Visible to the client:** only in a world that does not have such a banner pattern.
- **Bug reference:** none.
- **Test:** none.

### CCI-DEV-005: A new `update_tags` builds the tab lists again

- **Rule:** CCI-CNT-005, input 4.
- **Reference:** after `update_tags` in the play state, the vanilla client builds again only the
  data of the tag match. The tab lists keep the tags of the last configuration phase, for example
  the paintings of `minecraft:placeable` and the goat horns of `minecraft:goat_horns`.
- **Reason:** the tab lists must agree with the tags that the server uses. A data pack that a
  `/reload` changes must show in the creative screen.
- **Visible to the client:** yes, after a `/reload` that changes one of these tags.
- **Bug reference:** none.
- **Test:** a differential test does not use a `/reload` that changes a tag of an entry.

### CCI-DEV-006: One item tag matches both parts of a tag search

- **Rule:** CCI-SRH-002, step 4, and CCI-TIP-001, step 4.
- **Reference:** for `#a:b`, a stack matches when one of its tags has a namespace that contains
  `a`, and one of its tags, also another tag, has a path that contains `b`. For the tag lines of
  the tooltip, the reference compares the text before the lower case map. Thus an upper case
  letter after `#` gives no tag line.
- **Reason:** a player who types `#a:b` asks for one tag. The tooltip must show the tags that the
  match found.
- **Visible to the client:** yes, with tags in two namespaces, and with upper case letters after
  `#`.
- **Bug reference:** none.
- **Test:** a differential test uses only tag searches with no `:` and with lower case letters.

### CCI-DEV-007: A click outside the screen

- **Rule:** CCI-CLK-003.
- **Reference:** in the survival inventory tab, a click outside the screen drops the full cursor
  stack with each button. In the other tabs, a right click on a tab that is not selected drops one
  item of the cursor stack.
- **Reason:** a right click outside a menu drops one item in each other screen (ITX-JAV-007). A
  click on a tab must not drop.
- **Visible to the client:** yes.
- **Bug reference:** none.
- **Test:** a differential test does not right click outside the screen or on a tab with a cursor
  stack.

### CCI-DEV-008: The load of a stack with a disabled item

- **Rule:** CCI-HOT-002, step 2.
- **Reference:** the vanilla client puts the stack into its own `P[j]` and sends no packet. The
  player then sees an item that the server does not hold, until the server sends the slot.
- **Reason:** the client must not show a stack that the server does not hold.
- **Visible to the client:** yes, with a saved hotbar from a world with more enabled features.
- **Bug reference:** none.
- **Test:** none.

---

## 13. Unresolved

None.
