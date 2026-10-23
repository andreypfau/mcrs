//! Loads a data pack over the vanilla pack, prints every diagnostic and fails on an error.

use std::path::Path;
use std::process::ExitCode;

use mcrs_minecraft_datapack::Level;
use mcrs_minecraft_world::registry_load::{RegistryLoadError, load_registries, test_packs};

fn main() -> ExitCode {
    let Some(pack) = std::env::args_os().nth(1) else {
        eprintln!("usage: check_pack <data pack directory, holding pack.mcmeta and data/>");
        return ExitCode::FAILURE;
    };
    let loaded = test_packs()
        .with_only_pack(Path::new(&pack))
        .map_err(RegistryLoadError::from)
        .and_then(|packs| load_registries(&packs, Default::default()));
    let diagnostics = match loaded {
        Ok((_, diagnostics)) | Err(RegistryLoadError::Load(diagnostics)) => diagnostics,
        Err(error) => {
            eprintln!("Error: {error}");
            return ExitCode::FAILURE;
        }
    };
    for diagnostic in diagnostics.iter() {
        println!("{:?}: {diagnostic}", diagnostic.level());
    }
    match diagnostics.iter().any(|d| d.level() == Level::Error) {
        true => ExitCode::FAILURE,
        false => ExitCode::SUCCESS,
    }
}
