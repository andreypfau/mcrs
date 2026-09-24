//! Turns a blockstate into shaded quads, following the vanilla client's `FaceBakery`,
//! `ModelBlockRenderer` and `BlockModelLighter`.
//!
//! Positions come out in block space (0..1 for a full cube) and UVs in sprite space (0..1).
//! Ambient occlusion depends on the neighbours, so it is left to the mesher.

use bevy::math::{Mat4, Vec3};
use mcrs_minecraft_mesh::ambient::corner;

use crate::model::{
    BlockStateFile, Element, ElementRotation, Face, Pack, ResolvedModel, resolve_model,
};

const BLOCK_MIDDLE: Vec3 = Vec3::splat(0.5);

pub use mcrs_minecraft_core::Direction as Dir;

fn axis(dir: Dir) -> usize {
    match dir {
        Dir::West | Dir::East => 0,
        Dir::Down | Dir::Up => 1,
        Dir::North | Dir::South => 2,
    }
}

fn from_name(name: &str) -> Option<Dir> {
    Dir::all().into_iter().find(|d| d.name() == name)
}

/// The direction whose normal is most aligned with `v`, or `None` when `v` is degenerate.
pub fn nearest(v: Vec3) -> Option<Dir> {
    if !v.is_finite() {
        return None;
    }
    let mut best = None;
    let mut best_dot = 0.0f32;
    for dir in Dir::all() {
        let dot = v.dot(dir.normal().as_vec3());
        if dot > best_dot {
            best_dot = dot;
            best = Some(dir);
        }
    }
    best
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
    let landed = nearest(rotated.transform_vector3(Vec3::Z)).unwrap_or(dir);
    (uv_plane(landed).inverse() * rotated).inverse()
}

/// A face whose element is flat along that face's own axis is still drawn, but a flat element
/// drops the two faces that would be edge-on. Vanilla relies on this for `cross`-shaped models.
pub(crate) fn draws_face(element: &Element, dir: Dir) -> bool {
    let flat: Vec<usize> = (0..3)
        .filter(|&axis| element.from[axis] == element.to[axis])
        .collect();
    let face_axis = axis(dir);
    flat.iter().all(|&axis| axis == face_axis)
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
    pub shade: f32,
    /// The face's `tintindex`, if it has one.
    pub tint: Option<u32>,
}

#[derive(Debug)]
pub struct BakedBlock {
    pub quads: Vec<BakedQuad>,
    pub sprites: Vec<String>,
    pub ambient_occlusion: bool,
}

/// Directional face shade, overworld values from the client's `CardinalLighting`.
fn face_shade(dir: Dir) -> f32 {
    match dir {
        Dir::Down => 0.5,
        Dir::Up => 1.0,
        Dir::North | Dir::South => 0.8,
        Dir::West | Dir::East => 0.6,
    }
}

/// A face's corners, texture coordinates and orientation before any lighting is applied.
pub(crate) struct FaceGeometry {
    pub positions: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub facing: Dir,
    pub cull: Option<Dir>,
}

pub(crate) fn face_geometry(
    element: &Element,
    dir: Dir,
    face: &Face,
    rotation: VariantRotation,
    uvlock: bool,
) -> Result<FaceGeometry, String> {
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

    let mut cull = face.cullface.as_deref().and_then(from_name);
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
        cull = cull.and_then(|d| nearest(rotation.apply(d.normal().as_vec3())));
    }

    let normal = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
    let facing = nearest(normal);
    if element.rotation.is_none()
        && let Some(facing) = facing
    {
        recalculate_winding(&mut positions, &mut uvs, facing);
    }
    Ok(FaceGeometry {
        positions,
        uvs,
        facing: facing.unwrap_or(Dir::Up),
        cull,
    })
}

fn bake_face(
    element: &Element,
    dir: Dir,
    face: &Face,
    sprite: usize,
    rotation: VariantRotation,
    uvlock: bool,
) -> Result<BakedQuad, String> {
    let FaceGeometry {
        positions,
        uvs,
        facing,
        cull,
    } = face_geometry(element, dir, face, rotation, uvlock)?;

    let shade = if element.shade {
        face_shade(facing)
    } else {
        face_shade(Dir::Up)
    };
    Ok(BakedQuad {
        positions,
        uvs,
        dir: facing,
        sprite,
        cull,
        shade,
        tint: face.tint_index,
    })
}

