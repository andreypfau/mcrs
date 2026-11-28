use bevy_ecs::resource::Resource;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use glam::DVec3;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::WriterKey;
use mcrs_minecraft_physics::Rest;

use crate::mob_index::MobIndex;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntityWrite {
    pub target: EntityUuid,
    pub writer: WriterKey,
    pub sequence: u32,
    pub effect: DVec3,
}

#[derive(Resource, Debug, Default)]
pub struct EntityWrites(Vec<EntityWrite>);

impl EntityWrites {
    pub fn push(&mut self, write: EntityWrite) {
        self.0.push(write);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub(crate) fn has_writes(writes: Res<EntityWrites>) -> bool {
    !writes.is_empty()
}

pub(crate) fn apply_entity_writes(
    mut writes: ResMut<EntityWrites>,
    index: Res<MobIndex>,
    mut states: Query<&mut MotionState>,
) {
    let writes = &mut writes.0;
    writes.sort_unstable_by_key(|write| (write.target, write.writer, write.sequence));
    for write in writes.iter() {
        let Some(mut state) =
            (index.get(write.target)).and_then(|entity| states.get_mut(entity).ok())
        else {
            continue;
        };
        let state = &mut state.0;
        state.body.velocity += write.effect;
        state.rest = Rest::Awake;
    }
    writes.clear();
}
