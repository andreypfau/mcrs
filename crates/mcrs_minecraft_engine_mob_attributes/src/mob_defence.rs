use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Add;
use bevy_ecs::message::MessageReader;
use bevy_ecs::observer::On;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use mcrs_minecraft_damage::DamageTags;
use mcrs_minecraft_damage::Defence;
use mcrs_minecraft_engine_physics::Mob;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::entity_type_tag_keys;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::Registered;

use crate::attribute_changed::AttributeChanged;
use crate::mob_attributes::MobAttributes;

type Defended<'w, 's> = Query<'w, 's, (&'static Mob, &'static MobAttributes, &'static mut Defence)>;

pub(crate) fn fill_defence(
    add: On<Add<Mob>>,
    registries: Option<Res<CurrentRegistries>>,
    mut mobs: Defended,
) {
    let entity = add.event().entity;
    refresh_defence(entity, &mut mobs);
    if let Ok((mob, _, mut defence)) = mobs.get_mut(entity) {
        defence.immune = immunity(mob.kind, registries.as_deref());
    }
}

pub(crate) fn project_defence(mut changed: MessageReader<AttributeChanged>, mut mobs: Defended) {
    for change in changed.read() {
        if matches!(
            change.attribute,
            Attribute::Armor | Attribute::ArmorToughness
        ) {
            refresh_defence(change.target, &mut mobs);
        }
    }
}

fn refresh_defence(entity: Entity, mobs: &mut Defended) {
    let Ok((mob, attributes, mut defence)) = mobs.get_mut(entity) else {
        return;
    };
    let entity_type = mob.kind.entity_type;
    let value = |attribute| attributes.value(entity_type, attribute).unwrap_or(0.0) as f32;
    let next = Defence::new(
        value(Attribute::Armor),
        value(Attribute::ArmorToughness),
        defence.resistance,
        defence.immune,
    );
    defence.set_if_neq(next);
}

fn immunity(kind: &EntityTypeData, registries: Option<&CurrentRegistries>) -> DamageTags {
    let mut immune = DamageTags::empty();
    if kind.fire_immune {
        immune = immune.union(DamageTags::IS_FIRE);
    }
    if registries.is_some_and(|registries| fall_immune(registries, kind.entity_type)) {
        immune = immune.union(DamageTags::IS_FALL);
    }
    immune
}

fn fall_immune(registries: &CurrentRegistries, entity_type: EntityType) -> bool {
    let Some(tags) = registries.tags::<EntityType>() else {
        return false;
    };
    let Some(tag) = tags.id(entity_type_tag_keys::FALL_DAMAGE_IMMUNE) else {
        return false;
    };
    let Some(id) = registries.id_of(EntityType::REGISTRY.location(), entity_type.location()) else {
        return false;
    };
    tags.members(tag).any(|member| member.number() == id)
}
