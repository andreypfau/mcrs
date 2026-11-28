use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{On, ResMut};

use crate::coordinator::ops::{EditOps, OpList};

pub struct OpListHostPlugin;

impl Plugin for OpListHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OpList>();
        app.add_observer(apply_ops_edit);
    }
}

fn apply_ops_edit(edit: On<EditOps>, mut ops: ResMut<OpList>) {
    if ops.apply(edit.event()) {
        ops.write_file();
    }
}
