# Block entity data on the wire: behavioural specification

Version: see `assets/minecraft/version.json`.

This document states the wire form of the data of a block entity in two clientbound packets of the
PLAY state: `level_chunk_with_light` and `block_entity_data`. It states what a reader does with
each form, and which form the vanilla server writes.

## 0. Conventions

### 0.1 Terms

- **Server**, **client**, **vanilla server**, **vanilla client**, **clientbound**, **VarInt**: as
  `login.md`, section 0.1, defines them.
- **Network NBT**: as `item-stack.md`, section 0.1, defines it. The type byte of a Compound is
  `0a`.
- **End tag**: the single byte `00` in the place of a network NBT. It has no payload.
- **Block entity type id**: the numeric id of a block entity type in the registry
  `minecraft:block_entity_type`.
- **Client data**: the NBT compound that the vanilla server makes from a block entity for the
  clients. Client data can have no keys.
- **Chunk entry**: one element of the list of block entities in `level_chunk_with_light`.
- **Entry data**: the last field of a chunk entry.
- **Column**: the chunk column of a `level_chunk_with_light` packet. `cx` and `cz` are its
  coordinates.

### 0.2 Provenance

The labels of `registry.md`, section 0.3, apply. The kinds of rule are those of `README.md`,
section "Semantics, not accidents".

### 0.3 Artefacts cited

- `assets/mcrs/reports/registries.json`: the block entity type ids.
- `assets/mcrs/reports/packets.json`: the packet ids. In the play clientbound table,
  `minecraft:block_entity_data` is 6 and `minecraft:level_chunk_with_light` is 46.

### 0.4 ID prefixes

| Prefix | Subject |
|--------|---------|
| `BES-NET` | the data of a block entity in a clientbound packet |

### 0.5 Scope

In scope:

- the entry data of each chunk entry;
- the NBT field of `block_entity_data`.

Out of scope:

- the other fields of the two packets;
- the keys of the client data of each block entity type;
- the time at which the server sends `block_entity_data` (`block-simulation.md`, SIM-SET-005;
  `block-breaking.md`, BRK-REF-004);
- the entity data component of a stack. `item-stack.md`, STK-SHP-049, states its own form.

---

## 1. The chunk packet

### BES-NET-001: Chunk entry

- **Observer:** the vanilla client.
- **Behavior:** the list of block entities comes after the byte array of the sections in the chunk
  data of `level_chunk_with_light`. The light data comes after the list. The list is one VarInt
  with the number of chunk entries, then the chunk entries. A chunk entry has these fields, in this
  order:

| Field | Wire form | Value |
|-------|-----------|-------|
| packed xz | 1 byte | `(x & 15) << 4 \| (z & 15)`; `x` and `z` are the block coordinates |
| y | signed 16-bit integer, big-endian | the block coordinate `y` |
| type | VarInt | the block entity type id |
| entry data | one network NBT, or the end tag | BES-NET-002 and BES-NET-003 |

- **Position:** the reader adds `16 · cx` to the high 4 bits of packed xz. It adds `16 · cz` to
  the low 4 bits.
- **Entry data:** the entry data has one of two forms:
  - a network NBT with a Compound as its root tag;
  - the end tag, which means "no data".
- **Example:** `2c ff e8 01 00` is a chunk entry at the local `x` 2, the local `z` 12 and the `y`
  −24. The type id 1 is `minecraft:chest`. The entry data is the end tag.

Provenance: `source_suggested`. Kind: wire. Confidence: medium. Experiment E1 agrees.

### BES-NET-002: Reader of the entry data

- **Observer:** the vanilla client, and a user of the library that decodes the packet.
- **End tag:** the chunk entry has no data. This is not an error. The vanilla client loads no data
  into the block entity at the position.
- **Compound:** the chunk entry has data, also when the compound has no keys (`0a 00`). The vanilla
  client loads the compound into the block entity at the position, when that block entity has the
  type of the chunk entry. If not, it ignores the entry data.
- **Other root tag:** a root tag of another type is a decode error of the packet. A type byte that
  is not a tag type is also a decode error.
- **Limits:** the reader reads each entry data with its own limits. The limits are those of
  `item-stack.md`, STK-SHP-004: a depth of 512 levels and a quota of 2097152 bytes. An entry data
  past a limit is a decode error.

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

### BES-NET-003: Writer of the entry data

- **Observer:** the vanilla client, through the bytes on the wire.
- **Entries:** the vanilla server writes one chunk entry for each block entity of the column. It
  also writes the chunk entry of a block entity with no data. The order of the chunk entries has
  the kind unspecified.
- **Form:** the server must write the end tag when the client data has no keys. The server must
  write the compound when the client data has one key or more. Thus the vanilla server never
  writes `0a 00` as entry data.
- **Types with client data:** only these block entity types can have client data with keys:

| Block entity types |
|--------------------|
| `minecraft:banner`, `minecraft:beacon`, `minecraft:brushable_block`, `minecraft:campfire`, `minecraft:conduit` |
| `minecraft:creaking_heart`, `minecraft:decorated_pot`, `minecraft:end_gateway`, `minecraft:hanging_sign`, `minecraft:jigsaw` |
| `minecraft:mob_spawner`, `minecraft:piston`, `minecraft:shelf`, `minecraft:sign`, `minecraft:skull` |
| `minecraft:structure_block`, `minecraft:test_block`, `minecraft:test_instance_block`, `minecraft:trial_spawner`, `minecraft:vault` |

- **Other types:** for each other type, the client data has no keys. Its entry data is always the
  end tag. Examples: `minecraft:chest`, `minecraft:furnace`, `minecraft:copper_golem_statue`.
- **Types of the table:** the client data of such a type can also have no keys. An example is a
  `minecraft:brushable_block` that holds no item and has no hit direction. Its entry data is then
  the end tag.
- **Limits:** the server must not write entry data past the limits of BES-NET-002.

Provenance: `source_suggested`. Kind: wire, unspecified for the order of the chunk entries.
Confidence: medium. Experiment E1 agrees for `minecraft:chest` and `minecraft:mob_spawner`.

---

## 2. The block entity data packet

### BES-NET-004: NBT of `block_entity_data`

- **Observer:** the vanilla client, and a user of the library that decodes the packet.
- **Wire form:** the last field of `block_entity_data` is one network NBT with a Compound as its
  root tag. The two forms of BES-NET-001 do not apply to this field. The end tag is not a valid
  value here.
- **Reader:** these inputs are a decode error of the packet:
  - the end tag;
  - a root tag of another type than Compound.

  A compound with no keys (`0a 00`) is valid. The vanilla client reads the field with a depth of
  512 levels and with no quota of bytes.
- **Writer:** the server must write the client data as a compound, also when it has no keys. The
  server must not write the end tag in this field. For example, the vanilla server sends
  `block_entity_data` for a `minecraft:copper_golem_statue`, and the field is then `0a 00`.

Provenance: `source_suggested`. Kind: wire. Confidence: medium.

---

## 3. Experiments

Use a vanilla server of the game version of `assets/minecraft/version.json`, with
`network-compression-threshold=-1`. Connect one scripted client.

| Experiment | Steps | Rule |
|------------|-------|------|
| E1 | Load a column that holds two chests and one spawner. Read its `level_chunk_with_light` packet. Result: the list has three chunk entries. The entry data of each `minecraft:chest` entry is the end tag. The entry data of the `minecraft:mob_spawner` entry is a compound. | BES-NET-001, BES-NET-003 |

## 4. Intentional deviations

None.

## 5. Unresolved

None.
