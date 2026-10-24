#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a build script fails the build on a version file it cannot read"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "a build script reads the variables cargo sets for it"
)]
#![expect(
    clippy::disallowed_types,
    reason = "the fields are written as they come, so the struct literal checks them"
)]

use std::path::Path;

use serde_json::Value;

const VERSION_FILE: &str = "../../assets/minecraft/version.json";

/// A struct literal of `name` with one field per key, so a key `Version` lacks,
/// a field the file lacks and a value of the wrong type all fail to compile.
fn literal(name: &str, value: &Value) -> String {
    let Value::Object(fields) = value else {
        panic!("{VERSION_FILE}: {name} is not an object");
    };
    let fields: String = fields
        .iter()
        .map(|(key, value)| {
            let value = match value {
                Value::String(text) => format!("{text:?}"),
                Value::Number(_) | Value::Bool(_) => value.to_string(),
                Value::Object(_) => literal(&type_name(key), value),
                _ => panic!("{VERSION_FILE}: {key} holds no field value"),
            };
            format!("{key}: {value}, ")
        })
        .collect();
    format!("{name} {{ {fields}}}")
}

fn type_name(key: &str) -> String {
    key.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn main() {
    println!("cargo::rerun-if-changed={VERSION_FILE}");
    let text = std::fs::read_to_string(VERSION_FILE).expect("the version file reads");
    let value: Value = serde_json::from_str(&text).expect("the version file is JSON");
    let out = std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR");
    std::fs::write(
        Path::new(&out).join("version.rs"),
        literal("Version", &value),
    )
    .expect("OUT_DIR is writable");
}
