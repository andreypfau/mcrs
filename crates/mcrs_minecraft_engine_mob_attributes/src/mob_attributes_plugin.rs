use std::marker::PhantomData;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine_physics::{MobPublish, Mobs, RowMoves};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_entity_rows::RowStore;

use crate::apply_row_attribute_edits::apply_row_attribute_edits;
use crate::attribute_changed::AttributeChanged;
use crate::attribute_edit::AttributeEdit;
use crate::attribute_edits::AttributeEdits;
use crate::forget_removed_rows::{any_row_attributes, forget_removed_rows};
use crate::project_row_motion::project_row_motion;
use crate::row_attributes::RowAttributes;

#[derive(Debug)]
pub struct MobAttributesPlugin<M> {
    column: PhantomData<fn() -> M>,
}

impl<M> Clone for MobAttributesPlugin<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M> Copy for MobAttributesPlugin<M> {}

impl<M> Default for MobAttributesPlugin<M> {
    fn default() -> Self {
        Self {
            column: PhantomData,
        }
    }
}

impl<M: Send + Sync + 'static> Plugin for MobAttributesPlugin<M> {
    fn build(&self, app: &mut App) {
        app.init_resource::<RowAttributes>();
        app.init_resource::<RowStore<M>>();
        app.init_resource::<Mobs>();
        app.add_message::<AttributeEdit>();
        app.add_message::<AttributeChanged>();
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
                apply_row_attribute_edits.run_if(on_message::<AttributeEdit>),
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
