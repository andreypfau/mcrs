use std::collections::BTreeMap;
use std::io::Cursor;

use mcrs_minecraft_biome_file::BiomeFile;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_worldgen_builtin as builtin;
use mcrs_minecraft_worldgen_feature::template::{PaletteState, Template};
use mcrs_minecraft_worldgen_testing::{same_json, vanilla_dir};

#[derive(Default)]
struct Differences(Vec<String>);

impl Differences {
    fn compare(&mut self, path: &str, same: impl FnOnce(&[u8]) -> bool) {
        match std::fs::read(vanilla_dir().join(path)) {
            Ok(shipped) if same(&shipped) => {}
            Ok(_) => self.0.push(format!("{path} differs from the game's file")),
            Err(error) => self
                .0
                .push(format!("{path}: the game ships no file: {error}")),
        }
    }

    fn json<T: serde::Serialize>(&mut self, folder: &str, built: BTreeMap<ResourceLocation, T>) {
        for (id, value) in built {
            let built = serde_json::to_vec(&value).unwrap_or_else(|e| panic!("{id}: {e}"));
            self.compare(
                &format!("worldgen/{folder}/{}.json", id.path()),
                |shipped| same_json(&built, shipped),
            );
        }
    }
}

type TemplateCells = BTreeMap<[i32; 3], (PaletteState, Option<NbtCompound>)>;

fn template_cells(template: &Template) -> Option<TemplateCells> {
    let palette = template.palette.as_ref()?;
    template
        .blocks
        .iter()
        .map(|block| {
            let mut state = palette.get(usize::try_from(block.state).ok()?)?.clone();
            state.properties = state.properties.filter(|properties| !properties.is_empty());
            Some((block.pos, (state, block.nbt.clone())))
        })
        .collect()
}

// Two saves of one structure differ in palette order and block order.
fn same_template(built: &Template, shipped: &[u8]) -> bool {
    let Ok(shipped) = from_gzip_bytes::<Template, _>(Cursor::new(shipped)) else {
        return false;
    };
    if built.palettes.is_some() || shipped.palettes.is_some() {
        return *built == shipped;
    }
    built.size == shipped.size
        && built.data_version == shipped.data_version
        && built.entities == shipped.entities
        && built.blocks.len() == shipped.blocks.len()
        && template_cells(built).is_some_and(|cells| Some(cells) == template_cells(&shipped))
}

#[test]
fn every_built_in_entry_equals_the_file_the_game_ships_for_it() {
    let mut differences = Differences::default();
    differences.json("noise", builtin::vanilla_noises());
    differences.json("density_function", builtin::vanilla_density_functions());
    differences.json("noise_settings", builtin::vanilla_noise_settings());
    differences.json("template_pool", builtin::template_pools());

    let biomes = builtin::biomes().unwrap();
    assert!(!biomes.is_empty());
    for (id, built) in &biomes {
        differences.compare(&format!("worldgen/biome/{}.json", id.path()), |shipped| {
            serde_json::from_slice::<BiomeFile<Named>>(shipped)
                .is_ok_and(|shipped| shipped == *built)
        });
    }

    let templates = builtin::templates();
    assert!(!templates.is_empty());
    for (id, built) in &templates {
        differences.compare(&format!("structure/{}.nbt", id.path()), |shipped| {
            same_template(built, shipped)
        });
    }

    assert!(
        differences.0.is_empty(),
        "{} built-in entries are not the game's:\n{}",
        differences.0.len(),
        differences.0.join("\n")
    );
}
