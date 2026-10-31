use mcrs_minecraft_registries::dispatch::Buffered;
use serde::de::{self, MapAccess, Visitor, value};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::marker::PhantomData;

/// A bare value or the dispatch, where `constant` names a map over a single
/// `value` field.
pub(crate) enum ConstantOrDispatch<V, D> {
    Bare(V),
    Dispatched(D),
}

impl<'de, V: Deserialize<'de>, D: Deserialize<'de>> ConstantOrDispatch<V, D> {
    /// A bare value that is itself a map is told from the dispatch by the
    /// dispatch's `type` key.
    pub(crate) fn deserialize<De: Deserializer<'de>>(
        deserializer: De,
        bare_is_map: bool,
    ) -> Result<Self, De::Error> {
        struct ConstantOrDispatchVisitor<V, D> {
            bare_is_map: bool,
            human_readable: bool,
            shape: PhantomData<(V, D)>,
        }

        impl<'de, V: Deserialize<'de>, D: Deserialize<'de>> Visitor<'de>
            for ConstantOrDispatchVisitor<V, D>
        {
            type Value = ConstantOrDispatch<V, D>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a constant or a typed provider")
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                V::deserialize(value::I64Deserializer::new(v)).map(ConstantOrDispatch::Bare)
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                V::deserialize(value::U64Deserializer::new(v)).map(ConstantOrDispatch::Bare)
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                V::deserialize(value::F64Deserializer::new(v)).map(ConstantOrDispatch::Bare)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                if !self.bare_is_map {
                    return D::deserialize(value::MapAccessDeserializer::new(map))
                        .map(ConstantOrDispatch::Dispatched);
                }
                let buffered = Buffered::deserialize(value::MapAccessDeserializer::new(map))?;
                let typed = matches!(&buffered, Buffered::Map(entries)
                    if entries.iter().any(|(key, _)| matches!(key, Buffered::Str(key) if key == "type")));
                let deserializer = buffered.deserializer(self.human_readable);
                if typed {
                    D::deserialize(deserializer).map(ConstantOrDispatch::Dispatched)
                } else {
                    V::deserialize(deserializer).map(ConstantOrDispatch::Bare)
                }
            }
        }

        let human_readable = deserializer.is_human_readable();
        deserializer.deserialize_any(ConstantOrDispatchVisitor {
            bare_is_map,
            human_readable,
            shape: PhantomData,
        })
    }
}
