use mcrs_minecraft_registries::Named;
use serde::Deserializer;
use serde::de::DeserializeSeed;

use crate::data_component_type_keys::DataComponentType;
use crate::kind::ItemComponentValue;

pub(crate) struct ComponentValueSeed(pub(crate) DataComponentType);

impl<'de> DeserializeSeed<'de> for ComponentValueSeed {
    type Value = ItemComponentValue<Named>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        ItemComponentValue::deserialize_value(self.0, deserializer)
    }
}
