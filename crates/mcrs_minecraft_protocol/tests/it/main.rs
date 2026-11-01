#![expect(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "a test reports a failure by panicking"
)]

mod frames_golden;
mod handshake_and_payloads;
#[expect(
    dead_code,
    reason = "the identifier rows module is shared with the core crate's tests, and only some of its helpers are read here"
)]
#[path = "../../../mcrs_minecraft_core/tests/it/identifier_rows.rs"]
mod identifier_rows;
mod identifier_wire;
mod registry_data;

fn encoded(value: &impl mcrs_minecraft_protocol::Encode) -> Vec<u8> {
    let mut out = Vec::new();
    value.encode(&mut out).unwrap();
    out
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}
