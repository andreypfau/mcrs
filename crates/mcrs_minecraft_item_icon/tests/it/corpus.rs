use std::fs;
use std::sync::LazyLock;

use mcrs_minecraft_creative_content::CreativeContent;
use mcrs_minecraft_creative_content::OperatorCondition;
use mcrs_minecraft_creative_content::SessionInputs;
use mcrs_minecraft_creative_report::CreativeReport;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_engine_registries::test_item_table;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item_icon::{IconMeshes, IconSources, ItemIcon, resolve};
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_model::atlas::SpriteRegistry;
use mcrs_minecraft_model::item_model_asset::DisplayContext;
use mcrs_minecraft_model::item_model_bake::{ItemModels, bake_all};
use mcrs_minecraft_model::item_model_context::ItemModelContext;
use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_model::stack_view::StackView;
use mcrs_minecraft_resource::ResourceLocation;

const CREATIVE_REPORT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/creative_tabs.json"
);

pub static TABLE: LazyLock<ItemTable> = LazyLock::new(test_item_table);

pub static MODELS: LazyLock<ItemModels> = LazyLock::new(|| {
    bake_all(ResourceSet::corpus(), &mut SpriteRegistry::default())
        .expect("the item models of the corpus bake")
});

pub static MESHES: LazyLock<IconMeshes> = LazyLock::new(|| IconMeshes::build(&MODELS));

pub static SEARCH: LazyLock<Vec<ItemStack>> = LazyLock::new(|| {
    let bytes = fs::read(CREATIVE_REPORT).expect("the creative report is in the assets");
    let report = CreativeReport::read(&bytes).expect("the creative report is valid");
    let registries = test_registries();
    let vanilla = [ResourceLocation::read("minecraft:vanilla")
        .expect("a valid feature id")
        .to_buf()];
    let content = CreativeContent::build(
        &report,
        SessionInputs {
            names: &RegistryNames::new(registries, &TABLE),
            items: &TABLE,
            registries,
            enabled_features: &vanilla[..],
            operator: OperatorCondition::Met,
        },
    );
    content.search_list().to_vec()
});

pub const GUI: ItemModelContext = ItemModelContext::new(DisplayContext::Gui);

pub fn stack(path: &str, count: u8) -> ItemStack {
    let name = format!("minecraft:{path}");
    let location = ResourceLocation::read(&name).expect("the test names a valid id");
    let kind = Kind(
        TABLE
            .id(NameTable::Item, location)
            .expect("the test names an item"),
    );
    ItemStack::new(kind, Count::new(count).expect("the test count is valid"))
}

pub fn view(stack: &ItemStack) -> StackView<'_> {
    StackView {
        stack,
        table: &TABLE,
    }
}

pub fn sources() -> IconSources<'static> {
    IconSources {
        models: &MODELS,
        registries: None,
    }
}

pub fn session_sources() -> IconSources<'static> {
    IconSources {
        models: &MODELS,
        registries: Some(test_registries()),
    }
}

pub fn icon(stack: &ItemStack) -> ItemIcon {
    resolve(view(stack), &sources(), &GUI)
}
