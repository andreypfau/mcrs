# Configuration Protocol — Spec

Covers the Login→Configuration→Play protocol sequence and how `RegistrySnapshot` maps
to the wire format. Sources: `ServerConfigurationPacketListenerImpl.java`,
`RegistrySynchronization.java`, `RegistryDataLoader.java`, `TagNetworkSerialization.java`.

---

## 1. Packet Sequence

```
Server                                              Client
  │                                                    │
  ├─ ClientboundCustomPayloadPacket(BrandPayload) ────►│
  ├─ ClientboundServerLinksPacket (optional) ─────────►│
  ├─ ClientboundUpdateEnabledFeaturesPacket ──────────►│
  │                                                    │
  │  [SynchronizeRegistriesTask begins]                │
  │                                                    │
  ├─ ClientboundSelectKnownPacks([vanilla, ...]) ─────►│
  │◄─── ServerboundSelectKnownPacks([known packs]) ────┤
  │                                                    │
  │  [For each of 23 SYNCHRONIZED_REGISTRIES]:         │
  ├─ ClientboundRegistryDataPacket(registry, entries) ►│
  │  ...                                               │
  │  [After all registry packets]:                     │
  ├─ ClientboundUpdateTagsPacket(all tags) ───────────►│
  │                                                    │
  │  [Optional: resource pack task, EULA task, etc.]   │
  │                                                    │
  │  [PrepareSpawnTask, JoinWorldTask queued]           │
  │◄─── ServerboundFinishConfigurationPacket ──────────┤
  │                                                    │
  │  [Switch to PLAY protocol, spawn player]           │
```

Source: `ServerConfigurationPacketListenerImpl.java:94–116`.

---

## 2. The 23 Synchronized Registries

From `RegistryDataLoader.SYNCHRONIZED_REGISTRIES` (`RegistryDataLoader.java:164–190`).
Listed in **canonical send order**:

| # | Registry Key | Codec Used | requiredNonEmpty |
|---|-------------|------------|-----------------|
| 1 | `minecraft:worldgen/biome` | NETWORK_CODEC | false |
| 2 | `minecraft:chat_type` | DIRECT_CODEC | false |
| 3 | `minecraft:trim_pattern` | DIRECT_CODEC | false |
| 4 | `minecraft:trim_material` | DIRECT_CODEC | false |
| 5 | `minecraft:wolf_variant` | NETWORK_CODEC | **true** |
| 6 | `minecraft:wolf_sound_variant` | NETWORK_CODEC | **true** |
| 7 | `minecraft:pig_variant` | NETWORK_CODEC | **true** |
| 8 | `minecraft:frog_variant` | NETWORK_CODEC | **true** |
| 9 | `minecraft:cat_variant` | NETWORK_CODEC | **true** |
| 10 | `minecraft:cow_variant` | NETWORK_CODEC | **true** |
| 11 | `minecraft:chicken_variant` | NETWORK_CODEC | **true** |
| 12 | `minecraft:zombie_nautilus_variant` | NETWORK_CODEC | **true** |
| 13 | `minecraft:painting_variant` | DIRECT_CODEC | **true** |
| 14 | `minecraft:dimension_type` | NETWORK_CODEC | false |
| 15 | `minecraft:damage_type` | DIRECT_CODEC | false |
| 16 | `minecraft:banner_pattern` | DIRECT_CODEC | false |
| 17 | `minecraft:enchantment` | DIRECT_CODEC | false |
| 18 | `minecraft:jukebox_song` | DIRECT_CODEC | false |
| 19 | `minecraft:instrument` | DIRECT_CODEC | false |
| 20 | `minecraft:test_environment` | DIRECT_CODEC | false |
| 21 | `minecraft:test_instance` | DIRECT_CODEC | false |
| 22 | `minecraft:dialog` | DIRECT_CODEC | false |
| 23 | `minecraft:timeline` | NETWORK_CODEC | false |

