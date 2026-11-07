use std::sync::Arc;

use bevy_ecs::entity::Entity;
use bevy_ecs::message::{MessageReader, MessageWriter};
use bevy_ecs::query::With;
use bevy_ecs::system::{Local, Query, Res};
use mcrs_minecraft_dimension::Dimension as DimensionKey;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_physics::{Birth, BirthKey, BirthSource};
use mcrs_minecraft_resource::ResourceKeyBuf;

use crate::bus::InboundPlayerSpawn;
use crate::coordinator::clock::GameTime;
use crate::coordinator::dim::DimSubAppHandle;
use crate::to_coordinator::JoinRequested;
use crate::to_muxer::JoinPlaced;

/// The order of the logins placed in one tick.
#[derive(Default)]
pub struct JoinOrder {
    tick: u64,
    next: u32,
}

impl JoinOrder {
    fn next(&mut self, tick: u64) -> u32 {
        if self.tick != tick {
            *self = Self { tick, next: 0 };
        }
        let order = self.next;
        self.next = self.next.wrapping_add(1);
        order
    }
}

/// Places each joining player in the running dimension that its save names, or in the first
/// dimension of the list when the save names none or a dimension that does not run. A request
/// that finds no running dimension gets no answer, and the muxer asks again.
///
/// A player keeps the birth of its save. Without one, its birth key is that of a login, in the
/// order of the placements of the tick: the coordinator gives it, so it does not depend on the
/// cells.
pub fn place_joining_players(
    mut requests: MessageReader<JoinRequested>,
    live_dims: Query<(Entity, &ResourceKeyBuf<DimensionKey>), With<DimSubAppHandle>>,
    dimension_list: Res<DimensionList>,
    time: Option<Res<GameTime>>,
    mut order: Local<JoinOrder>,
    mut placed: MessageWriter<JoinPlaced>,
) {
    let tick = time.map_or(0, |time| u64::try_from(time.0).unwrap_or_default());
    let live = |key: &ResourceKeyBuf<DimensionKey>| {
        live_dims
            .iter()
            .find(|(_, live)| *live == key)
            .map(|(label, _)| label)
    };
    for request in requests.read() {
        let Some(dim) = request
            .saved_dimension
            .as_ref()
            .and_then(live)
            .or_else(|| live(dimension_list.keys().first()?))
        else {
            continue;
        };
        let birth = request.saved_birth.unwrap_or_else(|| {
            let source = BirthSource::Player {
                order: order.next(tick),
            };
            Birth::from(BirthKey::born(tick, &source, 0))
        });
        placed.write(JoinPlaced {
            dim,
            spawn: InboundPlayerSpawn {
                session: request.session,
                snapshot: request.snapshot.clone(),
                dimensions: Arc::clone(dimension_list.keys()),
                birth,
            },
        });
    }
}
