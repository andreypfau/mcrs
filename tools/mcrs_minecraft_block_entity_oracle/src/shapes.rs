//! The shapes, textures and placements of each static block entity, as the specification of
//! block entity models states them in its section on shapes.

use std::path::Path;

use glam::{DMat3, DMat4, DVec3};
use mcrs_minecraft_block::definition::schema::BlockDefinitionFile;

pub const DYES: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

const HORIZONTAL: [&str; 4] = ["north", "south", "west", "east"];

#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Text(&'static str),
    Number(u8),
    Boolean(bool),
}

impl Literal {
    pub fn molang(&self) -> String {
        match self {
            Literal::Text(text) => format!("'{text}'"),
            Literal::Number(number) => number.to_string(),
            Literal::Boolean(value) => value.to_string(),
        }
    }

    pub fn text(&self) -> String {
        match self {
            Literal::Text(text) => (*text).to_owned(),
            Literal::Number(number) => number.to_string(),
            Literal::Boolean(value) => value.to_string(),
        }
    }
}

/// One layer entry of the model artefact, in one pose, as the entity pack holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometrySource {
    pub identifier: &'static str,
    pub model: &'static str,
    pub layer: &'static str,
    pub pose: Option<&'static str>,
}

pub const GEOMETRIES: [GeometrySource; 20] = [
    rest("chest", "minecraft:chest", "main"),
    rest("double_chest_left", "minecraft:double_chest_left", "main"),
    rest("double_chest_right", "minecraft:double_chest_right", "main"),
    rest("shulker_box", "minecraft:shulker_box", "main"),
    rest("standing_banner", "minecraft:standing_banner", "main"),
    posed(
        "standing_banner.flag",
        "minecraft:standing_banner",
        "flag",
        "banner_flag",
    ),
    rest("wall_banner", "minecraft:wall_banner", "main"),
    posed(
        "wall_banner.flag",
        "minecraft:wall_banner",
        "flag",
        "banner_flag",
    ),
    rest("skeleton_skull", "minecraft:skeleton_skull", "main"),
    rest(
        "wither_skeleton_skull",
        "minecraft:wither_skeleton_skull",
        "main",
    ),
    rest("zombie_head", "minecraft:zombie_head", "main"),
    rest("creeper_head", "minecraft:creeper_head", "main"),
    posed(
        "piglin_head",
        "minecraft:piglin_head",
        "main",
        "piglin_head",
    ),
    posed(
        "dragon_skull",
        "minecraft:dragon_skull",
        "main",
        "dragon_head",
    ),
    rest("player_head", "minecraft:player_head", "main"),
    rest("decorated_pot_base", "minecraft:decorated_pot_base", "main"),
    rest(
        "decorated_pot_sides",
        "minecraft:decorated_pot_sides",
        "main",
    ),
    posed(
        "book.enchanting_table_book",
        "minecraft:book",
        "main",
        "enchanting_table_book",
    ),
    posed(
        "book.lectern_book",
        "minecraft:book",
        "main",
        "lectern_book",
    ),
    rest("bell", "minecraft:bell", "main"),
];

const fn rest(name: &'static str, model: &'static str, layer: &'static str) -> GeometrySource {
    GeometrySource {
        identifier: name,
        model,
        layer,
        pose: None,
    }
}

const fn posed(
    name: &'static str,
    model: &'static str,
    layer: &'static str,
    pose: &'static str,
) -> GeometrySource {
    GeometrySource {
        identifier: name,
        model,
        layer,
        pose: Some(pose),
    }
}

impl GeometrySource {
    pub fn identifier(&self) -> String {
        format!("geometry.mcrs.block.{}", self.identifier)
    }

    fn named(name: &str) -> GeometrySource {
        *GEOMETRIES
            .iter()
            .find(|source| source.identifier == name)
            .unwrap_or_else(|| panic!("no geometry {name}"))
    }
}

/// Where the reference states the placement of a shape, when it states one.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtefactPlacement {
    pub shape: &'static str,
    pub state: String,
}

#[derive(Clone, Debug)]
pub struct Shape {
    pub geometry: GeometrySource,
    pub texture: String,
    pub tint: Option<u8>,
    pub surface: &'static str,
    pub back_faces: bool,
    pub placement: DMat4,
    pub artefact_placement: Option<ArtefactPlacement>,
}

