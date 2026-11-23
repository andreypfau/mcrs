use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::message::MessageReader;
use bevy_ecs::system::Query;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_physics::Rest;

use crate::attribute_changed::AttributeChanged;
use crate::mob_attributes::MobAttributes;
use crate::motion_field::motion_field;

pub(crate) fn project_motion(
    mut changed: MessageReader<AttributeChanged>,
    mut mobs: Query<(&Mob, &MobAttributes, &mut MotionState)>,
) {
    for change in changed.read() {
        let Some(field) = motion_field(change.attribute) else {
            continue;
        };
        let Ok((mob, attributes, mut state)) = mobs.get_mut(change.target) else {
            continue;
        };
        let Some(value) = attributes.value(mob.kind.entity_type, change.attribute) else {
            continue;
        };
        let motion = &mut state.bypass_change_detection().0;
        let Some(living) = motion.living.as_mut() else {
            continue;
        };
        let slot = field(&mut living.attributes);
        if *slot != value {
            *slot = value;
            motion.rest = Rest::Awake;
            state.set_changed();
        }
    }
}
