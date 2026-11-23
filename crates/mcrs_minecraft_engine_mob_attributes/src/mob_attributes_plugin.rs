use bevy_app::App;
use bevy_app::FixedUpdate;
use bevy_app::Plugin;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_damage::Defence;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_engine_physics::MobPublish;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::apply_attribute_edits::apply_attribute_edits;
use crate::attribute_changed::AttributeChanged;
use crate::attribute_edit::AttributeEdit;
use crate::attribute_edits::AttributeEdits;
use crate::mob_attributes::MobAttributes;
use crate::mob_defence::fill_defence;
use crate::mob_defence::project_defence;
use crate::project_motion::project_motion;

#[derive(Clone, Copy, Debug, Default)]
pub struct MobAttributesPlugin;

impl Plugin for MobAttributesPlugin {
    fn build(&self, app: &mut App) {
        if let Err(error) = app.try_register_required_components::<Mob, MobAttributes>() {
            tracing::warn!(%error, "a mob spawned before its attributes were required");
        }
        if let Err(error) = app.try_register_required_components::<Mob, Defence>() {
            tracing::warn!(%error, "a mob spawned before its defence was required");
        }
        app.add_observer(fill_defence);
        app.add_message::<AttributeEdit>();
        app.add_message::<AttributeChanged>();
        app.configure_sets(
            FixedUpdate,
            AttributeEdits.in_set(TickSet::Project).before(MobPublish),
        );
        app.add_systems(
            FixedUpdate,
            (
                apply_attribute_edits.run_if(on_message::<AttributeEdit>),
                (project_motion, project_defence).run_if(on_message::<AttributeChanged>),
            )
                .chain()
                .in_set(AttributeEdits),
        );
    }
}