/// The shapes of the states where each property of `conditions` has its value.
pub struct Case {
    pub conditions: Vec<(&'static str, Literal)>,
    pub shapes: Vec<Shape>,
}

pub struct BlockShapes {
    pub block: String,
    pub cases: Vec<Case>,
}

impl BlockShapes {
    pub fn path(&self) -> &str {
        self.block.strip_prefix("minecraft:").unwrap_or(&self.block)
    }
}

/// The property names and values of a block, read from its block definition.
pub fn block_properties(path: &str) -> Vec<(String, Vec<String>)> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/mcrs/block_definition")
        .join(format!("{path}.json"));
    let bytes = std::fs::read(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
    let definition: BlockDefinitionFile = serde_json::from_slice(&bytes).unwrap();
    definition
        .block
        .description
        .properties
        .0
        .iter()
        .map(|property| {
            let values = property
                .values
                .iter()
                .map(|value| value.to_text())
                .collect();
            (property.name.to_string(), values)
        })
        .collect()
}

fn yaw(direction: &str) -> f64 {
    match direction {
        "south" => 0.0,
        "west" => 90.0,
        "north" => 180.0,
        "east" => 270.0,
        _ => panic!("{direction} has no yaw"),
    }
}

fn step(direction: &str) -> (f64, f64) {
    match direction {
        "north" => (0.0, -1.0),
        "south" => (0.0, 1.0),
        "east" => (1.0, 0.0),
        "west" => (-1.0, 0.0),
        _ => panic!("{direction} has no step"),
    }
}

fn opposite(direction: &str) -> &'static str {
    match direction {
        "north" => "south",
        "south" => "north",
        "east" => "west",
        "west" => "east",
        _ => panic!("{direction} has no opposite"),
    }
}

fn clockwise(direction: &str) -> &'static str {
    match direction {
        "north" => "east",
        "east" => "south",
        "south" => "west",
        "west" => "north",
        _ => panic!("{direction} has no clockwise direction"),
    }
}

fn translation(x: f64, y: f64, z: f64) -> DMat4 {
    DMat4::from_translation(DVec3::new(x, y, z))
}

fn scale(x: f64, y: f64, z: f64) -> DMat4 {
    DMat4::from_scale(DVec3::new(x, y, z))
}

fn turn_y(degrees: f64) -> DMat4 {
    DMat4::from_rotation_y(degrees.to_radians())
}

fn turn_z(degrees: f64) -> DMat4 {
    DMat4::from_rotation_z(degrees.to_radians())
}

fn shape(
    geometry: &str,
    texture: String,
    surface: &'static str,
    back_faces: bool,
    placement: DMat4,
) -> Shape {
    Shape {
        geometry: GeometrySource::named(geometry),
        texture,
        tint: None,
        surface,
        back_faces,
        placement,
        artefact_placement: None,
    }
}

fn from_artefact(mut shape: Shape, name: &'static str, state: &str) -> Shape {
    shape.artefact_placement = Some(ArtefactPlacement {
        shape: name,
        state: state.to_owned(),
    });
    shape
}

fn chests(blocks: &mut Vec<BlockShapes>) {
    let materials = [
        ("chest", "normal"),
        ("trapped_chest", "trapped"),
        ("ender_chest", "ender"),
        ("copper_chest", "copper"),
        ("exposed_copper_chest", "copper_exposed"),
        ("weathered_copper_chest", "copper_weathered"),
        ("oxidized_copper_chest", "copper_oxidized"),
        ("waxed_copper_chest", "copper"),
        ("waxed_exposed_copper_chest", "copper_exposed"),
        ("waxed_weathered_copper_chest", "copper_weathered"),
        ("waxed_oxidized_copper_chest", "copper_oxidized"),
    ];
    for (block, material) in materials {
        let has_type = block_properties(block)
            .iter()
            .any(|(name, _)| name == "type");
        let types: &[(&str, &str, &str)] = if has_type {
            &[
                ("single", "chest", ""),
                ("left", "double_chest_left", "_left"),
                ("right", "double_chest_right", "_right"),
            ]
        } else {
            &[("single", "chest", "")]
        };
        let mut cases = Vec::new();
        for facing in HORIZONTAL {
            for &(kind, geometry, suffix) in types {
                let placement = translation(0.5, 0.0, 0.5)
                    * turn_y(-yaw(facing))
                    * translation(-0.5, 0.0, -0.5);
                let texture = format!("minecraft:entity/chest/{material}{suffix}");
                let mut conditions = vec![("facing", Literal::Text(facing))];
                if has_type {
                    conditions.push(("type", Literal::Text(kind)));
                }
                cases.push(Case {
                    conditions,
                    shapes: vec![from_artefact(
                        shape(geometry, texture, "cutout", false, placement),
                        "chest",
                        facing,
                    )],
                });
            }
        }
        blocks.push(BlockShapes {
            block: format!("minecraft:{block}"),
            cases,
        });
    }
}