**NETWORK_CODEC vs DIRECT_CODEC**:
- `NETWORK_CODEC` omits server-side-only fields (generation settings, mob spawning) to reduce bandwidth
- `DIRECT_CODEC` sends the full data object as used in resource files
- Biome: NETWORK_CODEC omits `BiomeGenerationSettings` and `MobSpawnSettings`
- DimensionType: NETWORK_CODEC difference is `EnvironmentAttributeMap.NETWORK_CODEC` vs full

**`requiredNonEmpty = true`**: These variant registries **must** have at least one entry — they
correspond to entity appearance variants and must never be empty client-side.

---

## 3. Wire Formats

### ClientboundRegistryDataPacket

```
[Identifier]   registryName        e.g. "minecraft:worldgen/biome"
[VarInt]       entryCount
  for each entry:
    [Identifier]  entryId           e.g. "minecraft:plains"
    [bool]        hasData
    if hasData:
      [NBT Tag]   encodedData       NETWORK_CODEC or DIRECT_CODEC output
```

Entry IDs are sent in **registry insertion order** (= data pack load order).
The 0-based index of each entry in this stream becomes its **network numeric ID**.

### ClientboundUpdateTagsPacket

Sent **after** all `ClientboundRegistryDataPacket` packets, so numeric IDs are
already stable.

```
[VarInt]  registryCount
  for each registry:
    [ResourceKey]  registryKey
    [VarInt]  tagCount
      for each tag:
        [Identifier]  tagName       e.g. "minecraft:is_overworld"
        [VarInt]      entryCount
          [VarInt]    id1           numeric ID from RegistryDataPacket order
          [VarInt]    id2
          ...
```

Source: `TagNetworkSerialization.java`, `ClientboundUpdateTagsPacket.java`.

---

## 4. Known Packs Optimization

`ClientboundSelectKnownPacks` sends the server's list of data packs.
Client replies with `ServerboundSelectKnownPacks` listing only packs it already has locally.

For each registry entry, the server checks:
```
canSkip = registry.registrationInfo(entry.key())
              .flatMap(RegistrationInfo::knownPackInfo)
              .filter(clientKnownPacks::contains)
              .isPresent()
```

If `canSkip` → send `Optional.empty()` for the entry's data.
Client reconstructs the entry from its local pack files instead.

**Result**: For a vanilla client connecting to a vanilla server, almost all NBT data is
skipped — only entry IDs (Identifiers) are sent to establish the numeric-ID mapping.

---

## 5. RegistrySnapshot Design (Rust)

The `RegistrySnapshot` resource is built **once** during `OnEnter(AppState::WorldgenFreeze)`.
It provides:
1. The **canonical ordered list** of entries per registry (matching the 23-registry order above)
2. **Pre-serialized NBT** for each entry (encoded via NETWORK_CODEC equivalent)
3. A `biome_id(rl: &ResourceLocation) -> u32` lookup (for tag numeric IDs)

```rust
/// Built once at WorldgenFreeze; frozen for the duration of the server run.
/// Reconfiguration replaces this resource and flushes all chunk caches.
#[derive(Resource)]
pub struct RegistrySnapshot {
    /// One entry per synced registry, in canonical send order.
    pub registries: Vec<SnapshotRegistry>,
    /// Resolved tags for all synced registries, in the same order.
    pub tags: Vec<SnapshotTagRegistry>,
}

pub struct SnapshotRegistry {
    pub registry_key: ResourceLocation,
    /// Ordered entries. Index = network numeric ID.
    pub entries: Vec<SnapshotEntry>,
}

pub struct SnapshotEntry {
    pub id: ResourceLocation,
    /// Pre-serialized NBT bytes (None if vanilla / can be skipped for known-pack clients).
    pub data: Option<Vec<u8>>,
}

pub struct SnapshotTagRegistry {
    pub registry_key: ResourceLocation,
    pub tags: Vec<SnapshotTag>,
}

pub struct SnapshotTag {
    pub name: ResourceLocation,
    pub entries: Vec<u32>,  // Numeric IDs from SnapshotRegistry.entries order
}
```

### Build Process (WorldgenFreeze system):

