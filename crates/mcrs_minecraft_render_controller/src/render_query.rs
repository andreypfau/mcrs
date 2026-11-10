use mcrs_minecraft_entity_animation::EntityQuery;

/// A query that an expression of a render controller reads.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RenderQuery {
    Entity(EntityQuery),
    /// `query.equipment_texture('<slot>', '<layer type>')`.
    EquipmentTexture {
        slot: String,
        layer: String,
    },
}
