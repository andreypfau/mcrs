# Health, damage, death and respawn: design

Design note for the health of a living entity, the hit pipeline, the death and the respawn.
Behaviour comes from `docs/spec/entity-damage.md` (DMG), `docs/spec/player-death.md` (PLD),
`docs/spec/command-damage.md` and `docs/spec/player-attack.md`.

This note assumes that a mob is an ECS entity. The decision issue #1067 states that
migration. A name that does not exist in the code yet is planned.

## 1. Classification of the state

| Value | Kind | Owner |
|-------|------|-------|
| `Vitals` on a living entity | authoritative truth: `health`, `absorption`, `last_damage`, `damage_cooldown`, `hurt_time`, `death_time` | `apply_hits` and `tick_vitals` in `mcrs_minecraft_engine_damage` |
| `HitRecord` on a living entity | authoritative truth: the last accepted hit and the strike count | `apply_hits` |
| maximum health | derived from the attribute `max_health` on each read | none |
| "is alive", "is dying" | derived: `Vitals::is_alive`, `Vitals::death_time` | none |
| `Defence` on a living entity | materialized derived state: armor, toughness, resistance, protection | one replaceable system; it runs before the motion pass and in `DamageSet::Collect`; cost: one write for each change of an attribute or an effect |
| `DamageTypeRules` | materialized derived state: one bit row for each damage type id, baked from the tags of the `damage_type` registry | one bake system, gated on a registry change |
| `PendingHits` | work queue of the tick; empty after `DamageSet::Apply` | each hit source appends; `apply_hits` drains |
| `Damaged`, `Died` | temporal facts (messages) | `apply_hits` |
| `RespawnRequest` | temporal fact (message, a request) | a client packet handler, a minigame or a bot |
| health bar, death screen, hurt animation | derived on the client from `Vitals` and from the damage event packet | rendering plugins |

A player and a mob carry the same `Vitals` component. No second model of a living entity exists.
`Living` in `mcrs_minecraft_physics` loses the health fields. The motion step reads the alive bit
from `Vitals`.

## 2. Crate map

| Crate | Layer | Content |
|-------|-------|---------|
| `mcrs_minecraft_entity_key` (`no_std`) | 3, data | `EntityUuid`, `WriterKey`, `Cause { causing, direct }` |
| `mcrs_minecraft_damage` (`no_std`, feature `bevy` for derives only) | 3, rules | `Vitals`, `HitRecord`, `Hit`, `DamageSource`, `Defence`, `DamageTypeRules`, `HurtRules`, `HitReport`, `HitOutcome`, `hurt`, `preview`, `knockback`, `KnockbackRules`, `fall_damage` |
| `mcrs_minecraft_engine_damage` (new) | 4 | `DamagePlugin`, `DamageSet`, `PendingHits`, `DamageRequest`, `Damaged`, `Died`, `RespawnRequest`; no vanilla behaviour |
| `mcrs_minecraft_vanilla_player_attack` (new) | 5 | the attack packet becomes a `DamageRequest` and a knockback; needs #93 and #95 |
| `mcrs_minecraft_vanilla_death` (new) | 5 | death drops and death messages, two plugins |
| `mcrs_minecraft_vanilla_respawn` (new) | 5 | the respawn packet becomes a `RespawnRequest`; the respawn position rules |
| `mcrs_minecraft_command_vanilla` | 5 | `/damage` and `/kill` write a `DamageRequest` |
| `mcrs_minecraft_client_world` | 4 | `Vitals` on the own player, written from the health packet |
| `mcrs_minecraft_client_gui` | 6 | health HUD plugin, death screen plugin |

`mcrs_minecraft_damage` stops its dependency on `mcrs_minecraft_physics`. A balance tool takes
`mcrs_minecraft_damage` and `mcrs_minecraft_entity_key` only.

## 3. The kernel

```rust
pub fn hurt(
    vitals: &mut Vitals,
    record: &mut HitRecord,
    defence: &Defence,
    types: &DamageTypeRules,
    rules: &HurtRules,
    hit: Hit,
    tick: u64,
) -> HitReport
```

- `Hit { amount: f32, source: DamageSource }`.
- `DamageSource { damage_type: u32, cause: Cause, position: Option<DVec3> }`.
- `HitReport { outcome: HitOutcome, armor: f32, absorption: f32, health: f32, died: bool }`. The
  three amounts state where the hit went.
- `HurtRules` holds the rules `mcrs:dying_entities_targetable` and `mcrs:totem_covers_whole_hit`
  and the Java or Bedrock parity switches. An edge passes its own value; no fork of the kernel.
- `knockback(velocity, direction, strength, resistance, &KnockbackRules) -> DVec3` and
  `fall_damage(distance, multiplier, safe_distance) -> f32` are pure functions.

The kernel reads no registry and no world. A custom damage type from a data pack gets its row in
`DamageTypeRules` from its tags, with no code change.

## 4. The hit pipeline in the tick

`DamageSet` is a chain of four sets in `FixedUpdate`, in `TickSet::WorldWrite`, after the motion
pass.

1. `Collect`. Each source appends a `DamageRequest { target: Entity, hit: Hit, writer: WriterKey,
   tick: u64, sequence: u32 }` to `PendingHits`. `tick` is the write tick and `sequence` the order of
   the writes of one writer to one target in that tick. A write from another cell keeps both values. The motion step appends the own hits of an entity (step action, inside
   actions, freezing, fall, crowd; DMG-ORD-002) through a per-thread queue. The system that fills `Defence` runs here again, after the changes of the pass.
