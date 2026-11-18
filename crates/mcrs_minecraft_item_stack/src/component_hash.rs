use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;

use crate::Builtin;
use crate::HashError;
use crate::HashScratch;
use crate::NameLookup;
use crate::Profile;
use crate::entity_data::ENTITY_TYPE_KEY;
use crate::field_map::FieldMap;
use crate::node_hash::ListHash;
use crate::node_hash::MapEntry;
use crate::node_hash::NodeHash;
use crate::node_serializer::hash_of;
use crate::saved_form::Fault;
use crate::saved_form::saved_form;
use crate::saved_key::REMOVAL_MARKER;
use crate::saved_sink::Field;
use crate::saved_sink::KeyedEntry;
use crate::saved_sink::Leaf;
use crate::saved_sink::MapKey;
use crate::saved_sink::NamedEntry;
use crate::saved_sink::SavedSink;
use crate::saved_sink::TAG_MARKER;
use crate::saved_sink::WireOnly;

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

impl HashSink<'_> {
    fn entity_data_entries(
        &mut self,
        data: &NbtCompound,
        entity_type: ResourceLocation<'_>,
    ) -> Option<()> {
        for (key, tag) in &data.child_tags {
            let value = hash_of(tag, &mut self.scratch.entries).ok()?;
            self.scratch.entries.push(MapEntry {
                key: NodeHash::string(key),
                value,
            });
        }
        self.scratch.entries.push(MapEntry {
            key: NodeHash::string(ENTITY_TYPE_KEY),
            value: identifier(entity_type),
        });
        Some(())
    }
}

impl SavedSink for HashSink<'_> {
    type Node = NodeHash;

    const WIRE_ONLY: WireOnly = WireOnly::Refuse;

    fn leaf(&self, leaf: Leaf<'_>) -> NodeHash {
        match leaf {
            Leaf::Byte(value) => NodeHash::byte(value),
            Leaf::Int(value) => NodeHash::int(value),
            Leaf::Long(value) => NodeHash::long(value),
            Leaf::IntArray(values) => NodeHash::int_array(values),
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

    fn profile(&mut self, profile: &Profile) -> Option<NodeHash> {
        hash_of(profile, &mut self.scratch.entries).ok()
    }

    fn compound(&mut self, compound: &NbtCompound) -> Option<NodeHash> {
        hash_of(compound, &mut self.scratch.entries).ok()
    }

    fn tag(&mut self, tag: &NbtTag) -> Option<NodeHash> {
        hash_of(tag, &mut self.scratch.entries).ok()
    }

    fn entity_data(
        &mut self,
        data: &NbtCompound,
        entity_type: ResourceLocation<'_>,
    ) -> Option<NodeHash> {
        let start = self.scratch.entries.len();
        let hash = self
            .entity_data_entries(data, entity_type)
            .map(|()| NodeHash::map(self.scratch.entries.get_mut(start..).unwrap_or_default()));
        self.scratch.entries.truncate(start);
        hash
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
        let start = self.scratch.entries.len();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(fault) => {
                    self.scratch.entries.truncate(start);
                    return Err(fault);
                }
            };
            let value = self.leaf(entry.value);
            self.scratch.entries.push(MapEntry {
                key: identifier(entry.name),
                value,
            });
        }
        let hash = NodeHash::map(self.scratch.entries.get_mut(start..).unwrap_or_default());
        self.scratch.entries.truncate(start);
        Ok(hash)
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

    fn keyed_map<'a, T>(
        &mut self,
        items: impl IntoIterator<Item = T>,
        mut entry: impl FnMut(&mut Self, T) -> Result<KeyedEntry<'a, NodeHash>, Fault>,
    ) -> Result<NodeHash, Fault> {
        let start = self.scratch.entries.len();
        for item in items {
            let KeyedEntry { key, value } = match entry(self, item) {
                Ok(entry) => entry,
                Err(fault) => {
                    self.scratch.entries.truncate(start);
                    return Err(fault);
                }
            };
            let key = match key {
                MapKey::String(key) => NodeHash::string(key),
                MapKey::Name(name) => identifier(name),
                MapKey::Removal(name) => {
                    NodeHash::string_of_parts(&[REMOVAL_MARKER, name.namespace(), ":", name.path()])
                }
            };
            self.scratch.entries.push(MapEntry { key, value });
        }
        let hash = NodeHash::map(self.scratch.entries.get_mut(start..).unwrap_or_default());
        self.scratch.entries.truncate(start);
        Ok(hash)
    }
}
