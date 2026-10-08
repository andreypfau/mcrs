use bevy_math::DVec3;
use mcrs_minecraft_core::RegistryKey;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_entity::keys::{
    CAT_SOUND_VARIANT, CAT_VARIANT, VillagerProfession, VillagerType, ZOMBIE_NAUTILUS_VARIANT,
};
use mcrs_minecraft_entity::variant::{CatSoundVariant, CatVariant, ZombieNautilusVariant};
use mcrs_minecraft_item::PaintingVariantValue;
use mcrs_minecraft_item::keys::PAINTING_VARIANT;
use mcrs_minecraft_protocol::entity::{
    DyeColor, EquipmentSlot, MetaDataValue, Metadata, MetadataEntry, OptionalBlockState,
    OptionalUnsignedInt, Pose, VillagerData,
};
use mcrs_minecraft_protocol::item::RawStack;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    AttributeModifier, AttributeOperation, AttributeSnapshot, ClientboundAddEntity,
    ClientboundSetEntityData, ClientboundSetEquipment, ClientboundSetPassengers,
    ClientboundUpdateAttributes,
};
use mcrs_minecraft_protocol::{ByteAngle, Decode, Encode, LpVec3, ProtoStack, RegistryId, VarInt};
use mcrs_minecraft_registry::{BlockStateId, Id, NoRegistries, Registry, RegistrySet};
use std::sync::Arc;
use uuid::Uuid;

fn registry<R>(key: RegistryKey<R>, len: usize) -> Registry<R> {
    let names = (0..len)
        .map(|n| ResourceLocation::<Arc<str>>::read(&format!("minecraft:entry{n}")).unwrap());
    Registry::new(key, names).unwrap()
}

fn world_registries() -> RegistrySet {
    RegistrySet::new()
        .with(registry(CAT_VARIANT, 3))
        .unwrap()
        .with(registry(CAT_SOUND_VARIANT, 2))
        .unwrap()
        .with(registry(ZOMBIE_NAUTILUS_VARIANT, 2))
        .unwrap()
        .with(registry(PAINTING_VARIANT, 5))
        .unwrap()
}

fn id<R: 'static>(set: &RegistrySet, number: u16) -> Id<R> {
    set.registry::<R>().unwrap().id(number).unwrap()
}

fn raw(slot: ProtoStack) -> RawStack {
    RawStack::from_stack(&slot, &NoRegistries).unwrap()
}

fn round_trip<'a, P: Encode + Decode<'a> + PartialEq + std::fmt::Debug>(
    packet: &P,
    buf: &'a mut Vec<u8>,
) -> &'a [u8] {
    packet.encode(&mut *buf).expect("encode");
    let mut r: &[u8] = buf;
    let decoded = P::decode(&mut r).expect("decode");
    assert!(r.is_empty(), "{} trailing bytes", r.len());
    assert_eq!(&decoded, packet);
    buf
}

#[test]
fn add_entity_writes_pitch_before_yaw() {
    let packet = ClientboundAddEntity {
        id: VarInt(7),
        uuid: Uuid::nil(),
        kind: RegistryId(159),
        pos: DVec3::ZERO,
        movement: LpVec3(DVec3::ZERO),
        pitch: ByteAngle(1),
        yaw: ByteAngle(2),
        head_yaw: ByteAngle(3),
        data: VarInt(4),
    };
    let mut buf = Vec::new();
    let bytes = round_trip(&packet, &mut buf);
    let tail = &bytes[bytes.len() - 4..];
    assert_eq!(tail, [1, 2, 3, 4]);
}

#[test]
fn entity_data_uses_the_registered_serializer_ids() {
    let set = world_registries();
    let entries = vec![
        (0, MetaDataValue::Byte(0x20), 0),
        (1, MetaDataValue::VarInt(VarInt(300)), 1),
        (2, MetaDataValue::Float(1.5), 3),
        (3, MetaDataValue::String("mcrs"), 4),
        (4, MetaDataValue::OptionalText(None), 6),
        (
            5,
            MetaDataValue::Slot(raw(ProtoStack::new(
                Id::from_raw(974),
                1,
                Default::default(),
            ))),
            7,
        ),
        (6, MetaDataValue::Boolean(true), 8),
        (
            7,
            MetaDataValue::OptionalBlockState(OptionalBlockState(Some(BlockStateId(9)))),
            15,
        ),
        (
            8,
            MetaDataValue::VillagerData(VillagerData::new(
                VillagerType::Plains,
                VillagerProfession::Cleric,
                1,
            )),
            18,
        ),
        (
            9,
            MetaDataValue::OptionalUnsignedInt(OptionalUnsignedInt(Some(0))),
            19,
        ),
        (10, MetaDataValue::Pose(Pose::Sitting), 20),
        (11, MetaDataValue::CatVariant(id::<CatVariant>(&set, 1)), 21),
        (
            12,
            MetaDataValue::CatSoundVariant(id::<CatSoundVariant>(&set, 0)),
            22,
        ),
        (
            13,
            MetaDataValue::ZombieNautilusVariant(id::<ZombieNautilusVariant>(&set, 0)),
            32,
        ),
        (
            14,
            MetaDataValue::PaintingVariant(id::<PaintingVariantValue>(&set, 3)),
            34,
        ),
        (15, MetaDataValue::DyeColor(DyeColor::Black), 43),
    ];
    for (index, value, serializer) in &entries {
        let entry = MetadataEntry {
            index: *index,
            value: value.clone(),
        };
        let mut buf = Vec::new();
        entry.encode(&mut buf).unwrap();
        assert_eq!(buf[0], *index);
        assert_eq!(buf[1], *serializer, "{value:?}");
    }

    let packet = ClientboundSetEntityData {
        entity_id: VarInt(7),
        metadata: Metadata(
            entries
                .iter()
                .map(|(index, value, _)| MetadataEntry {
                    index: *index,
                    value: value.clone(),
                })
                .collect(),
        ),
    };
    let mut buf = Vec::new();
    let bytes = set.scope(|| round_trip(&packet, &mut buf));
    assert_eq!(*bytes.last().unwrap(), 0xFF);

    let empty = ClientboundSetEntityData {
        entity_id: VarInt(1),
        metadata: Metadata::default(),
    };
    let mut buf = Vec::new();
    assert_eq!(round_trip(&empty, &mut buf), [1, 0xFF]);
}

