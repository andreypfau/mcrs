use super::*;
use mcrs_minecraft_worldgen_structure::DecorationStep::*;

pub fn end_base(generation: Generation) -> Draft {
    let mut m = Mobs::default();
    m.end_spawns();
    Draft::new(false, 0.5, 0.5)
        .spawns(m.0)
        .generation(generation)
}

pub fn the_end() -> Draft {
    let mut g = Generation::default();
    g.feature(SurfaceStructures, placed_feature::END_SPIKE)
        .feature(TopLayerModification, placed_feature::END_PLATFORM);
    end_base(g)
}

pub fn end_highlands() -> Draft {
    let mut g = Generation::default();
    g.feature(SurfaceStructures, placed_feature::END_GATEWAY_RETURN)
        .feature(VegetalDecoration, placed_feature::CHORUS_PLANT);
    end_base(g)
}

pub fn small_end_islands() -> Draft {
    let mut g = Generation::default();
    g.feature(RawGeneration, placed_feature::END_ISLAND_DECORATED);
    end_base(g)
}
