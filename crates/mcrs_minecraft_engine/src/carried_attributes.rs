use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    AttributeInstances, AttributeSet, InstanceModifier, ModifierKind, ModifierOperation,
};
use mcrs_minecraft_resource::ResourceLocationBuf;
use serde::{Deserialize, Serialize};

use crate::persisted_registry::PersistedForm;

#[derive(Serialize, Deserialize)]
pub(crate) struct CarriedAttributes {
    instances: Vec<CarriedInstance>,
}

#[derive(Serialize, Deserialize)]
struct CarriedInstance {
    attribute: Attribute,
    base_bits: u64,
    modifiers: Vec<CarriedModifier>,
}

#[derive(Serialize, Deserialize)]
struct CarriedModifier {
    id: ResourceLocationBuf,
    amount_bits: u64,
    operation: ModifierOperation,
    kind: CarriedKind,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
enum CarriedKind {
    Permanent,
    Transient,
}

impl PersistedForm<AttributeInstances> for CarriedAttributes {
    fn from_value(attributes: &AttributeInstances) -> Self {
        let instances = attributes
            .set()
            .instances()
            .map(|instance| CarriedInstance {
                attribute: instance.attribute(),
                base_bits: instance.base().to_bits(),
                modifiers: instance.modifiers().iter().map(carried_modifier).collect(),
            })
            .collect();
        Self { instances }
    }

    fn into_value(self) -> Result<AttributeInstances, String> {
        let bases = mcrs_minecraft_entity::attribute_bases(EntityType::Player)
            .ok_or("the type `player` has no attribute bases")?;
        let mut set = AttributeSet::new();
        for instance in self.instances {
            let _ = set
                .set_base(
                    bases,
                    instance.attribute,
                    f64::from_bits(instance.base_bits),
                )
                .map_err(|error| format!("{:?}: {error}", instance.attribute))?;
            for modifier in instance.modifiers {
                let kind = match modifier.kind {
                    CarriedKind::Permanent => ModifierKind::Permanent,
                    CarriedKind::Transient => ModifierKind::Transient,
                };
                let modifier = InstanceModifier::new(
                    modifier.id,
                    f64::from_bits(modifier.amount_bits),
                    modifier.operation,
                    kind,
                );
                set.add_modifier(bases, instance.attribute, modifier)
                    .map_err(|error| format!("{:?}: {error}", instance.attribute))?;
            }
        }
        Ok(AttributeInstances::with_set(bases, set))
    }
}

fn carried_modifier(modifier: &InstanceModifier) -> CarriedModifier {
    CarriedModifier {
        id: modifier.id().to_buf(),
        amount_bits: modifier.amount().to_bits(),
        operation: modifier.operation(),
        kind: match modifier.kind() {
            ModifierKind::Permanent => CarriedKind::Permanent,
            ModifierKind::Transient => CarriedKind::Transient,
        },
    }
}
