use bevy_ecs::resource::Resource;
use mcrs_minecraft_core::rl;
use mcrs_minecraft_entity::attribute::MAX_HEALTH;
use mcrs_minecraft_entity::{Attribute, EntityType};
use mcrs_minecraft_registry::shared::SharedResource;
use mcrs_minecraft_registry::{Id, LoadReport, RegistrySet};
use std::ops::Deref;
use std::sync::Arc;

pub static ALLAY: EntityType = EntityType::new(rl!("minecraft:allay"));
pub static CAT: EntityType = EntityType::new(rl!("minecraft:cat"));
pub static CHEST_MINECART: EntityType = EntityType::new(rl!("minecraft:chest_minecart"));
pub static CHICKEN: EntityType = EntityType::new(rl!("minecraft:chicken"));
pub static DROWNED: EntityType = EntityType::new(rl!("minecraft:drowned"));
pub static ELDER_GUARDIAN: EntityType = EntityType::new(rl!("minecraft:elder_guardian"));
pub static EVOKER: EntityType = EntityType::new(rl!("minecraft:evoker"));
pub static ITEM: EntityType = EntityType::new(rl!("minecraft:item"));
pub static ITEM_FRAME: EntityType = EntityType::new(rl!("minecraft:item_frame"));
pub static SHULKER: EntityType = EntityType::new(rl!("minecraft:shulker"));
pub static PRIMED_TNT: EntityType = EntityType::new(rl!("minecraft:tnt"));
pub static VILLAGER: EntityType = EntityType::new(rl!("minecraft:villager"));
pub static VINDICATOR: EntityType = EntityType::new(rl!("minecraft:vindicator"));
pub static WITCH: EntityType = EntityType::new(rl!("minecraft:witch"));
pub static ZOMBIE_NAUTILUS: EntityType = EntityType::new(rl!("minecraft:zombie_nautilus"));
pub static ZOMBIE_VILLAGER: EntityType = EntityType::new(rl!("minecraft:zombie_villager"));
pub static PLAYER: EntityType = EntityType::new(rl!("minecraft:player"));

macro_rules! entity_ids {
    ($($field:ident: $named:ident),* $(,)?) => {
        pub struct NamedEntityIds {
            $(pub $field: Id<EntityType>,)*
            pub max_health: Id<Attribute>,
        }

        impl NamedEntityIds {
            fn resolve(set: &RegistrySet, report: &mut LoadReport) -> Option<Self> {
                let types = report.registry::<EntityType>(set);
                let attributes = report.registry::<Attribute>(set);
                $(
                    let $field = types
                        .as_ref()
                        .and_then(|types| report.require(types, $named.identifier.as_str()));
                )*
                let max_health = attributes.as_ref().and_then(|attributes| {
                    report.require(attributes, MAX_HEALTH.identifier.as_str())
                });
                Some(Self {
                    $($field: $field?,)*
                    max_health: max_health?,
                })
            }
        }
    };
}

entity_ids! {
    allay: ALLAY,
    cat: CAT,
    chest_minecart: CHEST_MINECART,
    chicken: CHICKEN,
    drowned: DROWNED,
    elder_guardian: ELDER_GUARDIAN,
    evoker: EVOKER,
    item: ITEM,
    item_frame: ITEM_FRAME,
    shulker: SHULKER,
    primed_tnt: PRIMED_TNT,
    villager: VILLAGER,
    vindicator: VINDICATOR,
    witch: WITCH,
    zombie_nautilus: ZOMBIE_NAUTILUS,
    zombie_villager: ZOMBIE_VILLAGER,
    player: PLAYER,
}

#[derive(Resource, Clone)]
pub struct EntityIds(Arc<NamedEntityIds>);

impl EntityIds {
    pub fn resolve(set: &RegistrySet, report: &mut LoadReport) -> Option<Self> {
        NamedEntityIds::resolve(set, report).map(|ids| Self(Arc::new(ids)))
    }
}

impl Deref for EntityIds {
    type Target = NamedEntityIds;

    fn deref(&self) -> &NamedEntityIds {
        &self.0
    }
}

impl SharedResource for EntityIds {
    fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_registry::LoadReport;
    use mcrs_minecraft_registry::static_report::from_report;

    #[test]
    fn a_named_entity_type_the_report_lacks_is_refused() {
        let mut report: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../../assets/mcrs/reports/registries.json"
        ))
        .unwrap();
        let entries = report["minecraft:entity_type"]["entries"]
            .as_object_mut()
            .unwrap();
        let removed = entries
            .remove("minecraft:witch")
            .expect("the report carries the witch")["protocol_id"]
            .as_u64()
            .unwrap();
        for entry in entries.values_mut() {
            let id = entry["protocol_id"].as_u64().unwrap();
            if id > removed {
                entry["protocol_id"] = (id - 1).into();
            }
        }
        let set = from_report(&serde_json::to_vec(&report).unwrap()).unwrap();

        let mut missing = LoadReport::new();
        assert!(EntityIds::resolve(&set, &mut missing).is_none());
        let text = missing.to_string();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.contains("minecraft:witch"), "{text}");
    }
}
