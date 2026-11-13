use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::Ref;
use glam::DVec3;
use mcrs_engine_muxer::EntityEvent;
use mcrs_minecraft_engine::tile_events::TileEvents;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::write_wire;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_registries::Wire;
use mcrs_minecraft_replication::MobReport;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::MobSpawn;
use mcrs_minecraft_replication::MobState;
use mcrs_minecraft_replication::ReplicationData;
use mcrs_minecraft_replication::SpawnLook;
use mcrs_minecraft_wire::Decode;
use rustc_hash::FxHashMap;
use uuid::Uuid;

use crate::ItemCollector;
use crate::PickedUp;
use crate::item_row::ItemRow;
use crate::step_items::Stepped;

/// The rows of the item entities that left the world in this tick, each in its tile.
#[derive(Resource, Debug, Default)]
pub(crate) struct Released(pub(crate) Vec<Placed>);

/// Records the tile events of the item entities of the tick (IEN-NET-001 to IEN-NET-004): a
/// spawn with the stack, the change of tile, a move, a new stack, and the removal, after the take
/// of a pickup. The rows of item entities are not living, so the row store keeps no tile, move or
/// crossing of them: the tile before the tick is the one of [`Dropped`], the tile after it the
/// one of the position in [`Stepped`].
#[expect(
    clippy::too_many_arguments,
    reason = "one system reads each source of the events of the tick"
)]
pub(crate) fn publish_items<M: Send + Sync + 'static>(
    rows: Res<RowStore<M>>,
    stepped: Res<Stepped>,
    items: Query<(Entity, Ref<ItemRow>, &Dropped, Ref<Count>)>,
    collectors: Query<&ItemCollector>,
    mut stacks: Stacks,
    table: Res<ItemTable>,
    mut picked: MessageReader<PickedUp>,
    mut released: ResMut<Released>,
    mut events: ResMut<TileEvents>,
) {
    let takes: FxHashMap<EntityId, EntityEvent<ReplicationData>> = picked
        .read()
        .filter_map(|pickup| {
            let collector = collectors.get(pickup.collector).ok()?;
            let take = MobReport::Signal(MobSignal::Taken {
                collector: collector.id,
                amount: i32::from(pickup.count),
            });
            Some((pickup.item, EntityEvent::Data(take.into())))
        })
        .collect();
    let positions: FxHashMap<Entity, DVec3> = stepped
        .0
        .iter()
        .map(|item| (item.stack, item.position))
        .collect();
    for (stack, row, dropped, count) in &items {
        let Some(&position) = positions.get(&stack) else {
            continue;
        };
        let tile = TileKey::at(position);
        let Some(state) = rows.state_of(Placed { tile, id: row.id }) else {
            continue;
        };
        if row.is_added() {
            if let Some(wire) = stacks
                .to_value(stack)
                .and_then(|value| wire_stack(&value, &table))
            {
                events.push(
                    tile,
                    row.id,
                    EntityEvent::Spawn(
                        MobReport::Spawn(MobSpawn {
                            kind: &EntityTypeData::ITEM,
                            uuid: Uuid::from_u128(row.key.0),
                            state: item_state(state),
                            look: SpawnLook::Item(wire),
                        })
                        .into(),
                    ),
                );
            }
            continue;
        }
        let before = TileKey::at(dropped.pos);
        if before != tile {
            events.push(before, row.id, EntityEvent::Leave { to: tile });
            events.push(tile, row.id, EntityEvent::Enter { from: before });
        }
        // ponytail: an item at rest on the ground sends no move; a velocity change at rest is not
        // seen until it moves.
        if position != dropped.pos || !state.body.flags.on_ground {
            events.push(
                tile,
                row.id,
                EntityEvent::Move(MobReport::Move(item_state(state)).into()),
            );
        }
        if count.is_changed()
            && let Some(wire) = stacks
                .to_value(stack)
                .and_then(|value| wire_stack(&value, &table))
        {
            // The take of a partial pickup goes first: the client subtracts it from the stack
            // that it shows (IEN-PCK-005).
            if let Some(take) = takes.get(&row.id) {
                events.push(tile, row.id, take.clone());
            }
            events.push(
                tile,
                row.id,
                EntityEvent::Data(MobReport::Stack(wire).into()),
            );
        }
    }
    for at in core::mem::take(&mut released.0) {
        if let Some(take) = takes.get(&at.id) {
            events.push(at.tile, at.id, take.clone());
        }
        events.push(at.tile, at.id, EntityEvent::Despawn);
    }
}

fn item_state(state: &EntityState) -> MobState {
    MobState {
        position: state.body.position,
        velocity: state.body.velocity,
        on_ground: state.body.flags.on_ground,
        ..MobState::default()
    }
}

// ponytail: the two stack models meet through the bytes of the wire; a direct conversion when
// the protocol holds `ItemStack`.
fn wire_stack(stack: &ItemStack, table: &ItemTable) -> Option<ProtoStack<Wire>> {
    let mut bytes = Vec::new();
    if let Err(error) = write_wire(Some(stack), table, &mut bytes) {
        tracing::warn!("the stack of an item entity has no wire form: {error:?}");
        return None;
    }
    ProtoStack::decode(&mut bytes.as_slice()).ok()
}
