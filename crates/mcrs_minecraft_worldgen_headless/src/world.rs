use mcrs_minecraft_worldgen_generator::stages::FillContext;

pub struct World {
    pub ctx: FillContext,
    pub biome_names: Vec<String>,
    pub sea_level: i32,
}
