use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::packets::configuration::clientbound::ClientboundRegistryData;
use mcrs_minecraft_resource::ResourceLocation;

use crate::encoded;

fn compound(name: &str, value: i32) -> NbtCompound {
    let mut compound = NbtCompound::new();
    compound.put_int(name, value);
    compound
}

/// Each entry's NBT stops at its own end, so the next entry and the packet after it decode.
#[test]
fn registry_data_decodes_exactly_its_own_bytes() {
    {
        let packet = ClientboundRegistryData {
            registry: ResourceLocation::read("minecraft:dimension_type").expect("registry id"),
            entries: vec![
                mcrs_minecraft_protocol::registry::Entry {
                    id: ResourceLocation::read("minecraft:overworld").expect("entry id"),
                    data: Some(std::borrow::Cow::Owned(compound("height", 384).into())),
                },
                mcrs_minecraft_protocol::registry::Entry {
                    id: ResourceLocation::read("minecraft:the_nether").expect("entry id"),
                    data: Some(std::borrow::Cow::Owned(compound("height", 256).into())),
                },
                mcrs_minecraft_protocol::registry::Entry {
                    id: ResourceLocation::read("minecraft:axe").expect("entry id"),
                    data: Some(std::borrow::Cow::Owned(
                        mcrs_minecraft_nbt::tag::NbtTag::List(vec![compound("weight", 1).into()]),
                    )),
                },
            ],
        };

        let buf = encoded(&packet);
        let mut r: &[u8] = &buf;
        let decoded = ClientboundRegistryData::decode(&mut r).expect("decode registry data");
        assert!(r.is_empty(), "{} trailing bytes", r.len());
        assert_eq!(encoded(&decoded), buf);
        assert_eq!(decoded.entries.len(), 3);
    }
}
