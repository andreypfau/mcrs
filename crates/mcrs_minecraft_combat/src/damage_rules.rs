use bevy_ecs::prelude::Resource;
use mcrs_minecraft_damage::DamageTypeRules;

#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct DamageTypeRulesResource(pub DamageTypeRules);