fn shulker_boxes(blocks: &mut Vec<BlockShapes>) {
    let rows: [(&str, [[f64; 3]; 3], [f64; 3]); 6] = [
        (
            "down",
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [0.5, -0.4995, 0.5],
        ),
        (
            "up",
            [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]],
            [0.5, 1.4995, 0.5],
        ),
        (
            "north",
            [[-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]],
            [0.5, 0.5, -0.4995],
        ),
        (
            "south",
            [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]],
            [0.5, 0.5, 1.4995],
        ),
        (
            "west",
            [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]],
            [-0.4995, 0.5, 0.5],
        ),
        (
            "east",
            [[0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0]],
            [1.4995, 0.5, 0.5],
        ),
    ];
    let colours = std::iter::once(None).chain(DYES.iter().map(Some));
    for colour in colours {
        let (block, texture) = match colour {
            None => (
                "shulker_box".to_owned(),
                "minecraft:entity/shulker/shulker".to_owned(),
            ),
            Some(colour) => (
                format!("{colour}_shulker_box"),
                format!("minecraft:entity/shulker/shulker_{colour}"),
            ),
        };
        let cases = rows
            .iter()
            .map(|(facing, matrix, [tx, ty, tz])| {
                let linear = DMat3::from_cols_array_2d(matrix).transpose() * 0.9995;
                let placement = translation(*tx, *ty, *tz) * DMat4::from_mat3(linear);
                Case {
                    conditions: vec![("facing", Literal::Text(facing))],
                    shapes: vec![from_artefact(
                        shape("shulker_box", texture.clone(), "cutout", true, placement),
                        "shulker_box",
                        facing,
                    )],
                }
            })
            .collect();
        blocks.push(BlockShapes {
            block: format!("minecraft:{block}"),
            cases,
        });
    }
}

fn banner_shapes(
    geometry: &str,
    dye: u8,
    angle: f64,
    artefact: &'static str,
    state: &str,
) -> Vec<Shape> {
    let placement =
        translation(0.5, 0.0, 0.5) * turn_y(-angle) * scale(2.0 / 3.0, -2.0 / 3.0, -2.0 / 3.0);
    let pole = shape(
        geometry,
        "minecraft:entity/banner/banner_base".to_owned(),
        "opaque",
        false,
        placement,
    );
    let mut flag = shape(
        &format!("{geometry}.flag"),
        "minecraft:entity/banner/base".to_owned(),
        "opaque",
        false,
        placement,
    );
    flag.tint = Some(dye);
    vec![
        from_artefact(pole, artefact, state),
        from_artefact(flag, artefact, state),
    ]
}

fn banners(blocks: &mut Vec<BlockShapes>) {
    for (dye, colour) in (0u8..).zip(DYES) {
        let standing = (0u8..16)
            .map(|rotation| Case {
                conditions: vec![("rotation", Literal::Number(rotation))],
                shapes: banner_shapes(
                    "standing_banner",
                    dye,
                    22.5 * f64::from(rotation),
                    "standing_banner",
                    &rotation.to_string(),
                ),
            })
            .collect();
        blocks.push(BlockShapes {
            block: format!("minecraft:{colour}_banner"),
            cases: standing,
        });
        let wall = HORIZONTAL
            .iter()
            .map(|facing| Case {
                conditions: vec![("facing", Literal::Text(facing))],
                shapes: banner_shapes("wall_banner", dye, yaw(facing), "wall_banner", facing),
            })
            .collect();
        blocks.push(BlockShapes {
            block: format!("minecraft:{colour}_wall_banner"),
            cases: wall,
        });
    }
}

