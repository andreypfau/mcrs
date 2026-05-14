//! Post-convergence emit-dirty systems.
//!
//! Three sequential systems run after the convergence sub-schedule terminates:
//!
//! 1. `downgrade_light_storage` inspects every `LightDirty` section's
//!    `LightStorage::Mixed(arr)` field and downgrades to `Null` (all-zero
//!    nibbles) or `Uniform(15)` (all-0xFF bytes — every cell holds 15) when
//!    the homogeneity check passes. Only just-touched sections need the
//!    check; running on all sections every tick is wasted work.
//!
//! 2. `clear_light_dirty_safety_net` removes `LightDirty` from sections
//!    whose `*Egress`, `*Incoming`, and workspaces are all empty. Each clear
//!    indicates a missed per-iteration clear inside the propagate systems —
//!    emit `tracing::debug!` so the discrepancy is observable.
//!
//! 3. `clear_light_tickets` removes `LightTicket` from sections that no
//!    longer have any pending work AND are not `LightDirty`. The ticket
//!    represents "my neighbours must stay loaded until I drain my queues";
//!    once everything is empty, the ticket can be dropped and the chunk
//!    unload path becomes free to evict the section if it goes stale.

use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Commands, Entity, Query, With, Without};
use mcrs_engine::world::column::{
    ChunkColumnPosComponent, InChunkColumn, SectionIndex, SectionLookup,
};
use mcrs_engine::world::lighting::LightTicket;

use crate::codec::{BlockLightDirty, SkyLightDirty};
use crate::components::{
    BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace, LightDirty, SkyEgress,
    SkyIncoming, SkyLight, SkyLightWorkspace,
};
use crate::storage::LightStorage;

/// Downgrades `LightStorage::Mixed` to `Null` (all-zero) or `Uniform(15)`
/// (all-fifteen) on every section flagged `LightDirty`. The check inspects
/// the raw 2048-byte nibble array for the two homogeneous patterns and
/// leaves Mixed as-is otherwise.
pub fn downgrade_light_storage(
    mut sections: Query<(&mut BlockLight, Option<&mut SkyLight>), With<LightDirty>>,
) {
    for (mut block_light, mut sky_light_opt) in sections.iter_mut() {
        downgrade_storage_in_place(&mut block_light.0);
        if let Some(mut sky_light) = sky_light_opt.as_deref_mut() {
            downgrade_storage_in_place(&mut sky_light.0);
        }
    }
}

#[inline]
fn downgrade_storage_in_place(storage: &mut LightStorage) {
    if let LightStorage::Mixed(arr) = storage {
        let bytes = &arr.0;
        if bytes.iter().all(|&b| b == 0) {
            *storage = LightStorage::Null;
            return;
        }
        if bytes.iter().all(|&b| b == 0xFF) {
            *storage = LightStorage::Uniform(15);
        }
    }
}

/// Removes `LightDirty` from sections whose egress, incoming, and workspace
/// queues are all empty. Emits `tracing::debug!` each time it clears anything
/// — every clear indicates a leftover `LightDirty` that the per-iteration
/// clear inside the propagate systems missed.
pub fn clear_light_dirty_safety_net(
    sections: Query<
        (
            Entity,
            &BlockEgress,
            &BlockIncoming,
            &SkyEgress,
            &SkyIncoming,
            &BlockLightWorkspace,
            &SkyLightWorkspace,
        ),
        With<LightDirty>,
    >,
    mut commands: Commands,
) {
    for (entity, be, bi, se, si, bws, sws) in sections.iter() {
        if be.0.is_empty()
            && bi.0.is_empty()
            && se.0.is_empty()
            && si.0.is_empty()
            && bws.increase_queue.is_empty()
            && bws.decrease_queue.is_empty()
            && sws.increase_queue.is_empty()
            && sws.decrease_queue.is_empty()
        {
            commands.entity(entity).remove::<LightDirty>();
            tracing::debug!(?entity, "LightDirty safety-net cleared");
        }
    }
}

