use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Context;
use glam::IVec3;
use mcrs_engine_store::{ChunkVersion, DiskStore, VersionedChunk};
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_region_disk::RegionDisk;
use mcrs_minecraft_vanilla_worldgen::stored_column;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{
    CONTINENTS, DEPTH, EROSION, RIDGES, TEMPERATURE, VEGETATION,
};
use mcrs_minecraft_worldgen_generator::staging::FilledSnapshot;
use mcrs_voxel::ColumnPos;
use serde::Serialize;

use crate::game_data::GameData;

#[derive(Serialize)]
struct Signals {
    temperature: Vec<f64>,
    vegetation: Vec<f64>,
    continentalness: Vec<f64>,
    erosion: Vec<f64>,
    depth: Vec<f64>,
    weirdness: Vec<f64>,
}

#[derive(Serialize)]
struct ClimateFile {
    x0: i32,
    z0: i32,
    step: i32,
    nx: i32,
    nz: i32,
    signals: Signals,
}

pub struct ClimateGrid {
    pub x0: i32,
    pub z0: i32,
    pub step: i32,
    pub nx: i32,
    pub nz: i32,
}

pub fn write_world(
    data: &GameData,
    columns: &BTreeMap<ColumnPos, FilledSnapshot>,
    world: &Path,
) -> anyhow::Result<()> {
    let region = world.join("region");
    let bottom = data.min_y >> 4;
    let rows = bottom..bottom + (data.height >> 4).cast_signed();
    let mut disk = RegionDisk::new(region.clone(), data.blocks.clone(), data.registries.clone());
    for (pos, snapshot) in columns {
        let blocks = snapshot.block_entities.clone();
        let chunk =
            stored_column(&data.fill, *pos, snapshot, rows.clone(), blocks).with_context(|| {
                format!(
                    "column {},{} lacks a section or its heightmaps",
                    pos.x, pos.z
                )
            })?;
        disk.write(
            *pos,
            VersionedChunk {
                version: ChunkVersion::default(),
                chunk,
            },
        );
    }
    disk.flush().with_context(|| region.display().to_string())
}

pub fn write_climate(data: &GameData, grid: &ClimateGrid, file: &Path) -> anyhow::Result<()> {
    let router = data
        .fill
        .router
        .as_ref()
        .context("the dimension has no noise router, so it has no climate")?;
    let indices = [TEMPERATURE, VEGETATION, CONTINENTS, EROSION, DEPTH, RIDGES];
    let volume = SampleGrid::new(
        IVec3::new(grid.nx, 1, grid.nz),
        IVec3::new(grid.x0, router.sea_level, grid.z0),
        IVec3::splat(grid.step),
    );
    let points = volume.len();
    let mut values = vec![0.0f32; indices.len() * points];
    router.fill_roots(&mut Workspace::new(), &volume, &indices, &mut values);
    let mut rows = values.chunks(points).map(|row| {
        row.iter()
            .map(|value| (f64::from(*value) * 1e4).round() / 1e4)
            .collect::<Vec<f64>>()
    });
    let mut next = || rows.next().unwrap_or_default();
    let document = ClimateFile {
        x0: grid.x0,
        z0: grid.z0,
        step: grid.step,
        nx: grid.nx,
        nz: grid.nz,
        signals: Signals {
            temperature: next(),
            vegetation: next(),
            continentalness: next(),
            erosion: next(),
            depth: next(),
            weirdness: next(),
        },
    };
    let text = serde_json::to_string(&document)?;
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).with_context(|| parent.display().to_string())?;
    }
    fs::write(file, text).with_context(|| file.display().to_string())
}
