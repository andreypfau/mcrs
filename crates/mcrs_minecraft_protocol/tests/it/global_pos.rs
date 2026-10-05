use std::io::Cursor;

use mcrs_minecraft_core::{BlockPos, ResourceKey, ResourceLocation};
use mcrs_minecraft_item::component::GlobalPos;
use mcrs_minecraft_protocol::{Decode, Encode};

fn far_corner() -> GlobalPos {
    GlobalPos {
        dimension: ResourceKey::from_location(ResourceLocation::minecraft("the_nether")),
        pos: BlockPos::new(-30_000_000, -64, 29_999_999),
    }
}

#[test]
fn a_global_pos_keeps_its_coordinates_on_the_wire_and_in_nbt() {
    let expected = far_corner();

    let mut wire = Vec::new();
    expected.encode(&mut wire).unwrap();
    let mut r = &wire[..];
    assert_eq!(GlobalPos::decode(&mut r).unwrap(), expected);
    assert!(r.is_empty());

    let mut nbt = Vec::new();
    mcrs_minecraft_nbt::to_bytes_unnamed(&expected, &mut nbt).unwrap();
    let read: GlobalPos = mcrs_minecraft_nbt::from_bytes_unnamed(Cursor::new(nbt)).unwrap();
    assert_eq!(read, expected);
}
