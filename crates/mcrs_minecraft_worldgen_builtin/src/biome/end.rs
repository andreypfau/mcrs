use super::defaults::end_spawns;
use super::*;
use mcrs_minecraft_worldgen_structure::DecorationStep::*;

pub fn end_base(generation: Generation) -> Biome {
    let mut m = Mobs::default();
    end_spawns(&mut m);
    biome(false, 0.5, 0.5, m, generation)
}

pub fn the_end() -> Biome {
    let mut g = Generation::default();
    g.feature(SurfaceStructures, placed::END_SPIKE)
        .feature(TopLayerModification, placed::END_PLATFORM);
    end_base(g)
}

pub fn end_highlands() -> Biome {
    let mut g = Generation::default();
    g.feature(SurfaceStructures, placed::END_GATEWAY_RETURN)
        .feature(VegetalDecoration, placed::CHORUS_PLANT);
    end_base(g)
}

pub fn small_end_islands() -> Biome {
    let mut g = Generation::default();
    g.feature(RawGeneration, placed::END_ISLAND_DECORATED);
    end_base(g)
}
