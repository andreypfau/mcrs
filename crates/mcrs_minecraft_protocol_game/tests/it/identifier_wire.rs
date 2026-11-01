use mcrs_minecraft_protocol::{Decode, Encode};
use mcrs_minecraft_resource::{ResourceLocation, ResourceLocationBuf};

use crate::identifier_rows::{ROWS, outcome};

#[test]
fn a_wire_identifier_decodes_as_vanilla_parses_it() {
    for row in ROWS {
        let text = row.text;
        let mut bytes = Vec::new();
        text.encode(&mut bytes).unwrap();
        assert_eq!(
            outcome(ResourceLocationBuf::decode(&mut bytes.as_slice())),
            row.parse,
            "wire decode {text:?}"
        );
        assert_eq!(
            outcome(ResourceLocation::decode(&mut bytes.as_slice())),
            row.parse,
            "wire decode borrowed {text:?}"
        );
    }
}
