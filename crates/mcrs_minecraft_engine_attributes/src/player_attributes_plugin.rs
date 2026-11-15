use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::tick::TickSet;

use crate::apply_player_attribute_edits::apply_player_attribute_edits;
use crate::attribute_changed::AttributeChanged;
use crate::attribute_edit::AttributeEdit;
use crate::attribute_edits::AttributeEdits;
use crate::attributes::Attributes;
use crate::player_attribute_edits::PlayerAttributeEdits;

/// The attribute instances of the players of a dimension world: [`Attributes`] on each
/// [`Player`]. The edits of a player apply in [`AttributeEdits`].
#[derive(Clone, Copy, Debug, Default)]
pub struct PlayerAttributesPlugin;

impl Plugin for PlayerAttributesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AttributeEdit>();
        app.add_message::<AttributeChanged>();
        if let Err(error) = app.try_register_required_components::<Player, Attributes>() {
            tracing::error!("a player does not require its attribute instances: {error}");
        }
        app.configure_sets(FixedUpdate, AttributeEdits.in_set(TickSet::Project));
        app.configure_sets(FixedUpdate, PlayerAttributeEdits.in_set(AttributeEdits));
        app.add_systems(
            FixedUpdate,
            apply_player_attribute_edits
                .run_if(on_message::<AttributeEdit>)
                .in_set(PlayerAttributeEdits),
        );
    }
}
