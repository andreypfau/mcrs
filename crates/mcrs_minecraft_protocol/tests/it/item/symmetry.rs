use std::sync::Arc;

use mcrs_minecraft_core::{Id, rl};
use mcrs_minecraft_item::keys::DataComponentType;
use mcrs_minecraft_protocol::item::ctx::WireCtx;
use mcrs_minecraft_protocol::item::for_each_data_component;
use mcrs_minecraft_protocol::item::{
    BannerPattern, BundleContents, ChargedProjectiles, ComponentPatch, Container, DecodeCtx,
    EncodeCtx, ItemComponentValue, ItemStackValue, PotDecorations, ProtoStack, RawStack,
    SulfurCubeContent, Template, UseRemainder,
};
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_registries::{Named, Ref, RegistryOf};
use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use rand::{RngExt, SeedableRng};

use crate::item::harness::{
    ctx, decode, entry, from_json, from_nbt, json_of, named, persistent_json, read_json, resolved,
    wire,
};
use mcrs_minecraft_item::keys::Item;

const ITERATIONS: usize = 256;
const ITEMS: [&str; 4] = ["stone", "diamond_sword", "apple", "bundle"];

// chisle: the pool is every kind's Sample::samples(); only the nested kinds
// are generated fresh, so scalar kinds are exercised in combination, not with
// random field values. Upgrade path: a Gen impl per value type.
macro_rules! sample_pool {
    ($($kind:ident : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        fn sample_pool() -> Vec<(DataComponentType, Vec<ItemComponentValue>)> {
            vec![$((
                DataComponentType::$kind,
                crate::item::harness::samples::<mcrs_minecraft_protocol::item::$ty>(),
            )),*]
        }
    };
}

for_each_data_component!(sample_pool);

struct Gen {
    rng: StdRng,
    pool: Vec<(DataComponentType, Vec<ItemComponentValue>)>,
}

impl Gen {
    fn new(seed: u64) -> Self {
        Gen {
            rng: StdRng::seed_from_u64(seed),
            pool: sample_pool(),
        }
    }

    fn item(&mut self) -> Id<Item> {
        let name = *ITEMS.choose(&mut self.rng).unwrap();
        entry(name)
    }

    fn template(&mut self, depth: u8) -> Template {
        let item = self.item();
        let count = self.rng.random_range(1..=99);
        Template::new(item, count, self.patch(depth, true)).unwrap()
    }

    fn templates(&mut self, depth: u8, max: usize) -> Vec<Template> {
        let n = self.rng.random_range(0..=max);
        (0..n).map(|_| self.template(depth)).collect()
    }

    fn nested(&mut self, kind: DataComponentType, depth: u8) -> Option<ItemComponentValue> {
        let depth = depth.checked_sub(1)?;
        Some(match kind {
            DataComponentType::UseRemainder => UseRemainder(self.template(depth)).into(),
            DataComponentType::SulfurCubeContent => SulfurCubeContent(self.template(depth)).into(),
            DataComponentType::BundleContents => BundleContents(self.templates(depth, 4)).into(),
            DataComponentType::ChargedProjectiles => {
                ChargedProjectiles::new(self.templates(depth, 3))
                    .unwrap()
                    .into()
            }
            DataComponentType::PotDecorations => {
                let mut side = || {
                    self.rng
                        .random_bool(0.5)
                        .then(|| Box::new(self.template(depth)))
                };
                PotDecorations {
                    back: side(),
                    left: side(),
                    right: side(),
                    front: side(),
                }
                .into()
            }
            DataComponentType::Container => {
                let n = self.rng.random_range(0..=5);
                let mut slots: Vec<Option<Template>> = (0..n)
                    .map(|_| self.rng.random_bool(0.6).then(|| self.template(depth)))
                    .collect();
                // The persistent form lists occupied slots only, so a trailing
                // empty slot would not survive the JSON round trip.
                if let Some(last @ None) = slots.last_mut() {
                    *last = Some(self.template(depth));
                }
                Container::new(slots).unwrap().into()
            }
            _ => return None,
        })
    }

    fn value(&mut self, kind: DataComponentType, depth: u8) -> ItemComponentValue {
        if self.rng.random_bool(0.5)
            && let Some(value) = self.nested(kind, depth)
        {
            return value;
        }
        let samples = &self.pool.iter().find(|(k, _)| *k == kind).unwrap().1;
        samples.choose(&mut self.rng).unwrap().clone()
    }

    fn patch(&mut self, depth: u8, persistent_only: bool) -> ComponentPatch {
        let kinds: Vec<DataComponentType> = DataComponentType::ALL
            .iter()
            .copied()
            .filter(|kind| !persistent_only || kind.is_persistent())
            .collect();
        let added = self.rng.random_range(0..=6);
        let removed = self.rng.random_range(0..=3);
        let mut patch = ComponentPatch::EMPTY;
        for kind in kinds.sample(&mut self.rng, added + removed) {
            if patch.added.len() < added {
                patch.set_value(self.value(*kind, depth));
            } else {
                patch.remove(*kind);
            }
        }
        patch
    }

    fn slot(&mut self, depth: u8) -> ProtoStack {
        let id = self.item();
        ProtoStack::new(id, self.rng.random_range(1..=99), self.patch(depth, false))
    }
}

fn persistent_round_trips(value: &ItemComponentValue) {
    let kind = value.kind();
    let json = persistent_json(value);
    assert_eq!(
        &from_json(kind, &json),
        value,
        "{kind:?}: JSON round trip of {json}"
    );

    let mut nbt = Vec::new();
    mcrs_minecraft_nbt::to_bytes_unnamed(&named(value), &mut nbt)
        .unwrap_or_else(|e| panic!("{kind:?}: to NBT {value:?}: {e}"));
    assert_eq!(&from_nbt(kind, &nbt), value, "{kind:?}: NBT round trip");
}

fn raw_stack_round_trip(lookup: &WireCtx<'_>, slot: &ProtoStack) {
    let mut wire = Vec::new();
    slot.encode_ctx(lookup, &mut wire)
        .unwrap_or_else(|e| panic!("encode {slot:?}: {e}"));
    let mut r = &wire[..];
    let raw = RawStack::decode(&mut r).unwrap_or_else(|e| panic!("measure {wire:02x?}: {e}"));
    assert!(r.is_empty(), "{} trailing bytes after {slot:?}", r.len());
    assert_eq!(
        &raw.0[..],
        &wire[..],
        "raw stack keeps the bytes of {slot:?}"
    );
    let back = raw
        .resolve(lookup)
        .unwrap_or_else(|e| panic!("resolve {wire:02x?} of {slot:?}: {e}"));
    assert_eq!(&back, slot, "raw stack round trip");
}

fn check_kind(kind: DataComponentType, iterations: usize) {
    let lookup = ctx();
    let mut generator = Gen::new(kind as u64);
    for _ in 0..iterations {
        let value = generator.value(kind, 2);
        assert_eq!(
            decode(&lookup, kind, &wire(&lookup, &value)),
            value,
            "{kind:?}: wire round trip"
        );
        if kind.is_persistent() {
            persistent_round_trips(&value);
        }
        let mut slot = generator.slot(1);
        slot.components.set_value(value);
        raw_stack_round_trip(&lookup, &slot);
    }
}

mod exhaustive {
    use mcrs_minecraft_item::keys::DataComponentType;

    #[test]
    fn random_values_of_every_kind_round_trip() {
        for &kind in DataComponentType::ALL.iter() {
            super::check_kind(kind, super::ITERATIONS);
        }
    }
}

#[test]
fn random_stacks_round_trip_on_the_wire_and_in_json_and_nbt() {
    let lookup = ctx();
    let mut generator = Gen::new(0x5107);
    for _ in 0..ITERATIONS {
        let slot = generator.slot(2);
        raw_stack_round_trip(&lookup, &slot);

        let template = generator.template(2);
        let json = json_of(&template);
        let back: Template =
            read_json(&json).unwrap_or_else(|e| panic!("template from JSON {json}: {e}"));
        assert_eq!(back, template, "template JSON round trip of {json}");
        assert_eq!(json_of(&back), json, "template JSON is stable");

        let mut nbt = Vec::new();
        mcrs_minecraft_nbt::to_bytes_unnamed(&named(&template.0), &mut nbt).unwrap();
        let back: ItemStackValue<Named> =
            mcrs_minecraft_nbt::from_bytes_unnamed(std::io::Cursor::new(&nbt))
                .unwrap_or_else(|e| panic!("stack from NBT {nbt:02x?} of {template:?}: {e}"));
        assert_eq!(
            resolved::<ItemStackValue>(back).unwrap(),
            template.0,
            "stack NBT round trip"
        );

        let mut wire = Vec::new();
        template.encode_ctx(&lookup, &mut wire).unwrap();
        let mut r = &wire[..];
        let back = Template::decode_ctx(&lookup, &mut r)
            .unwrap_or_else(|e| panic!("template from wire {wire:02x?}: {e}"));
        assert!(r.is_empty());
        assert_eq!(back, template, "template wire round trip");
    }
}

#[test]
fn inline_holders_are_a_zero_prefix_and_a_bare_prefix_fails() {
    type Pattern = Ref<RegistryOf<BannerPattern>, BannerPattern>;
    let lookup = ctx();
    let direct = Pattern::Inline(Arc::new(BannerPattern {
        asset_id: rl!("minecraft:globe").to_arc(),
        translation_key: "block.minecraft.banner.globe".into(),
    }));
    let mut wire = Vec::new();
    direct.encode_ctx(&lookup, &mut wire).unwrap();
    assert_eq!(wire[0], 0);
    let mut r = &wire[..];
    assert_eq!(Pattern::decode_ctx(&lookup, &mut r).unwrap(), direct);
    assert!(r.is_empty());

    assert!(Pattern::decode_ctx(&lookup, &mut &[0u8][..]).is_err());

    let mut unknown = Vec::new();
    VarInt(ITEMS.len() as i32 + 100)
        .encode(&mut unknown)
        .unwrap();
    assert!(Pattern::decode_ctx(&lookup, &mut &unknown[..]).is_err());
}
