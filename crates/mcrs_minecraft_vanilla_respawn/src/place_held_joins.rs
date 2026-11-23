use bevy_ecs::message::{MessageReader, MessageWriter};
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

pub(crate) fn place_held_joins(
    mut held: ResMut<HeldJoins>,
    mut spawns: MessageWriter<InboundPlayerSpawn>,
    mut despawns: MessageReader<InboundPlayerDespawn>,
    mut tickets: Local<Option<SpawnColumns>>,
    mut loading: Query<&mut ColumnLoading>,
    store: Option<Res<ColumnStoreLink>>,
    site: SpawnSite,
) {
    for despawn in despawns.read() {
        held.0.retain(|spawn| spawn.session != despawn.session);
    }
    if !(store.is_some_and(|store| store.generates()) && site.join_reads_columns()) {
        spawns.write_batch(held.0.drain(..));
    } else if let Some(translation) = site.join_position() {
        for mut spawn in held.0.drain(..) {
            let [yaw, pitch] = spawn.snapshot.rotation().ok().flatten().unwrap_or_default();
            spawn.snapshot.insert_transform(Transform {
                translation,
                rotation: Rotation::new(yaw, pitch),
            });
            spawns.write(spawn);
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
}
