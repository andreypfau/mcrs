use crate::registries::{static_registries, world_registries};
use mcrs_minecraft_registry::{
    Built, KnownPackEntries, LoadReport, PACKS_ROOT, Pack, PackFile, RegistrySet, VANILLA_PACK,
    WorldRegistries,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const DATAPACK_REPORT: &str = "mcrs/reports/datapack.json";

pub(crate) struct Directory {
    pub(crate) path: PathBuf,
    reads_bytes: bool,
}

/// The listing rules of one pack, shared by every reader. A reader supplies
/// directory walks and file reads; which namespaces, registry directories, tag
/// directories and built-in entries a pack has is decided only here.
pub(crate) struct PackScan<'a> {
    vanilla: bool,
    registries: &'a WorldRegistries,
    tag_directories: BTreeSet<&'a str>,
    found: BTreeMap<String, bool>,
}

impl<'a> PackScan<'a> {
    pub(crate) fn new(
        vanilla: bool,
        registries: &'a WorldRegistries,
        statics: &'a RegistrySet,
    ) -> Self {
        let tag_directories = statics
            .tables()
            .map(|table| table.registry().path())
            .chain(registries.declared().map(|registry| registry.path()))
            .collect();
        PackScan {
            vanilla,
            registries,
            tag_directories,
            found: BTreeMap::new(),
        }
    }

    pub(crate) fn keeps_namespace(&self, namespace: &str) -> bool {
        !(self.vanilla && namespace == "mcrs")
    }

    pub(crate) fn directories(&mut self, namespace: &str) -> Vec<Directory> {
        let registries = self.registries;
        let mut directories = Vec::new();
        for registry in registries.declared() {
            let reads_bytes = registries.parses(registry.as_str());
            if self.vanilla {
                let directory = format!("{namespace}/{}", registry.path());
                for path in mcrs_minecraft_worldgen_builtin::paths(&directory) {
                    *self.found.entry(path).or_default() |= reads_bytes;
                }
            }
            directories.push(Directory {
                path: Path::new(namespace).join(registry.path()),
                reads_bytes,
            });
        }
        for directory in &self.tag_directories {
            directories.push(Directory {
                path: Path::new(namespace).join("tags").join(directory),
                reads_bytes: true,
            });
        }
        directories
    }

    pub(crate) fn record(&mut self, directory: &Directory, root: &Path, file: &Path) {
        if let Some(path) = relative_to(root, file) {
            *self.found.entry(path).or_default() |= directory.reads_bytes;
        }
    }

    /// The files of the pack with whether their bytes are read, and the
    /// entries the pack carries as code.
    pub(crate) fn finish(self) -> (Vec<(String, bool)>, Vec<Built>) {
        let built = if self.vanilla
            && self
                .registries
                .parses(mcrs_minecraft_biome::keys::BIOME.location().as_static_str())
        {
            vec![mcrs_minecraft_worldgen_builtin::built_biomes()]
        } else {
            Vec::new()
        };
        (self.found.into_iter().collect(), built)
    }
}

fn relative_to(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .and_then(Path::to_str)
        .map(|path| path.replace('\\', "/"))
}

/// Loads the world registries from a data pack report and a reader of packs.
/// The reader is called once with the world registries over the statics, then
/// once with the reloadable registries over what the first load produced.
pub fn load_registry_set(
    statics: RegistrySet,
    datapack_report: &[u8],
    mut read: impl FnMut(&WorldRegistries, &RegistrySet) -> Result<Vec<Pack>, LoadReport>,
) -> Result<RegistrySet, LoadReport> {
    let world = world_registries(datapack_report)?;
    let packs = read(&world, &statics)?;
    let loaded = world.load(&statics, &packs)?;
    let reloadable = crate::registries::reloadable_registries(datapack_report)?;
    let packs = read(&reloadable, &loaded)?;
    reloadable.load(&loaded, &packs)
}

/// Loads the corpus under `root`: the vanilla data pack at `root` and every
/// pack under `root/mcrs/datapacks`, in name order.
pub fn load_from_directory(root: &Path) -> Result<RegistrySet, LoadReport> {
    let path = root.join(DATAPACK_REPORT);
    let report = std::fs::read(&path)
        .map_err(|error| LoadReport::invalid(format_args!("{}: {error}", path.display())))?;
    load_registry_set(static_registries()?, &report, |registries, statics| {
        read_packs_from_directory(root, registries, statics)
    })
}

