use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::OperatorCondition;

#[derive(Debug)]
pub struct SessionInputs<'a, L> {
    pub names: &'a L,
    pub items: &'a ItemTable,
    pub registries: &'a Registries,
    pub enabled_features: &'a [ResourceLocationBuf],
    pub operator: OperatorCondition,
}
