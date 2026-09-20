use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_protocol::for_each_data_component;
use mcrs_minecraft_protocol::item::harness::Sample;
use mcrs_minecraft_protocol::item::{
    BannerPattern, BundleContents, ChargedProjectiles, ComponentPatch, Container, DecodeCtx,
    EncodeCtx, Holder, ItemComponentKind, ItemComponentValue, ItemStackValue, PotDecorations,
    RawStack, Slot, SulfurCubeContent, Template, UseRemainder,
};
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_registry::ItemId;
use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use rand::{RngExt, SeedableRng};

use crate::harness::{PersistentValue, TestLookup, from_json, persistent_json};

const ITERATIONS: usize = 256;
const ITEMS: [(&str, u16); 4] = [
    ("stone", 1),
    ("diamond_sword", 2),
    ("apple", 3),
    ("bundle", 4),
];

// ponytail: the pool is every kind's Sample::samples(); only the nested kinds
// are generated fresh, so scalar kinds are exercised in combination, not with
// random field values. Upgrade path: a Gen impl per value type.
macro_rules! sample_pool {
    ($($id:literal $name:literal : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        fn sample_pool() -> Vec<(ItemComponentKind, Vec<ItemComponentValue>)> {
            vec![$((
                ItemComponentKind::$ty,
                <mcrs_minecraft_protocol::item::$ty as Sample>::samples()
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            )),*]
        }
    };
}

for_each_data_component!(sample_pool);

struct Gen {
    rng: StdRng,
    pool: Vec<(ItemComponentKind, Vec<ItemComponentValue>)>,
}

impl Gen {
    fn new(seed: u64) -> Self {
        Gen {
            rng: StdRng::seed_from_u64(seed),
            pool: sample_pool(),
        }
    }

    fn item(&mut self) -> (ResourceKey<mcrs_minecraft_protocol::item::ItemReg>, ItemId) {
        let (name, id) = *ITEMS.choose(&mut self.rng).unwrap();
        (
            ResourceKey::from_location(ResourceLocation::minecraft(name)),
            ItemId(id),
        )
    }

    fn template(&mut self, depth: u8) -> Template {
        let (item, _) = self.item();
        let count = self.rng.random_range(1..=99);
        Template::new(item, count, self.patch(depth, true)).unwrap()
    }

    fn templates(&mut self, depth: u8, max: usize) -> Vec<Template> {
        let n = self.rng.random_range(0..=max);
        (0..n).map(|_| self.template(depth)).collect()
    }

    fn nested(&mut self, kind: ItemComponentKind, depth: u8) -> Option<ItemComponentValue> {
        let depth = depth.checked_sub(1)?;
        Some(match kind {
            ItemComponentKind::UseRemainder => UseRemainder(self.template(depth)).into(),
            ItemComponentKind::SulfurCubeContent => SulfurCubeContent(self.template(depth)).into(),
            ItemComponentKind::BundleContents => BundleContents(self.templates(depth, 4)).into(),
            ItemComponentKind::ChargedProjectiles => {
                ChargedProjectiles::new(self.templates(depth, 3))
                    .unwrap()
                    .into()
            }
            ItemComponentKind::PotDecorations => {
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
            ItemComponentKind::Container => {
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

    fn value(&mut self, kind: ItemComponentKind, depth: u8) -> ItemComponentValue {
        if self.rng.random_bool(0.5)
            && let Some(value) = self.nested(kind, depth)
        {
            return value;
        }
        let samples = &self.pool.iter().find(|(k, _)| *k == kind).unwrap().1;
        samples.choose(&mut self.rng).unwrap().clone()
    }

    fn patch(&mut self, depth: u8, persistent_only: bool) -> ComponentPatch {
        let kinds: Vec<ItemComponentKind> = ItemComponentKind::ALL
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

    fn slot(&mut self, depth: u8) -> Slot {
        let (_, id) = self.item();
        Slot::new(id, self.rng.random_range(1..=99), self.patch(depth, false))
    }
}

fn wire_round_trip(lookup: &TestLookup, value: &ItemComponentValue) {
    let kind = value.kind();
    let mut wire = Vec::new();
    value
        .encode_ctx_value(lookup, &mut wire)
        .unwrap_or_else(|e| panic!("{kind}: encode {value:?}: {e}"));
    let mut r = &wire[..];
    let back = ItemComponentValue::decode_ctx_value(kind, lookup, &mut r)
        .unwrap_or_else(|e| panic!("{kind}: decode {wire:02x?} of {value:?}: {e}"));
    assert!(
        r.is_empty(),
        "{kind}: {} trailing bytes after {value:?}",
        r.len()
    );
    assert_eq!(&back, value, "{kind}: wire round trip");
}

fn persistent_round_trips(value: &ItemComponentValue) {
    let kind = value.kind();
    let json = persistent_json(value);
    assert_eq!(
        &from_json(kind, &json),
        value,
        "{kind}: JSON round trip of {json}"
    );

    let mut nbt = Vec::new();
    mcrs_minecraft_nbt::to_bytes_unnamed(&PersistentValue(value), &mut nbt)
        .unwrap_or_else(|e| panic!("{kind}: to NBT {value:?}: {e}"));
    let mut cursor = std::io::Cursor::new(&nbt);
    let mut d = mcrs_minecraft_nbt::deserializer::Deserializer::new(&mut cursor, false);
    let back = ItemComponentValue::deserialize_value(kind, &mut d)
        .unwrap_or_else(|e| panic!("{kind}: from NBT {nbt:02x?} of {value:?}: {e}"));
    assert_eq!(
        cursor.position() as usize,
        nbt.len(),
        "{kind}: NBT fully read"
    );
    assert_eq!(&back, value, "{kind}: NBT round trip");
}

fn raw_stack_round_trip(lookup: &TestLookup, slot: &Slot) {
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

fn check_kind(kind: ItemComponentKind) {
    let lookup = TestLookup::new();
    let mut generator = Gen::new(kind.wire_id() as u64);
    for _ in 0..ITERATIONS {
        let value = generator.value(kind, 2);
        wire_round_trip(&lookup, &value);
        if kind.is_persistent() {
            persistent_round_trips(&value);
        }
        let mut slot = generator.slot(1);
        slot.components.set_value(value);
        raw_stack_round_trip(&lookup, &slot);
    }
}

mod kinds {
    macro_rules! kind_tests {
        ($($id:literal $name:literal : $ty:ident [$($flag:ident),*]),* $(,)?) => {$(
            #[allow(non_snake_case)]
            mod $ty {
                #[test]
                fn random_values_round_trip() {
                    super::super::check_kind(mcrs_minecraft_protocol::item::ItemComponentKind::$ty);
                }
            }
        )*};
    }

    mcrs_minecraft_protocol::for_each_data_component!(kind_tests);
}

#[test]
fn random_stacks_round_trip_on_the_wire_and_in_json_and_nbt() {
    let lookup = TestLookup::new();
    let mut generator = Gen::new(0x5107);
    for _ in 0..ITERATIONS {
        let slot = generator.slot(2);
        raw_stack_round_trip(&lookup, &slot);

        let template = generator.template(2);
        let json = serde_json::to_string(&template).unwrap();
        let back: Template = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("template from JSON {json}: {e}"));
        assert_eq!(back, template, "template JSON round trip of {json}");
        assert_eq!(
            serde_json::to_string(&back).unwrap(),
            json,
            "template JSON is stable"
        );

        let mut nbt = Vec::new();
        mcrs_minecraft_nbt::to_bytes_unnamed(&template.0, &mut nbt).unwrap();
        let back: ItemStackValue =
            mcrs_minecraft_nbt::from_bytes_unnamed(std::io::Cursor::new(&nbt))
                .unwrap_or_else(|e| panic!("stack from NBT {nbt:02x?} of {template:?}: {e}"));
        assert_eq!(back, template.0, "stack NBT round trip");

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
    let lookup = TestLookup::new();
    let direct = Holder::Direct(BannerPattern {
        asset_id: ResourceLocation::minecraft("globe"),
        translation_key: "block.minecraft.banner.globe".into(),
    });
    let mut wire = Vec::new();
    direct.encode_ctx(&lookup, &mut wire).unwrap();
    assert_eq!(wire[0], 0);
    let mut r = &wire[..];
    assert_eq!(
        Holder::<BannerPattern>::decode_ctx(&lookup, &mut r).unwrap(),
        direct
    );
    assert!(r.is_empty());

    assert!(Holder::<BannerPattern>::decode_ctx(&lookup, &mut &[0u8][..]).is_err());

    let mut unknown = Vec::new();
    VarInt(ITEMS.len() as i32 + 100)
        .encode(&mut unknown)
        .unwrap();
    assert!(Holder::<BannerPattern>::decode_ctx(&lookup, &mut &unknown[..]).is_err());
}