/// Removes `LightTicket` from sections that no longer have any pending work
/// (egress / incoming / workspace queues all empty) and are not `LightDirty`.
/// Once the ticket is gone, the chunk-unload path is free to evict the
/// section if no observer view keeps it loaded.
pub fn clear_light_tickets(
    sections: Query<
        (
            Entity,
            &BlockEgress,
            &BlockIncoming,
            &SkyEgress,
            &SkyIncoming,
            &BlockLightWorkspace,
            &SkyLightWorkspace,
        ),
        (With<LightTicket>, Without<LightDirty>),
    >,
    mut commands: Commands,
) {
    for (entity, be, bi, se, si, bws, sws) in sections.iter() {
        if be.0.is_empty()
            && bi.0.is_empty()
            && se.0.is_empty()
            && si.0.is_empty()
            && bws.increase_queue.is_empty()
            && bws.decrease_queue.is_empty()
            && sws.increase_queue.is_empty()
            && sws.decrease_queue.is_empty()
        {
            commands.entity(entity).remove::<LightTicket>();
        }
    }
}

#[inline]
fn chunk_y_for_section(index: &SectionIndex, target: Entity) -> Option<i32> {
    let min_y = index.min_section_y;
    index.iter_wire().enumerate().find_map(|(idx, lookup)| {
        if let SectionLookup::Loaded(e) = lookup {
            if e == target {
                return Some(min_y + idx as i32 - 1);
            }
        }
        None
    })
}

// Producer half of the lighting codec wire. The per-iteration BFS inserts
// `LightDirty` whenever a section's block- or sky-light storage is touched;
// v1 dirty signaling is per-section, not per-layer, so both producer systems
// may fan out a message for a single-layer change. The downstream codec
// dedups by section and consults the actual `LightStorage` before setting any
// wire-mask bit, so an over-fanned-out message is a negligible NULL pass at
// the consumer. Per-layer-precise dirty markers (e.g. sparse
// `BlockLightTouchedThisTick` / `SkyLightTouchedThisTick`) are a follow-up if
// profiling shows the dedup work is hot.
pub fn emit_block_light_dirty(
    sections: Query<(Entity, &InChunkColumn), (With<LightDirty>, With<BlockLight>)>,
    columns: Query<(&ChunkColumnPosComponent, &SectionIndex)>,
    mut writer: MessageWriter<BlockLightDirty>,
) {
    for (section, in_column) in sections.iter() {
        let Ok((column_pos, section_index)) = columns.get(in_column.0) else {
            continue;
        };
        let Some(chunk_y) = chunk_y_for_section(section_index, section) else {
            continue;
        };
        writer.write(BlockLightDirty {
            section,
            column_pos: column_pos.0,
            chunk_y,
        });
    }
}

