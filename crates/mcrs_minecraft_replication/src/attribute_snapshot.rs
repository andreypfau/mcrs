use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{AttributeBases, AttributeModifier, Instance};
use mcrs_minecraft_protocol_game::clientbound::AttributeSnapshot;

/// The entry of the instance of `attribute` (ATR-NET-002): the stored base and every modifier in
/// the order of the sequence. `None` is the instance of a new entity of the type of `bases`.
pub(crate) fn attribute_snapshot(
    bases: Option<AttributeBases>,
    attribute: Attribute,
    instance: Option<&Instance>,
) -> AttributeSnapshot<'_> {
    let Some(instance) = instance else {
        let base = bases
            .and_then(|bases| bases.base(attribute))
            .unwrap_or(attribute.definition().default);
        return AttributeSnapshot {
            attribute,
            base,
            modifiers: Vec::new(),
        };
    };
    AttributeSnapshot {
        attribute,
        base: instance.base(),
        modifiers: instance
            .modifiers()
            .iter()
            .map(|modifier| AttributeModifier {
                id: modifier.id(),
                amount: modifier.amount(),
                operation: modifier.operation(),
            })
            .collect(),
    }
}
