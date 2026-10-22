use mcrs_minecraft_protocol::item::EncodeCtx;
use mcrs_minecraft_protocol::item::decode_component_value;
use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

use mcrs_minecraft_core::{Id, ResourceLocation};
use mcrs_minecraft_item::keys::DataComponentType;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::item;
use mcrs_minecraft_protocol::item::ctx::WireCtx;
use mcrs_minecraft_protocol::item::for_each_data_component;
use mcrs_minecraft_protocol::item::harness::Sample;
use mcrs_minecraft_protocol::item::harness::sample_registries;
use mcrs_minecraft_protocol::item::{ComponentPatch, ItemComponentValue, ItemDataComponent};
use mcrs_minecraft_registries::{IdSet, Key, Named, Registries, RegistryType, Resolve, Resolver};
use mcrs_minecraft_world::item::test_corpus;
use mcrs_minecraft_world::registry_load::test_registries_arc;

pub use crate::common::{hex, nbt_tree};

pub fn registries() -> &'static Registries {
    test_registries_arc()
}

pub fn ctx() -> WireCtx<'static> {
    WireCtx::new(registries())
}

pub fn block_ctx() -> WireCtx<'static> {
    ctx().with_blocks(&test_corpus().0)
}

pub fn named<T: Resolve + fmt::Debug>(value: &T) -> T::Named {
    value
        .name(registries())
        .unwrap_or_else(|e| panic!("{value:?}: name: {e}"))
}

pub fn resolved<T: Resolve>(value: T::Named) -> Result<T, String> {
    T::resolve(value, registries()).map_err(|e| e.to_string())
}

/// The samples are numbered against their own few entries; the wire numbers them as the loaded
/// registries do.
pub fn samples<T: Sample + Into<ItemComponentValue>>() -> Vec<ItemComponentValue> {
    T::samples()
        .into_iter()
        .map(|sample| in_registries(sample.into()))
        .collect()
}

fn in_registries(sample: ItemComponentValue) -> ItemComponentValue {
    let name = sample
        .name(sample_registries())
        .unwrap_or_else(|e| panic!("{sample:?}: name: {e}"));
    resolved(name).unwrap_or_else(|e| panic!("{sample:?}: resolve: {e}"))
}

fn key<R>(path: &str) -> Key<R> {
    Key::new(ResourceLocation::minecraft(path).unwrap())
}

pub fn entry<R: RegistryType>(path: &str) -> Id<R> {
    registries().id(&key(path)).unwrap()
}

pub fn tag_set<R: RegistryType>(path: &str) -> IdSet<R> {
    IdSet::Tag(registries().tag(&key(path)).unwrap())
}

pub fn one_set<R: RegistryType>(path: &str) -> IdSet<R> {
    list_set(&[path])
}

pub fn list_set<R: RegistryType>(paths: &[&str]) -> IdSet<R> {
    IdSet::List(paths.iter().map(|path| entry(path)).collect())
}

pub fn json_of<T: Resolve<Named: Serialize> + fmt::Debug>(value: &T) -> String {
    serde_json::to_string(&named(value)).unwrap()
}

pub fn read_json<T: Resolve<Named: DeserializeOwned>>(json: &str) -> Result<T, String> {
    resolved(serde_json::from_str::<T::Named>(json).map_err(|e| e.to_string())?)
}

pub fn persistent_json(value: &ItemComponentValue) -> String {
    let mut out = Vec::new();
    let mut s = serde_json::Serializer::new(&mut out);
    named(value)
        .serialize_value(&mut s)
        .expect("serialize_value");
    String::from_utf8(out).unwrap()
}

pub fn json_value(value: &ItemComponentValue) -> serde_json::Value {
    serde_json::from_str(&persistent_json(value)).unwrap()
}

pub fn from_json(kind: DataComponentType, json: &str) -> ItemComponentValue {
    try_from_json(kind, json).expect("deserialize_value")
}

pub fn try_from_json(kind: DataComponentType, json: &str) -> Result<ItemComponentValue, String> {
    let mut d = serde_json::Deserializer::from_str(json);
    resolved(ItemComponentValue::deserialize_value(kind, &mut d).map_err(|e| e.to_string())?)
}

pub fn wire(ctx: &WireCtx<'_>, value: &ItemComponentValue) -> Vec<u8> {
    let mut out = Vec::new();
    value
        .encode_ctx(ctx, &mut out)
        .unwrap_or_else(|e| panic!("{:?}: encode {value:?}: {e}", value.kind()));
    out
}

