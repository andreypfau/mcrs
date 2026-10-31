use std::path::PathBuf;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_datapack::{Level, PackDiagnostic};
use mcrs_minecraft_registries::{Diagnostic, Missing, Referrer, RegistryAdditions};
use mcrs_minecraft_world::registry_load::{RegistryLoadError, load_registries, test_additions};

use crate::common::directory_packs;

const METADATA: &str = r#"{"pack":{"description":"t","min_format":124,"max_format":124}}"#;
const OVERLAID: &str = r#"{"pack":{"description":"t","min_format":124,"max_format":124},"overlays":{"entries":[{"directory":"o","min_format":124,"max_format":124}]}}"#;

fn rl(text: &str) -> ResourceLocation {
    ResourceLocation::read(text).unwrap()
}

fn entry(registry: &str, pack: &str, file: &str, field: Option<&str>) -> Diagnostic {
    Diagnostic::Entry {
        registry: rl(registry),
        entry: rl("t:bad"),
        pack: Some(format!("file/{pack}")),
        file: Some(file.to_owned()),
        field: field.map(str::to_owned),
        message: String::new(),
    }
}

fn unworded(diagnostic: &Diagnostic) -> Diagnostic {
    let mut diagnostic = diagnostic.clone();
    match &mut diagnostic {
        Diagnostic::Entry { message, .. } => message.clear(),
        Diagnostic::Pack(PackDiagnostic::FilterIgnored { reason, .. }) => reason.clear(),
        _ => {}
    }
    diagnostic
}

fn write_pack(name: &str, metadata: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("broken_packs")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    for (path, text) in std::iter::once(&("pack.mcmeta", metadata)).chain(files) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    root
}

#[test]
fn each_broken_pack_over_vanilla_reports_its_one_error_with_pack_file_and_field() {
    let biome = r#"{"temperature":0.5,"downfall":0.5,"has_precipitation":true,"effects":{"water_color":true}}"#;
    let dimension = r#"{"type":"t:missing","generator":{"type":"minecraft:noise","biome_source":{"type":"minecraft:fixed","biome":"minecraft:plains"},"settings":"minecraft:overworld"}}"#;
    let filter = r#"{"pack":{"description":"t","min_format":124,"max_format":124},"filter":{"block":[{"path":"("}]}}"#;
    let rows: [(&str, &str, &[(&str, &str)], bool, Diagnostic); 6] = [
        (
            "broken_json",
            METADATA,
            &[("data/t/damage_type/bad.json", "not json")],
            true,
            entry(
                "minecraft:damage_type",
                "broken_json",
                "data/t/damage_type/bad.json",
                None,
            ),
        ),
        (
            "wrong_type",
            METADATA,
            &[("data/t/worldgen/biome/bad.json", biome)],
            true,
            entry(
                "minecraft:worldgen/biome",
                "wrong_type",
                "data/t/worldgen/biome/bad.json",
                Some("effects.water_color"),
            ),
        ),
        (
            "earlier_stage",
            METADATA,
            &[("data/t/dimension/bad.json", dimension)],
            true,
            entry(
                "minecraft:dimension",
                "earlier_stage",
                "data/t/dimension/bad.json",
                Some("type"),
            ),
        ),
        (
            "invalid_name",
            OVERLAID,
            &[(
                "o/data/t/damage_type/Bad.json",
                r#"{"message_id":"a","exhaustion":0.1}"#,
            )],
            false,
            Diagnostic::Pack(PackDiagnostic::InvalidFileName {
                pack: "file/invalid_name".to_owned(),
                zip: false,
                namespace: "t".to_owned(),
                path: "damage_type/Bad.json".to_owned(),
            }),
        ),
        (
            "absent_member",
            METADATA,
            &[("data/t/tags/block/bad.json", r#"{"values":["t:nope"]}"#)],
            false,
            Diagnostic::TagFailed {
                registry: rl("minecraft:block"),
                tag: rl("t:bad"),
                missing: vec![Missing {
                    name: rl("t:nope"),
                    tag: false,
                    referrer: Referrer::TagFile {
                        tag: rl("t:bad"),
                        pack: "file/absent_member".to_owned(),
                    },
                }],
            },
        ),
        (
            "bad_filter",
            filter,
            &[],
            false,
            Diagnostic::Pack(PackDiagnostic::FilterIgnored {
                pack: "file/bad_filter".to_owned(),
                reason: String::new(),
            }),
        ),
    ];
    for (name, metadata, files, fails, expected) in rows {
        let packs = directory_packs()
            .with_only_pack(&write_pack(name, metadata, files))
            .unwrap();
        let (failed, diagnostics) = match load_registries(&packs, with_local_pack(name)) {
            Ok((_, diagnostics)) => (false, diagnostics),
            Err(RegistryLoadError::Load(diagnostics)) => (true, diagnostics),
            Err(error) => panic!("{name}: {error}"),
        };
        let errors: Vec<Diagnostic> = (diagnostics.iter())
            .filter(|diagnostic| diagnostic.level() == Level::Error)
            .map(unworded)
            .collect();
        assert_eq!(errors, [expected], "{name}:\n{diagnostics}");
        assert_eq!(failed, fails, "{name}: whether the load fails");
    }
}

fn with_local_pack(name: &str) -> RegistryAdditions {
    let mut additions = test_additions();
    additions.local_packs.push(name.to_owned());
    additions
}
