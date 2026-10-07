use std::borrow::Cow;
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_protocol::{Decode, Encode};

use crate::identifier_rows::{ROWS, outcome};

#[test]
fn a_wire_identifier_decodes_as_vanilla_parses_it() {
    for row in ROWS {
        let text = row.text;
        let mut bytes = Vec::new();
        text.encode(&mut bytes).unwrap();
        assert_eq!(
            outcome(ResourceLocation::<Arc<str>>::decode(&mut bytes.as_slice())),
            row.parse,
            "wire decode {text:?}"
        );
        assert_eq!(
            outcome(ResourceLocation::<Cow<str>>::decode(&mut bytes.as_slice())),
            row.parse,
            "wire decode borrowed {text:?}"
        );
    }
}
