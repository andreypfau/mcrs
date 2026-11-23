use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;

use crate::attack_knockback::AttackKnockback;

#[derive(Resource, Clone, Debug, Default)]
pub(crate) struct AttackKnockbacks(pub(crate) Vec<AttackKnockback>);

pub(crate) fn has_attack_knockbacks(knockbacks: Res<AttackKnockbacks>) -> bool {
    !knockbacks.0.is_empty()
}
