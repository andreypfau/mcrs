//! The selected parts, the merged models and the chosen variants agree with the block model
//! artefact of the reference client.

use std::collections::{BTreeMap, BTreeSet};

use mcrs_minecraft_model::bake::BlockModels;
use mcrs_minecraft_model::block_state_file::{MISSING_MODEL, Variant};
use mcrs_minecraft_model::model_file::{DisplayContext, GuiLight};
use mcrs_minecraft_model::model_resolver::ModelResolver;
use mcrs_minecraft_model::property_table::{PropertyDomain, PropertyTable};
use mcrs_minecraft_fixtures::fixture;
use serde::Deserialize;

use crate::client_pack::client_pack;

const DISPLAY_TOLERANCE: f32 = 1e-6;

#[derive(Deserialize)]
struct ModelsArtefact {
    blocks: Vec<BlockEntry>,
    display_table: Vec<BTreeMap<String, TransformEntry>>,
    models: Vec<ModelEntry>,
    picks: Vec<PickEntry>,
}

#[derive(Deserialize)]
struct BlockEntry {
    block: String,
    part_table: Vec<Vec<VariantEntry>>,
    states: Vec<StateEntry>,
}

#[derive(Deserialize)]
struct StateEntry {
    state: String,
    parts: Vec<usize>,
    #[serde(default)]
    missing: bool,
}

