use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;

use crate::context_param::ContextParam;
use crate::param_not_allowed::ParamNotAllowed;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ContextParams(u32);

struct Columns {
    required: ContextParams,
    optional: ContextParams,
}

impl ContextParams {
    pub const EMPTY: Self = Self(0);

    #[must_use]
    pub const fn of(params: &[ContextParam]) -> Self {
        let mut set = Self::EMPTY;
        let mut rest = params;
        while let [param, tail @ ..] = rest {
            set = set.with(*param);
            rest = tail;
        }
        set
    }

    #[must_use]
    pub const fn with(self, param: ContextParam) -> Self {
        Self(self.0 | bit(param))
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, param: ContextParam) -> bool {
        self.0 & bit(param) != 0
    }

    #[must_use]
    pub fn required(key_set: ContextKeySet) -> Self {
        columns(key_set).required
    }

    #[must_use]
    pub fn allowed(key_set: ContextKeySet) -> Self {
        let columns = columns(key_set);
        columns.required.union(columns.optional)
    }

    pub fn check(self, key_set: ContextKeySet) -> Result<(), ParamNotAllowed> {
        let allowed = Self::allowed(key_set);
        match ContextParam::ALL
            .into_iter()
            .find(|&param| self.contains(param) && !allowed.contains(param))
        {
            Some(param) => Err(ParamNotAllowed { param, key_set }),
            None => Ok(()),
        }
    }
}

const fn bit(param: ContextParam) -> u32 {
    1 << param as u32
}

#[expect(clippy::too_many_lines, reason = "one flat match over the key sets")]
fn columns(key_set: ContextKeySet) -> Columns {
    let (required, optional): (&[ContextParam], &[ContextParam]) = match key_set {
        ContextKeySet::Empty => (&[], &[]),
        ContextKeySet::Generic => (&ContextParam::ALL, &[]),
        ContextKeySet::Chest | ContextKeySet::Command | ContextKeySet::CommandComputeDefault => {
            (&[ContextParam::Origin], &[ContextParam::ThisEntity])
        }
        ContextKeySet::CommandSlotSource => (
            &[ContextParam::Origin, ContextParam::Container],
            &[ContextParam::ThisEntity],
        ),
        ContextKeySet::CommandComputePosition => (
            &[ContextParam::Origin, ContextParam::BlockState],
            &[ContextParam::ThisEntity, ContextParam::BlockEntity],
        ),
        ContextKeySet::CommandComputeEntity => (
            &[ContextParam::Origin, ContextParam::TargetEntity],
            &[ContextParam::ThisEntity],
        ),
        ContextKeySet::Selector
        | ContextKeySet::Equipment
        | ContextKeySet::Gift
        | ContextKeySet::AdvancementReward
        | ContextKeySet::AdvancementEntity => {
            (&[ContextParam::Origin, ContextParam::ThisEntity], &[])
        }
        ContextKeySet::VillagerTrade => (
            &[
                ContextParam::Origin,
                ContextParam::ThisEntity,
                ContextParam::AdditionalCostComponentAllowed,
            ],
            &[],
        ),
        ContextKeySet::Fishing => (
            &[ContextParam::Origin, ContextParam::Tool],
            &[ContextParam::ThisEntity],
        ),
        ContextKeySet::Entity => (
            &[
                ContextParam::ThisEntity,
                ContextParam::Origin,
                ContextParam::DamageSource,
            ],
            &[
                ContextParam::AttackingEntity,
                ContextParam::DirectAttackingEntity,
                ContextParam::LastDamagePlayer,
            ],
        ),
        ContextKeySet::Archaeology | ContextKeySet::Shearing => (
            &[
                ContextParam::Origin,
                ContextParam::ThisEntity,
                ContextParam::Tool,
            ],
            &[],
        ),
        ContextKeySet::Barter => (&[ContextParam::ThisEntity], &[]),
        ContextKeySet::Vault => (
            &[ContextParam::Origin],
            &[ContextParam::ThisEntity, ContextParam::Tool],
        ),
        ContextKeySet::AdvancementLocation => (
            &[
                ContextParam::ThisEntity,
                ContextParam::Origin,
                ContextParam::Tool,
                ContextParam::BlockState,
            ],
            &[ContextParam::BlockEntity],
        ),
        ContextKeySet::BlockUse => (
            &[
                ContextParam::ThisEntity,
                ContextParam::Origin,
                ContextParam::BlockState,
            ],
            &[],
        ),
        ContextKeySet::Block => (
            &[
                ContextParam::BlockState,
                ContextParam::Origin,
                ContextParam::Tool,
            ],
            &[
                ContextParam::ThisEntity,
                ContextParam::BlockEntity,
                ContextParam::ExplosionRadius,
            ],
        ),
        ContextKeySet::EntityInteract => (
            &[ContextParam::TargetEntity, ContextParam::Tool],
            &[ContextParam::InteractingEntity],
        ),
        ContextKeySet::BlockInteract => (
            &[ContextParam::BlockState, ContextParam::Origin],
            &[
                ContextParam::BlockEntity,
                ContextParam::InteractingEntity,
                ContextParam::Tool,
            ],
        ),
        ContextKeySet::ContainerProcess => (
            &[
                ContextParam::BlockEntity,
                ContextParam::BlockState,
                ContextParam::Container,
                ContextParam::Origin,
            ],
            &[],
        ),
        ContextKeySet::EnchantedDamage => (
            &[
                ContextParam::ThisEntity,
                ContextParam::EnchantmentLevel,
                ContextParam::Origin,
                ContextParam::DamageSource,
            ],
            &[
                ContextParam::DirectAttackingEntity,
                ContextParam::AttackingEntity,
            ],
        ),
        ContextKeySet::EnchantedItem => {
            (&[ContextParam::Tool, ContextParam::EnchantmentLevel], &[])
        }
        ContextKeySet::EnchantedLocation => (
            &[
                ContextParam::ThisEntity,
                ContextParam::EnchantmentLevel,
                ContextParam::Origin,
                ContextParam::EnchantmentActive,
            ],
            &[],
        ),
        ContextKeySet::EnchantedEntity => (
            &[
                ContextParam::ThisEntity,
                ContextParam::EnchantmentLevel,
                ContextParam::Origin,
            ],
            &[],
        ),
        ContextKeySet::HitBlock => (
            &[
                ContextParam::ThisEntity,
                ContextParam::EnchantmentLevel,
                ContextParam::Origin,
                ContextParam::BlockState,
            ],
            &[],
        ),
    };
    Columns {
        required: ContextParams::of(required),
        optional: ContextParams::of(optional),
    }
}