pub fn bake(pack: &Pack, block: &str, props: &[(&str, &str)]) -> Result<BakedBlock, String> {
    let states = BlockStateFile::load(pack, block)?;
    let mut merged = BakedBlock {
        quads: Vec::new(),
        sprites: Vec::new(),
        ambient_occlusion: false,
    };
    // A multipart blockstate contributes several models at once (a fence post plus each connected
    // arm); their quads share one sprite table so the result bakes exactly like a single model.
    for (index, variant) in states.select_all(props)?.into_iter().enumerate() {
        let model = resolve_model(pack, &variant.model)?;
        if index == 0 {
            merged.ambient_occlusion = model.ambient_occlusion;
        }
        let rotation = VariantRotation::from_degrees(variant.x, variant.y, variant.z)?;
        let part = bake_model(&model, rotation, variant.uvlock)?;
        for mut quad in part.quads {
            quad.sprite = intern(&mut merged.sprites, &part.sprites[quad.sprite]);
            merged.quads.push(quad);
        }
    }
    Ok(merged)
}

fn intern(sprites: &mut Vec<String>, sprite: &str) -> usize {
    sprites.iter().position(|s| s == sprite).unwrap_or_else(|| {
        sprites.push(sprite.to_owned());
        sprites.len() - 1
    })
}

fn bake_model(
    model: &ResolvedModel,
    rotation: VariantRotation,
    uvlock: bool,
) -> Result<BakedBlock, String> {
    let mut sprites: Vec<String> = Vec::new();
    let mut quads = Vec::new();
    for element in &model.elements {
        // Ordinal order, not JSON key order, so the vertex layout matches the client's.
        for dir in Dir::all() {
            let Some(face) = element.faces.get(dir.name()) else {
                continue;
            };
            if !draws_face(element, dir) {
                continue;
            }
            let sprite = intern(&mut sprites, model.sprite_of(face)?);
            quads.push(bake_face(element, dir, face, sprite, rotation, uvlock)?);
        }
    }
    Ok(BakedBlock {
        quads,
        sprites,
        ambient_occlusion: model.ambient_occlusion,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oak_log(props: &[(&str, &str)]) -> BakedBlock {
        bake(Pack::corpus(), "minecraft:oak_log", props).expect("oak_log bakes")
    }

    fn quad(baked: &BakedBlock, dir: Dir) -> &BakedQuad {
        baked
            .quads
            .iter()
            .find(|q| q.dir == dir)
            .unwrap_or_else(|| panic!("no {dir:?} quad"))
    }

    #[test]
    fn geometry_and_sprites() {
        let baked = oak_log(&[("axis", "y")]);
        assert_eq!(baked.quads.len(), 6);
        assert_eq!(baked.sprites.len(), 2);

        for dir in Dir::all() {
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
            assert!(
                normal.dot(dir.normal().as_vec3()) > 0.0,
                "winding of {dir:?}"
            );
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
    fn face_shade_follows_the_direction() {
        let baked = oak_log(&[("axis", "y")]);
        assert!(baked.ambient_occlusion);
        for (dir, expected) in [
            (Dir::Down, 0.5),
            (Dir::Up, 1.0),
            (Dir::North, 0.8),
            (Dir::South, 0.8),
            (Dir::West, 0.6),
            (Dir::East, 0.6),
        ] {
            assert_eq!(quad(&baked, dir).shade, expected, "face shade of {dir:?}");
        }
    }

    #[test]
    fn object_form_texture_slots_resolve() {
        let baked = bake(Pack::corpus(), "minecraft:glass", &[]).expect("glass bakes");
        assert_eq!(baked.sprites, ["minecraft:block/glass"]);
        assert_eq!(baked.quads.len(), 6);
    }

    #[test]
    fn variant_rotation_turns_the_log_on_its_side() {
        let baked = oak_log(&[("axis", "x")]);
        assert_eq!(baked.quads.len(), 6);
        let sprite_of = |dir| baked.sprites[quad(&baked, dir).sprite].as_str();
        // A log along X shows its rings on the two X faces and bark everywhere else.
        assert_eq!(sprite_of(Dir::West), "minecraft:block/oak_log_top");
        assert_eq!(sprite_of(Dir::East), "minecraft:block/oak_log_top");
        for dir in [Dir::Up, Dir::Down, Dir::North, Dir::South] {
            assert_eq!(sprite_of(dir), "minecraft:block/oak_log", "{dir:?}");
        }
        for dir in Dir::all() {
            let q = quad(&baked, dir);
            for i in 0..4 {
                assert_eq!(q.positions[i], corner(dir, i, Vec3::ZERO, Vec3::ONE));
            }
        }
    }
}
