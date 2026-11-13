use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_replication::MobState;

/// The state of the row `state` that its viewers see.
#[must_use]
pub(crate) fn mob_state(state: &EntityState) -> MobState {
    let still;
    let living = match &state.living {
        Some(living) => living,
        None => {
            still = Living::default();
            &still
        }
    };
    MobState {
        position: state.body.position,
        velocity: state.body.velocity,
        yaw: living.orientation.yaw,
        pitch: living.orientation.pitch,
        head_yaw: living.orientation.head_yaw,
        on_ground: state.body.flags.on_ground,
        health: living.vitals.health,
        dying: !living.is_alive(),
        on_fire: living.on_fire(),
    }
}
