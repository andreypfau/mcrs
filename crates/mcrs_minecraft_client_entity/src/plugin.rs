use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;

use crate::client_entity::{ClientEntity, NetworkEntities};
use crate::living::Living;
use crate::motion::Motion;
use crate::receive;
use crate::server_ticking::ServerTicking;
use crate::tick::{ClientEntityTick, consume_step, run_world_ticks, tick_entities};
use mcrs_minecraft_client_world::block_breaking::{
    AttackCandidate, AttackCandidates, AttackCandidatesSet,
};
use mcrs_minecraft_client_world::player::Player;

/// Keeps the client entities that the server adds, and ticks them in `FixedUpdate` in
/// [`ClientEntityTick`]. Packets apply when they arrive, before the next tick.
pub struct ClientEntityPlugin;

impl Plugin for ClientEntityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetworkEntities>()
            .init_resource::<ServerTicking>()
            .init_resource::<AttackCandidates>()
            .add_systems(
                FixedUpdate,
                (tick_entities, consume_step)
                    .chain()
                    .in_set(ClientEntityTick)
                    .run_if(run_world_ticks),
            )
            .add_systems(
                FixedUpdate,
                collect_attack_candidates.in_set(AttackCandidatesSet),
            );
        receive::add_observers(app);
    }
}

fn collect_attack_candidates(
    entities: Query<(Entity, &ClientEntity, &Motion, Option<&Living>), Without<Player>>,
    mut candidates: ResMut<AttackCandidates>,
) {
    candidates.0.clear();
    for (entity, client, motion, living) in &entities {
        if living.is_some_and(|living| living.death_time > 0) {
            continue;
        }
        let Some(data) = client.type_data() else {
            continue;
        };
        candidates.0.push(AttackCandidate {
            entity,
            network_id: client.network_id,
            position: motion.current.position,
            width: f64::from(data.width),
            height: f64::from(data.height),
        });
    }
}
