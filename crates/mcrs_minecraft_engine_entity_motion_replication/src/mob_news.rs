use bevy_ecs::resource::Resource;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_engine_physics::MobChanges;
use mcrs_minecraft_engine_physics::Spawned;
use mcrs_minecraft_entity_key::TileKey;
use mcrs_minecraft_replication::MobSignal;

use crate::MotionId;
use crate::PublishedTile;
use crate::subject::Subject;

#[derive(Resource, Debug, Default)]
pub(crate) struct MobNews {
    pub(crate) spawned: Vec<Spawned>,
    pub(crate) strikes: Vec<Strike>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Strike {
    pub(crate) subject: Subject,
    pub(crate) order: usize,
    pub(crate) tile: TileKey,
    pub(crate) signal: MobSignal,
}

impl MobNews {
    pub(crate) fn strikes_of(&self, subject: Subject) -> &[Strike] {
        let start = self
            .strikes
            .partition_point(|strike| strike.subject < subject);
        let end = self
            .strikes
            .partition_point(|strike| strike.subject <= subject);
        self.strikes.get(start..end).unwrap_or_default()
    }
}

pub(crate) fn has_mob_changes(changes: Res<MobChanges>) -> bool {
    !changes.spawned().is_empty() || !changes.strikes().is_empty()
}

pub(crate) fn take_mob_news(
    changes: Res<MobChanges>,
    entities: Query<(&MotionId, &PublishedTile)>,
    mut news: ResMut<MobNews>,
) {
    let news = &mut *news;
    news.spawned.extend_from_slice(changes.spawned());
    for struck in changes.strikes() {
        let Ok((id, published)) = entities.get(struck.entity) else {
            continue;
        };
        let Some(tile) = published.tile() else {
            continue;
        };
        let subject = Subject {
            id: id.0,
            entity: struck.entity,
        };
        let mut signal = |signal: MobSignal| {
            let order = news.strikes.len();
            news.strikes.push(Strike {
                subject,
                order,
                tile,
                signal,
            });
        };
        if struck.outcome == HitOutcome::Full {
            signal(MobSignal::Hurt {
                damage_type: struck.damage_type,
            });
        }
        if struck.died {
            signal(MobSignal::Died);
        }
    }
    news.strikes
        .sort_unstable_by_key(|strike| (strike.subject, strike.order));
}
