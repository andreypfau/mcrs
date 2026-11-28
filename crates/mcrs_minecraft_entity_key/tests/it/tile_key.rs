use glam::DVec3;
use mcrs_minecraft_entity_key::TileKey;

#[test]
fn a_tile_holds_32_blocks_from_its_low_edge_on_both_signs() {
    let cases = [
        (DVec3::new(0.0, 300.0, 31.999), TileKey::new(0, 0, 0)),
        (DVec3::new(32.0, -64.0, 64.0), TileKey::new(1, 0, 2)),
        (DVec3::new(-0.001, 0.0, -32.0), TileKey::new(-1, 0, -1)),
        (DVec3::new(-32.001, 0.0, -64.5), TileKey::new(-2, 0, -3)),
    ];
    for (position, tile) in cases {
        assert_eq!(TileKey::at(position), tile, "the tile of {position}");
    }
}
