use std::path::{Path, PathBuf};

use mcrs_minecraft_protocol::item::{ComponentMap, ComponentPatch, ItemPredicate, Template};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/minecraft");

#[derive(Default)]
struct Family {
    files: usize,
    values: usize,
    failures: Vec<String>,
}

impl Family {
    fn check<T: Serialize + DeserializeOwned>(&mut self, file: &Path, at: &str, source: &Value) {
        self.values += 1;
        let file = file.strip_prefix(ASSETS).unwrap_or(file).display();
        let value = match serde_json::from_str::<T>(&source.to_string()) {
            Ok(value) => value,
            Err(error) => return self.failures.push(format!("{file} {at}: {error}")),
        };
        match serde_json::to_value(&value) {
            Ok(back) if back == *source => {}
            Ok(back) => self.failures.push(format!(
                "{file} {at}: re-serialised as {back} but the asset has {source}"
            )),
            Err(error) => self
                .failures
                .push(format!("{file} {at}: re-serialise: {error}")),
        }
    }

    fn finish(self, name: &str) {
        println!(
            "{name}: {} files, {} values, {} failures",
            self.files,
            self.values,
            self.failures.len()
        );
        assert!(self.files > 0, "{name}: no files");
        assert!(self.values > 0, "{name}: no values");
        assert!(
            self.failures.is_empty(),
            "{name}: {} failures\n{}",
            self.failures.len(),
            self.failures.join("\n")
        );
    }
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "json") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn run(name: &str, visit: impl Fn(&mut Family, &Path, &Value)) {
    let mut family = Family::default();
    for file in json_files(&Path::new(ASSETS).join(name)) {
        let text = std::fs::read_to_string(&file).unwrap();
        let root: Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        family.files += 1;
        visit(&mut family, &file, &root);
    }
    family.finish(name);
}

fn walk(family: &mut Family, file: &Path, at: &str, value: &Value) {
    let map = match value {
        Value::Object(map) => map,
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                walk(family, file, &format!("{at}[{i}]"), item);
            }
            return;
        }
        _ => return,
    };
    match map.get("type").and_then(Value::as_str) {
        Some("minecraft:set_components") => {
            if let Some(components) = map.get("components") {
                family.check::<ComponentPatch>(file, &format!("{at}.components"), components);
            }
        }
        Some("minecraft:match_tool") => {
            if let Some(predicate) = map.get("predicate") {
                family.check::<ItemPredicate>(file, &format!("{at}.predicate"), predicate);
            }
        }
        Some("minecraft:filtered") => {
            if let Some(filter) = map.get("item_filter") {
                family.check::<ItemPredicate>(file, &format!("{at}.item_filter"), filter);
            }
        }
        _ => {}
    }
    if map.contains_key("trigger")
        && let Some(Value::Object(conditions)) = map.get("conditions")
    {
        for field in ["item", "rod", "fired_from_weapon"] {
            if let Some(predicate) = conditions.get(field) {
                let at = format!("{at}.conditions.{field}");
                family.check::<ItemPredicate>(file, &at, predicate);
            }
        }
        for field in ["items", "ingredients"] {
            if let Some(Value::Array(predicates)) = conditions.get(field) {
                for (i, predicate) in predicates.iter().enumerate() {
                    let at = format!("{at}.conditions.{field}[{i}]");
                    family.check::<ItemPredicate>(file, &at, predicate);
                }
            }
        }
    }
    for field in ["minecraft:equipment", "minecraft:slots"] {
        if let Some(Value::Object(slots)) = map.get(field) {
            for (slot, predicate) in slots {
                let at = format!("{at}.{field}.{slot}");
                family.check::<ItemPredicate>(file, &at, predicate);
            }
        }
    }
    if let Some(icon) = map.get("display").and_then(|display| display.get("icon")) {
        family.check::<Template>(file, &format!("{at}.display.icon"), icon);
    }
    for (key, child) in map {
        walk(family, file, &format!("{at}.{key}"), child);
    }
}

#[test]
fn recipe_results_are_item_stack_templates() {
    run("recipe", |family, file, root| {
        let kind = root["type"].as_str().unwrap();
        let (field, result) = match kind {
            "minecraft:brewing" => ("output", &root["output"]),
            _ => ("result", &root["result"]),
        };
        if result.is_null() {
            return;
        }
        let transmute = matches!(
            kind,
            "minecraft:crafting_transmute" | "minecraft:crafting_special_mapextending"
        );
        if transmute && result.get("id").is_none() {
            if let Some(components) = result.get("components") {
                family.check::<ComponentPatch>(file, &format!("{field}.components"), components);
            }
            return;
        }
        family.check::<Template>(file, field, result);
    });
}

#[test]
fn loot_table_patches_and_predicates_round_trip() {
    run("loot_table", |family, file, root| {
        walk(family, file, "", root)
    });
}

#[test]
fn villager_trades_carry_templates_and_exact_predicates() {
    run("villager_trade", |family, file, root| {
        family.check::<Template>(file, "gives", &root["gives"]);
        for field in ["wants", "additional_wants"] {
            if let Some(components) = root.get(field).and_then(|cost| cost.get("components")) {
                family.check::<ComponentMap>(file, &format!("{field}.components"), components);
            }
        }
        walk(family, file, "", root);
    });
}

#[test]
fn predicate_files_hold_item_predicates() {
    run("predicate", |family, file, root| {
        walk(family, file, "", root)
    });
}

#[test]
fn advancement_icons_and_item_predicates_round_trip() {
    run("advancement", |family, file, root| {
        walk(family, file, "", root)
    });
}
