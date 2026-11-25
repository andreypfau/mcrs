use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::rl;

use crate::NameLookup;
use crate::NameTable;
use crate::data_component_type_keys::DataComponentType;

const ANY: ResourceLocation<'static> = rl!("mcrs:any");

pub(crate) struct AnyNames;

impl NameLookup for AnyNames {
    fn id(&self, _table: NameTable, _name: ResourceLocation<'_>) -> Option<u16> {
        Some(0)
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        match table {
            NameTable::ComponentType => {
                DataComponentType::from_protocol_id(id).map(DataComponentType::location)
            }
            _ => Some(ANY),
        }
    }
}
