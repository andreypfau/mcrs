use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use mcrs_minecraft_command_entity::{Candidate, EntityBounds};
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_entity::{PLAYER_HEIGHT, PLAYER_WIDTH};
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::uuid::Uuid;

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
