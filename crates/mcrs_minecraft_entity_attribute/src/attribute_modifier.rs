use mcrs_minecraft_resource::ResourceLocation;

use crate::ModifierOperation;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    feature = "wire",
    derive(mcrs_minecraft_wire::Encode, mcrs_minecraft_wire::Decode)
)]
pub struct AttributeModifier<'a> {
    pub id: ResourceLocation<'a>,
    pub amount: f64,
    pub operation: ModifierOperation,
}