#[test]
fn optional_wire_forms_reserve_zero_for_absent() {
    let mut buf = Vec::new();
    OptionalBlockState(None).encode(&mut buf).unwrap();
    OptionalUnsignedInt(None).encode(&mut buf).unwrap();
    OptionalUnsignedInt(Some(5)).encode(&mut buf).unwrap();
    assert_eq!(buf, [0, 0, 6]);
    let mut r: &[u8] = &buf;
    assert_eq!(
        OptionalBlockState::decode(&mut r).unwrap(),
        OptionalBlockState(None)
    );
    assert_eq!(
        OptionalUnsignedInt::decode(&mut r).unwrap(),
        OptionalUnsignedInt(None)
    );
    assert_eq!(
        OptionalUnsignedInt::decode(&mut r).unwrap(),
        OptionalUnsignedInt(Some(5))
    );
}

#[test]
fn equipment_chains_slots_with_the_continuation_bit() {
    let packet = ClientboundSetEquipment {
        entity_id: VarInt(9),
        slots: vec![
            (
                EquipmentSlot::MainHand,
                raw(ProtoStack::new(Id::from_raw(1483), 1, Default::default())),
            ),
            (EquipmentSlot::OffHand, RawStack::EMPTY),
            (
                EquipmentSlot::Head,
                raw(ProtoStack::new(Id::from_raw(1), 3, Default::default())),
            ),
        ],
    };
    let mut buf = Vec::new();
    let bytes = round_trip(&packet, &mut buf);
    // entity id; main hand continues: 1 x item 1483 (two-byte varint) with no
    // components; off hand continues: empty; head last: 3 x item 1.
    assert_eq!(
        bytes,
        [9, 0x80, 1, 0xCB, 0x0B, 0, 0, 0x81, 0, 5, 3, 1, 0, 0]
    );

    let single = ClientboundSetEquipment {
        entity_id: VarInt(1),
        slots: vec![(EquipmentSlot::Saddle, RawStack::EMPTY)],
    };
    let mut buf = Vec::new();
    assert_eq!(round_trip(&single, &mut buf), [1, 7, 0]);

    let mut buf = Vec::new();
    assert!(
        ClientboundSetEquipment {
            entity_id: VarInt(1),
            slots: vec![],
        }
        .encode(&mut buf)
        .is_err()
    );
}

#[test]
fn passengers_round_trip() {
    let packet = ClientboundSetPassengers {
        vehicle: VarInt(12),
        passengers: vec![VarInt(13), VarInt(300)],
    };
    let mut buf = Vec::new();
    assert_eq!(round_trip(&packet, &mut buf), [12, 2, 13, 0xAC, 0x02]);
}

#[test]
fn attributes_round_trip_with_modifiers() {
    let packet = ClientboundUpdateAttributes {
        entity_id: VarInt(5),
        attributes: vec![
            AttributeSnapshot {
                attribute: RegistryId(23),
                base: 26.0,
                modifiers: vec![],
            },
            AttributeSnapshot {
                attribute: RegistryId(20),
                base: 0.0,
                modifiers: vec![AttributeModifier {
                    id: ResourceLocation::read_cow("minecraft:random_spawn_bonus").unwrap(),
                    amount: 0.025,
                    operation: AttributeOperation::AddValue,
                }],
            },
        ],
    };
    let mut buf = Vec::new();
    let bytes = round_trip(&packet, &mut buf);
    assert_eq!(&bytes[..4], [5, 2, 23, 0x40]);
    assert_eq!(*bytes.last().unwrap(), 0);
}

#[test]
fn a_villager_data_on_the_wire_follows_the_reference_stream_codec() {
    let wire = |kind: i32, profession: i32, level: i32| {
        let mut bytes = Vec::new();
        for number in [kind, profession, level] {
            VarInt(number).encode(&mut bytes).unwrap();
        }
        bytes
    };
    let read = |bytes: Vec<u8>| {
        let mut r = bytes.as_slice();
        let read = VillagerData::decode(&mut r);
        assert!(read.is_err() || r.is_empty(), "{} trailing bytes", r.len());
        read
    };

    let swamp_nitwit = VillagerData::new(VillagerType::Swamp, VillagerProfession::Nitwit, 2);
    assert_eq!(read(wire(5, 11, 2)).unwrap(), swamp_nitwit);
    let mut written = Vec::new();
    swamp_nitwit.encode(&mut written).unwrap();
    assert_eq!(written, wire(5, 11, 2));

    assert_eq!(
        read(wire(2, 0, 0)).unwrap().level(),
        1,
        "a level is at least the first"
    );
    assert_eq!(read(wire(2, 0, -7)).unwrap().level(), 1);
    assert_eq!(
        read(wire(2, 0, 12)).unwrap().level(),
        12,
        "and has no upper bound"
    );
    assert!(read(wire(7, 0, 1)).is_err(), "no seventh villager type");
    assert!(read(wire(2, 15, 1)).is_err(), "no fifteenth profession");
}
