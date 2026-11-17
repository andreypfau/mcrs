use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ContextParam {
    ThisEntity,
    InteractingEntity,
    TargetEntity,
    LastDamagePlayer,
    DamageSource,
    AttackingEntity,
    DirectAttackingEntity,
    Origin,
    BlockState,
    BlockEntity,
    Tool,
    ExplosionRadius,
    EnchantmentLevel,
    EnchantmentActive,
    AdditionalCostComponentAllowed,
    Container,
}

impl ContextParam {
    pub const ALL: [ContextParam; 16] = [
        ContextParam::ThisEntity,
        ContextParam::InteractingEntity,
        ContextParam::TargetEntity,
        ContextParam::LastDamagePlayer,
        ContextParam::DamageSource,
        ContextParam::AttackingEntity,
        ContextParam::DirectAttackingEntity,
        ContextParam::Origin,
        ContextParam::BlockState,
        ContextParam::BlockEntity,
        ContextParam::Tool,
        ContextParam::ExplosionRadius,
        ContextParam::EnchantmentLevel,
        ContextParam::EnchantmentActive,
        ContextParam::AdditionalCostComponentAllowed,
        ContextParam::Container,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ContextParam::ThisEntity => "this_entity",
            ContextParam::InteractingEntity => "interacting_entity",
            ContextParam::TargetEntity => "target_entity",
            ContextParam::LastDamagePlayer => "last_damage_player",
            ContextParam::DamageSource => "damage_source",
            ContextParam::AttackingEntity => "attacking_entity",
            ContextParam::DirectAttackingEntity => "direct_attacking_entity",
            ContextParam::Origin => "origin",
            ContextParam::BlockState => "block_state",
            ContextParam::BlockEntity => "block_entity",
            ContextParam::Tool => "tool",
            ContextParam::ExplosionRadius => "explosion_radius",
            ContextParam::EnchantmentLevel => "enchantment_level",
            ContextParam::EnchantmentActive => "enchantment_active",
            ContextParam::AdditionalCostComponentAllowed => "additional_cost_component_allowed",
            ContextParam::Container => "container",
        }
    }
}

impl fmt::Display for ContextParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
