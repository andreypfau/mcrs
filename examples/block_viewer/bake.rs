//! Turns a blockstate into shaded quads, following the vanilla client's `FaceBakery`,
//! `ModelBlockRenderer` and `BlockModelLighter`.
//!
//! Positions come out in block space (0..1 for a full cube), UVs in sprite space (0..1), and
//! `color` holds the final vanilla vertex byte: ambient occlusion multiplied by the directional
//! face shade, in sRGB space exactly as the client writes it into its vertex buffer.

use std::collections::HashSet;

use bevy::math::{IVec3, Mat4, Vec3};

use crate::model::{BlockStateFile, Element, ElementRotation, Face, ResolvedModel, resolve_model};

const BLOCK_MIDDLE: Vec3 = Vec3::splat(0.5);

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Dir {
    Down = 0,
    Up = 1,
    North = 2,
    South = 3,
    West = 4,
    East = 5,
}

impl Dir {
    pub const ALL: [Dir; 6] = [
        Dir::Down,
        Dir::Up,
        Dir::North,
        Dir::South,
        Dir::West,
        Dir::East,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Dir::Down => "down",
            Dir::Up => "up",
            Dir::North => "north",
            Dir::South => "south",
            Dir::West => "west",
            Dir::East => "east",
        }
    }

    pub fn from_name(name: &str) -> Option<Dir> {
        Dir::ALL.into_iter().find(|d| d.name() == name)
    }

    pub fn inormal(self) -> IVec3 {
        match self {
            Dir::Down => IVec3::new(0, -1, 0),
            Dir::Up => IVec3::new(0, 1, 0),
            Dir::North => IVec3::new(0, 0, -1),
            Dir::South => IVec3::new(0, 0, 1),
            Dir::West => IVec3::new(-1, 0, 0),
            Dir::East => IVec3::new(1, 0, 0),
        }
    }

    pub fn normal(self) -> Vec3 {
        self.inormal().as_vec3()
    }

    pub fn axis(self) -> usize {
        match self {
            Dir::West | Dir::East => 0,
            Dir::Down | Dir::Up => 1,
            Dir::North | Dir::South => 2,
        }
    }

    /// The direction whose normal is most aligned with `v`, or `None` when `v` is degenerate.
    pub fn nearest(v: Vec3) -> Option<Dir> {
        if !v.is_finite() {
            return None;
        }
        let mut best = None;
        let mut best_dot = 0.0f32;
        for dir in Dir::ALL {
            let dot = v.dot(dir.normal());
            if dot > best_dot {
                best_dot = dot;
                best = Some(dir);
            }
        }
        best
    }
}

/// Per-direction vertex corner order, each entry selecting `from` (0) or `to` (1) per axis.
/// Winding is counter-clockwise seen from outside the block, matching the client's `FaceInfo`.
const FACE_CORNERS: [[[u8; 3]; 4]; 6] = [
    [[0, 0, 1], [0, 0, 0], [1, 0, 0], [1, 0, 1]], // down
    [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]], // up
    [[1, 1, 0], [1, 0, 0], [0, 0, 0], [0, 1, 0]], // north
    [[0, 1, 1], [0, 0, 1], [1, 0, 1], [1, 1, 1]], // south
    [[0, 1, 0], [0, 0, 0], [0, 0, 1], [0, 1, 1]], // west
    [[1, 1, 1], [1, 0, 1], [1, 0, 0], [1, 1, 0]], // east
];

fn corner(dir: Dir, index: usize, from: Vec3, to: Vec3) -> Vec3 {
    let select = FACE_CORNERS[dir as usize][index];
    Vec3::new(
        if select[0] == 0 { from.x } else { to.x },
        if select[1] == 0 { from.y } else { to.y },
        if select[2] == 0 { from.z } else { to.z },
    )
}

/// `(minU, minV, maxU, maxV)` in texel space. `minU > maxU` is legal and means mirrored, so the
/// pairs must never be sorted.
fn default_uv(dir: Dir, from: [f32; 3], to: [f32; 3]) -> [f32; 4] {
    let (f, t) = (from, to);
    match dir {
        Dir::Down => [f[0], 16.0 - t[2], t[0], 16.0 - f[2]],
        Dir::Up => [f[0], f[2], t[0], t[2]],
        Dir::North => [16.0 - t[0], 16.0 - t[1], 16.0 - f[0], 16.0 - f[1]],
        Dir::South => [f[0], 16.0 - t[1], t[0], 16.0 - f[1]],
        Dir::West => [f[2], 16.0 - t[1], t[2], 16.0 - f[1]],
        Dir::East => [16.0 - t[2], 16.0 - t[1], 16.0 - f[2], 16.0 - f[1]],
    }
}

