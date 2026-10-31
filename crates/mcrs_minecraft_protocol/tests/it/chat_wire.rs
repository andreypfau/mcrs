use mcrs_minecraft_command_tree::{CommandNode, CommandNodeKind, ParserProperties};
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol::chat::{ChatTypeId, FilterMask};
use mcrs_minecraft_resource::ResourceLocationBuf;

#[test]
fn an_argument_reads_the_properties_of_its_parser() {
    let registry =
        |id: &str| ParserProperties::Registry(ResourceLocationBuf::read(id).unwrap().into());
    let item = b"\x0eminecraft:item";
    let cases: [(u8, &[u8], ParserProperties); 5] = [
        (20, &[], ParserProperties::None),
        (31, &[1], ParserProperties::ScoreHolder { multiple: true }),
        (43, &[0, 0, 0, 20], ParserProperties::Time(20)),
        (47, item, registry("minecraft:item")),
        (48, item, registry("minecraft:item")),
    ];
    for (id, properties, expected) in cases {
        // Argument, no children, name "a", the parser id, its properties.
        let mut bytes = vec![0x02, 0x00, 0x01, b'a', id];
        bytes.extend_from_slice(properties);
        let mut slice = &bytes[..];
        let node = CommandNode::decode(&mut slice).unwrap();
        assert!(slice.is_empty(), "parser {id}");
        let CommandNodeKind::Argument { parser, .. } = node.kind else {
            panic!("{node:?}");
        };
        assert_eq!(parser.properties, expected, "parser {id}");
    }
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
