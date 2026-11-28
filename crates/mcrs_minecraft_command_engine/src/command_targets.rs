use crate::dimension_entry::DimensionEntry;
use crate::player_entry::PlayerEntry;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_resource::ResourceLocation;
use rustc_hash::FxHashMap;

#[derive(Resource, Debug, Default)]
pub struct CommandTargets {
    dimensions: Vec<DimensionEntry>,
    players: Vec<PlayerEntry>,
    by_session: FxHashMap<PlayerSession, usize>,
}

impl CommandTargets {
    pub fn dimensions(&self) -> &[DimensionEntry] {
        &self.dimensions
    }

    pub fn players(&self) -> &[PlayerEntry] {
        &self.players
    }

    pub fn dimension_of(&self, label: Entity) -> Option<usize> {
        dimension_position(&self.dimensions, label)
    }

    pub fn label_of(&self, index: usize) -> Option<Entity> {
        self.dimensions.get(index).map(|entry| entry.label)
    }

    pub fn dimension_named(&self, id: ResourceLocation<'_>) -> Option<&DimensionEntry> {
        self.dimensions
            .iter()
            .find(|entry| entry.key.namespace() == id.namespace() && entry.key.path() == id.path())
    }

    pub fn player(&self, session: PlayerSession) -> Option<&PlayerEntry> {
        self.players.get(*self.by_session.get(&session)?)
    }

    pub(crate) fn player_mut(&mut self, session: PlayerSession) -> Option<&mut PlayerEntry> {
        self.players.get_mut(*self.by_session.get(&session)?)
    }

    pub(crate) fn rebuild(
        &mut self,
        dimensions: impl IntoIterator<Item = DimensionEntry>,
        players: impl IntoIterator<Item = PlayerEntry>,
    ) {
        self.dimensions.clear();
        self.dimensions.extend(dimensions);
        self.players.clear();
        self.players.extend(players);
        self.players.sort_unstable_by_key(|player| player.session.0);
        self.by_session.clear();
        self.by_session.extend(
            self.players
                .iter()
                .enumerate()
                .map(|(index, player)| (player.session, index)),
        );
    }
}

pub(crate) fn dimension_position(entries: &[DimensionEntry], label: Entity) -> Option<usize> {
    entries.iter().position(|entry| entry.label == label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_engine_world::entity::physics::Transform;
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_pose::Dimensions;
    use mcrs_minecraft_protocol::uuid::Uuid;
    use mcrs_minecraft_resource::ResourceKeyBuf;
    use mcrs_minecraft_resource::ResourceLocationBuf;

    fn dimension(label: u32, path: &str) -> DimensionEntry {
        DimensionEntry {
            label: Entity::from_raw_u32(label).unwrap(),
            key: ResourceKeyBuf::from_location(ResourceLocationBuf::minecraft(path).unwrap()),
            coordinate_scale: 1.0,
        }
    }

    fn player(session: u64, dimension: usize, entity: Option<u32>) -> PlayerEntry {
        PlayerEntry {
            session: PlayerSession(session),
            dimension,
            entity: entity.map(|raw| Entity::from_raw_u32(raw).unwrap()),
            uuid: Uuid::from_u128(u128::from(session)),
            name: format!("player{session}"),
            transform: Transform::IDENTITY,
            mode: GameMode::Survival,
            dimensions: Dimensions::scalable(0.6, 1.8),
        }
    }

    #[test]
    fn rebuild_orders_players_by_session_and_keeps_each_entry() {
        let mut targets = CommandTargets::default();
        targets.rebuild(
            [dimension(10, "overworld"), dimension(11, "the_nether")],
            [
                player(3, 1, Some(30)),
                player(1, 0, None),
                player(2, 1, Some(20)),
            ],
        );
        let sessions: Vec<u64> = targets.players().iter().map(|p| p.session.0).collect();
        assert_eq!(sessions, [1, 2, 3]);
        let expected = [(1, 0, None), (2, 1, Some(20)), (3, 1, Some(30))];
        for (session, dimension, entity) in expected {
            let entry = targets.player(PlayerSession(session)).unwrap();
            assert_eq!(entry.dimension, dimension);
            assert_eq!(entry.entity.map(Entity::index_u32), entity);
        }
    }
}
