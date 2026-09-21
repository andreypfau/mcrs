use mcrs_minecraft_protocol::item::EncodeCtx;
use mcrs_minecraft_protocol::item::decode_component_value;
use std::collections::HashMap;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::for_each_data_component;
use mcrs_minecraft_protocol::item::harness::Sample;
use mcrs_minecraft_protocol::item::{
    ComponentPatch, ItemComponentKind, ItemComponentValue, ItemDataComponent,
};
use mcrs_minecraft_registry::RegistryLookup;

/// Every name a sample may reference, with the ids a capture session would
/// have handed out.
pub struct TestLookup {
    by_name: HashMap<&'static str, HashMap<ResourceLocation, u32>>,
    by_id: HashMap<&'static str, Vec<Option<ResourceLocation>>>,
    block_states: Vec<TestBlockState>,
}

struct TestBlockState {
    id: u32,
    block: ResourceLocation,
    properties: Vec<(String, String)>,
    default: bool,
}

impl TestLookup {
    pub fn new() -> Self {
        let mut lookup = TestLookup {
            by_name: HashMap::new(),
            by_id: HashMap::new(),
            block_states: Vec::new(),
        };
        lookup.registry(
            "item",
            &["air", "stone", "diamond_sword", "apple", "bundle"],
        );
        lookup.registry(
            "sound_event",
            &[
                "entity.item.break",
                "item.armor.equip_generic",
                "item.shears.snip",
                "item.armor.equip_iron",
                "entity.generic.eat",
            ],
        );
        lookup.registry("mob_effect", &["speed", "slowness", "haste"]);
        lookup.registry("enchantment", &["sharpness", "unbreaking"]);
        lookup.registry("damage_type", &["in_fire", "lava"]);
        lookup.registry("block", &["stone", "dirt"]);
        lookup.registry("entity_type", &["zombie", "pig"]);
        lookup.registry("block_entity_type", &["chest", "sign"]);
        lookup.registry("potion", &["water", "swiftness"]);
        lookup.registry("attribute", &["armor", "attack_damage"]);
        lookup.registry("banner_pattern", &["globe", "creeper"]);
        lookup.registry("block_transformer", &["axe", "shovel"]);
        lookup.registry("villager_type", &["plains", "desert"]);
        lookup.registry("wolf_variant", &["pale", "ashen"]);
        lookup.registry("wolf_sound_variant", &["classic", "big"]);
        lookup.registry("pig_variant", &["temperate", "cold"]);
        lookup.registry("pig_sound_variant", &["classic", "mini"]);
        lookup.registry("cow_variant", &["temperate", "warm"]);
        lookup.registry("cow_sound_variant", &["classic", "moody"]);
        lookup.registry("chicken_variant", &["temperate", "cold"]);
        lookup.registry("chicken_sound_variant", &["classic", "picky"]);
        lookup.registry("zombie_nautilus_variant", &["temperate", "warm"]);
        lookup.registry("frog_variant", &["temperate", "warm"]);
        lookup.registry("cat_variant", &["tabby", "jellie"]);
        lookup.registry("cat_sound_variant", &["classic", "royal"]);
        lookup.registry("decorated_pot_pattern", &["angler", "skull"]);
        lookup.registry("trim_material", &["amethyst"]);
        lookup.registry("trim_pattern", &["coast"]);
        lookup.registry("instrument", &["ponder_goat_horn"]);
        lookup.registry("jukebox_song", &["pigstep"]);
        lookup.registry("painting_variant", &["kebab"]);
        lookup
    }

    fn registry(&mut self, name: &'static str, paths: &[&str]) {
        let entries: Vec<(&str, u32)> = paths
            .iter()
            .enumerate()
            .map(|(id, p)| (*p, id as u32))
            .collect();
        self.registry_with_ids(name, &entries);
    }

    pub fn registry_with_ids(&mut self, name: &'static str, entries: &[(&str, u32)]) {
        let mut by_id = Vec::new();
        let mut by_name = HashMap::new();
        for (path, id) in entries {
            let location = ResourceLocation::minecraft(path);
            if by_id.len() <= *id as usize {
                by_id.resize(*id as usize + 1, None);
            }
            by_id[*id as usize] = Some(location.clone());
            by_name.insert(location, *id);
        }
        self.by_name.insert(name, by_name);
        self.by_id.insert(name, by_id);
    }

    pub fn block_state(&mut self, id: u32, block: &str, properties: &[(&str, &str)]) {
        self.add_block_state(id, block, properties, false);
    }

    pub fn default_block_state(&mut self, id: u32, block: &str, properties: &[(&str, &str)]) {
        self.add_block_state(id, block, properties, true);
    }

    fn add_block_state(
        &mut self,
        id: u32,
        block: &str,
        properties: &[(&str, &str)],
        default: bool,
    ) {
        self.block_states.push(TestBlockState {
            id,
            block: ResourceLocation::minecraft(block),
            properties: properties
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
            default,
        });
    }
}

impl RegistryLookup for TestLookup {
    fn id(&self, registry: &str, name: &ResourceLocation) -> Option<u32> {
        self.by_name.get(registry)?.get(name).copied()
    }

    fn name(&self, registry: &str, id: u32) -> Option<&ResourceLocation> {
        self.by_id.get(registry)?.get(id as usize)?.as_ref()
    }