```rust
fn build_registry_snapshot(
    biomes: Res<Assets<Biome>>,
    dimension_types: Res<Assets<DimensionType>>,
    // ... other registry assets
    static_registries: Res<StaticRegistries>,
    static_tags: Res<StaticTags<Block>>,
    tags: Res<Tags<Biome>>,
    mut commands: Commands,
) {
    let mut snapshot = RegistrySnapshot::default();

    // 1. Biomes (entry #0 in canonical order)
    let biome_snapshot = build_asset_registry_snapshot::<Biome, _>(
        &biomes,
        Biome::encode_network, // uses NETWORK_CODEC equivalent
    );
    snapshot.registries.push(biome_snapshot);

    // ... (23 registries in order)

    // 2. Build tag snapshots (after all registries, so numeric IDs are fixed)
    let biome_tags = build_asset_tag_snapshot::<Biome>(&tags, &snapshot.registries[0]);
    snapshot.tags.push(biome_tags);

    // ...

    commands.insert_resource(snapshot);
}
```

### Sending to a Client (per-connection):

```rust
fn send_registry_data(
    snapshot: Res<RegistrySnapshot>,
    client_known_packs: &KnownPackSet,
    conn: &mut Connection,
) {
    for registry in &snapshot.registries {
        let entries = registry.entries.iter().map(|e| PackedEntry {
            id: e.id.clone(),
            data: if client_known_packs.covers(&e.id) {
                None  // skip, client has it
            } else {
                e.data.clone()
            },
        }).collect();

        conn.send(ClientboundRegistryDataPacket {
            registry: registry.registry_key.clone(),
            entries,
        });
    }

    // Send tags after all registry packets
    conn.send(ClientboundUpdateTagsPacket {
        tags: snapshot.tags.iter().map(|t| t.into_packet_payload()).collect(),
    });
}
```

---

## 6. Reconfiguration (Hot Reload)

When data packs change at runtime, all connected clients must re-enter Configuration state:

```rust
fn trigger_reconfiguration(
    mut commands: Commands,
    clients: Query<Entity, With<InPlayState>>,
) {
    for client in &clients {
        commands.entity(client).insert(SendStartConfigurationPacket);
    }
    commands.insert_resource(ReconfigurationInProgress);
}

fn finish_reconfiguration(
    // Called after all clients ack and configuration round-trip completes
    mut snapshot: ResMut<RegistrySnapshot>,
    new_snapshot: Res<PendingRegistrySnapshot>,
    mut chunk_cache: ResMut<ChunkCache>,
) {
    *snapshot = new_snapshot.take();
    chunk_cache.flush_all();  // Old numeric IDs are now invalid
}
```

**Why `flush_all()`**: Chunk data includes biome numeric IDs (from biome palette in section NBT).
After reconfiguration, biome IDs may have shifted. All cached chunks must be regenerated.

---

## 7. Key Source Locations

| File | Purpose |
|------|---------|
| `server/network/ServerConfigurationPacketListenerImpl.java` | Server-side packet sequence |
| `core/RegistrySynchronization.java` | `packRegistries()`, known-pack filtering |
| `resources/RegistryDataLoader.java:164–190` | `SYNCHRONIZED_REGISTRIES` list |
| `network/protocol/configuration/ClientboundRegistryDataPacket.java` | Packet + wire codec |
| `network/protocol/common/ClientboundUpdateTagsPacket.java` | Tags packet |
| `tags/TagNetworkSerialization.java` | Tag → numeric ID serialization |
| `client/multiplayer/RegistryDataCollector.java` | Client-side collector + assembler |
| `world/level/biome/Biome.java:37–63` | DIRECT_CODEC vs NETWORK_CODEC example |

## See also

- [../worldgen/12-network-registry-sync.md](../worldgen/12-network-registry-sync.md) — Java registry sync spec (23 registries, NETWORK_CODEC vs DIRECT_CODEC)
- [06-registry-redesign.md](06-registry-redesign.md) — RegistrySnapshot design used in Configuration phase
- [10-implementation-order.md](10-implementation-order.md) — Step 10 where Configuration protocol is implemented
