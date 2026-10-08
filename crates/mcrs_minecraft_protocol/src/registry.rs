use crate::{Decode, Encode, VarInt};
use anyhow::Context;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_registry::{Id, Registered, Registry};
use std::borrow::Cow;
use std::io::Write;

macro_rules! static_registry_wire {
    ($ty:ty, $what:literal) => {
        impl crate::Encode for $ty {
            fn encode(&self, w: impl std::io::Write) -> anyhow::Result<()> {
                crate::registry::encode_registry_id(*self as u16, w)
            }
        }

        impl crate::Decode<'_> for $ty {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                let id = crate::registry::decode_registry_id(r)?;
                <$ty>::from_protocol_id(id)
                    .ok_or_else(|| anyhow::anyhow!(concat!("unknown ", $what, " {}"), id))
            }
        }
    };
}

pub(crate) use static_registry_wire;

macro_rules! world_registry_wire {
    ($($ty:ty),+ $(,)?) => {$(
        impl crate::Encode for Id<$ty> {
            fn encode(&self, w: impl std::io::Write) -> anyhow::Result<()> {
                crate::registry::encode_registry_id(self.number(), w)
            }
        }

        impl crate::Decode<'_> for Id<$ty> {
            fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
                crate::registry::decode_world_registry_id(r)
            }
        }
    )+};
}

static_registry_wire!(mcrs_minecraft_entity::keys::EntityType, "entity type");
static_registry_wire!(mcrs_minecraft_item::keys::MenuType, "menu type");
static_registry_wire!(mcrs_minecraft_entity::keys::Attribute, "attribute");
static_registry_wire!(
    mcrs_minecraft_block::keys::BlockEntityType,
    "block entity type"
);

world_registry_wire!(
    mcrs_minecraft_dimension::DimensionType,
    mcrs_minecraft_entity::variant::CatSoundVariant,
    mcrs_minecraft_entity::variant::CatVariant,
    mcrs_minecraft_entity::variant::ChickenSoundVariant,
    mcrs_minecraft_entity::variant::ChickenVariant,
    mcrs_minecraft_entity::variant::CowSoundVariant,
    mcrs_minecraft_entity::variant::CowVariant,
    mcrs_minecraft_entity::variant::FrogVariant,
    mcrs_minecraft_entity::variant::PigSoundVariant,
    mcrs_minecraft_entity::variant::PigVariant,
    mcrs_minecraft_entity::variant::WolfSoundVariant,
    mcrs_minecraft_entity::variant::WolfVariant,
    mcrs_minecraft_entity::variant::ZombieNautilusVariant,
    mcrs_minecraft_item::PaintingVariantValue,
);

fn decode_world_registry_id<R: Registered>(r: &mut &[u8]) -> anyhow::Result<Id<R>> {
    let number = decode_registry_id(r)?;
    Registry::<R>::in_scope(std::any::type_name::<Id<R>>(), |registry| {
        registry.id(number).with_context(|| {
            format!(
                "registry {} has {} entries and none is numbered {number}",
                R::REGISTRY,
                registry.len()
            )
        })
    })
    .with_context(|| format!("cannot read an id of registry {}", R::REGISTRY))?
}

pub fn encode_registry_id(id: u16, w: impl Write) -> anyhow::Result<()> {
    VarInt(i32::from(id)).encode(w)
}

pub fn decode_registry_id(r: &mut &[u8]) -> anyhow::Result<u16> {
    let VarInt(raw) = VarInt::decode(r)?;
    u16::try_from(raw).with_context(|| format!("registry id {raw} is outside 0..=65535"))
}

pub(crate) fn encode_holder_id(reference: Option<u16>, w: impl Write) -> anyhow::Result<()> {
    VarInt(reference.map_or(0, |id| i32::from(id) + 1)).encode(w)
}

