use std::collections::BTreeMap;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_registries::{Declaration, Stage};
use serde::Deserialize;

use crate::registries::typed_declarations;

const DIMENSION: &str = "minecraft:dimension";

#[derive(Deserialize)]
struct DatapackReport {
    registries: BTreeMap<ResourceLocation, Flags>,
}

#[derive(Deserialize)]
struct Flags {
    elements: bool,
    stable: bool,
}

/// The stage of every data registry the data pack report lists: one whose entries a reload may
/// change is reloadable.
fn data_registries(report: &[u8]) -> Result<BTreeMap<ResourceLocation, Stage>, String> {
    let report: DatapackReport =
        serde_json::from_slice(report).map_err(|error| format!("the data pack report: {error}"))?;
    Ok(report
        .registries
        .into_iter()
        .filter(|(_, flags)| flags.elements)
        .map(|(key, flags)| {
            let stage = match (flags.stable, key.as_str()) {
                (true, _) => Stage::Reloadable,
                (false, DIMENSION) => Stage::Dimension,
                (false, _) => Stage::World,
            };
            (key, stage)
        })
        .collect())
}

/// Every registry a load declares: the static registries, the data registries this crate reads
/// and every other data registry of the report by its names alone.
pub(crate) fn plan(report: &[u8]) -> Result<Vec<Declaration>, String> {
    let mut declarations = Vec::new();
    for (key, names) in mcrs_minecraft_registry_catalog::STATIC_REGISTRIES {
        let names = names.iter().map(|&name| name.into()).collect();
        declarations.push(Declaration::fixed((*key).into(), names));
    }

    let mut stages = data_registries(report)?;
    for declaration in typed_declarations() {
        let stage = stages.remove(declaration.key()).ok_or_else(|| {
            format!(
                "{} is not a data registry of the data pack report",
                declaration.key()
            )
        })?;
        declarations.push(declaration.in_stage(stage));
    }

    declarations.extend(
        stages
            .into_iter()
            .map(|(key, stage)| Declaration::names_only(key, stage)),
    );
    Ok(declarations)
}
