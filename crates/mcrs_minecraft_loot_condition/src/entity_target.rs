use serde::Deserialize;

use crate::context_param::ContextParam;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EntityTarget {
    This,
    Attacker,
    DirectAttacker,
    AttackingPlayer,
    TargetEntity,
    InteractingEntity,
}

impl EntityTarget {
    pub(crate) const fn param(self) -> ContextParam {
        match self {
            EntityTarget::This => ContextParam::ThisEntity,
            EntityTarget::Attacker => ContextParam::AttackingEntity,
            EntityTarget::DirectAttacker => ContextParam::DirectAttackingEntity,
            EntityTarget::AttackingPlayer => ContextParam::LastDamagePlayer,
            EntityTarget::TargetEntity => ContextParam::TargetEntity,
            EntityTarget::InteractingEntity => ContextParam::InteractingEntity,
        }
    }
}
