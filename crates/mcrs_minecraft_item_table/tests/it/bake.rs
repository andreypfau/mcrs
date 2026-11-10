use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;

use mcrs_minecraft_fixtures::vanilla_dir;
use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::Food;
use mcrs_minecraft_item_stack::ItemModel;
use mcrs_minecraft_item_stack::ItemName;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_table::BakedItems;
use mcrs_minecraft_item_table::DefinitionFile;
use mcrs_minecraft_item_table::Prototype;
use mcrs_minecraft_item_table::bake;
use mcrs_minecraft_resource::ResourceLocation;
use serde::Deserialize;

const ITEMS: u16 = 1686;
const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/mcrs");

#[derive(Deserialize)]
struct Registry {
    entries: HashMap<String, Entry>,
}

#[derive(Deserialize)]
struct Entry {
    protocol_id: u16,
}

struct Registries {
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
        let tables = HashMap::from([
            (NameTable::Attribute, static_table("minecraft:attribute")),
            (NameTable::Block, static_table("minecraft:block")),
            (
                NameTable::ComponentType,
                static_table("minecraft:data_component_type"),
            ),
            (
                NameTable::Enchantment,
                pack_table(&vanilla_dir().join("enchantment")),
            ),
            (
                NameTable::BlockTag,
                pack_table(&vanilla_dir().join("tags/block")),
            ),
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

struct Corpus {
    baked: BakedItems,
    identifiers: Vec<String>,
}

static CORPUS: LazyLock<Corpus> = LazyLock::new(|| {
    let mut files: Vec<(String, Vec<u8>)> = fs::read_dir(Path::new(ASSETS).join("item_definition"))
        .expect("the item definitions are in the assets")
        .map(|entry| {
            let path = entry.expect("the directory entry is readable").path();
            let bytes = fs::read(&path).expect("the definition is readable");
            (path.display().to_string(), bytes)
        })
        .collect();
    files.sort();
    let baked = bake(
        files
            .iter()
            .map(|(name, bytes)| DefinitionFile { name, bytes }),
        &Registries::load(),
    )
    .expect("the item definitions bake");
    let identifiers = (0..ITEMS)
        .map(|id| {
            baked
                .table
                .name(NameTable::Item, id)
                .expect("each item id has a name")
                .to_string()
        })
        .collect();
    Corpus { baked, identifiers }
});

fn prototypes() -> impl Iterator<Item = &'static Prototype> {
    (0..ITEMS).map(|id| {
        CORPUS
            .baked
            .table
            .prototype(Kind(id))
            .expect("each item id below the item count has a prototype")
    })
}

fn count(predicate: impl Fn(&Prototype) -> bool) -> usize {
    prototypes()
        .filter(|prototype| predicate(prototype))
        .count()
}

#[test]
fn the_item_ids_of_the_definitions_are_dense_and_name_each_item() {
    let table = CORPUS.baked.table;
    assert!(
        table.prototype(Kind(ITEMS)).is_none(),
        "STK-SHP-001 counts 1686 items"
    );
    for (id, identifier) in (0..ITEMS).zip(&CORPUS.identifiers) {
        let name = ResourceLocation::read(identifier).expect("an item name is an identifier");
        assert_eq!(table.id(NameTable::Item, name), Some(id), "{identifier}");
    }
    let unknown = ResourceLocation::read("minecraft:no_such_item").expect("an identifier");
    assert_eq!(table.id(NameTable::Item, unknown), None);
    assert_eq!(table.id(NameTable::Block, unknown), None);
}

#[test]
fn the_prototypes_have_the_counts_of_the_specification() {
    let stack_size = |size: i32| count(|prototype| prototype.get() == Some(&MaxStackSize(size)));
    let rarity = |rarity: Rarity| count(|prototype| prototype.get() == Some(&rarity));
    let has_max_damage = |prototype: &Prototype| prototype.get::<MaxDamage>().is_some();
    let modifiers = |prototype: &Prototype| {
        prototype
            .get::<AttributeModifiers>()
            .map_or(0, |modifiers| modifiers.modifiers.len())
    };
    let cases: [(&str, usize, usize); 18] = [
        (
            "STK-SHP-006 max stack size",
            count(|prototype| prototype.get::<MaxStackSize>().is_some()),
            1686,
        ),
        ("STK-SHP-006 size 64", stack_size(64), 1368),
        ("STK-SHP-006 size 1", stack_size(1), 249),
        ("STK-SHP-006 size 16", stack_size(16), 69),
        ("STK-SHP-007 max damage", count(has_max_damage), 84),
        (
            "STK-SHP-008 damage",
            count(|prototype| prototype.get::<Damage>().is_some()),
            84,
        ),
        (
            "STK-PRO-006 damage 0 and size 1 with max damage",
            count(|prototype| {
                has_max_damage(prototype)
                    && prototype.get() == Some(&Damage(0))
                    && prototype.get() == Some(&MaxStackSize(1))
            }),
            84,
        ),
        (
            "STK-SHP-009 unbreakable",
            count(|prototype| prototype.get::<Unbreakable>().is_some()),
            0,
        ),
        (
            "STK-SHP-011 item name",
            count(|prototype| prototype.get::<ItemName>().is_some()),
            1686,
        ),
        ("STK-SHP-014 common", rarity(Rarity::Common), 1570),
        ("STK-SHP-014 uncommon", rarity(Rarity::Uncommon), 79),
        ("STK-SHP-014 rare", rarity(Rarity::Rare), 18),
        ("STK-SHP-014 epic", rarity(Rarity::Epic), 19),
        (
            "STK-SHP-016 no modifier",
            count(|prototype| modifiers(prototype) == 0),
            1593,
        ),
        (
            "STK-SHP-016 1 to 3 modifiers",
            count(|prototype| (1..=3).contains(&modifiers(prototype))),
            93,
        ),
        (
            "STK-SHP-019 food",
            count(|prototype| prototype.get::<Food>().is_some()),
            44,
        ),
        (
            "STK-SHP-019 can always eat",
            count(|prototype| {
                prototype
                    .get::<Food>()
                    .is_some_and(|food| food.can_always_eat)
            }),
            5,
        ),
        (
            "STK-SHP-020 tool",
            count(|prototype| prototype.get::<Tool>().is_some()),
            38,
        ),
    ];
    for (rule, actual, expected) in cases {
        assert_eq!(actual, expected, "{rule}");
    }
}

#[test]
fn each_prototype_has_the_neutral_value_of_each_neutral_type() {
    for (prototype, identifier) in prototypes().zip(&CORPUS.identifiers) {
        assert!(
            prototype.get::<Rarity>().is_some(),
            "STK-SHP-014: {identifier}"
        );
        assert!(
            prototype.get::<AttributeModifiers>().is_some(),
            "STK-SHP-016: {identifier}"
        );
        assert!(
            prototype.is_neutral::<RepairCost>(),
            "STK-SHP-018: {identifier}"
        );
        assert!(prototype.is_neutral::<Lore>(), "STK-SHP-013: {identifier}");
        assert!(
            prototype.is_neutral::<Enchantments>(),
            "STK-SHP-015: {identifier}"
        );
        assert!(
            prototype.is_neutral::<TooltipDisplay>(),
            "STK-SHP-017: {identifier}"
        );
        let model = prototype
            .get::<ItemModel>()
            .map(|model| model.get().to_string());
        assert_eq!(model.as_ref(), Some(identifier), "STK-SHP-012");
    }
}
