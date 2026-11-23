use alloc::vec::Vec;
use bevy_ecs::message::{MessageCursor, MessageReader, Messages};
use bevy_ecs::system::{Local, Query, Res, ResMut};
use mcrs_minecraft_engine::HeldJoins;
use mcrs_minecraft_engine::bus::{InboundPlayerDespawn, InboundPlayerSpawn};
use mcrs_minecraft_engine::column_store::ColumnStoreLink;
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;

use crate::spawn_site::{SpawnColumns, SpawnSite};

pub(crate) fn holds_joins(held: Res<HeldJoins>) -> bool {
    !held.0.is_empty()
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub(crate) fn hold_unplaced_joins(
    mut messages: ResMut<Messages<InboundPlayerSpawn>>,
    mut cursor: Local<MessageCursor<InboundPlayerSpawn>>,
    mut fresh: Local<Vec<InboundPlayerSpawn>>,
    mut tickets: Local<Option<SpawnColumns>>,
    mut despawns: MessageReader<InboundPlayerDespawn>,
    mut held: ResMut<HeldJoins>,
    mut loading: Query<&mut ColumnLoading>,
    store: Option<Res<ColumnStoreLink>>,
    site: SpawnSite,
) {
    let holds = store.is_some_and(|store| store.generates()) && site.join_reads_columns();
    let unread = cursor.len(&messages);
    if holds && cursor.read(&messages).any(is_unplaced) {
        let seen = messages.len().saturating_sub(unread);
        fresh.extend(messages.drain().skip(seen));
        for spawn in fresh.drain(..) {
            if is_unplaced(&spawn) {
                held.0.push(spawn);
            } else {
                messages.write(spawn);
            }
        }
    }
    for despawn in despawns.read() {
        held.0.retain(|spawn| spawn.session != despawn.session);
    }
    if !held.0.is_empty()
        && let Some(translation) = site.join_position()
    {
        for mut spawn in held.0.drain(..) {
            let [yaw, pitch] = spawn.snapshot.rotation().ok().flatten().unwrap_or_default();
            spawn.snapshot.insert_transform(Transform {
                translation,
                rotation: Rotation::new(yaw, pitch),
            });
            messages.write(spawn);
        }
    }
    let wanted = (!held.0.is_empty()).then(|| site.join_columns());
    if *tickets != wanted
        && let Ok(mut loading) = loading.single_mut()
    {
        if let Some(old) = *tickets {
            for column in old.columns() {
                loading.remove_ticket(column);
            }
        }
        if let Some(new) = wanted {
            for column in new.columns() {
                loading.add_ticket(column);
            }
        }
        *tickets = wanted;
    }
    cursor.clear(&messages);
}

fn is_unplaced(spawn: &InboundPlayerSpawn) -> bool {
    matches!(spawn.snapshot.pos(), Ok(None))
}
