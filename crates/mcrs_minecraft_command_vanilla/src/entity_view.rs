use crate::mob_view::MobView;
use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use mcrs_minecraft_command_argument_entity::{Candidate, EntityBounds};
use mcrs_minecraft_command_engine::{CommandTargets, PlayerEntry, SourceState};
use mcrs_minecraft_core::Look;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_text::{IntoText, Text};

#[derive(Clone, Copy, Debug)]
pub(crate) enum EntityView<'c> {
    Player(&'c PlayerEntry),
    Mob { view: MobView<'c>, dimension: usize },
}

impl EntityView<'_> {
    pub(crate) fn entity(&self) -> Option<Entity> {
        match self {
            Self::Player(player) => player.entity,
            Self::Mob { view, .. } => Some(view.entity),
        }
    }

    pub(crate) fn position(&self) -> DVec3 {
        match self {
            Self::Player(player) => player.transform.translation,
            Self::Mob { view, .. } => view.position(),
        }
    }

    pub(crate) fn eye_height(&self) -> f64 {
        match self {
            Self::Player(player) => f64::from(player.dimensions.eye_height),
            Self::Mob { view, .. } => f64::from(view.mob.kind.eye_height),
        }
    }

    pub(crate) fn look(&self) -> Look {
        match self {
            Self::Player(player) => player.transform.rotation.into(),
            Self::Mob { view, .. } => view.look(),
        }
    }

    pub(crate) fn dimension_label(&self, targets: &CommandTargets, here: Entity) -> Entity {
        match self {
            Self::Player(player) => targets.label_of(player.dimension).unwrap_or(here),
            Self::Mob { .. } => here,
        }
    }

    pub(crate) fn run_as(&self, state: &SourceState) -> SourceState {
        let (player, mob) = match self {
            Self::Player(player) => (Some(player.session), None),
            Self::Mob { view, .. } => (None, Some(view.source())),
        };
        SourceState {
            player,
            mob,
            ..*state
        }
    }

    pub(crate) fn display_name(&self) -> Text {
        match self {
            Self::Player(player) => player.name.clone().into_text(),
            Self::Mob { view, .. } => view.display_name(),
        }
    }
}

impl Candidate for EntityView<'_> {
    fn uuid(&self) -> Uuid {
        match self {
            Self::Player(player) => player.uuid,
            Self::Mob { view, .. } => view.uuid(),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Player(player) => &player.name,
            Self::Mob { view, .. } => view.mob.custom_name.as_deref().unwrap_or_default(),
        }
    }

    fn entity_type(&self) -> EntityType {
        match self {
            Self::Player(_) => EntityType::Player,
            Self::Mob { view, .. } => view.entity_type(),
        }
    }

    fn position(&self) -> DVec3 {
        EntityView::position(self)
    }

    fn bounding_box(&self) -> EntityBounds {
        let (width, height) = match self {
            Self::Player(player) => (
                f64::from(player.dimensions.width),
                f64::from(player.dimensions.height),
            ),
            Self::Mob { view, .. } => (
                f64::from(view.mob.kind.width),
                f64::from(view.mob.kind.height),
            ),
        };
        let position = EntityView::position(self);
        let half = DVec3::new(width / 2.0, 0.0, width / 2.0);
        EntityBounds {
            min: position - half,
            max: position + half + DVec3::Y * height,
        }
    }

    fn yaw(&self) -> f32 {
        self.look().yaw
    }

    fn pitch(&self) -> f32 {
        self.look().pitch
    }

    fn game_mode(&self) -> Option<GameMode> {
        match self {
            Self::Player(player) => Some(player.mode),
            Self::Mob { .. } => None,
        }
    }

    fn experience_level(&self) -> Option<i32> {
        match self {
            Self::Player(_) => Some(0),
            Self::Mob { .. } => None,
        }
    }

    fn alive(&self) -> bool {
        match self {
            Self::Player(_) => true,
            Self::Mob { view, .. } => view.alive(),
        }
    }

    fn has_tag(&self, tag: &str) -> bool {
        match self {
            Self::Player(_) => false,
            Self::Mob { view, .. } => view.mob.tags.iter().any(|held| held == tag),
        }
    }

    fn has_any_tag(&self) -> bool {
        match self {
            Self::Player(_) => false,
            Self::Mob { view, .. } => !view.mob.tags.is_empty(),
        }
    }

    fn team(&self) -> Option<&str> {
        None
    }

    fn dimension(&self) -> usize {
        match self {
            Self::Player(player) => player.dimension,
            Self::Mob { dimension, .. } => *dimension,
        }
    }
}
