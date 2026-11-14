use bevy::ecs::component::Component;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::{
    AttributeBases, AttributeSet, InstanceChange, InstanceModifier, ModifierKind,
};
use mcrs_minecraft_protocol_game::clientbound::AttributeSnapshot;
use mcrs_minecraft_resource::ResourceLocation;

/// The attribute instances of a living entity on the client, with the bases of its type. A new
/// living entity has the base of the type and no modifier for each attribute (ATR-CLI-001).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct AttributeInstances {
    bases: AttributeBases,
    set: AttributeSet,
}

/// What a new local player takes from the old one at a respawn (ATR-CLI-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RespawnKeep {
    BasesAndModifiers,
    Bases,
}

impl AttributeInstances {
    #[must_use]
    pub fn new(bases: AttributeBases) -> Self {
        Self {
            bases,
            set: AttributeSet::new(),
        }
    }

    /// The value of ATR-VAL-001, or `None` when the attribute is not in the set of the type.
    #[must_use]
    pub fn value(&self, attribute: Attribute) -> Option<f64> {
        self.set.value(self.bases, attribute)
    }

    #[must_use]
    pub fn bases(&self) -> AttributeBases {
        self.bases
    }

    /// The instances that differ from those of a new entity of the type.
    #[must_use]
    pub fn set(&self) -> &AttributeSet {
        &self.set
    }

    /// Takes the base and the modifiers of `entry` (ATR-CLI-002). An attribute that is not in the
    /// set of the type changes nothing, and a second modifier of one identifier is ignored
    /// (ATR-CLI-003).
    pub fn receive(&mut self, entry: &AttributeSnapshot<'_>) {
        let attribute = entry.attribute;
        if self
            .set
            .set_base(self.bases, attribute, entry.base)
            .is_err()
        {
            return;
        }
        self.remove_modifiers(attribute);
        for modifier in &entry.modifiers {
            let modifier = InstanceModifier::new(
                modifier.id.to_buf(),
                modifier.amount,
                modifier.operation,
                ModifierKind::Transient,
            );
            let _ignored_duplicate = self.set.add_modifier(self.bases, attribute, modifier);
        }
    }

    #[must_use]
    pub fn respawned(&self, keep: RespawnKeep) -> Self {
        match keep {
            RespawnKeep::BasesAndModifiers => self.clone(),
            RespawnKeep::Bases => {
                let mut respawned = Self::new(self.bases);
                for instance in self.set.instances() {
                    let _in_set =
                        respawned
                            .set
                            .set_base(self.bases, instance.attribute(), instance.base());
                }
                respawned
            }
        }
    }

    /// Leaves an equal modifier in place (ATR-INS-004); `Unchanged` also when the attribute is
    /// not in the set of the type.
    pub(crate) fn put_modifier(
        &mut self,
        attribute: Attribute,
        modifier: InstanceModifier,
    ) -> InstanceChange {
        self.set
            .set_modifier(self.bases, attribute, modifier)
            .unwrap_or(InstanceChange::Unchanged)
    }

    pub(crate) fn remove_modifier(
        &mut self,
        attribute: Attribute,
        id: ResourceLocation<'_>,
    ) -> InstanceChange {
        self.set
            .remove_modifier(self.bases, attribute, id)
            .unwrap_or(InstanceChange::Unchanged)
    }

    fn remove_modifiers(&mut self, attribute: Attribute) {
        while let Some(id) = self
            .set
            .instance(attribute)
            .and_then(|instance| instance.modifiers().first())
            .map(|modifier| modifier.id().to_buf())
        {
            let removed = self
                .set
                .remove_modifier(self.bases, attribute, id.as_location());
            if !matches!(removed, Ok(InstanceChange::Changed)) {
                break;
            }
        }
    }
}
