use bevy::math::{Affine3A, Mat3, Vec2, Vec3};
use serde::Deserialize;

const MAX_BONES: usize = 128;
const MAX_DEPTH: usize = 16;
const MAX_CUBES: usize = 2048;
const OLDEST_CURRENT_FORMAT: [u32; 3] = [1, 12, 0];
const DEFAULT_TEXTURE_SIZE: f32 = 16.0;

/// One face of a baked geometry in internal space, in the rest pose of its bone.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GeometryFace {
    /// The corners `c0`, `c1`, `c2` and `c3` of the face.
    pub corners: [Vec3; 4],
    /// The texture coordinates of the corners, normalised by the texture size.
    pub uvs: [Vec2; 4],
    pub normal: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BakedGeometry {
    pub identifier: String,
    pub texture_size: Vec2,
    pub faces: Vec<GeometryFace>,
}

#[derive(Deserialize)]
struct GeometryFile {
    format_version: String,
    #[serde(rename = "minecraft:geometry")]
    geometries: Vec<GeometryEntry>,
}

#[derive(Deserialize)]
struct GeometryEntry {
    description: Description,
    #[serde(default)]
    bones: Vec<BoneEntry>,
}

#[derive(Deserialize)]
struct Description {
    identifier: String,
    texture_width: Option<f32>,
    texture_height: Option<f32>,
}

#[derive(Deserialize)]
struct BoneEntry {
    name: String,
    parent: Option<String>,
    #[serde(default)]
    pivot: [f32; 3],
    #[serde(default)]
    rotation: [f32; 3],
    #[serde(default)]
    mirror: bool,
    #[serde(default)]
    inflate: f32,
    #[serde(default)]
    cubes: Vec<CubeEntry>,
}

#[derive(Deserialize)]
struct CubeEntry {
    origin: [f32; 3],
    size: [f32; 3],
    #[serde(default)]
    rotation: [f32; 3],
    pivot: Option<[f32; 3]>,
    inflate: Option<f32>,
    mirror: Option<bool>,
    #[serde(default)]
    uv: CubeUv,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CubeUv {
    Box([f32; 2]),
    ByFace(Box<PerFaceUv>),
}

impl Default for CubeUv {
    fn default() -> Self {
        CubeUv::Box([0.0, 0.0])
    }
}

#[derive(Deserialize)]
struct PerFaceUv {
    north: Option<FaceUv>,
    south: Option<FaceUv>,
    east: Option<FaceUv>,
    west: Option<FaceUv>,
    up: Option<FaceUv>,
    down: Option<FaceUv>,
}

#[derive(Deserialize)]
struct FaceUv {
    uv: [f32; 2],
    uv_size: Option<[f32; 2]>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FaceName {
    East,
    North,
    West,
    South,
    Up,
    Down,
}

impl FaceName {
    const ALL: [FaceName; 6] = [
        FaceName::East,
        FaceName::North,
        FaceName::West,
        FaceName::South,
        FaceName::Up,
        FaceName::Down,
    ];

    fn normal(self) -> Vec3 {
        match self {
            FaceName::East => Vec3::X,
            FaceName::West => Vec3::NEG_X,
            FaceName::Up => Vec3::Y,
            FaceName::Down => Vec3::NEG_Y,
            FaceName::South => Vec3::Z,
            FaceName::North => Vec3::NEG_Z,
        }
    }

    fn corners(self, lo: Vec3, hi: Vec3) -> [Vec3; 4] {
        let at = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
        match self {
            FaceName::East => [
                at(hi.x, hi.y, hi.z),
                at(hi.x, hi.y, lo.z),
                at(hi.x, lo.y, hi.z),
                at(hi.x, lo.y, lo.z),
            ],
            FaceName::West => [
                at(lo.x, hi.y, lo.z),
                at(lo.x, hi.y, hi.z),
                at(lo.x, lo.y, lo.z),
                at(lo.x, lo.y, hi.z),
            ],
            FaceName::Up => [
                at(lo.x, hi.y, lo.z),
                at(hi.x, hi.y, lo.z),
                at(lo.x, hi.y, hi.z),
                at(hi.x, hi.y, hi.z),
            ],
            FaceName::Down => [
                at(lo.x, lo.y, hi.z),
                at(hi.x, lo.y, hi.z),
                at(lo.x, lo.y, lo.z),
                at(hi.x, lo.y, lo.z),
            ],
            FaceName::South => [
                at(lo.x, hi.y, hi.z),
                at(hi.x, hi.y, hi.z),
                at(lo.x, lo.y, hi.z),
                at(hi.x, lo.y, hi.z),
            ],
            FaceName::North => [
                at(hi.x, hi.y, lo.z),
                at(lo.x, hi.y, lo.z),
                at(hi.x, lo.y, lo.z),
                at(lo.x, lo.y, lo.z),
            ],
        }
    }

