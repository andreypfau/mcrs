use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;

use crate::AttackKnockback;

#[derive(Resource, Clone, Debug, Default)]
pub struct AttackKnockbacks(pub(crate) Vec<AttackKnockback>);

impl AttackKnockbacks {
    #[must_use]
    pub fn knockbacks_mut(&mut self) -> &mut [AttackKnockback] {
        &mut self.0
    }

    pub fn retain(&mut self, keep: impl FnMut(&AttackKnockback) -> bool) {
        self.0.retain(keep);
    }
}

pub(crate) fn has_attack_knockbacks(knockbacks: Res<AttackKnockbacks>) -> bool {
    !knockbacks.0.is_empty()
}
