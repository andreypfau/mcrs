use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;

use crate::Builtin;
use crate::HashError;
use crate::HashScratch;
use crate::NameLookup;
use crate::field_map::FieldMap;
use crate::node_hash::ListHash;
use crate::node_hash::MapEntry;
use crate::node_hash::NodeHash;
use crate::node_serializer::hash_of;
use crate::saved_form::Fault;
use crate::saved_form::saved_form;
use crate::saved_sink::Field;
use crate::saved_sink::Leaf;
use crate::saved_sink::NamedEntry;
use crate::saved_sink::SavedSink;
use crate::saved_sink::TAG_MARKER;

/// The hash of the saved form of a component: the tree that the saved writer writes.
pub(crate) fn component_hash<L: NameLookup>(
    value: &Builtin,
    names: &L,
    scratch: &mut HashScratch,
) -> Result<NodeHash, HashError> {
    saved_form(value, names, &mut HashSink { scratch }).map_err(Fault::into_hash)
}

fn identifier(location: ResourceLocation<'_>) -> NodeHash {
    NodeHash::string_of_parts(&[location.namespace(), ":", location.path()])
}

struct HashSink<'scratch> {
    scratch: &'scratch mut HashScratch,
}

impl SavedSink for HashSink<'_> {
    type Node = NodeHash;

    fn leaf(&self, leaf: Leaf<'_>) -> NodeHash {
        match leaf {
            Leaf::Int(value) => NodeHash::int(value),
            Leaf::Float(value) => NodeHash::float(value),
            Leaf::Double(value) => NodeHash::double(value),
            Leaf::Boolean(value) => NodeHash::boolean(value),
            Leaf::String(value) => NodeHash::string(value),
            Leaf::Name(name) => identifier(name),
            Leaf::TagName(name) => {
                NodeHash::string_of_parts(&[TAG_MARKER, name.namespace(), ":", name.path()])
            }
        }
    }

    fn text(&mut self, text: &Text) -> Option<NodeHash> {
        hash_of(text, &mut self.scratch.entries).ok()
    }

    fn fields(&self, fields: impl IntoIterator<Item = Field<NodeHash>>) -> NodeHash {
        let mut map = FieldMap::new();
        for field in fields {
            if let Some(value) = field.value {
                map.put(field.key, value);
            }
        }
        map.finish()
    }

    fn named_map<'a>(
        &mut self,
        entries: impl IntoIterator<Item = Result<NamedEntry<'a>, Fault>>,
    ) -> Result<NodeHash, Fault> {
        self.scratch.entries.clear();
        for entry in entries {
            let entry = entry?;
            let value = self.leaf(entry.value);
            self.scratch.entries.push(MapEntry {
                key: identifier(entry.name),
                value,
            });
        }
        Ok(NodeHash::map(&mut self.scratch.entries))
    }

    fn list<T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        mut element: impl FnMut(&mut Self, T) -> Result<NodeHash, Fault>,
    ) -> Result<NodeHash, Fault> {
        let mut list = ListHash::new();
        for item in items {
            list = list.push(element(self, item)?);
        }
        Ok(list.finish())
    }
}