pub(crate) fn decode_holder_id(r: &mut &[u8]) -> anyhow::Result<Option<u16>> {
    let VarInt(raw) = VarInt::decode(r)?;
    if raw == 0 {
        return Ok(None);
    }
    raw.checked_sub(1)
        .and_then(|id| u16::try_from(id).ok())
        .map(Some)
        .with_context(|| format!("holder id {raw} is outside 0..=65536"))
}

#[derive(Clone, Debug, Encode, Decode)]
pub struct Entry<'a> {
    pub id: ResourceLocation<Cow<'a, str>>,
    pub data: Option<Cow<'a, NbtTag>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::OptionalBlockState;
    use mcrs_minecraft_item::keys::DataComponentType;
    use mcrs_minecraft_item::keys::{DataComponentPredicateType, Item};
    use mcrs_minecraft_particle::keys::ParticleType;
    use mcrs_minecraft_registry::{BlockStateId, Id};

    type Decoder = fn(&mut &[u8]) -> anyhow::Result<()>;

    const DECODERS: [(&str, Decoder); 7] = [
        ("registry id", |r| decode_registry_id(r).map(drop)),
        ("block state", |r| BlockStateId::decode(r).map(drop)),
        ("item", |r| Id::<Item>::decode(r).map(drop)),
        ("optional block state", |r| {
            OptionalBlockState::decode(r).map(drop)
        }),
        ("particle type", |r| ParticleType::decode(r).map(drop)),
        ("data component type", |r| {
            DataComponentType::decode(r).map(drop)
        }),
        ("component predicate type", |r| {
            DataComponentPredicateType::decode(r).map(drop)
        }),
    ];

    fn var_int(value: i32) -> Vec<u8> {
        let mut bytes = Vec::new();
        VarInt(value).encode(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn an_id_outside_sixteen_bits_is_a_decode_error() {
        for (name, decode) in DECODERS {
            for raw in [-1, i32::MIN, 65537, i32::MAX] {
                let bytes = var_int(raw);
                assert!(decode(&mut bytes.as_slice()).is_err(), "{name} {raw}");
            }
        }
    }

    fn cat_variants(len: usize) -> mcrs_minecraft_registry::RegistrySet {
        use mcrs_minecraft_entity::keys::CAT_VARIANT;
        let names = (0..len).map(|n| {
            mcrs_minecraft_core::ResourceLocation::<std::sync::Arc<str>>::read(&format!(
                "minecraft:cat{n}"
            ))
            .unwrap()
        });
        mcrs_minecraft_registry::RegistrySet::new()
            .with(mcrs_minecraft_registry::Registry::new(CAT_VARIANT, names).unwrap())
            .unwrap()
    }

    fn with_number(valid: &[u8], prefix: usize, last: u16, number: u16) -> Vec<u8> {
        let tail = prefix + var_int(i32::from(last)).len();
        [
            &valid[..prefix],
            &var_int(i32::from(number)),
            &valid[tail..],
        ]
        .concat()
    }

    fn refused_past_the_table(
        registry: &str,
        table_len: usize,
        prefix: usize,
        valid: &[u8],
        decode_packet: impl Fn(&mut &[u8]) -> anyhow::Result<()>,
    ) {
        let last = table_len as u16 - 1;
        let decode = |number: u16| {
            let bytes = with_number(valid, prefix, last, number);
            decode_packet(&mut bytes.as_slice())
        };
        decode(last).unwrap_or_else(|e| panic!("{registry} {last}: {e:#}"));
        let refused = format!("{:#}", decode(last + 1).unwrap_err());
        assert!(refused.contains(registry), "{registry}: {refused}");
    }

    fn encoded(packet: &impl Encode) -> Vec<u8> {
        let mut bytes = Vec::new();
        packet.encode(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn a_static_registry_number_past_its_table_does_not_decode() {
        use crate::chunk::ChunkDataBlockEntity;
        use crate::packets::game::clientbound::{
            AttributeSnapshot, ClientboundAddEntity, ClientboundOpenScreen,
            ClientboundUpdateAttributes,
        };
        use crate::text::Text;
        use bevy_math::DVec3;
        use mcrs_minecraft_block::keys::BlockEntityType;
        use mcrs_minecraft_entity::keys::{Attribute, EntityType};
        use mcrs_minecraft_item::keys::MenuType;

        let add_entity = ClientboundAddEntity {
            id: VarInt(1),
            uuid: uuid::Uuid::nil(),
            kind: *EntityType::ALL.last().unwrap(),
            pos: DVec3::ZERO,
            movement: crate::LpVec3(DVec3::ZERO),
            pitch: crate::ByteAngle(0),
            yaw: crate::ByteAngle(0),
            head_yaw: crate::ByteAngle(0),
            data: VarInt(0),
        };
        refused_past_the_table(
            "entity type",
            EntityType::ALL.len(),
            1 + 16,
            &encoded(&add_entity),
            |r| ClientboundAddEntity::decode(r).map(drop),
        );

        let open_screen = ClientboundOpenScreen {
            container_id: VarInt(1),
            menu_type: *MenuType::ALL.last().unwrap(),
            title: Text::text("Chest"),
        };
        refused_past_the_table(
            "menu type",
            MenuType::ALL.len(),
            1,
            &encoded(&open_screen),
            |r| ClientboundOpenScreen::decode(r).map(drop),
        );

        let attributes = ClientboundUpdateAttributes {
            entity_id: VarInt(1),
            attributes: vec![AttributeSnapshot {
                attribute: *Attribute::ALL.last().unwrap(),
                base: 1.0,
                modifiers: vec![],
            }],
        };
        refused_past_the_table(
            "attribute",
            Attribute::ALL.len(),
            1 + 1,
            &encoded(&attributes),
            |r| ClientboundUpdateAttributes::decode(r).map(drop),
        );

        let block_entity = ChunkDataBlockEntity {
            packed_xz: 0,
            y: 0,
            kind: *BlockEntityType::ALL.last().unwrap(),
            data: Cow::Owned(Default::default()),
        };
        refused_past_the_table(
            "block entity type",
            BlockEntityType::ALL.len(),
            1 + 2,
            &encoded(&block_entity),
            |r| ChunkDataBlockEntity::decode(r).map(drop),
        );
    }

    #[test]
    fn a_world_registry_id_outside_its_registry_does_not_decode() {
        use mcrs_minecraft_entity::variant::CatVariant;

        let decode_cat = |number: i32| {
            let bytes = var_int(number);
            Id::<CatVariant>::decode(&mut bytes.as_slice())
        };

        cat_variants(3).scope(|| {
            assert_eq!(decode_cat(2).unwrap().number(), 2);
            let refused = format!("{:#}", decode_cat(3).unwrap_err());
            assert!(refused.contains("minecraft:cat_variant"), "{refused}");
            assert!(refused.contains('3'), "{refused}");
        });

        let outside = format!("{:#}", decode_cat(0).unwrap_err());
        assert!(outside.contains("minecraft:cat_variant"), "{outside}");
    }

    #[test]
    fn a_world_registry_id_writes_its_number_without_a_registry() {
        use mcrs_minecraft_entity::variant::CatVariant;

        let id = cat_variants(3)
            .registry::<CatVariant>()
            .unwrap()
            .id(2)
            .unwrap();
        let mut bytes = Vec::new();
        id.encode(&mut bytes).unwrap();
        assert_eq!(bytes, var_int(2));
    }

    #[test]
    fn the_widest_id_round_trips_in_var_int_bytes() {
        let mut bytes = Vec::new();
        encode_registry_id(u16::MAX, &mut bytes).unwrap();
        assert_eq!(bytes, var_int(65535));
        assert_eq!(decode_registry_id(&mut bytes.as_slice()).unwrap(), u16::MAX);
    }
}
