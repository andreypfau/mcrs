use bevy_ecs::resource::Resource;
use mcrs_minecraft_entity::attribute::MAX_HEALTH;
use mcrs_minecraft_keys::{Attribute, EntityType};
use mcrs_minecraft_registry::shared::SharedResource;
use mcrs_minecraft_registry::{Id, LoadReport, RegistrySet};
use std::ops::Deref;
use std::sync::Arc;

macro_rules! entity_ids {
    ($($field:ident: $named:literal),* $(,)?) => {
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
                        .and_then(|types| report.require(types, $named));
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
    allay: "minecraft:allay",
    cat: "minecraft:cat",
    chest_minecart: "minecraft:chest_minecart",
    chicken: "minecraft:chicken",
    drowned: "minecraft:drowned",
    elder_guardian: "minecraft:elder_guardian",
    evoker: "minecraft:evoker",
    item: "minecraft:item",
    item_frame: "minecraft:item_frame",
    shulker: "minecraft:shulker",
    primed_tnt: "minecraft:tnt",
    villager: "minecraft:villager",
    vindicator: "minecraft:vindicator",
    witch: "minecraft:witch",
    zombie_nautilus: "minecraft:zombie_nautilus",
    zombie_villager: "minecraft:zombie_villager",
    player: "minecraft:player",
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
