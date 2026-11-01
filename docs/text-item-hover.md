# Text with an item hover at the wire layer

This note states where `Text` lives, how a `show_item` hover is typed below and above the item crate, and what a packet holds. It records how other libraries solve the same problem.

## Problem

A text component can carry a `show_item` hover. The hover holds an item stack: an item id, a count and a patch of item data components. An item data component is game data. Its codec needs the item crate, and some components need the registries.

The protocol crate must be wire layer only. A proxy and a bot use it, and they must not pay for the item crate. Today the protocol crate defines `text::Text` as `mcrs_minecraft_item::Text<Named>`. So every packet that carries text pulls in the item crate. These packets include the configuration disconnect, the common disconnect and the resource pack prompt.

## Terms

- **Opaque item**: an item stack with a typed id and count, and with each data component value kept as the NBT tag that it arrived as.
- **Typed item**: an item stack whose components are decoded into the item crate's component types.
- **Text types**: the type parameter `I: TextTypes` of `Text<I>`. `I` is what a `show_item` hover carries.
- **Raw text**: a checked, borrowed slice of the bytes of one NBT tag that holds a text component.

## Other libraries

| Library | Text type and home | `show_item` storage | Protocol depends on items | Pass-through without decode | Cost |
|---------|--------------------|---------------------|---------------------------|-----------------------------|------|
| Adventure (Java) | `Component`, `api` module. The module depends only on `adventure-key`. | `Key` id, `int` count, `Map<Key, DataComponentValue>`. A value is opaque: a JSON tree for the gson serializer, SNBT text for `BinaryTagHolder`, or the `Removed` marker for `!key`. | No. | No raw text type. Unknown component values stay as untyped trees. | An eager tree per value. The hover is read twice. Value conversion goes through a converter registry. |
| MCProtocolLib (Java) | Adventure `Component`, with its own NBT serializer module. | Adventure `ShowItem`. Each value is `NbtDataComponentValue`, which holds raw NBT. Any other value kind fails on write. | No. Nothing converts its `ItemStack` to a hover. | No. Every packet decodes the text. | An NBT tree, then a `Component` tree, on each read and write. |
| Minestom (Java) | Adventure `Component`. | Typed values (`MinestomDataComponentValue`) or raw NBT values. | Yes. A typed hover needs the registries to write. Without them it fails. Read drops the components (a TODO). | Pre-encoded outbound packets only. | Streamed write. Eager read. A transcoder per component value. |
| Velocity (Java, proxy) | `ComponentHolder`: the raw NBT tree, or the JSON string, or a decoded `Component`. One form is set; the others are made on demand. | Never modelled. | No. | Yes. An undecoded holder writes back the tree it read. Unregistered packets pass as raw bytes. | A full NBT tree on each read, even for pass-through. A conversion from a `Component` loses NBT number types. |
| Azalea (Rust) | `FormattedText`, `azalea-chat`. It does not depend on `azalea-inventory`. | A placeholder with no fields. Decode drops every hover event. | No. | Raw packets only. | An owned tree per node; a clone of the tree on write. |
| Valence (Rust) | `Text`, `valence_text`. It depends on no item or registry crate. | `Ident` id, count, and the item NBT as an SNBT string (the format before data components). | No. | Frames only. | An owned tree; every string copied; JSON on the wire. |

No library makes the protocol layer depend on item types. The libraries that keep the hover lossless (Adventure with NBT values, MCProtocolLib) type the id and count and keep each component value opaque. Minestom types the values in its network layer and so needs registries there; its read path loses data. Azalea and Valence lose data or use an old format. Velocity alone forwards text that it does not decode, but it still builds a tree for each read.

## Design

### Where `Text` lives

`Text<I>` stays in `mcrs_minecraft_text`, layer 1. The crate depends on `wire`, `nbt`, `resource` and `profile`, and on no game crate.

### `show_item` at layer 1

The text crate has one text types instance, `ItemHover`:

```rust
pub struct ItemHover {
    pub id: ResourceLocationBuf,
    pub count: i32,
    pub components: NbtCompound,
}
```

