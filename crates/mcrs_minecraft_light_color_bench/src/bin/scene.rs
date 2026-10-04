use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use mcrs_minecraft_anvil::{
    Chunk, ChunkStatus, PaletteLookup, PaletteNames, Properties, RegionFile, write_chunk,
};
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::{BlockPos, ColumnPos, RegionPos};
use mcrs_minecraft_light_color_bench::corpus::Corpus;
use mcrs_minecraft_registry::BlockStateId;
use mcrs_minecraft_worldgen_testing::assets_dir;
use serde::Deserialize;

const USAGE: &str = "\
usage: scene apply <scene.json> <source-world> <target-world>
       scene check <scene.json> <world>";

const MAX_ENTRY_BLOCKS: i64 = 1 << 16;

fn main() {
    // Run outside cargo, bevy would look for `assets` beside the executable;
    // the corpus's tags already come from the workspace tree, so its blocks must too.
    if std::env::var_os("BEVY_ASSET_ROOT").is_none() {
        // SAFETY: no other thread has started yet.
        unsafe { std::env::set_var("BEVY_ASSET_ROOT", assets_dir().join("..")) };
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["apply", scene, source, target] => {
            load(Path::new(scene)).and_then(|s| apply(&s, Path::new(source), Path::new(target)))
        }
        ["check", scene, world] => load(Path::new(scene)).and_then(|s| check(&s, Path::new(world))),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(message) = result {
        eprintln!("scene: {message}");
        std::process::exit(1);
    }
}

/// Entries apply in order, so a later box overwrites an earlier one.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneFile {
    dimension: String,
    blocks: Vec<Placement>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Placement {
    from: [i32; 3],
    to: Option<[i32; 3]>,
    id: String,
    #[serde(default)]
    properties: BTreeMap<String, String>,
}

struct Scene {
    region_dir: PathBuf,
    blocks: Vec<Placed>,
}

/// A placement resolved against the corpus and spelled as vanilla saves it: a
/// default state by bare name, any other with every property, sorted by name.
struct Placed {
    from: [i32; 3],
    to: [i32; 3],
    state: VoxelId,
    name: String,
    properties: Vec<(String, String)>,
}

impl Placed {
    fn positions(&self) -> impl Iterator<Item = BlockPos> + '_ {
        (self.from[1]..=self.to[1]).flat_map(move |y| {
            (self.from[2]..=self.to[2])
                .flat_map(move |z| (self.from[0]..=self.to[0]).map(move |x| BlockPos::new(x, y, z)))
        })
    }
}

fn load(path: &Path) -> Result<Scene, String> {
    let at = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let text = std::fs::read_to_string(path).map_err(|e| at(&e))?;
    let file: SceneFile = serde_json::from_str(&text).map_err(|e| at(&e))?;
    let region_dir = dimension_region_dir(&file.dimension).map_err(|e| at(&e))?;
    let blocks = file
        .blocks
        .iter()
        .enumerate()
        .map(|(index, placement)| place(placement).map_err(|e| at(&format!("block {index}: {e}"))))
        .collect::<Result<_, _>>()?;
    Ok(Scene { region_dir, blocks })
}

/// `dimensions/<namespace>/<path>/region`, refusing any identifier that could
/// name a directory outside the world.
fn dimension_region_dir(dimension: &str) -> Result<PathBuf, String> {
    let invalid = || format!("`{dimension}` is not a dimension identifier");
    let (namespace, path) = dimension.split_once(':').ok_or_else(invalid)?;
    let segment_ok = |s: &str| {
        !s.is_empty()
            && s != "."
            && s != ".."
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
    };
    if !segment_ok(namespace) || !path.split('/').all(segment_ok) {
        return Err(invalid());
    }
    let mut dir = PathBuf::from("dimensions");
    dir.push(namespace);
    dir.extend(path.split('/'));
    dir.push("region");
    Ok(dir)
}

