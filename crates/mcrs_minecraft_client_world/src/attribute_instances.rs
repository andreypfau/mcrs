use bevy::prelude::*;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::{AttributeBases, AttributeInstance, AttributeSet, InstanceModifier};
use mcrs_minecraft_entity_attribute::{AttributeModifier, HomeSlot, Modifier, fold};
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::clientbound::{
    AttributeSnapshot, ClientboundLogin, ClientboundRespawn, ClientboundUpdateAttributes,
};
use mcrs_minecraft_resource::ResourceLocation;

use crate::joined_game::JoinedGame;
use crate::local_player::Sprint;
use crate::player::Player;

/// The attribute instances of a living entity on the client, with the bases of its type. A new
/// living entity has the base of the type and no modifier for each attribute (ATR-CLI-001).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct AttributeInstances {
    bases: AttributeBases,
    set: AttributeSet,
}

impl AttributeInstances {
    /// The instances of a new entity with the bases `bases` of its type.
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

    /// The value of ATR-VAL-001 for the instance of `attribute` without its modifier `id` and
    /// with `replacement` at the end of the sequence (ATR-INS-004), which this call does not
    /// store; `None` when the attribute is not in the set of the type.
    pub(crate) fn value_replacing(
        &self,
        attribute: Attribute,
        id: ResourceLocation<'_>,
        replacement: Option<AttributeModifier<'static>>,
    ) -> Option<f64> {
        let base = self.set.base(self.bases, attribute)?;
        let modifiers: &[InstanceModifier] = self
            .set
            .instance(attribute)
            .map_or(&[], AttributeInstance::modifiers);
        let appended = replacement.map(Modifier::from);
        let kept = modifiers
            .iter()
            .filter(move |kept| kept.id() != id)
            .map(|kept| Modifier {
                operation: kept.operation(),
                amount: kept.amount(),
                slot: HomeSlot::from(kept.id()),
            });
        let value = fold(base, kept.chain(appended));
        Some(attribute.definition().range.clamp(value))
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
        let _in_set = self.set.clear_modifiers(self.bases, attribute);
        for modifier in &entry.modifiers {
            let _ignored_duplicate =
                self.set
                    .add_modifier(self.bases, attribute, InstanceModifier::transient(modifier));
        }
    }

    /// Leaves an equal modifier in place (ATR-INS-004); changes nothing when the attribute is not
    /// in the set of the type.
    pub(crate) fn put_modifier(&mut self, attribute: Attribute, modifier: InstanceModifier) {
        let _in_set = self.set.set_modifier(self.bases, attribute, modifier);
    }

    pub(crate) fn remove_modifier(&mut self, attribute: Attribute, id: ResourceLocation<'_>) {
        let _in_set = self.set.remove_modifier(self.bases, attribute, id);
    }

    /// Drops every modifier and keeps every base.
    pub(crate) fn clear_modifiers(&mut self) {
        let attributes: Vec<Attribute> = self
            .set
            .instances()
            .map(AttributeInstance::attribute)
            .collect();
        for attribute in attributes {
            let _in_set = self.set.clear_modifiers(self.bases, attribute);
        }
    }
}

/// Gives the local [`Player`] the attribute instances of a new player, which does not sprint
/// (ATR-CLI-005).
pub(crate) fn on_login(
    _event: On<Received<ClientboundLogin>>,
    mut player: Query<(Entity, Option<&mut Sprint>), With<Player>>,
    mut commands: Commands,
) {
    let Ok((player, sprint)) = player.single_mut() else {
        return;
    };
    if let Some(mut sprint) = sprint {
        sprint.set_if_neq(Sprint::default());
    }
    if let Some(bases) = AttributeBases::of(EntityType::Player) {
        commands
            .entity(player)
            .insert(AttributeInstances::new(bases));
    }
}

/// Makes the local [`Player`] a new player, which does not sprint and keeps its modifiers only by
/// the bit of the packet (ATR-CLI-005).
pub(crate) fn on_respawn(
    event: On<Received<ClientboundRespawn>>,
    mut player: Query<(Option<&mut AttributeInstances>, Option<&mut Sprint>), With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let Ok((instances, sprint)) = player.single_mut() else {
        return;
    };
    if let Some(mut sprint) = sprint {
        sprint.set_if_neq(Sprint::default());
    }
    if packet.data_to_keep & ClientboundRespawn::KEEP_ATTRIBUTE_MODIFIERS == 0
        && let Some(mut instances) = instances
    {
        instances.clear_modifiers();
    }
}

/// Applies the entries of an `update_attributes` for the own entity id to the local [`Player`]
/// (ATR-CLI-002).
pub(crate) fn on_update_attributes(
    event: On<Received<ClientboundUpdateAttributes<'static>>>,
    connections: Query<&JoinedGame>,
    mut player: Query<&mut AttributeInstances, With<Player>>,
) {
    let Some(packet) = event.packet() else {
        return;
    };
    let own = connections
        .get(event.entity)
        .is_ok_and(|joined| joined.player_id == packet.entity_id.0);
    if !own {
        return;
    }
    if let Ok(mut instances) = player.single_mut() {
        for entry in &packet.attributes {
            instances.receive(entry);
        }
    }
}
