use std::marker::PhantomData;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::schedule::common_conditions::on_message;
use mcrs_minecraft_engine_physics::{MobPublish, Mobs, RowMoves};
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_entity_rows::RowStore;

use crate::apply_row_attribute_edits::apply_row_attribute_edits;
use crate::attribute_edits::AttributeEdits;
use crate::forget_removed_rows::{any_row_attributes, forget_removed_rows};
use crate::project_row_motion::project_row_motion;
use crate::row_attribute_changed::RowAttributeChanged;
use crate::row_attribute_edit::RowAttributeEdit;
use crate::row_attributes::RowAttributes;

#[derive(Debug)]
pub struct RowAttributesPlugin<M> {
    column: PhantomData<fn() -> M>,
}

impl<M> Clone for RowAttributesPlugin<M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M> Copy for RowAttributesPlugin<M> {}

impl<M> Default for RowAttributesPlugin<M> {
    fn default() -> Self {
        Self {
            column: PhantomData,
        }
    }
}

impl<M: Send + Sync + 'static> Plugin for RowAttributesPlugin<M> {
    fn build(&self, app: &mut App) {
        app.init_resource::<RowAttributes>();
        app.init_resource::<RowStore<M>>();
        app.init_resource::<Mobs>();
        app.add_message::<RowAttributeEdit>();
        app.add_message::<RowAttributeChanged>();
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
                apply_row_attribute_edits.run_if(on_message::<RowAttributeEdit>),
                project_row_motion::<M>.run_if(on_message::<RowAttributeChanged>),
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