fn heads(blocks: &mut Vec<BlockShapes>) {
    let kinds = [
        (
            "skeleton_skull",
            "skeleton_wall_skull",
            "skeleton_skull",
            "skeleton/skeleton",
        ),
        (
            "wither_skeleton_skull",
            "wither_skeleton_wall_skull",
            "wither_skeleton_skull",
            "skeleton/wither_skeleton",
        ),
        (
            "zombie_head",
            "zombie_wall_head",
            "zombie_head",
            "zombie/zombie",
        ),
        (
            "creeper_head",
            "creeper_wall_head",
            "creeper_head",
            "creeper/creeper",
        ),
        (
            "piglin_head",
            "piglin_wall_head",
            "piglin_head",
            "piglin/piglin",
        ),
        (
            "dragon_head",
            "dragon_wall_head",
            "dragon_skull",
            "enderdragon/dragon",
        ),
        (
            "player_head",
            "player_wall_head",
            "player_head",
            "player/wide/steve",
        ),
    ];
    for (standing, wall, geometry, texture) in kinds {
        let texture = format!("minecraft:entity/{texture}");
        let head = |placement| shape(geometry, texture.clone(), "blend", true, placement);
        let standing_cases = (0u8..16)
            .map(|rotation| {
                let placement = translation(0.5, 0.0, 0.5)
                    * turn_y(-22.5 * f64::from(rotation))
                    * scale(-1.0, -1.0, 1.0);
                Case {
                    conditions: vec![("rotation", Literal::Number(rotation))],
                    shapes: vec![from_artefact(
                        head(placement),
                        "standing_head",
                        &rotation.to_string(),
                    )],
                }
            })
            .collect();
        blocks.push(BlockShapes {
            block: format!("minecraft:{standing}"),
            cases: standing_cases,
        });
        let wall_cases = HORIZONTAL
            .iter()
            .map(|facing| {
                let (sx, sz) = step(facing);
                let placement = translation(0.5 - 0.25 * sx, 0.25, 0.5 - 0.25 * sz)
                    * turn_y(-yaw(opposite(facing)))
                    * scale(-1.0, -1.0, 1.0);
                Case {
                    conditions: vec![("facing", Literal::Text(facing))],
                    shapes: vec![from_artefact(head(placement), "wall_head", facing)],
                }
            })
            .collect();
        blocks.push(BlockShapes {
            block: format!("minecraft:{wall}"),
            cases: wall_cases,
        });
    }
}

fn decorated_pot(blocks: &mut Vec<BlockShapes>) {
    let cases = HORIZONTAL
        .iter()
        .map(|facing| {
            let placement = translation(0.5, 0.5, 0.5)
                * turn_y(180.0 - yaw(facing))
                * translation(-0.5, -0.5, -0.5);
            let layer = |geometry: &str, texture: &str| {
                from_artefact(
                    shape(
                        geometry,
                        format!("minecraft:entity/decorated_pot/{texture}"),
                        "opaque",
                        false,
                        placement,
                    ),
                    "decorated_pot",
                    facing,
                )
            };
            Case {
                conditions: vec![("facing", Literal::Text(facing))],
                shapes: vec![
                    layer("decorated_pot_base", "decorated_pot_base"),
                    layer("decorated_pot_sides", "decorated_pot_side"),
                ],
            }
        })
        .collect();
    blocks.push(BlockShapes {
        block: "minecraft:decorated_pot".to_owned(),
        cases,
    });
}

fn books_and_bell(blocks: &mut Vec<BlockShapes>) {
    let book_texture = "minecraft:entity/enchantment/enchanting_table_book";
    blocks.push(BlockShapes {
        block: "minecraft:enchanting_table".to_owned(),
        cases: vec![Case {
            conditions: Vec::new(),
            shapes: vec![shape(
                "book.enchanting_table_book",
                book_texture.to_owned(),
                "opaque",
                false,
                translation(0.5, 0.85, 0.5) * turn_z(80.0),
            )],
        }],
    });
    let lectern = HORIZONTAL
        .iter()
        .map(|facing| Case {
            conditions: vec![
                ("facing", Literal::Text(facing)),
                ("has_book", Literal::Boolean(true)),
            ],
            shapes: vec![shape(
                "book.lectern_book",
                book_texture.to_owned(),
                "opaque",
                false,
                translation(0.5, 1.0625, 0.5)
                    * turn_y(-yaw(clockwise(facing)))
                    * turn_z(67.5)
                    * translation(0.0, -0.125, 0.0),
            )],
        })
        .collect();
    blocks.push(BlockShapes {
        block: "minecraft:lectern".to_owned(),
        cases: lectern,
    });
    blocks.push(BlockShapes {
        block: "minecraft:bell".to_owned(),
        cases: vec![Case {
            conditions: Vec::new(),
            shapes: vec![shape(
                "bell",
                "minecraft:entity/bell/bell_body".to_owned(),
                "opaque",
                false,
                DMat4::IDENTITY,
            )],
        }],
    });
}

pub fn all() -> Vec<BlockShapes> {
    let mut blocks = Vec::new();
    chests(&mut blocks);
    shulker_boxes(&mut blocks);
    banners(&mut blocks);
    heads(&mut blocks);
    decorated_pot(&mut blocks);
    books_and_bell(&mut blocks);
    blocks
}
