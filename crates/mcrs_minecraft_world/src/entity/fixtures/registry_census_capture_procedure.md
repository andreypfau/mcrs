# Fixture Capture Procedure — `registry_census.bin`

**Source of truth:** vanilla `26.3-snapshot-10`, `world_version` 5015, read
through Fabric Loom's mapped jar. No server and no client is started.

**Harness:** `tools/vanilla-oracle/src/main/java/mcrs/oracle/RegistryCensusOracle.java`

```sh
cd tools/vanilla-oracle
./gradlew dumpRegistryCensus --console=plain --no-daemon \
    -PoracleOut=../../crates/mcrs_minecraft_world/src/entity/fixtures
```

Output is deterministic: re-running produces a byte-identical file (56 875
bytes).

**Consumer:** `crates/mcrs_minecraft_world/src/entity/census.rs`, which pins
the static tables whose ids cross the wire as registry indices — the
`entity_type` table, the items the shipped structures' mobs carry, villager
type and profession, and the entity attributes with their default, minimum,
maximum and syncable flag — and checks that the `cat_variant` and
`cat_sound_variant` assets, sorted by id as the registry snapshot sorts them,
land in the order the reference registers them.

## What is dumped

Every registry is walked by numeric id, `registry.byId(id)` for
`0 <= id < registry.size()`, so an entry's position in the list is the id the
wire carries for it.

| Registry | Source | Entries |
| --- | --- | --- |
| `minecraft:entity_type` | `BuiltInRegistries.ENTITY_TYPE` | 161 |
| `minecraft:item` | `BuiltInRegistries.ITEM` | 1658 |
| `minecraft:villager_type` | `BuiltInRegistries.VILLAGER_TYPE` | 7 |
| `minecraft:villager_profession` | `BuiltInRegistries.VILLAGER_PROFESSION` | 15 |
| `minecraft:attribute` | `BuiltInRegistries.ATTRIBUTE` | 40 |
| `minecraft:cat_variant` | the world registries loaded from the vanilla data pack through `RegistryDataLoader.load`, as `PlacementOracle.loadWorldRegistries` does | 11 |
| `minecraft:cat_sound_variant` | same | 2 |

The two data registries are loaded from the pack rather than taken from
`VanillaRegistries.createWorldLookup()`: the pack loader lists resources
through a `TreeMap`, so a server registers them sorted by id, which is the
order the client is told and the order `nextInt(size)` draws over.

The attribute block repeats the `ATTRIBUTE` registry with each entry's
`getDefaultValue()`, `RangedAttribute.getMinValue()`, `getMaxValue()` and
`isClientSyncable()`. Every shipped attribute is a `RangedAttribute`; the dump
casts and would fail loudly otherwise.

## Binary layout

Little-endian, same primitives as the other dumps: `u32` and `i32` are 4
bytes, `f64` is 8 (IEEE-754 from `Double.doubleToRawLongBits`), `u8` is one
raw byte, a `str` is a `u32` byte length followed by that many UTF-8 bytes.

```
magic            8 bytes, ASCII "MCREGCE0"
format_version   u32   currently 1
world_version    u32   SharedConstants.getCurrentVersion().dataVersion().version()

registry_count   u32
repeated registry_count times:
  registry_id    str   e.g. "minecraft:entity_type"
  entry_count    u32
  entry_ids      str * entry_count   in id order

attribute_count  u32
repeated attribute_count times, ATTRIBUTE id order:
  id             str
  default        f64
  min            f64
  max            f64
  syncable       u8    1 iff isClientSyncable()
```

The file ends after the last attribute; there is no trailer.
