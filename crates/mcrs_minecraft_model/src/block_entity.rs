//! Blocks the game draws in code instead of from block models: chests, shulker boxes, banners,
//! heads, decorated pots, the books on enchanting tables and lecterns, and the bell's body.
//!
//! Their block models carry only a particle texture; the shapes live in the client's layer
//! definitions (`ChestModel`, `ShulkerModel`, `BannerModel`, `SkullModel`, ...) and each block
//! entity renderer places them. This bakes the same cubes, with `ModelPart.Cube`'s box UVs and
//! the renderer's pose, into ordinary quads, so they mesh, light and cull like any block face.
//! The pose is the one the game shows at rest: lids closed, no wobble, no wind, a closed book
//! over an enchanting table no player is near.
//!
//! What the block entity's data would add is left out: banner patterns, pot sherds, a player
//! head's skin. Conduits, end portals and gateways, and copper golem statues are not drawn.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::math::{EulerRot, Mat4, Quat, Vec3};

use crate::bake::{BakedBlock, BakedQuad, Dir, face_shade, nearest};

/// One `CubeListBuilder.addBox`, in the model's pixels.
#[derive(Clone, Copy)]
struct Cube {
    tex: [f32; 2],
    from: [f32; 3],
    size: [f32; 3],
    grow: f32,
    mirror: bool,
    /// A cube built with a face set draws only that face.
    only: Option<Dir>,
}

const fn cube(tex: [f32; 2], from: [f32; 3], size: [f32; 3]) -> Cube {
    Cube {
        tex,
        from,
        size,
        grow: 0.0,
        mirror: false,
        only: None,
    }
}

impl Cube {
    const fn grow(self, grow: f32) -> Self {
        Self { grow, ..self }
    }

    const fn mirror(self) -> Self {
        Self {
            mirror: true,
            ..self
        }
    }

    const fn only(self, dir: Dir) -> Self {
        Self {
            only: Some(dir),
            ..self
        }
    }
}

/// A `ModelPart`: its pose relative to the parent, its cubes and its children.
struct Part {
    offset: [f32; 3],
    /// `xRot`, `yRot`, `zRot` in radians, applied as the game's `rotationZYX`.
    rotation: [f32; 3],
    scale: f32,
    cubes: Vec<Cube>,
    children: Vec<Part>,
}

fn part(offset: [f32; 3], rotation: [f32; 3], cubes: Vec<Cube>) -> Part {
    Part {
        offset,
        rotation,
        scale: 1.0,
        cubes,
        children: Vec::new(),
    }
}

impl Part {
    fn with(mut self, child: Part) -> Self {
        self.children.push(child);
        self
    }

    fn scaled(self, scale: f32) -> Self {
        Self { scale, ..self }
    }

    fn pose(&self) -> Mat4 {
        let [x, y, z] = self.rotation;
        Mat4::from_translation(Vec3::from(self.offset) / 16.0)
            * Mat4::from_quat(Quat::from_euler(EulerRot::ZYX, z, y, x))
            * Mat4::from_scale(Vec3::splat(self.scale))
    }
}

/// A model on one texture, `size` being the layer definition's texture size.
struct Layer {
    texture: String,
    size: [f32; 2],
    parts: Vec<Part>,
    tint: Option<u32>,
}

fn layer(texture: impl Into<String>, size: [f32; 2], parts: Vec<Part>) -> Layer {
    Layer {
        texture: texture.into(),
        size,
        parts,
        tint: None,
    }
}

