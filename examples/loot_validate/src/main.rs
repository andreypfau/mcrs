use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::{self, BlockDefinitions, CORPUS_DIRECTORY};
use mcrs_minecraft_datapack::{DirectoryPacks, Level, PackDiagnostic, PackSource};
use mcrs_minecraft_game_registries::registry_load::load_registries;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_loot_table::{LootIssue, LootTables};
use mcrs_minecraft_registries::{Diagnostic, RegistryAdditions};

const USAGE: &str = "usage: mcrs_minecraft_loot_validate \
                     <data pack directory> <vanilla directory> <assets directory>";

const NOT_RUN: u8 = 2;

fn main() -> ExitCode {
    match run() {
        Ok(findings) => {
            for diagnostic in &findings.diagnostics {
                println!("{diagnostic}");
            }
            for issue in &findings.issues {
                println!("{issue}");
            }
            if findings.fail() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("Error: {error:#}");
            ExitCode::from(NOT_RUN)
        }
    }
}

struct Findings {
    diagnostics: Vec<Diagnostic>,
    issues: Vec<LootIssue>,
}

impl Findings {
    fn fail(&self) -> bool {
        !self.issues.is_empty()
            || self
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.level() == Level::Error)
    }
}

fn run() -> anyhow::Result<Findings> {
    let mut args = env::args_os().skip(1);
    let pack = PathBuf::from(args.next().context(USAGE)?);
    let vanilla = PathBuf::from(args.next().context(USAGE)?);
    let assets = PathBuf::from(args.next().context(USAGE)?);
    anyhow::ensure!(args.next().is_none(), USAGE);
    let packs = DirectoryPacks::new(assets.clone(), vanilla).with_pack(&pack)?;
    let name = packs
        .standard_packs()
        .first()
        .cloned()
        .context("the data pack has no name")?;
    let pack_id = format!("file/{name}");
    let (registries, diagnostics) = load_registries(&packs, RegistryAdditions::default())?;
    let mut reported = Vec::new();
    for diagnostic in diagnostics {
        if let Diagnostic::Pack(
            PackDiagnostic::MissingPack { pack }
            | PackDiagnostic::UnavailablePack { pack, .. }
            | PackDiagnostic::UnknownCompatibility { pack, .. },
        ) = &diagnostic
            && *pack == pack_id
        {
            anyhow::bail!("{diagnostic}");
        }
        let other_pack = named_pack(&diagnostic).is_some_and(|pack| pack != pack_id);
        if diagnostic.level() != Level::Info && !other_pack {
            reported.push(diagnostic);
        }
    }
    let blocks = block_definitions(&assets)?;
    let (_, issues) = LootTables::bake(&blocks, &registries, &ItemTable::leak(Vec::new()));
    Ok(Findings {
        diagnostics: reported,
        issues,
    })
}

fn named_pack(diagnostic: &Diagnostic) -> Option<&str> {
    match diagnostic {
        Diagnostic::Pack(
            PackDiagnostic::MissingPack { pack }
            | PackDiagnostic::UnavailablePack { pack, .. }
            | PackDiagnostic::UnknownCompatibility { pack, .. }
            | PackDiagnostic::OverlaySkipped { pack, .. }
            | PackDiagnostic::PackFeaturesDisabled { pack, .. }
            | PackDiagnostic::PackFound { pack }
            | PackDiagnostic::PackNotAppended { pack, .. }
            | PackDiagnostic::FeaturePackAppended { pack }
            | PackDiagnostic::FilterIgnored { pack, .. }
            | PackDiagnostic::InvalidFileName { pack, .. },
        )
        | Diagnostic::TagFileSkipped { pack, .. }
        | Diagnostic::Entry {
            pack: Some(pack), ..
        } => Some(pack),
        _ => None,
    }
}

fn block_definitions(assets: &Path) -> anyhow::Result<BlockDefinitions> {
    let directory = assets.join(CORPUS_DIRECTORY);
    let mut files = Vec::new();
    for entry in fs::read_dir(&directory).with_context(|| directory.display().to_string())? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let bytes = fs::read(&path).with_context(|| path.display().to_string())?;
            files.push((path.display().to_string(), bytes));
        }
    }
    let names: Vec<&str> = Block::ALL
        .iter()
        .map(|block| block.as_static_str())
        .collect();
    let (definitions, _) = definition::build_block_definitions(&names, files)?;
    Ok(definitions)
}