#[derive(Deserialize, Clone, PartialEq, Eq, Debug)]
struct VariantEntry {
    model: String,
    x: u16,
    y: u16,
    z: u16,
    uvlock: bool,
    weight: u32,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
    geometry_from: Option<String>,
    ambient_occlusion: bool,
    gui_light: String,
    textures: BTreeMap<String, TextureEntry>,
    #[serde(default)]
    unresolved: BTreeSet<String>,
    display: Option<usize>,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
struct TextureEntry {
    sprite: String,
    #[serde(default)]
    force_translucent: bool,
}

#[derive(Deserialize)]
struct TransformEntry {
    rotation: [f32; 3],
    translation: [f32; 3],
    scale: [f32; 3],
}

#[derive(Deserialize)]
struct PickEntry {
    block: String,
    state: String,
    position: [i32; 3],
    seed: i64,
    chosen: Vec<usize>,
}

fn pairs(state: &str) -> Vec<(&str, &str)> {
    state
        .split(',')
        .filter(|term| !term.is_empty())
        .map(|term| term.split_once('=').unwrap())
        .collect()
}

/// The properties of a block, as the states of the artefact name them.
fn properties(block: &BlockEntry) -> PropertyTable {
    let mut table = PropertyTable::default();
    for state in &block.states {
        for (name, value) in pairs(&state.state) {
            let index = match table.index_of(name) {
                Some(index) => index,
                None => {
                    table.properties.push(PropertyDomain {
                        name: name.to_owned(),
                        values: Vec::new(),
                    });
                    table.properties.len() - 1
                }
            };
            let domain = &mut table.properties[index];
            if !domain.values.iter().any(|known| known == value) {
                domain.values.push(value.to_owned());
            }
        }
    }
    table
}

fn entry(variant: &Variant) -> VariantEntry {
    VariantEntry {
        model: variant.model.clone(),
        x: variant.x,
        y: variant.y,
        z: variant.z,
        uvlock: variant.uvlock,
        weight: variant.weight,
    }
}

#[test]
fn block_models_agree_with_the_reference_client() {
    let artefact: ModelsArtefact = serde_json::from_str(&fixture("block_models").text()).unwrap();
    let mut models = BlockModels::new(client_pack());
    selected_parts_agree(&artefact, &mut models);
    chosen_variants_agree(&artefact, &mut models);
    merged_models_agree(&artefact);
}

fn selected_parts_agree(artefact: &ModelsArtefact, models: &mut BlockModels<'_>) {
    let mut failures = Vec::new();
    for block in &artefact.blocks {
        let table = properties(block);
        for state in &block.states {
            let values = table.state(&pairs(&state.state)).unwrap();
            let parts = models.parts(&block.block, &table, &values);
            let ours: Vec<Vec<VariantEntry>> = parts
                .parts
                .iter()
                .map(|part| part.iter().map(entry).collect())
                .collect();
            let draws_missing =
                ours.len() == 1 && ours[0].len() == 1 && ours[0][0].model == MISSING_MODEL;
            let expected: Vec<Vec<VariantEntry>> = state
                .parts
                .iter()
                .map(|index| block.part_table[*index].clone())
                .collect();
            if (state.missing && !draws_missing) || (!state.missing && ours != expected) {
                failures.push(format!("{}[{}]", block.block, state.state));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} states disagree, first {:?}; content errors {:?}",
        failures.len(),
        &failures[..failures.len().min(5)],
        models.take_errors()
    );
}

fn chosen_variants_agree(artefact: &ModelsArtefact, models: &mut BlockModels<'_>) {
    let mut failures = Vec::new();
    for pick in &artefact.picks {
        let block = artefact
            .blocks
            .iter()
            .find(|block| block.block == pick.block)
            .unwrap();
        let table = properties(block);
        let values = table.state(&pairs(&pick.state)).unwrap();
        let state_pick = models
            .parts(&block.block, &table, &values)
            .pick(&table, &values);
        let seed = state_pick.seed(pick.position);
        let chosen = state_pick.chosen(pick.position);
        if seed != pick.seed || chosen != pick.chosen {
            failures.push(format!(
                "{}[{}] at {:?}: seed {seed} chosen {chosen:?}, expected {} {:?}",
                pick.block, pick.state, pick.position, pick.seed, pick.chosen
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} picks disagree, first {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

fn merged_models_agree(artefact: &ModelsArtefact) {
    let mut resolver = ModelResolver::new(client_pack());
    let mut failures = Vec::new();
    for expected in &artefact.models {
        let model = resolver.resolve(&expected.id);
        let mut wrong = Vec::new();
        if model.geometry_from != expected.geometry_from {
            wrong.push(format!("geometry_from {:?}", model.geometry_from));
        }
        if model.ambient_occlusion != expected.ambient_occlusion {
            wrong.push("ambient_occlusion".to_owned());
        }
        let gui_light = match model.gui_light {
            GuiLight::Front => "front",
            GuiLight::Side => "side",
        };
        if gui_light != expected.gui_light {
            wrong.push(format!("gui_light {gui_light}"));
        }
        let textures: BTreeMap<String, TextureEntry> = model
            .textures
            .iter()
            .map(|(slot, texture)| {
                (
                    slot.clone(),
                    TextureEntry {
                        sprite: texture.sprite.clone(),
                        force_translucent: texture.force_translucent,
                    },
                )
            })
            .collect();
        if textures != expected.textures {
            wrong.push(format!("textures {textures:?}"));
        }
        if model.unresolved != expected.unresolved {
            wrong.push(format!("unresolved {:?}", model.unresolved));
        }
        let table = expected.display.map(|index| &artefact.display_table[index]);
        for context in DisplayContext::ALL {
            let ours = model.display(context);
            let theirs = table.and_then(|table| table.get(context.name()));
            let agrees = match (ours, theirs) {
                (None, None) => true,
                (Some(ours), Some(theirs)) => [
                    (ours.rotation_deg, theirs.rotation),
                    (ours.translation, theirs.translation),
                    (ours.scale, theirs.scale),
                ]
                .iter()
                .all(|(ours, theirs)| {
                    ours.to_array()
                        .iter()
                        .zip(theirs)
                        .all(|(a, b)| (a - b).abs() <= DISPLAY_TOLERANCE)
                }),
                _ => false,
            };
            if !agrees {
                wrong.push(format!("display {} {ours:?}", context.name()));
            }
        }
        if !wrong.is_empty() {
            failures.push(format!("{}: {}", expected.id, wrong.join(", ")));
        }
    }
    assert!(
        failures.is_empty(),
        "{} models disagree, first {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}
