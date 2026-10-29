use bevy_ecs::entity::{Entity, EntityHashMap};
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_command::Identifier;
use mcrs_minecraft_command_entity::{Candidate, EntityBounds};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_dimension::keys::dimension::{OVERWORLD, THE_END, THE_NETHER};
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_entity::{PLAYER_HEIGHT, PLAYER_WIDTH};
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::uuid::Uuid;

/// A dimension that runs, as a command sees it.
#[derive(Clone, Debug)]
pub struct DimensionEntry {
    /// The host entity that labels the sub-app of the dimension.
    pub label: Entity,
    pub key: ResourceKey<Dimension>,
    pub coordinate_scale: f64,
}

/// A player of any dimension, as a command finds it.
#[derive(Clone, Debug)]
pub struct PlayerEntry {
    /// The host anchor: the identity of the player in every dimension.
    pub anchor: Entity,
    /// The position of its dimension in [`CommandTargets::dimensions`].
    pub dimension: usize,
    /// The entity of the player in the world of its dimension.
    pub entity: Entity,
    pub uuid: Uuid,
    pub name: String,
    pub transform: Transform,
    pub mode: GameMode,
    /// The session id: sessions count up as players join, so it orders the player list, and a
    /// change of dimension keeps it (DIM-CHG-001).
    pub(crate) list_order: u64,
}

/// The dimensions in the dimension order (ARG-ENT-019) and the online players of all of them in
/// the order of the player list. The host collects it at the start of a pass, and the
/// [`Worlds`](crate::command::Worlds) operations that change a player update its entry; a
/// handler reads it in the world it runs in.
#[derive(Resource, Debug, Default)]
pub struct CommandTargets {
    pub(crate) dimensions: Vec<DimensionEntry>,
    pub(crate) players: Vec<PlayerEntry>,
    pub(crate) by_anchor: EntityHashMap<usize>,
}

impl CommandTargets {
    pub fn dimensions(&self) -> &[DimensionEntry] {
        &self.dimensions
    }

    pub fn players(&self) -> &[PlayerEntry] {
        &self.players
    }

    pub fn dimension_of(&self, label: Entity) -> Option<usize> {
        self.dimensions
            .iter()
            .position(|entry| entry.label == label)
    }

    /// The label of the dimension at `index` in the dimension order.
    pub fn label_of(&self, index: usize) -> Option<Entity> {
        self.dimensions.get(index).map(|entry| entry.label)
    }

    pub fn dimension_named(&self, id: Identifier<'_>) -> Option<&DimensionEntry> {
        self.dimensions
            .iter()
            .find(|entry| entry.key.namespace() == id.namespace && entry.key.path() == id.path)
    }

    pub fn player(&self, anchor: Entity) -> Option<&PlayerEntry> {
        self.players.get(*self.by_anchor.get(&anchor)?)
    }

    pub(crate) fn player_mut(&mut self, anchor: Entity) -> Option<&mut PlayerEntry> {
        self.players.get_mut(*self.by_anchor.get(&anchor)?)
    }

    pub(crate) fn index_players(&mut self) {
        self.players
            .sort_unstable_by_key(|player| player.list_order);
        self.by_anchor.clear();
        self.by_anchor.extend(
            self.players
                .iter()
                .enumerate()
                .map(|(index, player)| (player.anchor, index)),
        );
    }
}

/// The rank of a dimension in the dimension order: the three vanilla dimensions first, then
/// the others by identifier (ARG-ENT-019).
fn dimension_rank(key: &ResourceKey<Dimension>) -> (u8, &str) {
    let rank = [OVERWORLD, THE_NETHER, THE_END]
        .iter()
        .position(|vanilla| vanilla.as_str() == key.as_str())
        .unwrap_or(3) as u8;
    (rank, key.as_str())
}

/// The worlds that `read` accepts, each with what it read, in the dimension order of `key`.
pub(crate) fn in_dimension_order<'w, T>(
    worlds: impl Iterator<Item = &'w mut World>,
    mut read: impl FnMut(&mut World) -> Option<T>,
    key: impl Fn(&T) -> &ResourceKey<Dimension>,
    found: &mut Vec<(T, &'w mut World)>,
) {
    found.clear();
    found.extend(worlds.filter_map(|world| Some((read(world)?, world))));
    found.sort_by(|(a, _), (b, _)| dimension_rank(key(a)).cmp(&dimension_rank(key(b))));
}

impl Candidate for PlayerEntry {
    fn uuid(&self) -> Uuid {
        self.uuid
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn entity_type(&self) -> EntityType {
        EntityType::Player
    }

    fn position(&self) -> DVec3 {
        self.transform.translation
    }

    fn bounding_box(&self) -> EntityBounds {
        let half = DVec3::new(PLAYER_WIDTH / 2.0, 0.0, PLAYER_WIDTH / 2.0);
        let position = self.transform.translation;
        EntityBounds {
            min: position - half,
            max: position + half + DVec3::Y * PLAYER_HEIGHT,
        }
    }

    fn yaw(&self) -> f32 {
        self.transform.rotation.yaw()
    }

    fn pitch(&self) -> f32 {
        self.transform.rotation.pitch()
    }

    fn game_mode(&self) -> Option<GameMode> {
        Some(self.mode)
    }

    // chisle: experience, tags and teams have no state in the server yet: every player is at
    // level 0, has no tag and no team. Their components lift it.
    fn experience_level(&self) -> Option<i32> {
        Some(0)
    }

    fn alive(&self) -> bool {
        true
    }

    fn has_tag(&self, _: &str) -> bool {
        false
    }

    fn has_any_tag(&self) -> bool {
        false
    }

    fn team(&self) -> Option<&str> {
        None
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}
