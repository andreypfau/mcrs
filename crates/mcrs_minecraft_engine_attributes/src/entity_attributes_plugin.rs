use std::marker::PhantomData;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::schedule::common_conditions::on_message;
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};
use mcrs_minecraft_engine_physics::{MobPublish, Mobs, RowMoves};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_entity_rows::RowStore;

use crate::apply_attribute_edits::apply_attribute_edits;
use crate::attribute_changed::AttributeChanged;
use crate::attribute_edit::AttributeEdit;
use crate::attributes::Attributes;
use crate::forget_removed_rows::{any_row_attributes, forget_removed_rows};
use crate::project_row_motion::project_row_motion;
use crate::row_attributes::RowAttributes;

/// The attribute instances of a dimension world: [`Attributes`] on each [`Player`],
/// [`RowAttributes`] for the mob rows of the [`RowStore<M>`] with the mob column `M`. The edits
/// apply in [`AttributeEdits`], which also writes the changed motion values of a row into its
/// row; the entry of a row leaves in the tick of its removal.
#[derive(Debug)]
pub struct EntityAttributesPlugin<M> {
    column: PhantomData<fn() -> M>,
}

impl<M> Clone for EntityAttributesPlugin<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M> Copy for EntityAttributesPlugin<M> {}

impl<M> Default for EntityAttributesPlugin<M> {
    fn default() -> Self {
        Self {
            column: PhantomData,
        }
    }
}

/// The application of the [`AttributeEdit`]s of the tick, in `Project` after [`RowMoves`] and
/// before [`MobPublish`]. A system that writes edits runs before it; a system that reads
/// [`AttributeChanged`] runs after it.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttributeEdits;

impl<M: Send + Sync + 'static> Plugin for EntityAttributesPlugin<M> {
    fn build(&self, app: &mut App) {
        app.init_resource::<RowAttributes>();
        app.init_resource::<RowStore<M>>();
        app.init_resource::<Mobs>();
        app.add_message::<AttributeEdit>();
        app.add_message::<AttributeChanged>();
        if let Err(error) = app.try_register_required_components::<Player, Attributes>() {
            tracing::error!("a player does not require its attribute instances: {error}");
        }
        app.configure_sets(
            FixedUpdate,
            AttributeEdits
                .in_set(TickSet::Project)
                .after(RowMoves)
                .before(MobPublish),
        );
        app.add_systems(
            FixedUpdate,
            (
                apply_attribute_edits.run_if(on_message::<AttributeEdit>),
                project_row_motion::<M>.run_if(on_message::<AttributeChanged>),
            )
                .chain()
                .in_set(AttributeEdits),
        );
        app.add_systems(
            FixedUpdate,
            forget_removed_rows::<M>
                .run_if(any_row_attributes)
                .in_set(MobPublish),
        );
    }
}
