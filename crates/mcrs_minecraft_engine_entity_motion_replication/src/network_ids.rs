use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Discard;
use bevy_ecs::lifecycle::Insert;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_engine::network_id::player_network_id;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_replication::wire_id;
use rustc_hash::FxHashMap;

use crate::MotionId;

#[derive(Resource, Debug, Default)]
pub struct NetworkIds(FxHashMap<i32, Entity>);

impl NetworkIds {
    #[must_use]
    pub fn get(&self, id: i32) -> Option<Entity> {
        self.0.get(&id).copied()
    }
}

type Named<'w, 's> = Query<'w, 's, (Option<&'static Owner>, Option<&'static MotionId>)>;

fn network_id(named: &Named, entity: Entity) -> Option<i32> {
    match named.get(entity) {
        Ok((Some(owner), _)) => Some(player_network_id(owner.0)),
        Ok((None, Some(motion))) => Some(wire_id(motion.0).0),
        Ok((None, None)) | Err(_) => None,
    }
}

pub(crate) fn remember_network_id(
    insert: On<Insert<(Owner, MotionId)>>,
    named: Named,
    mut ids: ResMut<NetworkIds>,
) {
    let entity = insert.event().entity;
    if let Some(id) = network_id(&named, entity) {
        ids.0.insert(id, entity);
    }
}

pub(crate) fn forget_network_id(
    discard: On<Discard<(Owner, MotionId)>>,
    named: Named,
    mut ids: ResMut<NetworkIds>,
) {
    let entity = discard.event().entity;
    if let Some(id) = network_id(&named, entity)
        && ids.get(id) == Some(entity)
    {
        ids.0.remove(&id);
    }
}
