use md5::Digest;
use md5::Md5;
use serde_json::Value;
use serde_json::json;
use uuid::Builder;
use uuid::Uuid;

pub fn offline_uuid(name: &str) -> Uuid {
    let digest = Md5::digest(format!("OfflinePlayer:{name}").as_bytes());
    Builder::from_md5_bytes(digest.into()).into_uuid()
}

pub fn operators_json(names: &[&str]) -> String {
    let operators: Vec<Value> = names
        .iter()
        .map(|name| {
            json!({ "uuid": offline_uuid(name).to_string(), "name": name, "level": 4, "bypassesPlayerLimit": false })
        })
        .collect();
    Value::Array(operators).to_string()
}