fn place(placement: &Placement) -> Result<Placed, String> {
    let Placement {
        from,
        to,
        id,
        properties,
    } = placement;
    let to = to.unwrap_or(*from);
    if (0..3).any(|axis| to[axis] < from[axis]) {
        return Err(format!("`to` {to:?} is below `from` {from:?}"));
    }
    let count: i64 = (0..3)
        .map(|axis| i64::from(to[axis]) - i64::from(from[axis]) + 1)
        .product();
    if count > MAX_ENTRY_BLOCKS {
        return Err(format!(
            "covers {count} blocks, more than {MAX_ENTRY_BLOCKS}"
        ));
    }
    let block = Corpus::get()
        .blocks
        .block(id)
        .ok_or_else(|| format!("`{id}` is not a block the corpus knows"))?;
    let mut state = block.default_state_id;
    for (property, value) in properties {
        state = block
            .with_text(state, property, value)
            .ok_or_else(|| format!("`{id}` declares no `{property}={value}`"))?;
    }
    let mut spelled: Vec<(String, String)> = Vec::new();
    if state != block.default_state_id {
        spelled = block
            .properties
            .0
            .iter()
            .map(|property| {
                let value = block
                    .value_of(state, &property.name)
                    .expect("the state is the block's own");
                (property.name.to_string(), value.to_text())
            })
            .collect();
        spelled.sort();
    }
    Ok(Placed {
        from: *from,
        to,
        state: VoxelId(state.0),
        name: id.clone(),
        properties: spelled,
    })
}

struct Regions {
    dir: PathBuf,
    files: HashMap<RegionPos, Option<RegionFile>>,
}

impl Regions {
    fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            files: HashMap::new(),
        }
    }

    fn get(&mut self, pos: RegionPos) -> Result<Option<&RegionFile>, String> {
        if !self.files.contains_key(&pos) {
            let path = self.dir.join(region_file_name(pos));
            let file = if path.exists() {
                Some(RegionFile::open(&path).map_err(|e| e.to_string())?)
            } else {
                None
            };
            self.files.insert(pos, file);
        }
        Ok(self.files[&pos].as_ref())
    }

    /// Only a full chunk is one the server serves as saved.
    fn full_chunk(
        &mut self,
        pos: ColumnPos,
        blocks: &impl PaletteLookup<VoxelId>,
        biomes: &impl PaletteLookup<u8>,
    ) -> Result<Chunk, String> {
        let missing = || format!("chunk {},{} is not in the save", pos.x, pos.z);
        let region = self.get(RegionPos::from(pos))?.ok_or_else(missing)?;
        let chunk = region
            .read_chunk(pos, blocks, biomes)
            .map_err(|e| e.to_string())?
            .ok_or_else(missing)?;
        if chunk.status != ChunkStatus::Full {
            return Err(format!(
                "chunk {},{} is `{:?}`, not `{:?}`",
                pos.x,
                pos.z,
                chunk.status,
                ChunkStatus::Full
            ));
        }
        Ok(chunk)
    }
}

fn region_file_name(pos: RegionPos) -> String {
    format!("r.{}.{}.mca", pos.x, pos.z)
}

struct Edit {
    chunk: Chunk,
    blocks: PaletteNames<VoxelId>,
    biomes: PaletteNames<u8>,
    block_entities: HashSet<BlockPos>,
}

fn apply(scene: &Scene, source: &Path, target: &Path) -> Result<(), String> {
    let canonical = |world: &Path| {
        world
            .canonicalize()
            .map_err(|e| format!("{}: {e}", world.display()))
    };
    if canonical(source)? == canonical(target)? {
        return Err(format!(
            "{} is the source world; a scene is only written into a copy",
            target.display()
        ));
    }
    let mut regions = Regions::new(source.join(&scene.region_dir));
    let mut edits: BTreeMap<ColumnPos, Edit> = BTreeMap::new();
    let mut placed = 0usize;
    for (index, entry) in scene.blocks.iter().enumerate() {
        for pos in entry.positions() {
            let column = ColumnPos::from(pos);
            let edit = match edits.entry(column) {
                Entry::Occupied(e) => e.into_mut(),
                Entry::Vacant(v) => {
                    let (blocks, biomes) = (PaletteNames::new(), PaletteNames::new());
                    let chunk = regions.full_chunk(column, &blocks, &biomes)?;
                    let block_entities = chunk.block_entities.iter().filter_map(|be| {
                        Some(BlockPos::new(
                            be.get_int("x")?,
                            be.get_int("y")?,
                            be.get_int("z")?,
                        ))
                    });
                    v.insert(Edit {
                        block_entities: block_entities.collect(),
                        chunk,
                        blocks,
                        biomes,
                    })
                }
            };
            if edit.block_entities.contains(&pos) {
                return Err(format!("block {index}: {pos} holds a block entity"));
            }
            let properties = entry
                .properties
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()));
            let id = edit
                .blocks
                .intern(&entry.name, properties)
                .ok_or_else(|| format!("chunk {},{}: too many block states", column.x, column.z))?;
            edit.chunk
                .set_block(pos, id)
                .map_err(|e| format!("block {index}: {e}"))?;
            placed += 1;
        }
    }

    let mut by_region: HashMap<RegionPos, Vec<(ColumnPos, Vec<u8>)>> = HashMap::new();
    for (column, edit) in &edits {
        let nbt = write_chunk(&edit.chunk, &edit.blocks, &edit.biomes)
            .map_err(|e| format!("chunk {},{}: {e}", column.x, column.z))?;
        by_region
            .entry(RegionPos::from(*column))
            .or_default()
            .push((*column, nbt));
    }
    let target_dir = target.join(&scene.region_dir);
    for (pos, replaced) in &by_region {
        let region = regions
            .get(*pos)?
            .expect("every edited chunk was read from it");
        region
            .write(replaced, &target_dir.join(region_file_name(*pos)))
            .map_err(|e| e.to_string())?;
    }
    eprintln!(
        "placed {placed} blocks in {} chunks across {} regions",
        edits.len(),
        by_region.len()
    );
    Ok(())
}