/// The quads code draws for this block state, in block space like a baked block model's, or
/// `None` for a block drawn by its model alone.
pub(crate) fn bake(block: &str, props: &[(&str, &str)]) -> Option<BakedBlock> {
    let name = block.strip_prefix("minecraft:").unwrap_or(block);
    let prop = |key: &str| props.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
    let facing = || prop("facing").and_then(dir_named).unwrap_or(Dir::North);
    let rotation = || {
        prop("rotation")
            .and_then(|r| r.parse::<f32>().ok())
            .unwrap_or(0.0)
            * 22.5
    };

    let layers: Vec<(Layer, Mat4)> = if let Some(material) = chest_material(name) {
        let kind = prop("type").unwrap_or("single");
        let transform = around(Vec3::new(0.5, 0.0, 0.5), y_degrees(-y_rot(facing())));
        let texture = match kind {
            "single" => format!("minecraft:entity/chest/{material}"),
            side => format!("minecraft:entity/chest/{material}_{side}"),
        };
        vec![(layer(texture, [64.0, 64.0], chest(kind)), transform)]
    } else if let Some(color) = name.strip_suffix("shulker_box") {
        let texture = match color.strip_suffix('_') {
            Some(color) => format!("minecraft:entity/shulker/shulker_{color}"),
            None => "minecraft:entity/shulker/shulker".to_owned(),
        };
        vec![(
            layer(texture, [64.0, 64.0], shulker()),
            shulker_pose(facing()),
        )]
    } else if let Some((_, wall)) = banner_color(name) {
        banner(wall, if wall { y_rot(facing()) } else { rotation() })
    } else if let Some((head, wall)) = head_kind(name) {
        let transform = if wall {
            let step = facing().normal().as_vec3();
            Mat4::from_translation(Vec3::new(0.5 - step.x * 0.25, 0.25, 0.5 - step.z * 0.25))
                * y_degrees(-y_rot(facing().opposite()))
        } else {
            Mat4::from_translation(Vec3::new(0.5, 0.0, 0.5)) * y_degrees(-rotation())
        } * Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0));
        vec![(skull(head), transform)]
    } else {
        furniture(name, facing(), prop("has_book") == Some("true"))?
    };

    let mut out = BakedBlock {
        quads: Vec::new(),
        sprites: Vec::new(),
        ambient_occlusion: true,
    };
    for (layer, transform) in &layers {
        let sprite = out.sprites.len();
        out.sprites.push(layer.texture.clone());
        for part in &layer.parts {
            emit(&mut out.quads, layer, sprite, part, *transform);
        }
    }
    Some(out)
}

/// `BannerRenderer`: the wooden pole and bar, and the flag in its dye.
fn banner(wall: bool, degrees: f32) -> Vec<(Layer, Mat4)> {
    let transform = Mat4::from_translation(Vec3::new(0.5, 0.0, 0.5))
        * y_degrees(-degrees)
        * Mat4::from_scale(Vec3::new(2.0 / 3.0, -2.0 / 3.0, -2.0 / 3.0));
    let pole = layer(
        "minecraft:entity/banner/banner_base",
        [64.0, 64.0],
        banner_pole(!wall),
    );
    let mut flag = layer(
        "minecraft:entity/banner/base",
        [64.0, 64.0],
        banner_flag(!wall),
    );
    flag.tint = Some(0);
    vec![(pole, transform), (flag, transform)]
}

/// The pot, the books and the bell, which have one shape each.
fn furniture(name: &str, facing: Dir, has_book: bool) -> Option<Vec<(Layer, Mat4)>> {
    Some(match name {
        "decorated_pot" => {
            let transform = around(Vec3::splat(0.5), y_degrees(180.0 - y_rot(facing)));
            let base = layer(
                "minecraft:entity/decorated_pot/decorated_pot_base",
                [32.0, 32.0],
                pot_base(),
            );
            let sides = layer(
                "minecraft:entity/decorated_pot/decorated_pot_side",
                [16.0, 16.0],
                pot_sides(),
            );
            vec![(base, transform), (sides, transform)]
        }
        // `EnchantTableRenderer` with no player near: the book closed, hovering at rest.
        "enchanting_table" => vec![(
            book(0.0, 0.1, 0.9),
            Mat4::from_translation(Vec3::new(0.5, 0.85, 0.5))
                * Mat4::from_rotation_z(80f32.to_radians()),
        )],
        // `LecternRenderer`'s fixed `forAnimation(0, 0.1, 0.9, 1.2)`.
        "lectern" if has_book => vec![(
            book(1.25 * 1.2, 0.1, 0.9),
            Mat4::from_translation(Vec3::new(0.5, 1.0625, 0.5))
                * y_degrees(-y_rot(facing.clockwise()))
                * Mat4::from_rotation_z(67.5f32.to_radians())
                * Mat4::from_translation(Vec3::new(0.0, -0.125, 0.0)),
        )],
        "bell" => vec![(
            layer("minecraft:entity/bell/bell_body", [32.0, 32.0], bell()),
            Mat4::IDENTITY,
        )],
        _ => return None,
    })
}

