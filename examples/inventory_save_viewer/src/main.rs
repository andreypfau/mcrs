use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::Context;
use mcrs_minecraft_inventory_file::read;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::DefinitionFile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::bake;
use mcrs_minecraft_nbt::nbt_compress::read_gzip_compound_tag;
use serde::Deserialize;
use serde::Serialize;

const USAGE: &str = "usage: mcrs_minecraft_inventory_save_viewer <player file> <item definitions>";

#[derive(Deserialize, Serialize)]
struct ItemFile {
    #[serde(rename = "minecraft:item")]
    item: Item,
}

#[derive(Deserialize, Serialize)]
struct Item {
    description: Description,
    // The components name entries of registries that only the data pack gives; the viewer needs
    // the item names alone, so it bakes each item with no component.
    #[serde(skip_deserializing)]
    components: BTreeMap<String, u8>,
}

#[derive(Deserialize, Serialize)]
struct Description {
    identifier: String,
    protocol_id: u16,
}

fn main() -> anyhow::Result<()> {
    let mut args = env::args_os().skip(1);
    let player_path = args.next().context(USAGE)?;
    let definitions = args.next().context(USAGE)?;
    let table = item_names(Path::new(&definitions))?;
    let player_file = File::open(&player_path)
        .with_context(|| format!("{}", Path::new(&player_path).display()))?;
    let player = read_gzip_compound_tag(BufReader::new(player_file))?;
    let inventory = read(&player, &table)?;
    for (slot, stack) in inventory.items.slots.iter().enumerate() {
        if let Some(stack) = stack {
            let name = table
                .name(NameTable::Item, stack.kind.0)
                .context("the stack has an item without a name")?;
            println!("slot {slot}: {} {name}", stack.count.get());
        }
    }
    println!("selected slot: {}", inventory.items.selected);
    for (key, _) in &inventory.kept_equipment.child_tags {
        println!("kept equipment key: {key}");
    }
    Ok(())
}

fn item_names(directory: &Path) -> anyhow::Result<ItemTable> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in fs::read_dir(directory).with_context(|| format!("{}", directory.display()))? {
        let path = entry?.path();
        let item: ItemFile = serde_json::from_slice(&fs::read(&path)?)
            .with_context(|| format!("{}", path.display()))?;
        files.push((path.display().to_string(), serde_json::to_vec(&item)?));
    }
    let baked = bake(
        files
            .iter()
            .map(|(name, bytes)| DefinitionFile { name, bytes }),
        &ItemTable::leak([]),
    )?;
    Ok(baked.table)
}