pub fn decode(ctx: &WireCtx<'_>, kind: DataComponentType, bytes: &[u8]) -> ItemComponentValue {
    let mut r = bytes;
    let value = decode_component_value(kind, ctx, &mut r)
        .unwrap_or_else(|e| panic!("{kind:?}: decode {bytes:02x?}: {e}"));
    assert!(r.is_empty(), "{kind:?}: {} trailing bytes", r.len());
    value
}

pub fn custom_data() -> item::CustomData {
    let mut tag = NbtCompound::new();
    tag.put_int("x", 100000);
    tag.put_string("name", "mcrs".into());
    item::CustomData(tag)
}

pub fn check_samples<T: Sample + ItemDataComponent + Into<ItemComponentValue>>() {
    let lookup = ctx();
    let samples = T::samples();
    assert!(
        !samples.is_empty(),
        "{} is implemented but has no samples",
        T::KIND.location()
    );
    for sample in samples {
        let value = in_registries(sample.clone().into());
        let kind = value.kind();

        if kind.is_persistent() {
            let json = persistent_json(&value);
            let back = from_json(kind, &json);
            assert_eq!(back, value, "{kind:?}: JSON round trip of {json}");
            assert_eq!(persistent_json(&back), json, "JSON is stable for {kind:?}");

            let mut nbt = Vec::new();
            mcrs_minecraft_nbt::to_bytes_unnamed(&named(&value), &mut nbt).expect("to nbt");
            let back = from_nbt(kind, &nbt);
            assert_eq!(back, value, "NBT round trip of {kind:?}");
            check_tag_widths(kind, &nbt, &sample.nbt_tags());

            if kind.is_unit() {
                assert_eq!(json, "{}", "{kind:?} is a unit kind");
            }
        } else {
            let mut out = Vec::new();
            let mut s = serde_json::Serializer::new(&mut out);
            let value_named = named(&value);
            assert!(
                value_named.serialize_value(&mut s).is_err(),
                "{kind:?} is transient"
            );
            let patch = ComponentPatch::<Named> {
                added: vec![value_named],
                removed: vec![],
            };
            assert_eq!(serde_json::to_string(&patch).unwrap(), "{}");
            let named = format!("{{\"{}\":{{}}}}", kind.location());
            let error = serde_json::from_str::<ComponentPatch<Named>>(&named).unwrap_err();
            assert!(
                error.to_string().contains("is not a persistent component"),
                "{error}"
            );
        }

        let bytes = wire(&lookup, &value);
        assert_eq!(
            decode(&lookup, kind, &bytes),
            value,
            "wire round trip of {kind:?}"
        );
        if kind.is_unit() {
            let expected: &[u8] = if kind.is_nbt_wire() {
                &[0x0A, 0x00]
            } else {
                &[]
            };
            assert_eq!(bytes, expected, "unit wire form of {kind:?}");
        }
    }
}

fn check_tag_widths(kind: DataComponentType, bytes: &[u8], expected: &[(&str, u8)]) {
    let root: NbtTag =
        mcrs_minecraft_nbt::from_bytes_unnamed(&mut std::io::Cursor::new(bytes)).expect("nbt tree");
    for (path, id) in expected {
        let mut tag = &root;
        for key in path.split('.').filter(|key| !key.is_empty()) {
            tag = tag
                .extract_compound()
                .and_then(|compound| compound.get(key))
                .unwrap_or_else(|| panic!("{kind:?}: no tag at {path}"));
        }
        assert_eq!(tag.get_type_id(), *id, "{kind:?}: tag id at '{path}'");
    }
}

pub fn from_nbt(kind: DataComponentType, bytes: &[u8]) -> ItemComponentValue {
    let mut cursor = std::io::Cursor::new(bytes);
    let mut d = mcrs_minecraft_nbt::deserializer::Deserializer::new(&mut cursor, false);
    let value = ItemComponentValue::deserialize_value(kind, &mut d).expect("from nbt");
    assert_eq!(
        cursor.position() as usize,
        bytes.len(),
        "NBT fully read for {kind:?}"
    );
    resolved(value).expect("resolve from nbt")
}

macro_rules! samples_round_trip {
    ($($kind:ident : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        #[test]
        fn every_kinds_samples_round_trip() {
            $(check_samples::<mcrs_minecraft_protocol::item::$ty>();)*
        }
    };
}

for_each_data_component!(samples_round_trip);