fn emit(quads: &mut Vec<BakedQuad>, layer: &Layer, sprite: usize, part: &Part, parent: Mat4) {
    let pose = parent * part.pose();
    // Non-square textures sit in the top-left of a square layer, so both axes divide by the side.
    let side = layer.size[0].max(layer.size[1]);
    for cube in &part.cubes {
        for (dir, corners, [u1, v1, u2, v2]) in polygons(cube) {
            if cube.only.is_some_and(|only| only != dir) {
                continue;
            }
            // `ModelPart.Polygon`: corners get (u2,v1), (u1,v1), (u1,v2), (u2,v2); a mirrored
            // cube lists them backwards so they still wind outwards.
            let mut uvs =
                [[u2, v1], [u1, v1], [u1, v2], [u2, v2]].map(|[u, v]| [u / side, v / side]);
            let mut positions = corners.map(|corner| pose.transform_point3(corner / 16.0));
            if cube.mirror {
                positions.reverse();
                uvs.reverse();
            }
            let normal = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
            let facing = nearest(normal).unwrap_or(Dir::Up);
            quads.push(BakedQuad {
                positions,
                uvs,
                dir: facing,
                sprite,
                cull: None,
                shade: face_shade(facing),
                tint: layer.tint,
            });
        }
    }
    for child in &part.children {
        emit(quads, layer, sprite, child, pose);
    }
}

/// `ModelPart.Cube`'s six faces: the direction, the corners in pixels and the texel rectangle
/// `(u1, v1, u2, v2)` of the box UV layout.
fn polygons(cube: &Cube) -> [(Dir, [Vec3; 4], [f32; 4]); 6] {
    let grow = Vec3::splat(cube.grow);
    let min = Vec3::from(cube.from) - grow;
    let max = Vec3::from(cube.from) + Vec3::from(cube.size) + grow;
    let (mut x0, mut x1) = (min.x, max.x);
    if cube.mirror {
        std::mem::swap(&mut x0, &mut x1);
    }
    let (y0, y1, z0, z1) = (min.y, max.y, min.z, max.z);
    let v1 = Vec3::new(x0, y0, z0);
    let v2 = Vec3::new(x1, y0, z0);
    let v3 = Vec3::new(x1, y1, z0);
    let v4 = Vec3::new(x0, y1, z0);
    let v5 = Vec3::new(x0, y0, z1);
    let v6 = Vec3::new(x1, y0, z1);
    let v7 = Vec3::new(x1, y1, z1);
    let v8 = Vec3::new(x0, y1, z1);

    let [u, v] = cube.tex;
    let [w, h, d] = cube.size;
    let (l, m, n, o, p, q) = (
        u,
        u + d,
        u + d + w,
        u + d + w + w,
        u + d + w + d,
        u + d + w + d + w,
    );
    let (r, s, t) = (v, v + d, v + d + h);
    [
        (Dir::Down, [v6, v5, v1, v2], [m, r, n, s]),
        (Dir::Up, [v3, v4, v8, v7], [n, s, o, r]),
        (Dir::West, [v1, v5, v8, v4], [l, s, m, t]),
        (Dir::North, [v2, v1, v4, v3], [m, s, n, t]),
        (Dir::East, [v6, v2, v3, v7], [n, s, p, t]),
        (Dir::South, [v5, v6, v7, v8], [p, s, q, t]),
    ]
}

fn dir_named(name: &str) -> Option<Dir> {
    Dir::all().into_iter().find(|dir| dir.name() == name)
}

/// `Direction.toYRot`: south 0, west 90, north 180, east 270.
fn y_rot(dir: Dir) -> f32 {
    match dir {
        Dir::South => 0.0,
        Dir::West => 90.0,
        Dir::North => 180.0,
        Dir::East => 270.0,
        Dir::Up | Dir::Down => 0.0,
    }
}

