mod corpus;

use std::sync::OnceLock;

use corpus::{asset_server, block_tags, blocks};
use mcrs_minecraft_block::definition::BlockEntry;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_light_color::colors::{LightColors, LightType};

fn shipped() -> &'static LightColors {
    static COLORS: OnceLock<LightColors> = OnceLock::new();
    COLORS.get_or_init(|| {
        LightColors::load(asset_server(), blocks(), block_tags()).unwrap_or_else(|e| panic!("{e}"))
    })
}

fn block(id: &str) -> &'static BlockEntry {
    blocks()
        .block(id)
        .unwrap_or_else(|| panic!("the corpus has no `{id}`"))
}

fn states(block: &BlockEntry) -> impl Iterator<Item = VoxelId> + '_ {
    (0..block.state_count).map(|offset| VoxelId(block.base_state_id.0 + offset))
}

fn colour_of_every_state(id: &str) -> Option<[u8; 3]> {
    let colors = shipped();
    let block = block(id);
    let first = colors.rgb(colors.light_type(block.base_state_id.into()));
    for state in states(block) {
        assert_eq!(
            colors.rgb(colors.light_type(state)),
            first,
            "{id} state {}",
            state.0
        );
    }
    first
}

#[test]
fn soul_file_colours_every_soul_emitter() {
    let soul = Some([0x36, 0xd9, 0xe6]);
    assert_eq!(colour_of_every_state("minecraft:soul_lantern"), soul);
    assert_eq!(
        colour_of_every_state("minecraft:calibrated_sculk_sensor"),
        soul
    );
    let torch = block("minecraft:torch").default_state_id;
    assert_eq!(shipped().light_type(torch.into()), LightType::DEFAULT);
}
