use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::chat::{ChatTypeId, FilterMask};
use mcrs_minecraft_protocol::command::{CommandNode, CommandNodeKind, ParserProperties};

#[test]
fn a_time_argument_carries_its_minimum_as_a_big_endian_int() {
    // Argument, no children, name "time", parser id 43 (`minecraft:time`), minimum 20.
    let bytes = [0x02, 0x00, 0x04, b't', b'i', b'm', b'e', 43, 0, 0, 0, 20];
    let mut slice = &bytes[..];
    let node = CommandNode::decode(&mut slice).unwrap();
    assert!(slice.is_empty());
    let CommandNodeKind::Argument { parser, .. } = node.kind else {
        panic!("{node:?}");
    };
    assert_eq!(parser.properties, ParserProperties::Time(20));
}

#[test]
fn a_chat_type_names_the_registry_entry_one_below_it() {
    assert_eq!(ChatTypeId::decode(&mut &[1u8][..]).unwrap(), ChatTypeId(0));
    assert!(
        ChatTypeId::decode(&mut &[0u8][..]).is_err(),
        "an inline chat type"
    );
}

#[test]
fn a_filter_mask_hides_nothing_everything_or_bit_by_bit() {
    let cases: [(&[u8], &[usize]); 3] = [
        (&[0], &[]),
        (
            &[1],
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        ),
        (&[2, 2, 0b0000_0101, 0b1000_0000], &[0, 2, 15]),
    ];
    for (bytes, expected) in cases {
        let mut slice = bytes;
        let mask = FilterMask::decode(&mut slice).unwrap();
        assert!(slice.is_empty(), "{bytes:?}");
        let hidden: Vec<usize> = (0..16).filter(|&i| mask.hides(i)).collect();
        assert_eq!(hidden, expected, "{bytes:?}");
    }
}
