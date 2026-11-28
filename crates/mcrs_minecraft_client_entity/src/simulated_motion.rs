use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::{MotionKind, MotionState};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_visual::ClientEntity;
use mcrs_minecraft_physics::{Body, EntityState, MotionRow};

pub(crate) fn simulated_motion(
    client: &ClientEntity,
    position: DVec3,
    velocity: DVec3,
) -> Option<(MotionState, MotionKind)> {
    let row = motion_row(client.kind)?;
    let key = EntityUuid(client.uuid);
    let body = Body {
        position,
        velocity,
        ..Body::default()
    };
    let mut state = EntityState::new(key, body, None);
    state.phase = key.phase_offset();
    Some((MotionState(state), MotionKind(row)))
}

fn motion_row(kind: EntityType) -> Option<MotionRow> {
    match kind {
        EntityType::Item => Some(MotionRow::item()),
        EntityType::FallingBlock => Some(MotionRow::falling_block()),
        EntityType::Tnt => Some(MotionRow::primed_tnt()),
        EntityType::ExperienceOrb => Some(MotionRow::experience_orb()),
        _ => None,
    }
}
