mod area_view;
mod arguments;
mod game_data;
mod image;
mod outputs;

use std::env;
use std::fs;
use std::process::ExitCode;

use anyhow::Context;
use mcrs_minecraft_worldgen_generator::generate_area;

use crate::area_view::AreaView;
use crate::arguments::{Arguments, Slice};
use crate::game_data::GameData;
use crate::image::Image;
use crate::outputs::{ClimateGrid, write_climate, write_world};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

const AREA_TOO_LARGE: &str = "the area does not fit block coordinates";

fn block_range(from: i32, to: i32) -> Option<(i32, i32)> {
    Some((from.checked_mul(16)?, to.checked_mul(16)?.checked_add(15)?))
}

fn run() -> anyhow::Result<()> {
    let arguments = Arguments::parse(env::args_os().skip(1))?;
    let data = GameData::load(&arguments)?;
    let (min_x, _) = block_range(arguments.from.x, arguments.to.x).context(AREA_TOO_LARGE)?;
    let (min_z, _) = block_range(arguments.from.z, arguments.to.z).context(AREA_TOO_LARGE)?;
    let chunks = |from: i32, to: i32| u32::try_from(to.saturating_sub(from).saturating_add(1));
    let width = chunks(arguments.from.x, arguments.to.x)?.saturating_mul(16);
    let depth = chunks(arguments.from.z, arguments.to.z)?.saturating_mul(16);
    if let Some(file) = &arguments.climate {
        let step = u32::try_from(arguments.climate_step)?;
        let grid = ClimateGrid {
            x0: min_x,
            z0: min_z,
            step: arguments.climate_step,
            nx: i32::try_from(width.div_ceil(step))?,
            nz: i32::try_from(depth.div_ceil(step))?,
        };
        grid.nx
            .checked_mul(grid.nz)
            .and_then(|points| points.checked_mul(6))
            .context("the climate grid has too many samples, raise --climate-step")?;
        write_climate(&data, &grid, file)?;
    }
    let columns = generate_area(&data.fill, arguments.from, arguments.to);
    let view = AreaView {
        data: &data,
        columns: &columns,
        min_x,
        min_z,
        width,
        depth,
    };

    if let Some(world) = &arguments.world {
        write_world(&data, &columns, world)?;
    }

    fs::create_dir_all(&arguments.out).with_context(|| arguments.out.display().to_string())?;
    let write = |name: &str, image: &Image| -> anyhow::Result<()> {
        let path = arguments.out.join(name);
        fs::write(&path, image.png()?).with_context(|| path.display().to_string())
    };
    write("height.png", &view.height_map())?;
    write("top_block.png", &view.top_block_map())?;
    let biomes = view.biome_map();
    write("biome.png", &biomes.image)?;
    for slice in &arguments.slices {
        let name = match slice {
            Slice::X(x) => format!("slice_x{x}.png"),
            Slice::Z(z) => format!("slice_z{z}.png"),
        };
        write(&name, &view.slice(*slice))?;
    }
    for biome in biomes.seen {
        let [red, green, blue, _] = view.biome_color(biome).0;
        if let Some(name) = view.biome_name(biome) {
            println!("#{red:02x}{green:02x}{blue:02x} {name}");
        }
    }
    Ok(())
}
