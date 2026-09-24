use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_registry::{StaticRegistry, StaticRegistryTable};

use super::EntityType;

pub static ALLAY: EntityType = EntityType::new(rl!("minecraft:allay"), 2);
pub static CAT: EntityType = EntityType::new(rl!("minecraft:cat"), 21);
pub static CHEST_MINECART: EntityType = EntityType::new(rl!("minecraft:chest_minecart"), 25);
pub static CHICKEN: EntityType = EntityType::new(rl!("minecraft:chicken"), 26);
pub static DROWNED: EntityType = EntityType::new(rl!("minecraft:drowned"), 39);
pub static ELDER_GUARDIAN: EntityType = EntityType::new(rl!("minecraft:elder_guardian"), 41);
pub static EVOKER: EntityType = EntityType::new(rl!("minecraft:evoker"), 47);
pub static ITEM: EntityType = EntityType::new(rl!("minecraft:item"), 72);
pub static ITEM_FRAME: EntityType = EntityType::new(rl!("minecraft:item_frame"), 74);
pub static SHULKER: EntityType = EntityType::new(rl!("minecraft:shulker"), 115);
pub static PRIMED_TNT: EntityType = EntityType::new(rl!("minecraft:tnt"), 136);
pub static VILLAGER: EntityType = EntityType::new(rl!("minecraft:villager"), 143);
pub static VINDICATOR: EntityType = EntityType::new(rl!("minecraft:vindicator"), 144);
pub static WITCH: EntityType = EntityType::new(rl!("minecraft:witch"), 148);
pub static ZOMBIE_NAUTILUS: EntityType = EntityType::new(rl!("minecraft:zombie_nautilus"), 156);
pub static ZOMBIE_VILLAGER: EntityType = EntityType::new(rl!("minecraft:zombie_villager"), 157);
pub static PLAYER: EntityType = EntityType::new(rl!("minecraft:player"), 159);

static NAMED: &[&EntityType] = &[
    &ALLAY,
    &CAT,
    &CHEST_MINECART,
    &CHICKEN,
    &DROWNED,
    &ELDER_GUARDIAN,
    &EVOKER,
    &ITEM,
    &ITEM_FRAME,
    &SHULKER,
    &PRIMED_TNT,
    &VILLAGER,
    &VINDICATOR,
    &WITCH,
    &ZOMBIE_NAUTILUS,
    &ZOMBIE_VILLAGER,
    &PLAYER,
];

pub fn register_all_entity_types(
    registry: &mut StaticRegistry<EntityType>,
    table: &StaticRegistryTable,
) {
    let entries = table
        .registry("entity_type")
        .expect("the registry report lists entity_type");
    for (protocol_id, name) in entries.names().iter().enumerate() {
        let protocol_id = protocol_id as u32;
        let entity_type = match NAMED
            .iter()
            .find(|t| t.identifier.as_str() == name.as_str())
        {
            Some(named) => {
                assert_eq!(named.protocol_id, protocol_id, "{name}");
                *named
            }
            None => Box::leak(Box::new(EntityType::new(
                ResourceLocation::new_static(name.as_str().to_owned().leak()),
                protocol_id,
            ))),
        };
        registry.register(name.clone(), entity_type);
    }
    for named in NAMED {
        assert!(
            registry.id_of(named.identifier.as_str()).is_some(),
            "{} is not in the registry report",
            named.identifier
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_ids_match_the_registry_report() {
        let table = StaticRegistryTable::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/mcrs/reports/registries.json"
        ))
        .unwrap();
        let mut registry = StaticRegistry::new();
        register_all_entity_types(&mut registry, &table);
        assert_eq!(registry.len(), 161);
        for (id, location, entity_type) in registry.iter() {
            assert_eq!(entity_type.protocol_id, id.raw(), "{location}");
            assert_eq!(entity_type.identifier.as_str(), location.as_str());
        }
        for named in NAMED {
            let id = registry.id_of(named.identifier.as_str()).unwrap();
            assert_eq!(named.protocol_id, id.raw(), "{}", named.identifier);
        }
    }
}
