#![expect(clippy::unwrap_used, reason = "a test reports a failure by panicking")]

use mcrs_minecraft_item::definition::CORPUS_DIRECTORY;
use mcrs_minecraft_item::definition::schema::ItemDefinitionFile;
use mcrs_minecraft_item::keys::DataComponentType;
use mcrs_minecraft_item::{ComponentPatch, ItemHoverMap, Template};
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_resource::ResourceKeyBuf;
use mcrs_minecraft_text::{ItemHover, TextMap};
use mcrs_minecraft_worldgen_testing::json_files;

fn to_hover(template: &Template<Named>) -> ItemHover {
    TextMap::<Template<Named>, ItemHover>::item(&ItemHoverMap, template).unwrap()
}

fn to_template(hover: &ItemHover) -> Result<Template<Named>, String> {
    TextMap::<ItemHover, Template<Named>>::item(&ItemHoverMap, hover).map_err(|e| e.to_string())
}

#[test]
fn every_item_with_its_default_components_survives_the_wire_form() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/").to_owned() + CORPUS_DIRECTORY;
    let files = json_files(std::path::Path::new(&root));
    assert!(files.len() > 1000, "{} files", files.len());
    for path in &files {
        let file: ItemDefinitionFile = serde_json::from_slice(&std::fs::read(path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let item = file.item.description.identifier;
        if item.as_str() == "minecraft:air" {
            continue;
        }
        let mut patch = ComponentPatch::EMPTY;
        for value in file.item.components.0 {
            patch.set_value(value);
        }
        if !patch
            .added
            .iter()
            .any(|value| value.kind() == DataComponentType::CustomName)
        {
            patch.remove(DataComponentType::CustomName);
        }
        let template = Template::new(ResourceKeyBuf::from_location(item), 2, patch).unwrap();

        let hover = to_hover(&template);
        let decoded = to_template(&hover).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(decoded, template, "{}", path.display());
        assert_eq!(to_hover(&decoded), hover, "{}", path.display());
    }
}

#[test]
fn a_removal_keeps_no_fields_of_its_object() {
    let hover: ItemHover =
        serde_json::from_str(r#"{"id": "stick", "components": {"!lore": {"ignored": 1}}}"#)
            .unwrap();
    let written: ItemHover =
        serde_json::from_str(r#"{"id": "minecraft:stick", "components": {"!minecraft:lore": {}}}"#)
            .unwrap();
    assert_eq!(to_hover(&to_template(&hover).unwrap()), written);
}