    fn block_state_id(&self, block: &ResourceLocation, properties: &[(&str, &str)]) -> Option<u32> {
        let states: Vec<&TestBlockState> = self
            .block_states
            .iter()
            .filter(|state| &state.block == block)
            .collect();
        let default = states.iter().find(|state| state.default)?;
        let wanted: Vec<(&str, &str)> = default
            .properties
            .iter()
            .map(|(name, value)| {
                let given = properties
                    .iter()
                    .find(|(k, v)| {
                        k == name
                            && states
                                .iter()
                                .any(|s| s.properties.contains(&(k.to_string(), v.to_string())))
                    })
                    .map(|(_, v)| *v);
                (name.as_str(), given.unwrap_or(value.as_str()))
            })
            .collect();
        states
            .iter()
            .find(|state| {
                state
                    .properties
                    .iter()
                    .all(|(k, v)| wanted.contains(&(k.as_str(), v.as_str())))
            })
            .map(|state| state.id)
    }

    fn block_state(&self, id: u32) -> Option<(ResourceLocation, Vec<(String, String)>)> {
        let state = self.block_states.iter().find(|state| state.id == id)?;
        let properties = if state.default {
            Vec::new()
        } else {
            state.properties.clone()
        };
        Some((state.block.clone(), properties))
    }
}

pub fn persistent_json(value: &ItemComponentValue) -> String {
    let mut out = Vec::new();
    let mut s = serde_json::Serializer::new(&mut out);
    value.serialize_value(&mut s).expect("serialize_value");
    String::from_utf8(out).unwrap()
}

pub fn from_json(kind: ItemComponentKind, json: &str) -> ItemComponentValue {
    let mut d = serde_json::Deserializer::from_str(json);
    ItemComponentValue::deserialize_value(kind, &mut d).expect("deserialize_value")
}

pub fn check_samples<T: Sample + ItemDataComponent + Into<ItemComponentValue>>() {
    let lookup = TestLookup::new();
    let samples = T::samples();
    assert!(
        !samples.is_empty(),
        "{} is implemented but has no samples",
        T::KIND.id()
    );
    for sample in samples {
        let value: ItemComponentValue = sample.clone().into();
        let kind = value.kind();

        if kind.is_persistent() {
            let json = persistent_json(&value);
            let back = from_json(kind, &json);
            assert_eq!(back, value, "JSON round trip of {json}");
            assert_eq!(persistent_json(&back), json, "JSON is stable for {kind}");

            let mut nbt = Vec::new();
            mcrs_minecraft_nbt::to_bytes_unnamed(&PersistentValue(&value), &mut nbt)
                .expect("to nbt");
            let back = from_nbt(kind, &nbt);
            assert_eq!(back, value, "NBT round trip of {kind}");
            check_tag_widths(kind, &nbt, &sample.nbt_tags());

            if kind.is_unit() {
                assert_eq!(json, "{}", "{kind} is a unit kind");
            }
        } else {
            let mut out = Vec::new();
            let mut s = serde_json::Serializer::new(&mut out);
            assert!(
                value.serialize_value(&mut s).is_err(),
                "{kind} is transient"
            );
            let patch = ComponentPatch {
                added: vec![value.clone()],
                removed: vec![],
            };
            assert_eq!(serde_json::to_string(&patch).unwrap(), "{}");
            let named = format!("{{\"{}\":{{}}}}", kind.id());
            let error = serde_json::from_str::<ComponentPatch>(&named).unwrap_err();
            assert!(
                error.to_string().contains("is not a persistent component"),
                "{error}"
            );
        }

        let mut wire = Vec::new();
        value
            .encode_ctx(&lookup, &mut wire)
            .expect("encode_ctx_value");
        let mut r = &wire[..];
        let back = decode_component_value(kind, &lookup, &mut r).expect("decode_ctx_value");
        assert!(r.is_empty(), "{} trailing bytes after {kind}", r.len());
        assert_eq!(back, value, "wire round trip of {kind}");
        if kind.is_unit() {
            let expected: &[u8] = if kind.is_nbt_wire() {
                &[0x0A, 0x00]
            } else {
                &[]
            };
            assert_eq!(wire, expected, "unit wire form of {kind}");
        }
    }
}

pub struct PersistentValue<'a>(pub &'a ItemComponentValue);

impl serde::Serialize for PersistentValue<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize_value(s)
    }
}

fn check_tag_widths(kind: ItemComponentKind, bytes: &[u8], expected: &[(&str, u8)]) {
    let root: NbtTag =
        mcrs_minecraft_nbt::from_bytes_unnamed(&mut std::io::Cursor::new(bytes)).expect("nbt tree");
    for (path, id) in expected {
        let mut tag = &root;
        for key in path.split('.').filter(|key| !key.is_empty()) {
            tag = tag
                .extract_compound()
                .and_then(|compound| compound.get(key))
                .unwrap_or_else(|| panic!("{kind}: no tag at {path}"));
        }
        assert_eq!(tag.get_type_id(), *id, "{kind}: tag id at '{path}'");
    }
}

fn from_nbt(kind: ItemComponentKind, bytes: &[u8]) -> ItemComponentValue {
    let mut cursor = std::io::Cursor::new(bytes);
    let mut d = mcrs_minecraft_nbt::deserializer::Deserializer::new(&mut cursor, false);
    let value = ItemComponentValue::deserialize_value(kind, &mut d).expect("from nbt");
    assert_eq!(
        cursor.position() as usize,
        bytes.len(),
        "NBT fully read for {kind}"
    );
    value
}

macro_rules! kind_tests {
    ($($id:literal $name:literal : $ty:ident [$($flag:ident),*]),* $(,)?) => {$(
        #[allow(non_snake_case)]
        mod $ty {
            #[test]
            fn samples_round_trip() {
                super::check_samples::<mcrs_minecraft_protocol::item::$ty>();
            }
        }
    )*};
}

for_each_data_component!(kind_tests);