/// The network form of the entries of the vanilla pack under `root`, built
/// entries included. Loads the pack alone and drops the set it loaded.
pub fn known_pack_entries(root: &Path) -> Result<KnownPackEntries, LoadReport> {
    let path = root.join(DATAPACK_REPORT);
    let report = std::fs::read(&path)
        .map_err(|error| LoadReport::invalid(format_args!("{}: {error}", path.display())))?;
    let statics = static_registries()?;
    let world = world_registries(&report)?;
    let mut problems = LoadReport::new();
    let vanilla = vec![read_pack(
        root,
        VANILLA_PACK,
        true,
        &world,
        &statics,
        &mut problems,
    )];
    if !problems.is_empty() {
        return Err(problems);
    }
    world
        .load(&statics, &vanilla)
        .map(|loaded| KnownPackEntries::from_set(&loaded))
}

pub fn read_packs_from_directory(
    root: &Path,
    registries: &WorldRegistries,
    statics: &RegistrySet,
) -> Result<Vec<Pack>, LoadReport> {
    let mut report = LoadReport::new();
    let mut packs = vec![read_pack(
        root,
        VANILLA_PACK,
        true,
        registries,
        statics,
        &mut report,
    )];
    for name in pack_names(root) {
        let base = root.join(PACKS_ROOT).join(&name);
        packs.push(read_pack(
            &base,
            &name,
            false,
            registries,
            statics,
            &mut report,
        ));
    }
    if report.is_empty() {
        Ok(packs)
    } else {
        Err(report)
    }
}

fn pack_names(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root.join(PACKS_ROOT))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    names
}

fn read_pack(
    base: &Path,
    name: &str,
    vanilla: bool,
    registries: &WorldRegistries,
    statics: &RegistrySet,
    report: &mut LoadReport,
) -> Pack {
    let mut scan = PackScan::new(vanilla, registries, statics);
    let namespaces: Vec<String> = std::fs::read_dir(base)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|namespace| scan.keeps_namespace(namespace))
        .collect();
    for namespace in &namespaces {
        for directory in scan.directories(namespace) {
            for file in walk_files(&base.join(&directory.path)) {
                scan.record(&directory, base, &file);
            }
        }
    }
    let (listing, built) = scan.finish();
    let files = listing
        .into_iter()
        .map(|(path, reads_bytes)| {
            let bytes = reads_bytes
                .then(|| match read_file(base, &path, vanilla) {
                    Ok(bytes) => Some(bytes),
                    Err(error) => {
                        report.invalid_report(format_args!(
                            "{}: {error}",
                            base.join(&path).display()
                        ));
                        None
                    }
                })
                .flatten();
            PackFile { path, bytes }
        })
        .collect();
    Pack {
        name: name.to_owned(),
        files,
        built,
    }
}

fn read_file(base: &Path, path: &str, vanilla: bool) -> std::io::Result<Vec<u8>> {
    match std::fs::read(base.join(path)) {
        // The built-in entries of the vanilla pack are listed but have no file.
        Err(error) if vanilla && error.kind() == ErrorKind::NotFound => {
            mcrs_minecraft_worldgen_builtin::asset(path).ok_or(error)
        }
        read => read,
    }
}

fn walk_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    #[test]
    fn the_corpus_loads_into_a_registry_set_without_bevy() {
        let set = load_from_directory(&assets()).unwrap_or_else(|report| panic!("{report}"));

        let report = std::fs::read(assets().join(DATAPACK_REPORT)).unwrap();
        let world = world_registries(&report).unwrap();
        for registry in world.declared() {
            assert!(
                set.table(registry.as_str()).is_some(),
                "{registry} is missing from the loaded set"
            );
        }

        let biomes = set.table("minecraft:worldgen/biome").expect("biome table");
        assert!(!biomes.is_empty());
        assert!(
            (0..biomes.len()).any(|id| set.pack_of("minecraft:worldgen/biome", id) == Some("beta")),
            "no biome comes from the beta pack"
        );
        assert!(!set.table("minecraft:loot_table").unwrap().is_empty());
    }
}
