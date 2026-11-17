use mcrs_minecraft_entity_animation::EntityQuery;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RenderQuery {
    Entity(EntityQuery),
    EquipmentTexture { slot: String, layer: String },
}
