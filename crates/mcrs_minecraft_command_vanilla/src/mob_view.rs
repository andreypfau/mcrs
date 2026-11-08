use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use mcrs_minecraft_command_engine::{CommandTargets, PlayerEntry, SourceState};
use mcrs_minecraft_core::Look;
use mcrs_minecraft_engine::command_handoff::SourceMob;
use mcrs_minecraft_engine_physics::{Mob, Placed};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::{HoverEvent, IntoText, Text};

/// A mob row of this world as a command sees it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MobView<'a> {
    pub(crate) at: Placed,
    pub(crate) state: &'a EntityState,
    pub(crate) mob: &'a Mob,
}

/// A mob that a selector found, which outlives the borrow of its row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MobTarget {
    pub(crate) at: Placed,
    pub(crate) position: DVec3,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

impl MobView<'_> {
    pub(crate) fn target(&self) -> MobTarget {
        let living = self.state.living.unwrap_or_default();
        MobTarget {
            at: self.at,
            position: self.state.body.position,
            yaw: living.yaw,
            pitch: living.pitch,
        }
    }

    pub(crate) fn alive(&self) -> bool {
        self.state.living.is_some_and(|living| living.is_alive())
    }

    pub(crate) fn uuid(&self) -> Uuid {
        Uuid::from_u128(self.mob.uuid)
    }

    pub(crate) fn entity_type(&self) -> EntityType {
        self.mob.kind.entity_type()
    }

    /// The display name (TRK-SUM-004): the custom name, or the name of the type, with the type
    /// and the UUID on hover and the UUID as the insertion.
    pub(crate) fn display_name(&self) -> Text {
        display_name(
            self.entity_type(),
            self.uuid(),
            self.mob.custom_name.as_deref(),
        )
    }
}

impl MobTarget {
    pub(crate) fn source(&self) -> SourceMob {
        SourceMob {
            tile: self.at.tile,
            id: self.at.id,
        }
    }
}

/// An entity that a selector found: a player of any dimension or a mob of this world.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Found<'a> {
    Player(&'a PlayerEntry),
    Mob(&'a MobTarget),
}

impl Found<'_> {
    pub(crate) fn position(&self) -> DVec3 {
        match self {
            Self::Player(player) => player.transform.translation,
            Self::Mob(mob) => mob.position,
        }
    }

    pub(crate) fn look(&self) -> Look {
        match self {
            Self::Player(player) => player.transform.rotation.into(),
            Self::Mob(mob) => Look {
                yaw: mob.yaw,
                pitch: mob.pitch,
            },
        }
    }

    /// The label of the dimension of the entity; a mob is in the dimension `here` of the source.
    pub(crate) fn dimension(&self, targets: &CommandTargets, here: Entity) -> Entity {
        match self {
            Self::Player(player) => targets.label_of(player.dimension).unwrap_or(here),
            Self::Mob(_) => here,
        }
    }

    /// The source that runs as the entity.
    pub(crate) fn run_as(&self, state: &SourceState) -> SourceState {
        let (player, mob) = match self {
            Self::Player(player) => (Some(player.session), None),
            Self::Mob(mob) => (None, Some(mob.source())),
        };
        SourceState {
            player,
            mob,
            ..*state
        }
    }
}

/// The display name of an entity that is not a player.
pub(crate) fn display_name(entity_type: EntityType, uuid: Uuid, custom_name: Option<&str>) -> Text {
    let location = entity_type.location();
    let key = format!("entity.{}.{}", location.namespace(), location.path());
    let type_name = Text::translate(key, vec![]);
    let name = match custom_name {
        Some(name) => name.to_owned().into_text(),
        None => type_name.clone(),
    };
    let mut text = name.insertion(uuid.hyphenated().to_string());
    text.style.hover_event = Some(HoverEvent::ShowEntity {
        id: ResourceLocationBuf::from(location),
        uuid,
        name: Some(custom_name.map_or(type_name, |name| name.to_owned().into_text())),
    });
    text
}