2. `Filter`. A user system takes `ResMut<PendingHits>`. It changes an amount or removes a request.
   The engine adds no system to this set. A mutable queue, not an observer: the order is stable,
   the hits of a tick are one batch, and no dispatch runs for each hit.
3. `Apply`. `apply_hits` sorts the queue by (`EntityUuid` of the target, `WriterKey`, write tick,
   sequence). It calls `hurt` for each request. It writes `Vitals` only on a real change. The
   attack writes its sweep hits (ATK-SWP-002) in `Collect`, together with the primary hit, and
   `Filter` sees them as other requests. Each sweep hit names its primary hit. `apply_hits` holds
   the sweep hits back from the sorted queue. After the last request of the queue, it applies the
   sweep hits of each accepted primary hit, in the order of PHY-CMP-015 (ATK-ORD-003). It drops
   the sweep hits of a rejected primary hit.
4. `Report`. `apply_hits` has written one `Damaged { target, source, report }` for each accepted hit
   and one `Died { target, position, source }` for each death. `position` is the position of the
   target (DMG-DTH-002). Consumers run here.

`tick_vitals` runs in `TickSet::Simulate`. It decreases `damage_cooldown` and `hurt_time` and
advances `death_time`. It is gated on a set of entities with a non-zero counter, not on the
population.

Rules:

- One apply point exists for each tick for the hits of other entities and the own hits of
  DMG-ORD-002. A hit that is written before `Apply` applies in the same tick.
- Every hit source, also a hit of the base entity tick, writes a request to `PendingHits`. No source
  calls `hurt` directly.
- The hits of the base entity tick are the exception to the one apply point. They apply at their
  step (DMG-ORD-002): that step takes its own requests from the queue and applies them in the order
  of `Apply`. `Apply` after the pass takes all other requests. Later issues of
  `docs/spec/environment-damage.md` decide the systems of that step and how `Filter` sees its requests.
- A hit of the base entity tick reads `Defence` from the run before the motion pass. That value has
  each change before the pass. The hit comes before the effect tick of its step (ENV-ORD-001,
  EFX-TCK-001), so no later change of the same tick reaches it in the reference either.
- The order of the queue does not depend on the thread order or on the spawn order.
- A death removes nothing by itself. A layer 5 plugin appends a mob to the sorted despawn queue when its death
  time reaches 20 (DMG-DTH-003). A
  player stays until a `RespawnRequest` arrives.
- A respawn starts only from a `RespawnRequest`. The engine never writes one.
- The owner of a player gets the health packet when the health, the food level or the state "the
  saturation is 0" changes (DMG-NET-005), on players only. Viewers get
  the damage event from `Damaged` through the tile events, encoded once.
- Knockback is a velocity write through the ping bracket of #93 for a player and a motion impulse
  for a mob. `apply_hits` writes no velocity.

## 5. Use cases

| User | How the design serves it |
|------|--------------------------|
| PvP minigame | A system in `DamageSet::Filter` removes lobby hits and friendly fire. The kill feed reads `Died` with `Cause`. It omits the death screen plugin and the respawn plugin, and writes `RespawnRequest` or sets spectator mode itself. |
| Creative or build server | It adds no environment damage plugin. `/kill` still writes a `DamageRequest`. |
| RPG server | A data pack adds damage types. It replaces the system that fills `Defence`. Damage indicators and lifesteal read `HitReport` in `Damaged`. |
| Headless bot | It reads `Vitals` on its own player. The damage event packet gives the `Cause`. |
| Anticheat or proxy | It reads `DamageRequest` in `Filter`: the reach (#95), the attack rate, the `Cause`. |
| Online map or statistics | It reads `Died`: the position of the target and the `DamageSource`. |
| Balance calculator | It calls `hurt` in a loop with a `Defence` value. No Bevy. |
| Bedrock edge (#655) | `KnockbackRules` and `HurtRules` hold the differences. |

## 6. Migration path

1. PR 0, `refactor` (#1068, merged): `EntityKey` becomes `EntityUuid` over `uuid::Uuid`. Same bits,
   same order.
2. PR 1, docs: the decision issue #1067, this note, `docs/spec/player-attack.md`, and the changes
   of the specifications that the decision needs.
3. PR 2: mobs become ECS entities; `mcrs_minecraft_entity_rows` is deleted. Health stays in
   `Living`; behaviour does not change.
4. PR 3, Closes #103: `Vitals` leaves `Living`; the kernel of section 3; `mcrs_minecraft_engine_damage`;
   crowd, `/damage` and `/kill` go through `PendingHits`. `hurt_mob` and `MobChanges::strikes` are
   deleted.
5. PR 4, Refs #106, Closes #116: `Vitals` on a player, health packets, `Died`, death plugins,
   respawn plugin. The player attack joins when #93 and #95 are ready; if not, it is a sixth PR.
6. PR 5, Closes #106: client `Vitals`, HUD, death screen, attack on the entity under the cursor.

## 7. Open points

- The player attack depends on #93 and #95.
- The save of `Vitals` goes through the persisted registry (#159). No rule states the save of a mob.