pub fn emit_sky_light_dirty(
    sections: Query<(Entity, &InChunkColumn), (With<LightDirty>, With<SkyLight>)>,
    columns: Query<(&ChunkColumnPosComponent, &SectionIndex)>,
    mut writer: MessageWriter<SkyLightDirty>,
) {
    for (section, in_column) in sections.iter() {
        let Ok((column_pos, section_index)) = columns.get(in_column.0) else {
            continue;
        };
        let Some(chunk_y) = chunk_y_for_section(section_index, section) else {
            continue;
        };
        writer.write(SkyLightDirty {
            section,
            column_pos: column_pos.0,
            chunk_y,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{
        BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace, LightDirty, SkyEgress,
        SkyIncoming, SkyLight, SkyLightWorkspace, Wavefront,
    };
    use crate::nibble::NibbleArray;
    use bevy_app::{App, Update};
    use mcrs_engine::world::lighting::LightTicket;

    fn build_downgrade_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, downgrade_light_storage);
        app
    }

    fn build_safety_net_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, clear_light_dirty_safety_net);
        app
    }

    fn build_clear_tickets_app() -> App {
        let mut app = App::new();
        app.add_systems(Update, clear_light_tickets);
        app
    }

    #[test]
    fn downgrade_light_storage_converts_all_zero_mixed_to_null() {
        let mut app = build_downgrade_app();
        let arr = NibbleArray::zeros();
        let entity = app
            .world_mut()
            .spawn((
                BlockLight(LightStorage::Mixed(Box::new(arr))),
                LightDirty,
            ))
            .id();
        app.update();
        let bl = app
            .world()
            .get::<BlockLight>(entity)
            .expect("block light");
        assert!(
            matches!(bl.0, LightStorage::Null),
            "all-zero Mixed downgrades to Null"
        );
    }

    #[test]
    fn downgrade_light_storage_converts_all_fifteen_mixed_to_uniform_15() {
        let mut app = build_downgrade_app();
        let arr = NibbleArray::filled(15);
        let entity = app
            .world_mut()
            .spawn((
                BlockLight(LightStorage::Mixed(Box::new(arr))),
                LightDirty,
            ))
            .id();
        app.update();
        let bl = app
            .world()
            .get::<BlockLight>(entity)
            .expect("block light");
        assert!(
            matches!(bl.0, LightStorage::Uniform(15)),
            "all-fifteen Mixed downgrades to Uniform(15)"
        );
    }

    #[test]
    fn downgrade_light_storage_leaves_heterogeneous_mixed_unchanged() {
        let mut app = build_downgrade_app();
        let mut arr = NibbleArray::filled(15);
        arr.set(3, 4, 5, 7);
        let entity = app
            .world_mut()
            .spawn((
                BlockLight(LightStorage::Mixed(Box::new(arr))),
                LightDirty,
            ))
            .id();
        app.update();
        let bl = app
            .world()
            .get::<BlockLight>(entity)
            .expect("block light");
        assert!(
            matches!(bl.0, LightStorage::Mixed(_)),
            "heterogeneous Mixed must stay Mixed"
        );
        if let LightStorage::Mixed(a) = &bl.0 {
            assert_eq!(a.get(3, 4, 5), 7);
        }
    }

    fn spawn_clean_section(app: &mut App, dirty: bool, ticket: bool) -> bevy_ecs::entity::Entity {
        let mut e = app.world_mut().spawn((
            BlockEgress::default(),
            BlockIncoming::default(),
            SkyEgress::default(),
            SkyIncoming::default(),
            BlockLightWorkspace::default(),
            SkyLightWorkspace::default(),
        ));
        if dirty {
            e.insert(LightDirty);
        }
        if ticket {
            e.insert(LightTicket);
        }
        e.id()
    }

    #[test]
    fn clear_light_dirty_safety_net_clears_when_all_queues_and_buffers_empty() {
        let mut app = build_safety_net_app();
        let entity = spawn_clean_section(&mut app, true, false);
        app.update();
        assert!(
            app.world().get::<LightDirty>(entity).is_none(),
            "LightDirty cleared when queues/buffers all empty"
        );
    }

    #[test]
    fn clear_light_dirty_safety_net_keeps_when_egress_nonempty() {
        let mut app = build_safety_net_app();
        let entity = spawn_clean_section(&mut app, true, false);
        let mut e = BlockEgress::default();
        e.0.push(Wavefront::new(0, 1, 2, 3));
        app.world_mut().entity_mut(entity).insert(e);
        app.update();
        assert!(
            app.world().get::<LightDirty>(entity).is_some(),
            "LightDirty retained when BlockEgress is non-empty"
        );
    }

    #[test]
    fn clear_light_dirty_safety_net_keeps_when_workspace_queue_nonempty() {
        let mut app = build_safety_net_app();
        let entity = spawn_clean_section(&mut app, true, false);
        let mut ws = BlockLightWorkspace::default();
        ws.increase_queue.push(0u64);
        app.world_mut().entity_mut(entity).insert(ws);
        app.update();
        assert!(
            app.world().get::<LightDirty>(entity).is_some(),
            "LightDirty retained when workspace queue is non-empty"
        );
    }

    #[test]
    fn clear_light_tickets_skips_sections_with_pending_work() {
        let mut app = build_clear_tickets_app();
        let entity = spawn_clean_section(&mut app, false, true);
        let mut i = BlockIncoming::default();
        i.0.push(Wavefront::new(0, 1, 2, 3));
        app.world_mut().entity_mut(entity).insert(i);
        app.update();
        assert!(
            app.world().get::<LightTicket>(entity).is_some(),
            "LightTicket retained when BlockIncoming is non-empty"
        );
    }

    #[test]
    fn clear_light_tickets_removes_when_all_pending_work_drained() {
        let mut app = build_clear_tickets_app();
        let entity = spawn_clean_section(&mut app, false, true);
        app.update();
        assert!(
            app.world().get::<LightTicket>(entity).is_none(),
            "LightTicket cleared when all queues/buffers empty and not dirty"
        );
    }

    #[test]
    fn clear_light_tickets_skips_dirty_sections() {
        let mut app = build_clear_tickets_app();
        let entity = spawn_clean_section(&mut app, true, true);
        app.update();
        assert!(
            app.world().get::<LightTicket>(entity).is_some(),
            "LightTicket retained on LightDirty sections (Without<LightDirty> filter)"
        );
    }
}
