use super::*;
use mcrs_minecraft_worldgen_structure::DecorationStep::*;

pub fn end_base(generation: Generation) -> Biome {
    let mut m = Mobs::default();
    m.end_spawns();
    Biome::new(false, 0.5, 0.5)
        .spawns(m.0)
        .generation(generation)
}

pub fn the_end() -> Biome {
    let mut g = Generation::default();
    g.feature(SurfaceStructures, placed!("end_spike"))
        .feature(TopLayerModification, placed!("end_platform"));
    end_base(g)
}

pub fn end_highlands() -> Biome {
    let mut g = Generation::default();
    g.feature(SurfaceStructures, placed!("end_gateway_return"))
        .feature(VegetalDecoration, placed!("chorus_plant"));
    end_base(g)
}

pub fn small_end_islands() -> Biome {
    let mut g = Generation::default();
    g.feature(RawGeneration, placed!("end_island_decorated"));
    end_base(g)
}