fn y_degrees(degrees: f32) -> Mat4 {
    Mat4::from_rotation_y(degrees.to_radians())
}

fn around(center: Vec3, rotation: Mat4) -> Mat4 {
    Mat4::from_translation(center) * rotation * Mat4::from_translation(-center)
}

fn chest_material(name: &str) -> Option<&'static str> {
    Some(match name.strip_prefix("waxed_").unwrap_or(name) {
        "chest" => "normal",
        "trapped_chest" => "trapped",
        "ender_chest" => "ender",
        "copper_chest" => "copper",
        "exposed_copper_chest" => "copper_exposed",
        "weathered_copper_chest" => "copper_weathered",
        "oxidized_copper_chest" => "copper_oxidized",
        _ => return None,
    })
}

/// `ChestModel`'s single and double halves.
fn chest(kind: &str) -> Vec<Part> {
    let (x, width, lock) = match kind {
        "left" => (0.0, 15.0, 0.0),
        "right" => (1.0, 15.0, 15.0),
        _ => (1.0, 14.0, 7.0),
    };
    let lock_width = if kind == "single" { 2.0 } else { 1.0 };
    vec![
        part(
            [0.0; 3],
            [0.0; 3],
            vec![cube([0.0, 19.0], [x, 0.0, 1.0], [width, 10.0, 14.0])],
        ),
        part(
            [0.0, 9.0, 1.0],
            [0.0; 3],
            vec![cube([0.0, 0.0], [x, 0.0, 0.0], [width, 5.0, 14.0])],
        ),
        part(
            [0.0, 9.0, 1.0],
            [0.0; 3],
            vec![cube([0.0, 0.0], [lock, -2.0, 14.0], [lock_width, 4.0, 1.0])],
        ),
    ]
}

/// `ShulkerModel.createShellMesh` with the lid shut.
fn shulker() -> Vec<Part> {
    vec![
        part(
            [0.0, 24.0, 0.0],
            [0.0; 3],
            vec![cube([0.0, 0.0], [-8.0, -16.0, -8.0], [16.0, 12.0, 16.0])],
        ),
        part(
            [0.0, 24.0, 0.0],
            [0.0; 3],
            vec![cube([0.0, 28.0], [-8.0, -8.0, -8.0], [16.0, 8.0, 16.0])],
        ),
    ]
}

/// `ShulkerBoxRenderer.createModelTransform`.
fn shulker_pose(facing: Dir) -> Mat4 {
    let rotation = match facing {
        Dir::Down => Quat::from_rotation_x(PI),
        Dir::Up => Quat::IDENTITY,
        Dir::North => Quat::from_euler(EulerRot::XYZ, FRAC_PI_2, 0.0, PI),
        Dir::South => Quat::from_rotation_x(FRAC_PI_2),
        Dir::West => Quat::from_euler(EulerRot::XYZ, FRAC_PI_2, 0.0, FRAC_PI_2),
        Dir::East => Quat::from_euler(EulerRot::XYZ, FRAC_PI_2, 0.0, -FRAC_PI_2),
    };
    Mat4::from_translation(Vec3::splat(0.5))
        * Mat4::from_scale(Vec3::splat(0.9995))
        * Mat4::from_quat(rotation)
        * Mat4::from_scale(Vec3::new(1.0, -1.0, -1.0))
        * Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0))
}

