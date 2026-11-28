# NBT budget: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states the budget that limits the network NBT in one serverbound packet and in one
wire stack. With the budget, a client cannot stall the server with a large book or a large stack.

The budget adds to the limits that other documents state. These limits stay as they are:

- the frame limit of 2 097 151 bytes and the limit of 8 388 608 bytes for an inflated packet
  (`compression.md`, FRAME-003 and FRAME-007);
- the depth of 512 levels and the quota of 2097152 bytes for one text (`text-component.md`,
  TXT-NBT-005; `item-stack.md`, STK-SHP-004);
- the limits on entries, lines and nested stacks in `item-stack.md`.

Each of these limits applies to one value. A packet can hold many values, and a stack can hold
stacks. Thus the sum of the values can be much larger than each limit. The budget limits that sum.

This document states only what a client, a player or a user of the library can see. It does not
prescribe an architecture.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **vanilla client**, **clientbound**,
  **serverbound**, **VarInt**: as `login.md`, section 0.1, defines them.
- **Network NBT**, **stack**, **wire stack**, **component**: as `item-stack.md`, section 0.1,
  defines them.
- **Tag**: one NBT value of one of the twelve kinds Byte, Short, Int, Long, Float, Double, String,
  Byte array, Int array, Long array, List and Compound. A tag in a list or in a compound is also
  a tag.
- **Root tag**: the first tag of a network NBT.
- **Declared length**: the length that comes before the data of a String, an array or a List on
  the wire. For a String, it is the unsigned 16-bit number of bytes. For an array or a List, it is
  the signed 32-bit number of elements.
- **Charge**: a number that the reader adds to a budget for each part of a tag that it reads
  (NBQ-CHG-001). A charge is not a count of bytes on the wire.
- **Budget**: two limits and two totals: the byte limit with the charge total, and the tag limit
  with the tag total.
- **Decode unit**: the input that one budget covers (NBQ-UNIT-001).
- **Quota error**: the decode error that the reader gives when a total of the budget is larger
  than its limit.
- **Plugin**: the server network plugin. Each input of this document is an input of this plugin.

### 0.2 Provenance

This document uses the labels of `registry.md`, section 0.3. Each limit of this document is a
choice of mcrs. No rule states a behaviour of the vanilla server. A `design` rule states the
reason for the choice.

### 0.3 ID prefixes

| Prefix | Subject |
|--------|---------|
| `NBQ-CFG` | the plugin input and its defaults |
| `NBQ-UNIT` | the decode unit and the network NBT that it covers |
| `NBQ-CHG` | the charge of each part of a tag, and the time of the charge |
| `NBQ-REF` | the refusal |
| `NBQ-BOOK` | the serverbound `edit_book` packet |
| `NBQ-VEC` | the test vectors |

### 0.4 Scope

In scope:

- each network NBT in a serverbound packet of the CONFIGURATION state and of the PLAY state;
- each network NBT in a wire stack that a user of the library decodes alone;
- the limits of the serverbound `edit_book` packet.

Out of scope:

- NBT in a file, for example a region file or a player file. The budget does not apply to it.
- Clientbound packets. Section 7, entry 2, states the default for a client.
- A limit on the sum of the elements of the arrays of components that are not NBT. Section 7,
  entry 4, states the reason.

---

## 1. Plugin input

### NBQ-CFG-001: Input and defaults

| Input | Default | Value that turns the budget off |
|-------|---------|---------------------------------|
| NBT budget: the byte limit | `33554432` | none |
| NBT budget: the tag limit | `262144` | none |

