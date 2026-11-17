use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_replication::MobState;

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
        horizontal_collision: state.body.flags.horizontal_collision,
        vertical_collision: state.body.flags.vertical_collision,
        bounced: false,
        health: living.health,
        dying: !living.is_alive(),
    }
}
