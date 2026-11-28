use bevy_ecs::entity::Entity;
use bevy_math::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_engine::command_handoff::SourceMob;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::{IntoText, Text};

#[derive(Clone, Copy, Debug)]
pub(crate) struct MobView<'a> {
    pub(crate) entity: Entity,
    pub(crate) id: EntityId,
    pub(crate) state: &'a EntityState,
    pub(crate) vitals: Option<&'a Vitals>,
    pub(crate) mob: &'a Mob,
}

impl MobView<'_> {
    pub(crate) fn position(&self) -> DVec3 {
        self.state.body.position
    }

    pub(crate) fn look(&self) -> Look {
        let living = self.state.living.unwrap_or_default();
        Look {
            yaw: living.orientation.yaw,
            pitch: living.orientation.pitch,
        }
    }

    pub(crate) fn source(&self) -> SourceMob {
        SourceMob {
            entity: self.entity,
            id: self.id,
        }
    }

    pub(crate) fn alive(&self) -> bool {
        self.state.living.is_some() && self.vitals.is_none_or(Vitals::is_alive)
    }

    pub(crate) fn uuid(&self) -> Uuid {
        Uuid::from_u128(self.mob.uuid)
    }

    pub(crate) fn entity_type(&self) -> EntityType {
        self.mob.kind.entity_type
    }

    pub(crate) fn display_name(&self) -> Text {
        display_name(
            self.entity_type(),
            self.uuid(),
            self.mob.custom_name.as_deref(),
        )
    }
}

pub(crate) fn display_name(entity_type: EntityType, uuid: Uuid, custom_name: Option<&str>) -> Text {
    let location = entity_type.location();
    let key = format!("entity.{}.{}", location.namespace(), location.path());
    let type_name = Text::translate(key, vec![]);
    let name = match custom_name {
        Some(name) => name.to_owned().into_text(),
        None => type_name.clone(),
    };
    name.insertion(uuid.hyphenated().to_string())
        .on_hover_show_entity(
            ResourceLocationBuf::from(location),
            uuid,
            Some(custom_name.map_or(type_name, |name| name.to_owned().into_text())),
        )
}