fn uv_u(index: usize, uv: [f32; 4]) -> f32 {
    if index != 0 && index != 1 {
        uv[2]
    } else {
        uv[0]
    }
}

fn uv_v(index: usize, uv: [f32; 4]) -> f32 {
    if index != 0 && index != 3 {
        uv[3]
    } else {
        uv[1]
    }
}

/// Quarter turns applied to a whole variant, built from exact signed permutations. Trig-built
/// matrices are not usable here: `recalculate_winding` matches vertices by exact float equality.
#[derive(Copy, Clone, Debug, Default)]
pub struct VariantRotation {
    x: u32,
    y: u32,
    z: u32,
}

impl VariantRotation {
    pub fn from_degrees(x: i32, y: i32, z: i32) -> Result<Self, String> {
        let turns = |deg: i32| -> Result<u32, String> {
            if deg % 90 != 0 {
                return Err(format!("variant rotation {deg} is not a multiple of 90"));
            }
            Ok(deg.rem_euclid(360) as u32 / 90)
        };
        Ok(Self {
            x: turns(x)?,
            y: turns(y)?,
            z: turns(z)?,
        })
    }

    pub fn is_identity(self) -> bool {
        self.x == 0 && self.y == 0 && self.z == 0
    }

    pub fn apply(self, mut v: Vec3) -> Vec3 {
        for _ in 0..self.x {
            v = Vec3::new(v.x, v.z, -v.y);
        }
        for _ in 0..self.y {
            v = Vec3::new(-v.z, v.y, v.x);
        }
        for _ in 0..self.z {
            v = Vec3::new(v.y, -v.x, v.z);
        }
        v
    }

    fn matrix(self) -> Mat4 {
        Mat4::from_cols(
            self.apply(Vec3::X).extend(0.0),
            self.apply(Vec3::Y).extend(0.0),
            self.apply(Vec3::Z).extend(0.0),
            Vec3::ZERO.extend(1.0),
        )
    }
}

fn element_rotation_matrix(rotation: &ElementRotation) -> Result<Mat4, String> {
    let axis = match rotation.axis.as_str() {
        "x" => Vec3::X,
        "y" => Vec3::Y,
        "z" => Vec3::Z,
        other => return Err(format!("unknown rotation axis `{other}`")),
    };
    let mut matrix = Mat4::from_axis_angle(axis, rotation.angle.to_radians());
    if rotation.rescale {
        let stretch = |unit: Vec3| {
            let t = matrix.transform_vector3(unit);
            1.0 / t.x.abs().max(t.y.abs()).max(t.z.abs())
        };
        let scale = Vec3::new(stretch(Vec3::X), stretch(Vec3::Y), stretch(Vec3::Z));
        matrix *= Mat4::from_scale(scale);
    }
    Ok(matrix)
}

/// Local UV plane -> world orientation. Local `+Z` is the face normal.
fn uv_plane(dir: Dir) -> Mat4 {
    use std::f32::consts::FRAC_PI_2;
    match dir {
        Dir::South => Mat4::IDENTITY,
        Dir::East => Mat4::from_rotation_y(FRAC_PI_2),
        Dir::West => Mat4::from_rotation_y(-FRAC_PI_2),
        Dir::North => Mat4::from_rotation_y(std::f32::consts::PI),
        Dir::Up => Mat4::from_rotation_x(-FRAC_PI_2),
        Dir::Down => Mat4::from_rotation_x(FRAC_PI_2),
    }
}

fn uvlock_transform(rotation: VariantRotation, dir: Dir) -> Mat4 {
    let rotated = rotation.matrix() * uv_plane(dir);
    let landed = Dir::nearest(rotated.transform_vector3(Vec3::Z)).unwrap_or(dir);
    (uv_plane(landed).inverse() * rotated).inverse()
}

/// A face whose element is flat along that face's own axis is still drawn, but a flat element
/// drops the two faces that would be edge-on. Vanilla relies on this for `cross`-shaped models.
fn draws_face(element: &Element, dir: Dir) -> bool {
    let flat: Vec<usize> = (0..3)
        .filter(|&axis| element.from[axis] == element.to[axis])
        .collect();
    flat.iter().all(|&axis| axis == dir.axis())
}

