# mcrs_minecraft_wire

The `Encode` and `Decode` traits of the Minecraft: Java Edition protocol, the
variable-length integers, and the codecs for primitives, strings, sequences
and maps.

The crate knows no game types. A crate that owns a type implements the traits
for it, usually with the derive macros that this crate re-exports.

Every codec returns `Error`. `Error::kind` gives an `ErrorKind` that a caller
can match on, and `Error::context` gives the fields that led to the error.

Optional features add the codecs of foreign types: `glam` for the vectors,
`uuid` for `Uuid`, and `serde` for the serde traits on `Bounded`, `VarInt` and `VarLong`.