const DYES: [&str; 16] = [
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

/// The dye of a banner block and whether it hangs on a wall.
pub(crate) fn banner_color(name: &str) -> Option<(usize, bool)> {
    let (color, wall) = match name.strip_suffix("_wall_banner") {
        Some(color) => (color, true),
        None => (name.strip_suffix("_banner")?, false),
    };
    DYES.iter()
        .position(|dye| *dye == color)
        .map(|dye| (dye, wall))
}

/// `BannerModel`: the pole stands only on the ground; the bar sits lower on a wall.
fn banner_pole(ground: bool) -> Vec<Part> {
    let mut cubes = vec![];
    if ground {
        cubes.push(part(
            [0.0; 3],
            [0.0; 3],
            vec![cube([44.0, 0.0], [-1.0, -42.0, -1.0], [2.0, 42.0, 2.0])],
        ));
    }
    let bar = if ground {
        [-10.0, -44.0, -1.0]
    } else {
        [-10.0, -20.5, 9.5]
    };
    cubes.push(part(
        [0.0; 3],
        [0.0; 3],
        vec![cube([0.0, 42.0], bar, [20.0, 2.0, 2.0])],
    ));
    cubes
}

/// `BannerFlagModel`, still: its wind sway at phase zero.
fn banner_flag(ground: bool) -> Vec<Part> {
    let offset = if ground {
        [0.0, -44.0, 0.0]
    } else {
        [0.0, -20.5, 10.5]
    };
    vec![part(
        offset,
        [(-0.0125 + 0.01) * PI, 0.0, 0.0],
        vec![cube([0.0, 0.0], [-10.0, 0.0, -2.0], [20.0, 40.0, 1.0])],
    )]
}

enum Head {
    Mob(&'static str),
    Humanoid(&'static str),
    Piglin,
    Dragon,
}

fn head_kind(name: &str) -> Option<(Head, bool)> {
    let (kind, wall) = match name
        .strip_suffix("_wall_skull")
        .or_else(|| name.strip_suffix("_wall_head"))
    {
        Some(kind) => (kind, true),
        None => (
            name.strip_suffix("_skull")
                .or_else(|| name.strip_suffix("_head"))?,
            false,
        ),
    };
    let head = match kind {
        "skeleton" => Head::Mob("minecraft:entity/skeleton/skeleton"),
        "wither_skeleton" => Head::Mob("minecraft:entity/skeleton/wither_skeleton"),
        "creeper" => Head::Mob("minecraft:entity/creeper/creeper"),
        "zombie" => Head::Humanoid("minecraft:entity/zombie/zombie"),
        "player" => Head::Humanoid("minecraft:entity/player/wide/steve"),
        "piglin" => Head::Piglin,
        "dragon" => Head::Dragon,
        _ => return None,
    };
    Some((head, wall))
}

/// `SkullModel`, `PiglinHeadModel` and `DragonHeadModel` at rest.
fn skull(head: Head) -> Layer {
    let skull = cube([0.0, 0.0], [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0]);
    match head {
        Head::Mob(texture) => layer(
            texture,
            [64.0, 32.0],
            vec![part([0.0; 3], [0.0; 3], vec![skull])],
        ),
        Head::Humanoid(texture) => layer(
            texture,
            [64.0, 64.0],
            vec![part(
                [0.0; 3],
                [0.0; 3],
                vec![
                    skull,
                    cube([32.0, 0.0], [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0]).grow(0.25),
                ],
            )],
        ),
        Head::Piglin => {
            // `setupAnim` at animation position zero: `(cos 0 + 2.5) * 0.2`.
            let ear = 0.7;
            layer(
                "minecraft:entity/piglin/piglin",
                [64.0, 64.0],
                vec![
                    part(
                        [0.0; 3],
                        [0.0; 3],
                        vec![
                            cube([0.0, 0.0], [-5.0, -8.0, -4.0], [10.0, 8.0, 8.0]),
                            cube([31.0, 1.0], [-2.0, -4.0, -5.0], [4.0, 4.0, 1.0]),
                            cube([2.0, 4.0], [2.0, -2.0, -5.0], [1.0, 2.0, 1.0]),
                            cube([2.0, 0.0], [-3.0, -2.0, -5.0], [1.0, 2.0, 1.0]),
                        ],
                    )
                    .with(part(
                        [4.5, -6.0, 0.0],
                        [0.0, 0.0, -ear],
                        vec![cube([51.0, 6.0], [0.0, 0.0, -2.0], [1.0, 5.0, 4.0])],
                    ))
                    .with(part(
                        [-4.5, -6.0, 0.0],
                        [0.0, 0.0, ear],
                        vec![cube([39.0, 6.0], [-1.0, 0.0, -2.0], [1.0, 5.0, 4.0])],
                    )),
                ],
            )
        }
        Head::Dragon => layer(
            "minecraft:entity/enderdragon/dragon",
            [256.0, 256.0],
            vec![
                part(
                    [0.0, -7.986666, 0.0],
                    [0.0; 3],
                    vec![
                        cube([176.0, 44.0], [-6.0, -1.0, -24.0], [12.0, 5.0, 16.0]),
                        cube([112.0, 30.0], [-8.0, -8.0, -10.0], [16.0, 16.0, 16.0]),
                        cube([0.0, 0.0], [-5.0, -12.0, -4.0], [2.0, 4.0, 6.0]).mirror(),
                        cube([112.0, 0.0], [-5.0, -3.0, -22.0], [2.0, 2.0, 4.0]).mirror(),
                        cube([0.0, 0.0], [3.0, -12.0, -4.0], [2.0, 4.0, 6.0]),
                        cube([112.0, 0.0], [3.0, -3.0, -22.0], [2.0, 2.0, 4.0]),
                    ],
                )
                .scaled(0.75)
                // `(sin 0 + 1) * 0.2`: the jaw a little open.
                .with(part(
                    [0.0, 4.0, -8.0],
                    [0.2, 0.0, 0.0],
                    vec![cube([176.0, 65.0], [-6.0, 0.0, -16.0], [12.0, 4.0, 16.0])],
                )),
            ],
        ),
    }
}

/// `DecoratedPotRenderer.createBaseLayer`: the neck, flipped over the top, and the lid and floor.
fn pot_base() -> Vec<Part> {
    let face = cube([-14.0, 13.0], [0.0; 3], [14.0, 0.0, 14.0]);
    vec![
        part(
            [0.0, 37.0, 16.0],
            [PI, 0.0, 0.0],
            vec![
                cube([0.0, 0.0], [4.0, 17.0, 4.0], [8.0, 3.0, 8.0]).grow(-0.1),
                cube([0.0, 5.0], [5.0, 20.0, 5.0], [6.0, 1.0, 6.0]).grow(0.2),
            ],
        ),
        part([1.0, 16.0, 1.0], [0.0; 3], vec![face]),
        part([1.0, 0.0, 1.0], [0.0; 3], vec![face]),
    ]
}

/// `DecoratedPotRenderer.createSidesLayer`: four single faces, all plain sides.
fn pot_sides() -> Vec<Part> {
    let side = || vec![cube([1.0, 0.0], [0.0; 3], [14.0, 16.0, 0.0]).only(Dir::North)];
    vec![
        part([15.0, 16.0, 1.0], [0.0, 0.0, PI], side()),
        part([1.0, 16.0, 1.0], [0.0, -FRAC_PI_2, PI], side()),
        part([15.0, 16.0, 15.0], [0.0, FRAC_PI_2, PI], side()),
        part([1.0, 16.0, 15.0], [PI, 0.0, 0.0], side()),
    ]
}

/// `BookModel` after `setupAnim` with this openness and these page flips.
fn book(open: f32, flip1: f32, flip2: f32) -> Layer {
    let pages_x = open.sin();
    let flip = |amount: f32| {
        part(
            [pages_x, 0.0, 0.0],
            [0.0, open - 2.0 * open * amount, 0.0],
            vec![cube([24.0, 10.0], [0.0, -4.0, 0.0], [5.0, 8.0, 0.005])],
        )
    };
    layer(
        "minecraft:entity/enchantment/enchanting_table_book",
        [64.0, 32.0],
        vec![
            part(
                [0.0, 0.0, -1.0],
                [0.0, PI + open, 0.0],
                vec![cube([0.0, 0.0], [-6.0, -5.0, -0.005], [6.0, 10.0, 0.005])],
            ),
            part(
                [0.0, 0.0, 1.0],
                [0.0, -open, 0.0],
                vec![cube([16.0, 0.0], [0.0, -5.0, -0.005], [6.0, 10.0, 0.005])],
            ),
            part(
                [0.0; 3],
                [0.0, FRAC_PI_2, 0.0],
                vec![cube([12.0, 0.0], [-1.0, -5.0, 0.0], [2.0, 10.0, 0.005])],
            ),
            part(
                [pages_x, 0.0, 0.0],
                [0.0, open, 0.0],
                vec![cube([0.0, 10.0], [0.0, -4.0, -0.99], [5.0, 8.0, 1.0])],
            ),
            part(
                [pages_x, 0.0, 0.0],
                [0.0, -open, 0.0],
                vec![cube([12.0, 10.0], [0.0, -4.0, -0.01], [5.0, 8.0, 1.0])],
            ),
            flip(flip1),
            flip(flip2),
        ],
    )
}

/// `BellModel`, unrung.
fn bell() -> Vec<Part> {
    vec![
        part(
            [8.0, 12.0, 8.0],
            [0.0; 3],
            vec![cube([0.0, 0.0], [-3.0, -6.0, -3.0], [6.0, 7.0, 6.0])],
        )
        .with(part(
            [-8.0, -12.0, -8.0],
            [0.0; 3],
            vec![cube([0.0, 13.0], [4.0, 4.0, 4.0], [8.0, 2.0, 8.0])],
        )),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(block: &str, props: &[(&str, &str)]) -> (Vec3, Vec3) {
        let baked = bake(block, props).expect("drawn in code");
        let points = baked.quads.iter().flat_map(|quad| quad.positions);
        let min = points.clone().fold(Vec3::INFINITY, Vec3::min);
        let max = points.fold(Vec3::NEG_INFINITY, Vec3::max);
        (min, max)
    }

    fn near(a: Vec3, b: Vec3) -> bool {
        (a - b).abs().max_element() < 1e-4
    }

    #[test]
    fn a_chest_fills_its_block_less_a_pixel_around_and_two_on_top() {
        let (min, max) = bounds(
            "minecraft:chest",
            &[("facing", "north"), ("type", "single")],
        );
        assert!(near(min, Vec3::new(1.0, 0.0, 0.0) / 16.0), "{min}");
        assert!(near(max, Vec3::new(15.0, 14.0, 15.0) / 16.0), "{max}");
    }

    #[test]
    fn a_chest_lock_pokes_out_of_the_side_it_faces() {
        let reach = |facing| {
            let (min, max) = bounds("minecraft:chest", &[("facing", facing), ("type", "single")]);
            [min.z, 1.0 - max.z, min.x, 1.0 - max.x]
        };
        assert!(reach("north")[0].abs() < 1e-4);
        assert!(reach("south")[1].abs() < 1e-4);
        assert!(reach("west")[2].abs() < 1e-4);
        assert!(reach("east")[3].abs() < 1e-4);
    }

    #[test]
    fn every_face_winds_outwards() {
        // One box per model, turned, flipped and mirrored: each face points away from its middle.
        let plain = cube([0.0, 0.0], [-4.0, -8.0, -4.0], [8.0, 8.0, 8.0]);
        for (cube, rotation) in [
            (plain, [0.0; 3]),
            (plain.mirror(), [0.0; 3]),
            (plain, [0.3, 1.1, -0.7]),
            (plain.mirror(), [PI, 0.0, FRAC_PI_2]),
        ] {
            let model = layer(
                "minecraft:test",
                [64.0, 32.0],
                vec![part([1.0, 2.0, 3.0], rotation, vec![cube])],
            );
            for transform in [
                Mat4::IDENTITY,
                shulker_pose(Dir::West),
                Mat4::from_scale(Vec3::new(-1.0, -1.0, 1.0)),
            ] {
                let mut quads = Vec::new();
                emit(&mut quads, &model, 0, &model.parts[0], transform);
                assert_eq!(quads.len(), 6);
                let middle = quads.iter().flat_map(|q| q.positions).sum::<Vec3>() / 24.0;
                for quad in &quads {
                    let centre = quad.positions.iter().sum::<Vec3>() / 4.0;
                    let normal = (quad.positions[1] - quad.positions[0])
                        .cross(quad.positions[2] - quad.positions[0]);
                    assert!(
                        (centre - middle).dot(normal) > 0.0,
                        "a face at {centre} points inwards"
                    );
                }
            }
        }
    }

    #[test]
    fn a_shulker_box_is_a_whole_block() {
        for facing in ["up", "down", "north", "east"] {
            let (min, max) = bounds("minecraft:shulker_box", &[("facing", facing)]);
            assert!(near(min, Vec3::splat(0.00025)), "{facing}: {min}");
            assert!(near(max, Vec3::splat(0.99975)), "{facing}: {max}");
        }
    }

    #[test]
    fn heads_sit_on_the_floor_or_against_the_wall() {
        let (min, max) = bounds("minecraft:creeper_head", &[("rotation", "0")]);
        assert!(near(min, Vec3::new(0.25, 0.0, 0.25)), "{min}");
        assert!(near(max, Vec3::new(0.75, 0.5, 0.75)), "{max}");
        // A wall head facing north hangs on the south wall.
        let (min, max) = bounds("minecraft:skeleton_wall_skull", &[("facing", "north")]);
        assert!(near(min, Vec3::new(0.25, 0.25, 0.5)), "{min}");
        assert!(near(max, Vec3::new(0.75, 0.75, 1.0)), "{max}");
    }

    #[test]
    fn a_standing_banner_reaches_up_into_the_block_above() {
        // The bar's top is 44 model pixels up at two thirds scale; the flag's sway lifts a corner.
        let (min, max) = bounds("minecraft:red_banner", &[("rotation", "0")]);
        assert!(
            min.y.abs() < 1e-4 && (max.y - 44.0 * 2.0 / 3.0 / 16.0).abs() < 0.01,
            "{min} {max}"
        );
        let tinted = bake("minecraft:red_banner", &[("rotation", "0")])
            .unwrap()
            .quads
            .iter()
            .filter(|quad| quad.tint.is_some())
            .count();
        assert_eq!(tinted, 6, "only the flag takes the dye");
    }

    #[test]
    fn a_decorated_pot_is_a_pixel_in_with_its_neck_on_top() {
        // The neck's rim, shrunk by a tenth of a pixel, ends at 19.9.
        let (min, max) = bounds("minecraft:decorated_pot", &[("facing", "north")]);
        assert!(near(min, Vec3::new(1.0, 0.0, 1.0) / 16.0), "{min}");
        assert!((max.y - 19.9 / 16.0).abs() < 1e-4, "{max}");
    }

    #[test]
    fn books_appear_only_where_the_game_draws_one() {
        assert!(bake("minecraft:enchanting_table", &[]).is_some());
        assert!(
            bake(
                "minecraft:lectern",
                &[("has_book", "true"), ("facing", "north")]
            )
            .is_some()
        );
        assert!(
            bake(
                "minecraft:lectern",
                &[("has_book", "false"), ("facing", "north")]
            )
            .is_none()
        );
        assert!(bake("minecraft:stone", &[]).is_none());
    }

    #[test]
    fn child_parts_are_drawn_in_their_parent_pose() {
        // Only a piglin's ears reach past its head's five pixels to each side.
        let baked = bake("minecraft:piglin_head", &[("rotation", "0")]).unwrap();
        let ears = baked
            .quads
            .iter()
            .flat_map(|quad| quad.positions)
            .filter(|p| (p.x - 0.5).abs() > 5.0 / 16.0 + 1e-4)
            .count();
        assert!(ears > 0, "the ears are missing");
        // The bell's lip, a child of its body, is the only box wider than six pixels.
        let (min, max) = bounds(
            "minecraft:bell",
            &[("attachment", "floor"), ("facing", "north")],
        );
        assert!(near(min, Vec3::new(4.0, 4.0, 4.0) / 16.0), "{min}");
        assert!(near(max, Vec3::new(12.0, 13.0, 12.0) / 16.0), "{max}");
    }

    #[test]
    fn a_banner_names_its_dye() {
        assert_eq!(banner_color("light_blue_wall_banner"), Some((3, true)));
        assert_eq!(banner_color("black_banner"), Some((15, false)));
        assert_eq!(banner_color("banner"), None);
    }
}