fn recalculate_winding(positions: &mut [Vec3; 4], uvs: &mut [[f32; 2]; 4], dir: Dir) {
    let mut min = positions[0];
    let mut max = positions[0];
    for p in positions.iter() {
        min = min.min(*p);
        max = max.max(*p);
    }
    for slot in 0..4 {
        let want = corner(dir, slot, min, max);
        let found = (slot..4).find(|&i| positions[i] == want);
        match found {
            Some(i) => {
                positions.swap(slot, i);
                uvs.swap(slot, i);
            }
            None => panic!("cannot find vertex to swap while winding a {dir:?} face"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BakedQuad {
    pub positions: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub dir: Dir,
    pub sprite: usize,
    pub cull: Option<Dir>,
    /// Occlusion factor per vertex, before the directional shade is folded in.
    #[allow(
        dead_code,
        reason = "the viewer draws `color`; the factor is what the tests pin"
    )]
    pub ao: [f32; 4],
    /// Vanilla's final vertex byte, sRGB space, before any tint colour is multiplied in.
    pub color: [u8; 4],
}

#[derive(Debug)]
pub struct BakedBlock {
    pub quads: Vec<BakedQuad>,
    pub sprites: Vec<String>,
}

/// What the mesher needs to know about the blocks around the one being baked. Implementing this
/// over a real chunk section is the only change needed to mesh actual terrain.
pub trait Neighborhood {
    fn shade_brightness(&self, pos: IVec3) -> f32;
    fn is_view_blocking(&self, pos: IVec3) -> bool;
    fn light_dampening(&self, pos: IVec3) -> u8;
    fn is_collision_shape_full_block(&self, pos: IVec3) -> bool;
    fn light_emission(&self, _pos: IVec3) -> u8 {
        0
    }
}

/// Every listed position is a full opaque cube; everything else is air.
#[derive(Debug, Default, Clone)]
pub struct TinyWorld {
    solid: HashSet<IVec3>,
}

impl TinyWorld {
    pub fn new(blocks: impl IntoIterator<Item = IVec3>) -> Self {
        Self {
            solid: blocks.into_iter().collect(),
        }
    }

    pub fn is_solid(&self, pos: IVec3) -> bool {
        self.solid.contains(&pos)
    }
}

impl Neighborhood for TinyWorld {
    fn shade_brightness(&self, pos: IVec3) -> f32 {
        if self.is_solid(pos) { 0.2 } else { 1.0 }
    }

    fn is_view_blocking(&self, pos: IVec3) -> bool {
        self.is_solid(pos)
    }

    fn light_dampening(&self, pos: IVec3) -> u8 {
        if self.is_solid(pos) { 15 } else { 0 }
    }

    fn is_collision_shape_full_block(&self, pos: IVec3) -> bool {
        self.is_solid(pos)
    }
}

const AO_CORNERS: [[Dir; 4]; 6] = [
    [Dir::West, Dir::East, Dir::North, Dir::South], // down
    [Dir::East, Dir::West, Dir::North, Dir::South], // up
    [Dir::Up, Dir::Down, Dir::East, Dir::West],     // north
    [Dir::West, Dir::East, Dir::Down, Dir::Up],     // south
    [Dir::Up, Dir::Down, Dir::North, Dir::South],   // west
    [Dir::Down, Dir::Up, Dir::North, Dir::South],   // east
];

/// Directional face shade, overworld values from the client's `CardinalLighting`.
fn face_shade(dir: Dir) -> f32 {
    match dir {
        Dir::Down => 0.5,
        Dir::Up => 1.0,
        Dir::North | Dir::South => 0.8,
        Dir::West | Dir::East => 0.6,
    }
}

/// Position along `dir`'s axis, oriented so the result is 0 at the face opposite `dir`.
fn coord_towards(dir: Dir, p: Vec3) -> f32 {
    let axis = dir.axis();
    let c = p[axis];
    if dir.inormal()[axis] > 0 { c } else { 1.0 - c }
}

fn ambient_occlusion(
    positions: &[Vec3; 4],
    dir: Dir,
    pos: IVec3,
    world: &dyn Neighborhood,
) -> [f32; 4] {
    const EPS: f32 = 1.0e-4;
    const NEAR_ONE: f32 = 0.9999;

    let mut min = positions[0];
    let mut max = positions[0];
    for p in positions.iter() {
        min = min.min(*p);
        max = max.max(*p);
    }
    let full = world.is_collision_shape_full_block(pos);
    let face_cubic = match dir {
        Dir::Down => min.y == max.y && (min.y < EPS || full),
        Dir::Up => min.y == max.y && (max.y > NEAR_ONE || full),
        Dir::North => min.z == max.z && (min.z < EPS || full),
        Dir::South => min.z == max.z && (max.z > NEAR_ONE || full),
        Dir::West => min.x == max.x && (min.x < EPS || full),
        Dir::East => min.x == max.x && (max.x > NEAR_ONE || full),
    };

    let base = if face_cubic { pos + dir.inormal() } else { pos };
    let c = AO_CORNERS[dir as usize];

    let mut shade = [0.0f32; 4];
    let mut translucent = [false; 4];
    for i in 0..4 {
        shade[i] = world.shade_brightness(base + c[i].inormal());
        // The occlusion probe sits one block further out than the side sample.
        let probe = base + c[i].inormal() + dir.inormal();
        translucent[i] = !world.is_view_blocking(probe) || world.light_dampening(probe) == 0;
    }
    let diagonal =
        |a: usize, b: usize| world.shade_brightness(base + c[a].inormal() + c[b].inormal());
    // The `shade[0]` reuse in all four branches is vanilla, including where symmetry would call
    // for `shade[1]`. "Fixing" it diverges from the client on every inside corner.
    let corner_02 = if !translucent[2] && !translucent[0] {
        shade[0]
    } else {
        diagonal(0, 2)
    };
    let corner_03 = if !translucent[3] && !translucent[0] {
        shade[0]
    } else {
        diagonal(0, 3)
    };
    let corner_12 = if !translucent[2] && !translucent[1] {
        shade[0]
    } else {
        diagonal(1, 2)
    };
    let corner_13 = if !translucent[3] && !translucent[1] {
        shade[0]
    } else {
        diagonal(1, 3)
    };
    let center = world.shade_brightness(if face_cubic { base } else { pos });

    let slot = [
        (shade[3] + shade[0] + corner_03 + center) * 0.25,
        (shade[2] + shade[0] + corner_02 + center) * 0.25,
        (shade[2] + shade[1] + corner_12 + center) * 0.25,
        (shade[3] + shade[1] + corner_13 + center) * 0.25,
    ];

    // Vanilla assigns the four slots to vertices through a per-direction table for full faces and
    // through weight products for partial ones. Bilinear interpolation over the face square is
    // both: evaluated at the corners of a full face it reproduces the table exactly for all six
    // directions, and it is the closed form of the weight products in between.
    let mut ao = [0.0f32; 4];
    for (i, ao) in ao.iter_mut().enumerate() {
        // Vanilla blends at the face's bounding-box corners, not at the vertices themselves: a
        // rotated element keeps the unrotated corner order, and an element poking outside its own
        // block extrapolates instead of clamping.
        let p = corner(dir, i, min, max);
        let u = coord_towards(c[1], p);
        let v = coord_towards(c[3], p);
        let blend = (1.0 - u) * (1.0 - v) * slot[1]
            + (1.0 - u) * v * slot[0]
            + u * (1.0 - v) * slot[2]
            + u * v * slot[3];
        *ao = blend.clamp(0.0, 1.0);
    }
    ao
}

fn as_8bit(value: f32) -> u8 {
    (value * 255.0).floor().clamp(0.0, 255.0) as u8
}

fn scale_rgb(component: u8, scale: f32) -> u8 {
    ((component as f32 * scale) as i32).clamp(0, 255) as u8
}

struct BakeContext<'a> {
    rotation: VariantRotation,
    uvlock: bool,
    smooth: bool,
    pos: IVec3,
    world: &'a dyn Neighborhood,
}

fn bake_face(
    element: &Element,
    dir: Dir,
    face: &Face,
    sprite: usize,
    ctx: &BakeContext<'_>,
) -> Result<BakedQuad, String> {
    let BakeContext {
        rotation,
        uvlock,
        smooth,
        pos,
        world,
    } = *ctx;
    if face.rotation % 90 != 0 {
        return Err(format!(
            "face rotation {} is not a multiple of 90",
            face.rotation
        ));
    }
    let shift = face.rotation.rem_euclid(360) as usize / 90;
    let uv = face
        .uv
        .unwrap_or_else(|| default_uv(dir, element.from, element.to));

    let from = Vec3::from(element.from) / 16.0;
    let to = Vec3::from(element.to) / 16.0;
    let mut positions = [Vec3::ZERO; 4];
    let mut uvs = [[0.0f32; 2]; 4];
    for i in 0..4 {
        positions[i] = corner(dir, i, from, to);
        let source = (i + shift) % 4;
        uvs[i] = [uv_u(source, uv) / 16.0, uv_v(source, uv) / 16.0];
    }

    if let Some(rot) = &element.rotation {
        let matrix = element_rotation_matrix(rot)?;
        // `origin` is scaled into block space once, at parse time; `from`/`to` are scaled here.
        let origin = Vec3::from(rot.origin) * 0.0625;
        for p in positions.iter_mut() {
            *p = origin + matrix.transform_vector3(*p - origin);
        }
    }

    let mut cull = face.cullface.as_deref().and_then(Dir::from_name);
    if !rotation.is_identity() {
        for p in positions.iter_mut() {
            *p = BLOCK_MIDDLE + rotation.apply(*p - BLOCK_MIDDLE);
        }
        if uvlock {
            let transform = uvlock_transform(rotation, dir);
            for uv in uvs.iter_mut() {
                let p = transform.transform_point3(Vec3::new(uv[0] - 0.5, uv[1] - 0.5, 0.0));
                *uv = [p.x + 0.5, p.y + 0.5];
            }
        }
        cull = cull.and_then(|d| Dir::nearest(rotation.apply(d.normal())));
    }

    let normal = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
    let facing = Dir::nearest(normal);
    if element.rotation.is_none()
        && let Some(facing) = facing
    {
        recalculate_winding(&mut positions, &mut uvs, facing);
    }
    let facing = facing.unwrap_or(Dir::Up);

    let ao = if smooth {
        ambient_occlusion(&positions, facing, pos, world)
    } else {
        [1.0; 4]
    };
    let shade = if element.shade {
        face_shade(facing)
    } else {
        face_shade(Dir::Up)
    };
    let color = std::array::from_fn(|i| scale_rgb(as_8bit(ao[i]), shade));

    Ok(BakedQuad {
        positions,
        uvs,
        dir: facing,
        sprite,
        cull,
        ao,
        color,
    })
}

pub fn bake(
    block: &str,
    props: &[(&str, &str)],
    pos: IVec3,
    world: &dyn Neighborhood,
) -> Result<BakedBlock, String> {
    let states = BlockStateFile::load(block)?;
    let variant = states.select(props)?;
    let model = resolve_model(&variant.model)?;
    let rotation = VariantRotation::from_degrees(variant.x, variant.y, variant.z)?;
    bake_model(&model, rotation, variant.uvlock, pos, world)
}

fn bake_model(
    model: &ResolvedModel,
    rotation: VariantRotation,
    uvlock: bool,
    pos: IVec3,
    world: &dyn Neighborhood,
) -> Result<BakedBlock, String> {
    let ctx = BakeContext {
        rotation,
        uvlock,
        smooth: model.ambient_occlusion && world.light_emission(pos) == 0,
        pos,
        world,
    };
    let mut sprites: Vec<String> = Vec::new();
    let mut quads = Vec::new();
    for element in &model.elements {
        // Ordinal order, not JSON key order, so the vertex layout matches the client's.
        for dir in Dir::ALL {
            let Some(face) = element.faces.get(dir.name()) else {
                continue;
            };
            if !draws_face(element, dir) {
                continue;
            }
            let sprite_id = model.sprite_of(face)?;
            let sprite = match sprites.iter().position(|s| s == sprite_id) {
                Some(index) => index,
                None => {
                    sprites.push(sprite_id.to_string());
                    sprites.len() - 1
                }
            };
            quads.push(bake_face(element, dir, face, sprite, &ctx)?);
        }
    }
    Ok(BakedBlock { quads, sprites })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oak_log(props: &[(&str, &str)], world: &TinyWorld) -> BakedBlock {
        bake("minecraft:oak_log", props, IVec3::ZERO, world).expect("oak_log bakes")
    }

    fn quad(baked: &BakedBlock, dir: Dir) -> &BakedQuad {
        baked
            .quads
            .iter()
            .find(|q| q.dir == dir)
            .unwrap_or_else(|| panic!("no {dir:?} quad"))
    }

    fn lone_block() -> TinyWorld {
        TinyWorld::new([IVec3::ZERO])
    }

    /// A solid floor under the block, wide enough that every AO sample around it is covered.
    fn block_on_floor(extra: &[IVec3]) -> TinyWorld {
        let mut blocks = vec![IVec3::ZERO];
        blocks.extend_from_slice(extra);
        for x in -2..=2 {
            for z in -2..=2 {
                blocks.push(IVec3::new(x, -1, z));
            }
        }
        TinyWorld::new(blocks)
    }

    #[test]
    fn geometry_and_sprites() {
        let baked = oak_log(&[("axis", "y")], &lone_block());
        assert_eq!(baked.quads.len(), 6);
        assert_eq!(baked.sprites.len(), 2);

        for dir in Dir::ALL {
            let q = quad(&baked, dir);
            for i in 0..4 {
                assert_eq!(q.positions[i], corner(dir, i, Vec3::ZERO, Vec3::ONE));
            }
            assert_eq!(
                q.uvs,
                [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
                "default uv derivation for {dir:?}"
            );
            let normal = (q.positions[1] - q.positions[0]).cross(q.positions[2] - q.positions[0]);
            assert!(normal.dot(dir.normal()) > 0.0, "winding of {dir:?}");
            assert_eq!(q.cull, Some(dir));
        }

        let sprite_of = |dir| baked.sprites[quad(&baked, dir).sprite].as_str();
        assert_eq!(sprite_of(Dir::Up), "minecraft:block/oak_log_top");
        assert_eq!(sprite_of(Dir::Down), "minecraft:block/oak_log_top");
        for dir in [Dir::North, Dir::South, Dir::West, Dir::East] {
            assert_eq!(sprite_of(dir), "minecraft:block/oak_log");
        }
    }

    #[test]
    fn face_shade_in_open_air() {
        let baked = oak_log(&[("axis", "y")], &lone_block());
        for (dir, expected) in [
            (Dir::Down, 127u8),
            (Dir::Up, 255),
            (Dir::North, 204),
            (Dir::South, 204),
            (Dir::West, 153),
            (Dir::East, 153),
        ] {
            let q = quad(&baked, dir);
            assert_eq!(q.ao, [1.0; 4], "isolated block has no occlusion on {dir:?}");
            assert_eq!(q.color, [expected; 4], "face shade of {dir:?}");
        }
    }

    #[test]
    fn ambient_occlusion_darkens_toward_the_floor() {
        let baked = oak_log(&[("axis", "y")], &block_on_floor(&[]));
        let q = quad(&baked, Dir::West);
        // v0 and v3 are the top corners, v1 and v2 the bottom ones.
        assert_eq!(q.ao, [1.0, 0.6, 0.6, 1.0]);
        assert_eq!(q.color, [153, 91, 91, 153]);
    }

    #[test]
    fn inside_corner_reuses_the_first_side_sample() {
        // Occludes the west face's `north` corner probe, firing the `corner_12` short circuit.
        let world = block_on_floor(&[IVec3::new(-2, 0, -1)]);
        let baked = oak_log(&[("axis", "y")], &world);
        let q = quad(&baked, Dir::West);
        // Vanilla substitutes the *up* sample (1.0) here, not the *down* one (0.2): the corner
        // averages to 0.8 rather than 0.6.
        assert_eq!(q.ao[1], 0.8);
        assert_eq!(q.color[1], 122);
    }

    #[test]
    fn object_form_texture_slots_resolve() {
        let baked = bake("minecraft:glass", &[], IVec3::ZERO, &lone_block()).expect("glass bakes");
        assert_eq!(baked.sprites, ["minecraft:block/glass"]);
        assert_eq!(baked.quads.len(), 6);
    }

    #[test]
    fn variant_rotation_turns_the_log_on_its_side() {
        let baked = oak_log(&[("axis", "x")], &lone_block());
        assert_eq!(baked.quads.len(), 6);
        let sprite_of = |dir| baked.sprites[quad(&baked, dir).sprite].as_str();
        // A log along X shows its rings on the two X faces and bark everywhere else.
        assert_eq!(sprite_of(Dir::West), "minecraft:block/oak_log_top");
        assert_eq!(sprite_of(Dir::East), "minecraft:block/oak_log_top");
        for dir in [Dir::Up, Dir::Down, Dir::North, Dir::South] {
            assert_eq!(sprite_of(dir), "minecraft:block/oak_log", "{dir:?}");
        }
        for dir in Dir::ALL {
            let q = quad(&baked, dir);
            for i in 0..4 {
                assert_eq!(q.positions[i], corner(dir, i, Vec3::ZERO, Vec3::ONE));
            }
        }
    }
}