- `ItemHover` implements `TextTypes` with `EntityKey = ResourceLocationBuf` and `Dialog = DialogRef`.
- Its serde implementation reads and writes the `show_item` fields: `id`, `count` and `components`. It reads only the object form. It ignores an unknown field.
- `components` keeps each value as the tag that arrived. A removal stays under its `!key` with the object that arrived.
- `NbtCompound` keeps its entries in order, and each tag keeps its NBT type. So a read and a write give the same bytes.
- `ItemHover` is the default type parameter of `Text`, `Style`, `ClickEvent` and `IntoText`. There is no other text types instance below the item crate. A text that holds no item hover is a `Text<ItemHover>` that holds none.

### Typed item hovers at layer 3 and above

The item crate has `ItemHoverMap`. It implements `TextMap<ItemHover, Template<Named>>` and `TextMap<Template<Named>, ItemHover>`.

- The map writes the value to an NBT tag and reads the tag with the codec of the target type. So the typed direction applies the checks of the `Template<Named>` codec, and the other direction writes what that codec writes.
- The typed direction fails on a component key that the item crate does not know, on a value that its codec refuses, and on a count outside 1 to 99. The error is a `ResolveError` with the path of the hover in the text.
- The registries crate maps `Template<Named>` to `Template<Resolved>` and back. That step checks that the item id names a registered item.
- A server or a client that needs typed hovers calls `text.map(&ItemHoverMap)` after decode and before use. A proxy and a bot never call the map and never link the item crate.

### Text in a data pack

A data-pack field that holds text has the type `Text<Template<Named>>`, and the load resolves it to `Text<Template<Resolved>>`. A data-pack field never holds `Text<ItemHover>`.

- So the load checks each `show_item` hover as REG-FLD-442 states: a registered item that is not `minecraft:air`, a count from 1 to 99, component keys with a persistent form, values that decode, and no two keys for the same component in the same form.
- A failed check fails the text, and so the file that holds it. The load never keeps an unchecked hover.
- A `!key` value must be an object. The load ignores its fields, and the encoder writes `{}` (REG-FLD-502).
- The load ignores an unknown field of the hover object, as vanilla does.

### What the packets hold

The rule follows the layer of the packet.

- **A packet in `mcrs_minecraft_protocol`** (handshake, status, login, configuration, and the play packets without game data) holds `Text<ItemHover>`, as `Cow<'a, Text<ItemHover>>` where the packet borrows. These packets are rare: a disconnect, a resource pack prompt. The network state machine reads their text itself, for example to report a disconnect reason. So the decoded form costs nothing that matters.
- **A packet in `mcrs_minecraft_protocol_game`** holds `RawText<'a>` for each text field of the packet. These packets are the hot path: system chat, player chat, titles, the action bar, the boss bar and the tab list. A proxy forwards them, and a server sends them to many players. `RawText<'a>` checks at decode that the bytes are one well-formed NBT tag, and holds a borrowed slice of them. It does not build a tree and it does not allocate.
- **Decode of a `RawText<'a>`** is an explicit call that gives `Text<ItemHover>`. A reader that needs typed hovers then calls `ItemHoverMap`.
- **Encode** writes the slice of a `RawText<'a>` unchanged. A server makes a `RawText<'a>` from a `Text<ItemHover>` by one encode into a buffer that it owns, and shares that frame between players.
- **Text inside a game data value**, for example the `custom_name` component of an item in a container packet, is part of that value. It has the typed form of the value, not `RawText<'a>`.

### Round trip

- **Wire to `Text<ItemHover>` to wire:** symmetric by construction. Each component value is the tag that was read.
- **Wire to `RawText<'a>` to wire:** the same bytes.
- **`Text<ItemHover>` to `Text<Template<Named>>` to `Text<ItemHover>`:** symmetric when the item component codec is symmetric. A test maps every item of the item definition corpus, with all its default components and one removal, through both directions and compares the results.
- **A component key that the item crate does not know:** the typed map refuses it. Layer 1 keeps it. A proxy therefore passes a modded component through, and a server reports it.

## Cost

- A decode to `Text<ItemHover>` builds the text tree once. `ItemHover` adds one `ResourceLocationBuf` and one `NbtCompound` per hover.
- `ItemHoverMap` clones each hover into an NBT tag. It runs only where a caller asks for typed hovers, never on a hot path.
- A `RawText<'a>` costs one validation pass over its bytes on decode and one copy on encode.
- No `dyn` and no `Arc` are involved.
