use mcrs_minecraft_keys::{Item, item};
use mcrs_minecraft_protocol::item::{HashedPatchMap, HashedStack};
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_registry::{Id, StaticRegistry};

#[test]
fn the_first_and_last_item_cross_the_wire_by_protocol_id() {
    let last = u16::try_from(Item::NAMES.len() - 1).unwrap();
    for (id, number) in [(item::AIR, 0), (Id::<Item>::from_static(last), last)] {
        assert_eq!(id.number(), number);
        let stack = HashedStack {
            id,
            count: 1,
            components: HashedPatchMap::default(),
        };

        let mut wire = Vec::new();
        stack.encode(&mut wire).unwrap();
        let mut expected = Vec::new();
        VarInt(i32::from(number)).encode(&mut expected).unwrap();
        expected.extend([1, 0, 0]);
        assert_eq!(wire, expected);

        let decoded = HashedStack::decode(&mut &wire[..]).unwrap();
        assert_eq!(decoded.id, id);
        assert_eq!(decoded, stack);
    }
}
