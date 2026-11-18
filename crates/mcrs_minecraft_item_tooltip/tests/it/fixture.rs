use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;

use mcrs_minecraft_fixtures::vanilla_dir;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::DefinitionFile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_item_table::bake;
use mcrs_minecraft_resource::ResourceLocation;
use serde::Deserialize;

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/mcrs");

#[derive(Deserialize)]
struct Registry {
    entries: HashMap<String, Entry>,
}

#[derive(Deserialize)]
struct Entry {
    protocol_id: u16,
}

pub(crate) struct Registries {
    tables: HashMap<NameTable, HashMap<String, u16>>,
}

impl Registries {
    fn load() -> Self {
        let report = fs::read(Path::new(ASSETS).join("reports/registries.json"))
            .expect("the registry report is in the assets");
        let mut report: HashMap<String, Registry> =
            serde_json::from_slice(&report).expect("the registry report is JSON");
        let mut static_table = |registry: &str| -> HashMap<String, u16> {
            report
                .remove(registry)
                .expect("the report has the registry")
                .entries
                .into_iter()
                .map(|(name, entry)| (name, entry.protocol_id))
                .collect()
        };
        let pack = |path: &str| pack_table(&vanilla_dir().join(path));
        let tables = HashMap::from([
            (NameTable::Attribute, static_table("minecraft:attribute")),
            (NameTable::Block, static_table("minecraft:block")),
            (
                NameTable::ComponentType,
                static_table("minecraft:data_component_type"),
            ),
            (NameTable::SoundEvent, static_table("minecraft:sound_event")),
            (NameTable::EntityType, static_table("minecraft:entity_type")),
            (NameTable::MobEffect, static_table("minecraft:mob_effect")),
            (NameTable::Potion, static_table("minecraft:potion")),
            (NameTable::Enchantment, pack("enchantment")),
            (NameTable::BlockTag, pack("tags/block")),
            (NameTable::Instrument, pack("instrument")),
            (NameTable::TrimMaterial, pack("trim_material")),
            (NameTable::TrimPattern, pack("trim_pattern")),
            (NameTable::EntityTypeTag, pack("tags/entity_type")),
        ]);
        Self { tables }
    }
}

fn pack_table(root: &Path) -> HashMap<String, u16> {
    let mut names: Vec<String> = Vec::new();
    let mut pending: Vec<PathBuf> = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("the data pack directory exists") {
            let path = entry.expect("the directory entry is readable").path();
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(relative) = path.with_extension("").strip_prefix(root) {
                names.push(format!("minecraft:{}", relative.to_string_lossy()));
            }
        }
    }
    names.sort();
    (0..).zip(names).map(|(id, name)| (name, id)).collect()
}

impl NameLookup for Registries {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        self.tables.get(&table)?.get(&name.to_string()).copied()
    }

    fn name(&self, _: NameTable, _: u16) -> Option<ResourceLocation<'_>> {
        None
    }
}

pub(crate) struct Corpus {
    pub(crate) items: ItemTable,
    pub(crate) names: Registries,
    components: HashMap<String, usize>,
}

impl Corpus {
    pub(crate) fn stack(&self, item: &str) -> ItemStack {
        let name = ResourceLocation::read(item).expect("an item name is an identifier");
        let id = self
            .items
            .id(NameTable::Item, name)
            .expect("the item is defined");
        ItemStack::new(Kind(id), Count::new(1).expect("a count above 0"))
    }

    pub(crate) fn prototype_components(&self, item: &str) -> usize {
        *self.components.get(item).expect("the item is defined")
    }
}

#[derive(Deserialize)]
struct Definition {
    #[serde(rename = "minecraft:item")]
    item: DefinedItem,
}

#[derive(Deserialize)]
struct DefinedItem {
    description: Description,
    components: HashMap<String, serde::de::IgnoredAny>,
}

#[derive(Deserialize)]
struct Description {
    identifier: String,
}

pub(crate) static CORPUS: LazyLock<Corpus> = LazyLock::new(|| {
    let mut files: Vec<(String, Vec<u8>)> = fs::read_dir(Path::new(ASSETS).join("item_definition"))
        .expect("the item definitions are in the assets")
        .map(|entry| {
            let path = entry.expect("the directory entry is readable").path();
            let bytes = fs::read(&path).expect("the definition is readable");
            (path.display().to_string(), bytes)
        })
        .collect();
    files.sort();
    let names = Registries::load();
    let items = bake(
        files
            .iter()
            .map(|(name, bytes)| DefinitionFile { name, bytes }),
        &names,
    )
    .expect("the item definitions bake")
    .table;
    let components = files
        .iter()
        .map(|(_, bytes)| {
            let definition: Definition =
                serde_json::from_slice(bytes).expect("the definition is JSON");
            (
                definition.item.description.identifier,
                definition.item.components.len(),
            )
        })
        .collect();
    Corpus {
        items,
        names,
        components,
    }
});