    fn per_face(self, uv: &PerFaceUv) -> Option<&FaceUv> {
        match self {
            FaceName::East => uv.east.as_ref(),
            FaceName::North => uv.north.as_ref(),
            FaceName::West => uv.west.as_ref(),
            FaceName::South => uv.south.as_ref(),
            FaceName::Up => uv.up.as_ref(),
            FaceName::Down => uv.down.as_ref(),
        }
    }
}

/// A texture rectangle `[u1, v1, u2, v2]` in texels.
#[derive(Clone, Copy)]
struct Rect {
    u1: f32,
    v1: f32,
    u2: f32,
    v2: f32,
}

fn box_rect(face: FaceName, origin: [f32; 2], size: Vec3, mirror: bool) -> Rect {
    let [u, v] = origin;
    let (w, h, d) = (size.x.floor(), size.y.floor(), size.z.floor());
    let rect = |u1, v1, u2, v2| Rect { u1, v1, u2, v2 };
    let face = match (mirror, face) {
        (true, FaceName::East) => FaceName::West,
        (true, FaceName::West) => FaceName::East,
        (_, face) => face,
    };
    let picked = match face {
        FaceName::East => rect(u, v + d, u + d, v + d + h),
        FaceName::North => rect(u + d, v + d, u + d + w, v + d + h),
        FaceName::West => rect(u + d + w, v + d, u + 2.0 * d + w, v + d + h),
        FaceName::South => rect(u + 2.0 * d + w, v + d, u + 2.0 * d + 2.0 * w, v + d + h),
        FaceName::Up => rect(u + d + w, v + d, u + d, v),
        FaceName::Down => rect(u + d + 2.0 * w, v, u + d + w, v + d),
    };
    if mirror {
        Rect {
            u1: picked.u2,
            u2: picked.u1,
            ..picked
        }
    } else {
        picked
    }
}

fn per_face_rect(face: FaceName, uv: &FaceUv, size: Vec3) -> Rect {
    let [x, y] = uv.uv;
    let [w, h] = uv.uv_size.unwrap_or(match face {
        FaceName::North | FaceName::South => [size.x, size.y],
        FaceName::East | FaceName::West => [size.z, size.y],
        FaceName::Up | FaceName::Down => [size.x, size.z],
    });
    match face {
        FaceName::Up | FaceName::Down => Rect {
            u1: x + w,
            v1: y + h,
            u2: x,
            v2: y,
        },
        _ => Rect {
            u1: x,
            v1: y,
            u2: x + w,
            v2: y + h,
        },
    }
}

fn euler(rotation_degrees: Vec3) -> Mat3 {
    Mat3::from_rotation_z(rotation_degrees.z.to_radians())
        * Mat3::from_rotation_y(rotation_degrees.y.to_radians())
        * Mat3::from_rotation_x(rotation_degrees.x.to_radians())
}

fn internal_point([x, y, z]: [f32; 3]) -> Vec3 {
    Vec3::new(-x, y, z)
}

fn internal_rotation([x, y, z]: [f32; 3]) -> Vec3 {
    Vec3::new(-x, -y, z)
}

fn about(pivot: Vec3, rotation: Mat3) -> Affine3A {
    Affine3A::from_translation(pivot)
        * Affine3A::from_mat3(rotation)
        * Affine3A::from_translation(-pivot)
}

fn is_current_format(version: &str) -> bool {
    let parts: Vec<u32> = version
        .split('.')
        .map_while(|part| part.parse().ok())
        .collect();
    parts.len() == version.split('.').count()
        && parts.as_slice() >= OLDEST_CURRENT_FORMAT.as_slice()
}

/// The geometries of one geometry file, each baked in its rest pose.
pub(crate) fn read_geometry_file(bytes: &[u8]) -> Result<Vec<BakedGeometry>, String> {
    let file: GeometryFile = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if !is_current_format(&file.format_version) {
        return Err(format!(
            "format_version `{}` is not supported",
            file.format_version
        ));
    }
    file.geometries.iter().map(bake_geometry).collect()
}

fn bake_geometry(entry: &GeometryEntry) -> Result<BakedGeometry, String> {
    let identifier = &entry.description.identifier;
    let bones = &entry.bones;
    if bones.is_empty() || bones.len() > MAX_BONES {
        return Err(format!("{identifier} has {} bones", bones.len()));
    }
    let cube_count: usize = bones.iter().map(|bone| bone.cubes.len()).sum();
    if cube_count > MAX_CUBES {
        return Err(format!("{identifier} has {cube_count} cubes"));
    }
    let parents = parent_indices(identifier, bones)?;
    let texture_size = Vec2::new(
        entry
            .description
            .texture_width
            .unwrap_or(DEFAULT_TEXTURE_SIZE),
        entry
            .description
            .texture_height
            .unwrap_or(DEFAULT_TEXTURE_SIZE),
    );
    let mut faces = Vec::new();
    for (index, bone) in bones.iter().enumerate() {
        let transform = model_transform(identifier, index, bones, &parents)?;
        for cube in &bone.cubes {
            bake_cube(bone, cube, transform, texture_size, &mut faces);
        }
    }
    Ok(BakedGeometry {
        identifier: identifier.clone(),
        texture_size,
        faces,
    })
}

fn parent_indices(identifier: &str, bones: &[BoneEntry]) -> Result<Vec<Option<usize>>, String> {
    let find = |name: &str| {
        bones
            .iter()
            .position(|bone| bone.name.eq_ignore_ascii_case(name))
    };
    let mut parents = Vec::with_capacity(bones.len());
    for (index, bone) in bones.iter().enumerate() {
        if find(&bone.name) != Some(index) {
            return Err(format!("{identifier} names the bone `{}` twice", bone.name));
        }
        let parent = match &bone.parent {
            None => None,
            Some(name) => Some(find(name).ok_or_else(|| {
                format!(
                    "{identifier}: the bone `{}` names the missing parent `{name}`",
                    bone.name
                )
            })?),
        };
        parents.push(parent);
    }
    Ok(parents)
}

fn model_transform(
    identifier: &str,
    index: usize,
    bones: &[BoneEntry],
    parents: &[Option<usize>],
) -> Result<Affine3A, String> {
    let mut chain = vec![index];
    let mut at = index;
    while let Some(Some(parent)) = parents.get(at) {
        if chain.len() >= MAX_DEPTH || chain.contains(parent) {
            return Err(format!(
                "{identifier}: the parent chain of a bone is too deep or has a cycle"
            ));
        }
        chain.push(*parent);
        at = *parent;
    }
    let mut transform = Affine3A::IDENTITY;
    for &link in chain.iter().rev() {
        if let Some(bone) = bones.get(link) {
            let pivot = internal_point(bone.pivot);
            let rotation = euler(internal_rotation(bone.rotation));
            transform *= about(pivot, rotation);
        }
    }
    Ok(transform)
}

fn bake_cube(
    bone: &BoneEntry,
    cube: &CubeEntry,
    transform: Affine3A,
    texture_size: Vec2,
    faces: &mut Vec<GeometryFace>,
) {
    let [ox, oy, oz] = cube.origin;
    let size = Vec3::from(cube.size).max(Vec3::ZERO);
    let inflate = Vec3::splat(cube.inflate.unwrap_or(bone.inflate));
    let lo = Vec3::new(-(ox + size.x), oy, oz) - inflate;
    let hi = Vec3::new(-ox, oy + size.y, oz + size.z) + inflate;
    let pivot = cube.pivot.map_or((lo + hi) / 2.0, internal_point);
    let local = about(pivot, euler(internal_rotation(cube.rotation)));
    let placed = transform * local;
    let normals = Mat3::from(placed.matrix3).inverse().transpose();
    let mirror = cube.mirror.unwrap_or(bone.mirror);
    for face in FaceName::ALL {
        let rect = match &cube.uv {
            CubeUv::Box(origin) => box_rect(face, *origin, size, mirror),
            CubeUv::ByFace(per_face) => match face.per_face(per_face) {
                Some(uv) => per_face_rect(face, uv, size),
                None => continue,
            },
        };
        let texels = [
            Vec2::new(rect.u1, rect.v1),
            Vec2::new(rect.u2, rect.v1),
            Vec2::new(rect.u1, rect.v2),
            Vec2::new(rect.u2, rect.v2),
        ];
        faces.push(GeometryFace {
            corners: face
                .corners(lo, hi)
                .map(|corner| placed.transform_point3(corner)),
            uvs: texels.map(|texel| texel / texture_size),
            normal: (normals * face.normal()).normalize_or_zero(),
        });
    }
}