/// Saved states resolve strictly against the corpus: a name it lacks fails the
/// read instead of turning into air.
struct CorpusStates;

impl PaletteLookup<VoxelId> for CorpusStates {
    fn resolve(&self, name: &str, properties: Properties<'_>) -> Option<VoxelId> {
        let block = Corpus::get().blocks.block(name)?;
        let mut id = block.default_state_id;
        for (property, text) in properties.iter() {
            id = block.with_text(id, property, text)?;
        }
        Some(VoxelId(id.0))
    }
}

struct AnyBiome;

impl PaletteLookup<u8> for AnyBiome {
    fn resolve(&self, _name: &str, _properties: Properties<'_>) -> Option<u8> {
        Some(0)
    }
}

fn check(scene: &Scene, world: &Path) -> Result<(), String> {
    let mut expected: Vec<(BlockPos, VoxelId)> = Vec::new();
    let mut slot: HashMap<BlockPos, usize> = HashMap::new();
    for entry in &scene.blocks {
        for pos in entry.positions() {
            match slot.get(&pos) {
                Some(&at) => expected[at].1 = entry.state,
                None => {
                    slot.insert(pos, expected.len());
                    expected.push((pos, entry.state));
                }
            }
        }
    }

    let mut regions = Regions::new(world.join(&scene.region_dir));
    let mut chunks: HashMap<ColumnPos, Chunk> = HashMap::new();
    for &(pos, want) in &expected {
        let column = ColumnPos::from(pos);
        if !chunks.contains_key(&column) {
            let chunk = regions.full_chunk(column, &CorpusStates, &AnyBiome)?;
            chunks.insert(column, chunk);
        }
        let found = state_at(&chunks[&column], pos)?;
        if found != want {
            return Err(format!(
                "{pos}: expected {}, found {}",
                describe(want),
                describe(found)
            ));
        }
    }
    eprintln!("all {} declared blocks match", expected.len());
    Ok(())
}

fn state_at(chunk: &Chunk, pos: BlockPos) -> Result<VoxelId, String> {
    let (x, y, z) = pos.into();
    chunk
        .sections
        .iter()
        .find(|section| i32::from(section.y) == y >> 4)
        .and_then(|section| section.block_states.as_ref())
        .map(|states| states.get((x & 15) as usize, (y & 15) as usize, (z & 15) as usize))
        .ok_or_else(|| format!("{pos}: the chunk holds no block states there"))
}

fn describe(state: VoxelId) -> String {
    let blocks = Corpus::get().blocks;
    let id = BlockStateId(state.0);
    let block = blocks.owner(id);
    let properties: Vec<String> = block
        .properties
        .0
        .iter()
        .filter_map(|p| {
            Some(format!(
                "{}={}",
                p.name,
                block.value_of(id, &p.name)?.to_text()
            ))
        })
        .collect();
    if properties.is_empty() {
        block.identifier.to_string()
    } else {
        format!("{}[{}]", block.identifier, properties.join(","))
    }
}