- **Behavior:** the user sets the input through the plugin (`README.md`, section "Requirements
  that each specification inherits"). The server reads no input from an environment variable or
  from a file.
- **Behavior:** the input applies to each connection. The input does not change while the server
  runs.
- **Behavior:** the depth limit of 512 levels is not an input. It stays as TXT-NBT-005 states.
- **Behavior:** a user of the library who decodes a wire stack alone can give a budget. When the
  user gives no budget, the reader uses the defaults of this table.

Provenance: `design`. Both defaults are policy of mcrs, not values of the vanilla server. The byte
limit `33554432` is `2^25`, the smallest power of two above the charge of the largest stack that
survival play makes (NBQ-CFG-002). That charge is 33 186 000. The byte limit is also twice the
largest charge that one serverbound packet of plain text can carry (NBQ-CFG-002). The tag limit
is the default of the per-tag limiter of the NBT library of the public ViaVersion proxy. It is
more than 48 times the tag total of the largest stack of survival play. The depth limit is also
the default of that library. Kind: semantic. Confidence: high.

### NBQ-CFG-002: A stack of survival play passes

- **Behavior:** with the defaults of NBQ-CFG-001, the budget does not refuse a stack that survival
  play can make. The largest such stack is the shulker box of this rule. A player in creative mode
  who moves it thus keeps the connection.
- **Inputs:** the limits of survival play:
  - a written book comes from a book and quill. `edit_book` gives at most 100 pages of 1024 UTF-16
    code units (NBQ-BOOK-001).
  - each page is a plain text, thus one String tag on the wire (`item-stack.md`, STK-SHP-004).
    When the server filters text, a page has a raw text and a filtered text.
  - the modified UTF-8 form of NBT uses at most 3 bytes for each UTF-16 code unit. A String of
    1024 code units thus has at most `L = 3072`.
  - the title, the author and the generation of `minecraft:written_book_content` are not network
    NBT. They add nothing (NBQ-UNIT-001).
  - an anvil gives a `minecraft:custom_name` of at most 50 code units, thus `L = 150`.
  - a shulker box holds 27 stacks. Each written book in it is one stack.
- **Behavior:** the charges of NBQ-CHG-001 then give these totals:

| Part | Charge | Tags |
|------|--------|------|
| one page: raw text `2 × 3072` and filtered text `2 × 3072` | 12 288 | 2 |
| one book: 100 pages | 1 228 800 | 200 |
| the name of one book | 300 | 1 |
| 27 named books | 33 185 700 | 5 427 |
| the name of the shulker box | 300 | 1 |
| the shulker box | 33 186 000 | 5 428 |

- **Behavior:** such a shulker box does not fit in one serverbound packet. Its wire form has about
  16 600 000 bytes: each page has 6 151 bytes. An inflated packet has at most 8 388 608 bytes
  (`compression.md`, FRAME-007). FRAME-007 refuses the packet before the budget applies.
- **Behavior:** in one packet, a shulker box can carry 13 such books and a part of a 14th book.
  Its charge is less than 16 777 216. For each String, the charge `2 × L` is less than two times
  its `L + 3` bytes on the wire. When each network NBT of a packet is a String, the charge total is
  thus less than two times the bytes of the packet.

Provenance: `design` for the budget. `documentation` for the limits of `edit_book` (NBQ-BOOK-001)
and for the name of 50 characters of the anvil and the 27 slots of a shulker box, on the Minecraft
Wiki. Kind: semantic. Confidence: high.

## 2. The decode unit

### NBQ-UNIT-001: One budget for each packet and for each stack

- **Behavior:** each decode unit has one budget. At the start of the decode unit, the charge
  total and the tag total are 0. A decode unit is one of these:
  - one serverbound packet that the server reads in the CONFIGURATION state or in the PLAY state;
  - one wire stack that a user of the library decodes alone, out of a packet, from input that
    the user does not trust.
- **Behavior:** a stack that the server made, and that the server decodes again, is not a decode
  unit. The budget does not apply to it. For example, the stacks of the sync of a menu and the
  stack of an item entity are not decode units.
- **Behavior:** each network NBT in the decode unit adds to the same budget. The totals do not go
  back to 0 between two network NBT of one decode unit. This includes:
  - each component whose wire form is a network NBT or holds one, for example
    `minecraft:custom_data`;
  - each text in the wire form of a component, for example each line of `minecraft:lore` and each
    page of `minecraft:written_book_content`;
  - each network NBT in each stack inside a stack, at each depth, for example in
    `minecraft:container` and in `minecraft:bundle_contents`;
  - each network NBT in a field of the packet, for example the payload of
    `custom_click_action`.
- **Behavior:** a string or an array that is not inside a network NBT adds nothing. For example,
  a page of `minecraft:writable_book_content` adds nothing.
- **Behavior:** the budget of the next packet does not depend on the packet before it.
- **Behavior:** the server applies the budget when it decodes the packet, before each check of the
  packet. For example, the budget of a `set_creative_mode_slot` packet applies before the checks
  of `inventory.md`, INV-CRE-003. A player who is not in creative mode thus gets the same refusal.
- **Behavior:** the limits that the start of this document lists apply together with the budget.
  The first limit that fails gives the decode error.
- **Example:** a library user gives a byte limit of 2 097 152. A stack holds a container with two
  stacks. Each of the two stacks has a `minecraft:custom_data` with a charge of 1 100 000. Each
  charge is less than the byte limit, but the sum is larger. The stack is refused (NBQ-VEC-010).

Provenance: `design`. A limit for each value lets the sum grow with the number of values. A
packet with one stack with nested stacks can hold thousands of network NBT. One budget for the
whole packet bounds the memory that the reader allocates for the NBT of the packet. The time of
the decode has a weaker bound (NBQ-REF-001). Kind:
deviation (section 6). Confidence: high.

## 3. The charge

### NBQ-CHG-001: Charge of each part of a tag

The reader adds the charges of this table to the charge total and the tag counts to the tag total.
`N` is the declared length. `L` is the declared length of a String.

| Part that the reader reads | Charge | Tag count |
|----------------------------|--------|-----------|
| the kind byte of the root tag | 0 | 1 |
| a Byte | 1 | 0 |
| a Short | 2 | 0 |
| an Int, a Float | 4 | 0 |
| a Long, a Double | 8 | 0 |
| a String | `2 × L` | 0 |
| a Byte array | `N` | 0 |
| an Int array | `4 × N` | 0 |
| a Long array | `8 × N` | 0 |
| the head of a List: the element kind and `N` | `5 + 4 × N` | `N` |
| an element of a List | the charge of its kind | 0 |
| the kind byte of an entry of a Compound | 1 | 1 |
| the name of an entry of a Compound | `2 × L` | 0 |
| the value of an entry of a Compound | the charge of its kind | 0 |
| the End byte that ends a Compound | 1 | 0 |

- **Behavior:** the charge of a List or of a Compound is the sum of the charges of its parts.
- **Behavior:** an empty Compound has the charge 1. An empty List has the charge 5. An empty
  String has the charge 0.
- **Behavior:** the tag count of a List is `N` at its head. Its elements do not count again.
- **Example:** the root tag `{"a": [B; 3 bytes]}` has the charge `1 + 2 + 3 + 1 = 7` and the tag
  total `1 + 1 = 2`.

Provenance: `design`. The charges follow the per-tag limiter of the NBT library of the public
ViaVersion proxy, with two changes. A String costs `2 × L` with `L` in bytes, not in characters.
A List adds its `N` tags at the head. Both changes let the reader charge before it reads the data
(NBQ-CHG-002). The charge of an element slot and of an end byte makes an empty Compound or an empty
List cost more than its one byte on the wire. Kind: deviation (section 6). Confidence: high.

### NBQ-CHG-002: Time of the charge and the limit

- **Behavior:** the reader adds the charge of a declared length immediately after it reads that
  length. It adds the charge before it reads the data that the length declares, and before it
  allocates memory for that data.
- **Behavior:** after each addition, the reader compares the two totals with the two limits. When
  the charge total is larger than the byte limit, or the tag total is larger than the tag limit,
  the reader gives a quota error.
- **Behavior:** a total that is equal to its limit passes.
- **Behavior:** the quota error occurs also when the input ends before the declared data. The
  reader does not need the data to give the quota error.
- **Behavior:** a negative declared length stays a decode error, as before this document.

Provenance: `design`. A charge before the data bounds the memory that the reader allocates by the
budget. A declared length of `2147483647` thus costs no allocation. Kind: semantic.
Confidence: high.

## 4. The refusal

### NBQ-REF-001: Quota error of the reader

- **Behavior:** the reader stops at the first quota error. It reads no more bytes of the decode
  unit and gives no partial value.
- **Behavior:** a user of the library can tell a quota error from an error of input that ends too
  early.
- **Behavior:** the reader allocates memory only for data that it charged, and it charges the data
  before it allocates the memory. The memory of a decode unit is thus bounded by the budget.
- **Behavior:** the time of the decode is bounded by the bytes of the packet times a factor that
  grows with the depth of the NBT. The budget does not bound the time. Unresolved entry 6 states
  why.
- **Behavior:** a quota error is not a panic. It does not stop the thread of the reader.

Provenance: `design`. A user of the library must know that the input is too large, not
incomplete, for example to choose between a disconnect and a wait for more bytes. Kind: semantic.
Confidence: high.

### NBQ-REF-002: Refusal of a serverbound packet

- **Behavior:** a quota error in a serverbound packet is a decode error of that packet. The server
  does not handle that packet, and no later packet of the connection.
- **Behavior:** the server sends the disconnect packet of the current state, and then closes the
  connection:

| State | Packet |
|-------|--------|
| CONFIGURATION | `disconnect` of CONFIGURATION |
| PLAY | `disconnect` of PLAY |

- **Behavior:** the reason is the translation key `disconnect.genericReason` with one argument, as
  for each other decode error of a serverbound packet. The text of the argument is not part of
  this specification.
- **Behavior:** the refusal changes no slot and no other state of the world.
- **Behavior:** the other connections continue. The tick that reads the packet ends in the same
  time as with a packet at the limits.
- **Behavior:** a flood of refused packets does not write one log line for each packet.

Provenance: `design`. With the defaults, a stack of survival play does not reach the budget
(NBQ-CFG-002). A packet past the budget thus holds a stack from a command, or it does not come from
a vanilla client. A disconnect stops further work for the connection. The reason is the same as
for each other decode error (`compression.md`, FRAME-009).
Kind: semantic. Confidence: high.

## 5. The book packet

### NBQ-BOOK-001: Limits of `edit_book`

- **Inputs:** the serverbound `edit_book` packet: a slot (VarInt), a list of pages, and an
  optional title.
- **Behavior:** the packet has these limits:

| Field | Limit |
|-------|-------|
| number of pages | at most 100 |
| one page | a String (1024): at most 1024 UTF-16 code units |
| the title | a String (32): at most 32 UTF-16 code units |

- **Behavior:** a packet past a limit is a decode error. NBQ-REF-002 applies, with the same
  disconnect and reason.
- **Behavior:** the packet holds no network NBT. The budget of NBQ-UNIT-001 adds nothing for it.

Provenance: `documentation`. The limits are in the public description of the protocol of the game
on the Minecraft Wiki. The String (n) form is that of `login.md`, section 0.1. The disconnect is
`design`, as in NBQ-REF-002. Kind: wire. Confidence: high.

## 6. Intentional deviations

- **Rule:** NBQ-UNIT-001 and NBQ-CHG-001.
  - **Reference:** for a text of an item, the vanilla server limits each network NBT alone, with
    its own measure of the size (TXT-NBT-005). That measure is not specified. A limit on the sum
    of a packet is not known.
  - **Reason:** a limit for each NBT alone lets one packet hold many NBT at that limit. The work
    for one packet then grows with the number of NBT.
  - **Visible to the client:** only for a packet with a charge larger than the byte limit, or with
    more tags than the tag limit. The player gets the disconnect of NBQ-REF-002. A stack of
    survival play never gets it (NBQ-CFG-002). A stack from a command can get it when the player
    moves it in creative mode, for example a `minecraft:custom_data` with a List of more than
    262 144 elements.
  - **Bug reference:** none.
  - **Test:** a differential test does not send a stack with more than 100 000 tags, or with a
    charge above 16 000 000.

## 7. Unresolved

Each entry has a default. The default applies until the owner decides. An entry is not an open
question for an implementer.

1. **The defaults** (NBQ-CFG-001). Default: a byte limit of `33554432` and a tag limit of `262144`.
   With these values, the tag limit refuses most hostile NBT that fits in one packet. The byte
   limit refuses a declared length past the data (NBQ-CHG-002). Inside one packet, the tag limit
   refuses a List of small elements before the byte limit does.
2. **A budget in the client.** Default: a client that decodes a clientbound stack uses the
   default budget for each top-level stack, with no budget for the whole packet. Reason: a
   clientbound packet, for example `container_set_content`, can hold many large stacks that the
   server made.
3. **A drop in place of a disconnect** (NBQ-REF-002). Default: a disconnect. The other choice:
   the server ignores the packet and keeps the connection. A drop lets a hostile client send the
   packet again in each tick.
4. **A limit on the elements of the arrays that are not NBT.** Default: no such limit. A proxy
   can sum the elements of the arrays of each stack and of each packet, because each limit of one
   array multiplies with the others. In this version, each such element takes at least one byte
   of the packet, and an inflated packet has at most 8 388 608 bytes. The serverbound
   `container_click` packet carries hashed stacks with no components (`item-stack.md`,
   STK-HSH-001), so only `set_creative_mode_slot` carries a full stack. A later issue can add the
   limit when a packet carries many full stacks.
5. **Follow-up limits on single components.** Default: none in this document. These belong to the
   documents of their values: a limit on each array of a component, the depth of the hidden effect
   of a potion effect, and the depth of the arguments of a `translate` text.
6. **The time of the decode** (NBQ-REF-001). Default: the time is bounded by the bytes of the
   packet times a factor of the depth, not by the budget. Two causes are known:
   - after the budget accepts a text or a value of a component, the reader decodes it a second
     time, and the second decode does not charge the budget;
   - to read a Compound with one entry with an empty name, the reader can skip the value and
     then read it again. At each level of depth, the reader can skip the same bytes again. The
     work is then the depth times the size.

   A later issue can bound the time. This document does not state the fix.

## 8. Test vectors

"Reader" vectors decode one network NBT, or one wire stack alone. "Packet" vectors send a
`set_creative_mode_slot` packet from a player in creative mode. A packet with more than
2 097 151 bytes needs compression to fit in a frame (FRAME-005).

The budget of each vector is one of these:

- NBQ-VEC-001 to NBQ-VEC-015: a byte limit of `2097152` that the test gives, and the default tag
  limit. A reader vector gives it as a library user (NBQ-CFG-001). A packet vector sets it in the
  plugin. With this byte limit, the vectors stay small. A stack at the default byte limit does not
  fit in one packet.
- NBQ-VEC-016: a byte limit of `1000000` in the plugin.
- NBQ-VEC-017 to NBQ-VEC-020: the defaults of NBQ-CFG-001.

### NBQ-VEC-001 to NBQ-VEC-006: Reader, one network NBT

| Id | Root tag | Charge | Tags | Result |
|----|----------|--------|------|--------|
| NBQ-VEC-001 | Byte array, `N = 2097152`, with the data | 2 097 152 | 1 | passes |
| NBQ-VEC-002 | Byte array, `N = 2097153`, the input ends after `N` | 2 097 153 | 1 | quota error |
| NBQ-VEC-003 | Int array, `N = 2147483647`, the input ends after `N` | 8 589 934 588 | 1 | quota error, with no allocation for the data |
| NBQ-VEC-004 | List of Compound, `N = 600000`, then 600 000 End bytes | 2 400 005 at the head | 600 001 at the head | quota error at the head |
| NBQ-VEC-005 | List of Byte, `N = 262143`, with the data | 1 310 720 | 262 144 | passes |
| NBQ-VEC-006 | List of Byte, `N = 262144`, with the data | 1 310 725 | 262 145 at the head | quota error at the head |

### NBQ-VEC-007 to NBQ-VEC-011: Reader, one wire stack

Each stack is one `minecraft:stone` unless the row states a different item. `{"a": [B; N]}` is a
Compound with one entry with the name `a` and a Byte array of `N` bytes. Its charge is `N + 4`.

| Id | Stack | Charge | Tags | Result |
|----|-------|--------|------|--------|
| NBQ-VEC-007 | `minecraft:custom_data` = `{"a": [B; 2097148]}` | 2 097 152 | 2 | passes |
| NBQ-VEC-008 | `minecraft:custom_data` = `{"a": [B; 2097149]}` | 2 097 153 | 2 | quota error |
| NBQ-VEC-009 | `minecraft:container` with two stacks, each with `minecraft:custom_data` = `{"a": [B; 1048572]}` | 2 097 152 | 4 | passes |
| NBQ-VEC-010 | as NBQ-VEC-009, with `N = 1048573` in each | 2 097 154 | 4 | quota error in the second nested stack |
| NBQ-VEC-011 | `minecraft:written_book` with `minecraft:written_book_content`: title `t`, author `a`, 100 pages, each page a String text of 10 486 ASCII characters, no filtered text | 2 097 200 | 100 | quota error |

- NBQ-VEC-011 with 10 485 characters in each page has the charge 2 097 000 and passes. Each page
  alone has the charge 20 972, far below the quota for one text. Thus only the budget refuses
  NBQ-VEC-011.
- NBQ-VEC-012: `minecraft:written_book_content` with 262 145 pages, each page an empty String
  text. The charge is 0. The tag total is 262 145. The result is a quota error.

### NBQ-VEC-013 to NBQ-VEC-019: Packet

| Id | Packet | Result |
|----|--------|--------|
| NBQ-VEC-013 | the stack of NBQ-VEC-008 in a `set_creative_mode_slot` packet, slot 36 | `disconnect` of PLAY with `disconnect.genericReason`, then the close. Slot 36 does not change. The server does not panic, and the next tick runs. |
| NBQ-VEC-014 | the stack of NBQ-VEC-007 in the same packet | slot 36 holds the stack. The connection stays open. |
| NBQ-VEC-015 | the stack of NBQ-VEC-007, two times, in two packets | both pass, because each packet has its own budget |
| NBQ-VEC-016 | the stack of NBQ-VEC-007 | the refusal of NBQ-VEC-013 |
| NBQ-VEC-017 | an `edit_book` packet with 101 pages of 1 character | the refusal of NBQ-VEC-013 |
| NBQ-VEC-018 | the stack of NBQ-VEC-020 with 13 books and no names, slot 36 | slot 36 holds the stack. The connection stays open. |
| NBQ-VEC-019 | `minecraft:custom_data` = `{"a": [List of Byte, N = 300000]}`, with the data, slot 36 | the refusal of NBQ-VEC-013, at the head of the List: the tag total is 300 002 |

- NBQ-VEC-018 is the shulker box of a player in creative mode (NBQ-CFG-002). Its charge is
  15 974 400 and its tag total is 2 600. The packet has about 7 996 500 bytes, less than the limit
  of 8 388 608 bytes of an inflated packet.

### NBQ-VEC-020: Reader, the largest stack of survival play

- **Stack:** one `minecraft:shulker_box` with `minecraft:custom_name` of 50 times `U+3042`. Its
  `minecraft:container` holds 27 stacks of one `minecraft:written_book`, each with:
  - `minecraft:custom_name` of 50 times `U+3042`;
  - `minecraft:written_book_content`: title `t`, author `a`, 100 pages. Each page has a raw text
    and a filtered text, each a String text of 1024 times `U+3042`.
- **Charge:** 33 186 000, as NBQ-CFG-002 states. **Tags:** 5 428.
- **Result:** passes. The wire form has about 16 600 000 bytes, so only a library user can decode
  it alone. It does not fit in one packet.
