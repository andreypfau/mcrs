use std::collections::BTreeMap;

use bevy_math::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::{BoundingBox, Direction};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::{Deserialize, Serialize};

use mcrs_minecraft_core::{Mirror, Rotation};

pub const TEMPLATE_DATA_VERSION: i32 = 5015;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub size: [i32; 3],
    pub entities: Vec<TemplateEntity>,
    pub blocks: Vec<TemplateBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<Vec<PaletteState>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palettes: Option<Vec<Vec<PaletteState>>>,
    #[serde(rename = "DataVersion")]
    pub data_version: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateBlock {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nbt: Option<NbtCompound>,
    pub pos: [i32; 3],
    pub state: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateEntity {
    pub nbt: NbtCompound,
    #[serde(rename = "blockPos")]
    pub block_pos: [i32; 3],
    pub pos: [f64; 3],
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteState {
    pub id: ResourceLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, String>>,
}

impl std::fmt::Display for PaletteState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)?;
        if let Some(properties) = &self.properties {
            f.write_str("[")?;
            for (i, (k, v)) in properties.iter().enumerate() {
                if i > 0 {
                    f.write_str(",")?;
                }
                write!(f, "{k}={v}")?;
            }
            f.write_str("]")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedState {
    pub id: VoxelId,
    pub full_block: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrozenBlock {
    pub pos: [u16; 3],
    pub state: VoxelId,
    pub nbt: Option<NbtCompound>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrozenTemplate {
    pub size: [u16; 3],
    pub palettes: Vec<Box<[FrozenBlock]>>,
}

impl FrozenTemplate {
    pub fn empty() -> Self {
        FrozenTemplate {
            size: [0; 3],
            palettes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Joint {
    Rollable,
    Aligned,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JigsawBlock {
    pub pos: [u16; 3],
    pub front: Direction,
    pub top: Direction,
    pub joint: Joint,
    pub name: ResourceLocation,
    pub pool: ResourceLocation,
    pub target: ResourceLocation,
    pub placement_priority: i32,
    pub selection_priority: i32,
    pub final_state: Option<VoxelId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateManifest {
    pub size: [u16; 3],
    pub jigsaws: Vec<Vec<JigsawBlock>>,
}

impl TemplateManifest {
    pub fn empty() -> Self {
        TemplateManifest {
            size: [0; 3],
            jigsaws: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TemplateError {
    #[error("{id}: DataVersion {found}, expected {TEMPLATE_DATA_VERSION}")]
    DataVersion { id: ResourceLocation, found: i32 },
    #[error("{id}: has both `palette` and `palettes`")]
    BothPalettes { id: ResourceLocation },
    #[error("{id}: has neither `palette` nor `palettes`")]
    NoPalette { id: ResourceLocation },
    #[error(
        "{id}: size {size:?} must be at least 1 on every axis and at most {}",
        u16::MAX
    )]
    Size {
        id: ResourceLocation,
        size: [i32; 3],
    },
    #[error("{id}: palette {palette} has {len} entries, palette 0 has {expected}")]
    PaletteLength {
        id: ResourceLocation,
        palette: usize,
        len: usize,
        expected: usize,
    },
    #[error("{id}: block {index} names palette entry {state} of {len}")]
    StateIndex {
        id: ResourceLocation,
        index: usize,
        state: i32,
        len: usize,
    },
    #[error("{id}: block {index} at {pos:?} lies outside size {size:?}")]
    OutOfBounds {
        id: ResourceLocation,
        index: usize,
        pos: [i32; 3],
        size: [u16; 3],
    },
    #[error("{id}: unknown block state {state}")]
    UnknownState { id: ResourceLocation, state: String },
    #[error("{id}: jigsaw at {pos:?}: {what}")]
    Jigsaw {
        id: ResourceLocation,
        pos: [u16; 3],
        what: String,
    },
}

const JIGSAW: &str = "minecraft:jigsaw";
const STRUCTURE_VOID: &str = "minecraft:structure_void";

fn orientation(name: &str) -> Option<(Direction, Direction)> {
    use Direction::*;
    Some(match name {
        "down_east" => (Down, East),
        "down_north" => (Down, North),
        "down_south" => (Down, South),
        "down_west" => (Down, West),
        "up_east" => (Up, East),
        "up_north" => (Up, North),
        "up_south" => (Up, South),
        "up_west" => (Up, West),
        "west_up" => (West, Up),
        "east_up" => (East, Up),
        "north_up" => (North, Up),
        "south_up" => (South, Up),
        _ => return None,
    })
}

/// `id[k=v,…]`; anything after the closing `]` is ignored, as the block-state
/// parser never asserts end of input.
fn parse_final_state(text: &str) -> Result<PaletteState, String> {
    let text = text.trim();
    let (id, rest) = match text.split_once('[') {
        Some((id, rest)) => (id, Some(rest)),
        None => (text, None),
    };
    let id = ResourceLocation::parse(id.trim()).map_err(|e| e.to_string())?;
    let Some(rest) = rest else {
        return Ok(PaletteState {
            id,
            properties: None,
        });
    };
    let Some((inner, _)) = rest.split_once(']') else {
        return Err(format!("`{text}` has no closing `]`"));
    };
    let mut properties = BTreeMap::new();
    for pair in inner.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let Some((key, value)) = pair.split_once('=') else {
            return Err(format!("`{text}`: `{pair}` is not `key=value`"));
        };
        if properties
            .insert(key.trim().to_owned(), value.trim().to_owned())
            .is_some()
        {
            return Err(format!("`{text}`: duplicate property `{}`", key.trim()));
        }
    }
    Ok(PaletteState {
        id,
        properties: Some(properties),
    })
}

fn int_or_zero(nbt: &NbtCompound, key: &str) -> Result<i32, String> {
    Ok(match nbt.get(key) {
        None => 0,
        Some(NbtTag::Byte(v)) => *v as i32,
        Some(NbtTag::Short(v)) => *v as i32,
        Some(NbtTag::Int(v)) => *v,
        Some(NbtTag::Long(v)) => *v as i32,
        Some(NbtTag::Float(v)) => *v as i32,
        Some(NbtTag::Double(v)) => *v as i32,
        Some(_) => return Err(format!("{key}: not a number")),
    })
}

fn id_or_empty(nbt: &NbtCompound, key: &str) -> Result<ResourceLocation, String> {
    match nbt.get_string(key) {
        Some(text) => ResourceLocation::parse(text).map_err(|e| format!("{key}: {e}")),
        None => Ok(ResourceLocation::minecraft("empty")),
    }
}

impl Template {
    pub fn freeze(
        &self,
        id: &ResourceLocation,
        resolve: &dyn Fn(&PaletteState) -> Option<ResolvedState>,
    ) -> Result<(FrozenTemplate, TemplateManifest), TemplateError> {
        let err_id = || id.clone();
        if self.data_version != TEMPLATE_DATA_VERSION {
            return Err(TemplateError::DataVersion {
                id: err_id(),
                found: self.data_version,
            });
        }
        let palettes: Vec<&[PaletteState]> = match (&self.palette, &self.palettes) {
            (Some(p), None) => vec![p.as_slice()],
            (None, Some(ps)) if !ps.is_empty() => ps.iter().map(Vec::as_slice).collect(),
            (None, _) => return Err(TemplateError::NoPalette { id: err_id() }),
            (Some(_), Some(_)) => return Err(TemplateError::BothPalettes { id: err_id() }),
        };
        let size: [u16; 3] = {
            let fits = |v: i32| (v >= 1 && v <= i32::from(u16::MAX)).then_some(v as u16);
            match self.size.map(fits) {
                [Some(x), Some(y), Some(z)] => [x, y, z],
                _ => {
                    return Err(TemplateError::Size {
                        id: err_id(),
                        size: self.size,
                    });
                }
            }
        };
        let expected = palettes[0].len();
        let mut resolved = Vec::with_capacity(palettes.len());
        for (index, palette) in palettes.iter().enumerate() {
            if palette.len() != expected {
                return Err(TemplateError::PaletteLength {
                    id: err_id(),
                    palette: index,
                    len: palette.len(),
                    expected,
                });
            }
            let states = palette
                .iter()
                .map(|state| {
                    resolve(state).ok_or_else(|| TemplateError::UnknownState {
                        id: err_id(),
                        state: state.to_string(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            resolved.push(states);
        }
        let mut blocks = Vec::with_capacity(self.blocks.len());
        for (index, block) in self.blocks.iter().enumerate() {
            let inside = |axis: usize| {
                let p = block.pos[axis];
                (p >= 0 && p < i32::from(size[axis])).then_some(p as u16)
            };
            let Some(pos) = inside(0).zip(inside(1)).zip(inside(2)) else {
                return Err(TemplateError::OutOfBounds {
                    id: err_id(),
                    index,
                    pos: block.pos,
                    size,
                });
            };
            let state = usize::try_from(block.state)
                .ok()
                .filter(|&s| s < expected)
                .ok_or_else(|| TemplateError::StateIndex {
                    id: err_id(),
                    index,
                    state: block.state,
                    len: expected,
                })?;
            blocks.push(([pos.0.0, pos.0.1, pos.1], state, block.nbt.as_ref()));
        }

        let mut frozen = Vec::with_capacity(palettes.len());
        let mut jigsaws = Vec::with_capacity(palettes.len());
        for (palette, states) in palettes.iter().zip(&resolved) {
            let mut full = Vec::new();
            let mut other = Vec::new();
            let mut with_nbt = Vec::new();
            for &(pos, state, nbt) in &blocks {
                let list = if nbt.is_some() {
                    &mut with_nbt
                } else if states[state].full_block {
                    &mut full
                } else {
                    &mut other
                };
                list.push((pos, state, nbt));
            }
            let mut ordered = Vec::with_capacity(blocks.len());
            for mut list in [full, other, with_nbt] {
                list.sort_by_key(|&(pos, ..)| (pos[1], pos[0], pos[2]));
                ordered.extend(list);
            }
            let mut palette_jigsaws = Vec::new();
            for &(pos, state, nbt) in &ordered {
                if palette[state].id.as_str() != JIGSAW {
                    continue;
                }
                let jigsaw = |what: String| TemplateError::Jigsaw {
                    id: err_id(),
                    pos,
                    what,
                };
                let Some(nbt) = nbt else {
                    return Err(jigsaw("missing nbt".into()));
                };
                let (front, top) = palette[state]
                    .properties
                    .as_ref()
                    .and_then(|p| p.get("orientation"))
                    .ok_or_else(|| jigsaw("missing orientation".into()))
                    .and_then(|name| {
                        orientation(name)
                            .ok_or_else(|| jigsaw(format!("unknown orientation `{name}`")))
                    })?;
                let joint = match nbt.get_string("joint") {
                    Some("rollable") => Joint::Rollable,
                    Some("aligned") => Joint::Aligned,
                    Some(other) => return Err(jigsaw(format!("unknown joint `{other}`"))),
                    None if front.is_vertical() => Joint::Rollable,
                    None => Joint::Aligned,
                };
                let final_state =
                    parse_final_state(nbt.get_string("final_state").unwrap_or("minecraft:air"))
                        .map_err(|e| jigsaw(format!("final_state {e}")))?;
                let final_state = if final_state.id.as_str() == STRUCTURE_VOID {
                    None
                } else {
                    Some(
                        resolve(&final_state)
                            .ok_or_else(|| jigsaw(format!("unknown final_state {final_state}")))?
                            .id,
                    )
                };
                palette_jigsaws.push(JigsawBlock {
                    pos,
                    front,
                    top,
                    joint,
                    name: id_or_empty(nbt, "name").map_err(&jigsaw)?,
                    pool: id_or_empty(nbt, "pool").map_err(&jigsaw)?,
                    target: id_or_empty(nbt, "target").map_err(&jigsaw)?,
                    placement_priority: int_or_zero(nbt, "placement_priority").map_err(&jigsaw)?,
                    selection_priority: int_or_zero(nbt, "selection_priority").map_err(&jigsaw)?,
                    final_state,
                });
            }
            frozen.push(
                ordered
                    .into_iter()
                    .map(|(pos, state, nbt)| FrozenBlock {
                        pos,
                        state: states[state].id,
                        nbt: nbt.cloned(),
                    })
                    .collect(),
            );
            jigsaws.push(palette_jigsaws);
        }
        Ok((
            FrozenTemplate {
                size,
                palettes: frozen,
            },
            TemplateManifest { size, jigsaws },
        ))
    }
}

pub fn transform(pos: IVec3, mirror: Mirror, rotation: Rotation, pivot: IVec3) -> IVec3 {
    let IVec3 { x, y, z } = match mirror {
        Mirror::None => pos,
        Mirror::LeftRight => IVec3::new(pos.x, pos.y, -pos.z),
        Mirror::FrontBack => IVec3::new(-pos.x, pos.y, pos.z),
    };
    let (px, pz) = (pivot.x, pivot.z);
    match rotation {
        Rotation::None => IVec3::new(x, y, z),
        Rotation::Counterclockwise90 => IVec3::new(px - pz + z, y, px + pz - x),
        Rotation::Clockwise90 => IVec3::new(px + pz - z, y, pz - px + x),
        Rotation::Clockwise180 => IVec3::new(px + px - x, y, pz + pz - z),
    }
}

pub fn bounding_box(
    size: [u16; 3],
    position: IVec3,
    rotation: Rotation,
    mirror: Mirror,
    pivot: IVec3,
) -> BoundingBox {
    let far = IVec3::new(
        i32::from(size[0]) - 1,
        i32::from(size[1]) - 1,
        i32::from(size[2]) - 1,
    );
    let a = transform(IVec3::ZERO, mirror, rotation, pivot);
    let b = transform(far, mirror, rotation, pivot);
    BoundingBox::from_corners(a.into(), b.into()).moved(position)
}

/// Where the template's `(0, 0, 0)` corner lands when a piece placed at
/// `zero_pos` is transformed in place, so its box minimum stays put.
pub fn zero_position_with_transform(
    zero_pos: IVec3,
    mirror: Mirror,
    rotation: Rotation,
    size_x: i32,
    size_z: i32,
) -> IVec3 {
    let (size_x, size_z) = (size_x - 1, size_z - 1);
    let mirror_dx = if mirror == Mirror::FrontBack {
        size_x
    } else {
        0
    };
    let mirror_dz = if mirror == Mirror::LeftRight {
        size_z
    } else {
        0
    };
    let (dx, dz) = match rotation {
        Rotation::Counterclockwise90 => (mirror_dz, size_x - mirror_dx),
        Rotation::Clockwise90 => (size_z - mirror_dz, mirror_dx),
        Rotation::Clockwise180 => (size_x - mirror_dx, size_z - mirror_dz),
        Rotation::None => (mirror_dx, mirror_dz),
    };
    zero_pos + IVec3::new(dx, 0, dz)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Projection {
    Rigid,
    TerrainMatching,
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_core::BlockPos;
    use mcrs_minecraft_nbt::nbt_compress::{from_gzip_bytes, read_gzip_compound_tag};
    use mcrs_minecraft_nbt::to_nbt_compound;
    use mcrs_minecraft_worldgen_testing::{assets_dir, nbt_files};
    use std::io::Cursor;

    fn canonical(compound: &NbtCompound) -> NbtCompound {
        let mut child_tags: Vec<_> = compound
            .child_tags
            .iter()
            .map(|(k, v)| (k.clone(), canonical_tag(v)))
            .collect();
        child_tags.sort_by(|a, b| a.0.cmp(&b.0));
        NbtCompound { child_tags }
    }

    fn canonical_tag(tag: &NbtTag) -> NbtTag {
        match tag {
            NbtTag::Compound(c) => NbtTag::Compound(canonical(c)),
            NbtTag::List(items) => NbtTag::List(items.iter().map(canonical_tag).collect()),
            other => other.clone(),
        }
    }

    #[test]
    fn every_template_round_trips_and_is_pinned() {
        let files = nbt_files(&assets_dir().join("minecraft/structure"));
        let (mut with_palettes, mut with_entities) = (0, 0);
        for path in &files {
            let bytes = std::fs::read(path).unwrap();
            let direct = read_gzip_compound_tag(Cursor::new(&bytes)).unwrap();
            let template: Template = from_gzip_bytes(Cursor::new(&bytes))
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(
                template.data_version,
                TEMPLATE_DATA_VERSION,
                "{}",
                path.display()
            );
            let back = to_nbt_compound(&template).unwrap();
            assert_eq!(
                canonical(&back),
                canonical(&direct),
                "{} does not round-trip",
                path.display()
            );
            with_palettes += usize::from(template.palettes.is_some());
            with_entities += usize::from(!template.entities.is_empty());
        }
        assert_eq!(files.len(), 1511);
        assert_eq!(with_palettes, 20);
        assert_eq!(with_entities, 172);
    }

    fn state(id: &str, properties: &[(&str, &str)]) -> PaletteState {
        PaletteState {
            id: ResourceLocation::parse(id).unwrap(),
            properties: (!properties.is_empty()).then(|| {
                properties
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect()
            }),
        }
    }

    fn block(pos: [i32; 3], state: i32, nbt: Option<NbtCompound>) -> TemplateBlock {
        TemplateBlock { nbt, pos, state }
    }

    fn jigsaw_nbt(entries: &[(&str, NbtTag)]) -> NbtCompound {
        let mut nbt = NbtCompound::new();
        for (k, v) in entries {
            nbt.put(k, v.clone());
        }
        nbt
    }

    fn s(text: &str) -> NbtTag {
        NbtTag::String(text.to_owned())
    }

    /// Ids are the palette id's length plus the property count, so every
    /// distinct state in a test maps to a distinct voxel; `_planks` is full.
    fn resolve(state: &PaletteState) -> Option<ResolvedState> {
        if state.id.path().starts_with("unknown") {
            return None;
        }
        let properties = state.properties.as_ref();
        if properties.is_some_and(|p| p.values().any(|v| v == "bogus")) {
            return None;
        }
        let id = state.id.as_str().len() + properties.map_or(0, BTreeMap::len);
        Some(ResolvedState {
            id: VoxelId(id as u16),
            full_block: state.id.path().ends_with("_planks"),
        })
    }

    fn template(palette: Vec<PaletteState>, blocks: Vec<TemplateBlock>) -> Template {
        Template {
            size: [2, 2, 2],
            entities: vec![],
            blocks,
            palette: Some(palette),
            palettes: None,
            data_version: TEMPLATE_DATA_VERSION,
        }
    }

    fn id() -> ResourceLocation {
        ResourceLocation::minecraft("test")
    }

    #[test]
    fn blocks_are_ordered_full_then_other_then_nbt_by_y_x_z() {
        let planks = state("minecraft:oak_planks", &[]);
        let torch = state("minecraft:torch", &[]);
        let chest = state("minecraft:chest", &[("facing", "north")]);
        let t = template(
            vec![planks, torch, chest],
            vec![
                block([1, 1, 1], 0, None),
                block([1, 0, 0], 2, Some(NbtCompound::new())),
                block([0, 0, 1], 1, None),
                block([1, 1, 0], 0, None),
                block([1, 0, 1], 0, None),
                block([0, 1, 0], 1, None),
                block([0, 1, 1], 0, None),
                block([0, 0, 0], 0, None),
            ],
        );
        let (frozen, manifest) = t.freeze(&id(), &resolve).unwrap();
        assert_eq!(frozen.size, [2, 2, 2]);
        assert_eq!(manifest.size, [2, 2, 2]);
        assert!(manifest.jigsaws == vec![vec![]]);
        let order: Vec<[u16; 3]> = frozen.palettes[0].iter().map(|b| b.pos).collect();
        assert_eq!(
            order,
            [
                [0, 0, 0],
                [1, 0, 1],
                [0, 1, 1],
                [1, 1, 0],
                [1, 1, 1],
                [0, 0, 1],
                [0, 1, 0],
                [1, 0, 0]
            ]
        );
        assert_eq!(frozen.palettes[0][7].nbt, Some(NbtCompound::new()));
        assert_eq!(
            frozen.palettes[0][0].state,
            resolve(&state("minecraft:oak_planks", &[])).unwrap().id
        );
    }

    #[test]
    fn palettes_are_ordered_independently() {
        let t = Template {
            palette: None,
            palettes: Some(vec![
                vec![
                    state("minecraft:oak_planks", &[]),
                    state("minecraft:torch", &[]),
                ],
                vec![
                    state("minecraft:torch", &[]),
                    state("minecraft:oak_planks", &[]),
                ],
            ]),
            ..template(
                vec![],
                vec![block([0, 0, 0], 1, None), block([1, 0, 0], 0, None)],
            )
        };
        let (frozen, _) = t.freeze(&id(), &resolve).unwrap();
        let order =
            |p: usize| -> Vec<[u16; 3]> { frozen.palettes[p].iter().map(|b| b.pos).collect() };
        assert_eq!(order(0), [[1, 0, 0], [0, 0, 0]]);
        assert_eq!(order(1), [[0, 0, 0], [1, 0, 0]]);
    }

    #[test]
    fn jigsaw_defaults() {
        let t = template(
            vec![
                state("minecraft:jigsaw", &[("orientation", "north_up")]),
                state("minecraft:jigsaw", &[("orientation", "up_east")]),
            ],
            vec![
                block(
                    [0, 1, 0],
                    1,
                    Some(jigsaw_nbt(&[(
                        "final_state",
                        s("minecraft:structure_void"),
                    )])),
                ),
                block([0, 0, 0], 0, Some(jigsaw_nbt(&[]))),
                block(
                    [1, 0, 0],
                    0,
                    Some(jigsaw_nbt(&[
                        ("joint", s("rollable")),
                        ("name", s("minecraft:a")),
                        ("pool", s("minecraft:b")),
                        ("target", s("minecraft:c")),
                        ("placement_priority", NbtTag::Short(2)),
                        ("selection_priority", NbtTag::Long(3)),
                        (
                            "final_state",
                            s("minecraft:acacia_fence[east=false,north=false]]"),
                        ),
                    ])),
                ),
            ],
        );
        let (_, manifest) = t.freeze(&id(), &resolve).unwrap();
        let jigsaws = &manifest.jigsaws[0];
        assert_eq!(jigsaws.len(), 3);
        let empty = ResourceLocation::minecraft("empty");
        assert_eq!(
            jigsaws[0],
            JigsawBlock {
                pos: [0, 0, 0],
                front: Direction::North,
                top: Direction::Up,
                joint: Joint::Aligned,
                name: empty.clone(),
                pool: empty.clone(),
                target: empty.clone(),
                placement_priority: 0,
                selection_priority: 0,
                final_state: Some(resolve(&state("minecraft:air", &[])).unwrap().id),
            }
        );
        assert_eq!(
            jigsaws[1],
            JigsawBlock {
                pos: [1, 0, 0],
                front: Direction::North,
                top: Direction::Up,
                joint: Joint::Rollable,
                name: ResourceLocation::minecraft("a"),
                pool: ResourceLocation::minecraft("b"),
                target: ResourceLocation::minecraft("c"),
                placement_priority: 2,
                selection_priority: 3,
                final_state: Some(
                    resolve(&state(
                        "minecraft:acacia_fence",
                        &[("east", "false"), ("north", "false")]
                    ))
                    .unwrap()
                    .id
                ),
            }
        );
        assert_eq!(jigsaws[2].pos, [0, 1, 0]);
        assert_eq!(
            (jigsaws[2].front, jigsaws[2].top),
            (Direction::Up, Direction::East)
        );
        assert_eq!(jigsaws[2].joint, Joint::Rollable);
        assert_eq!(jigsaws[2].final_state, None);
    }

    #[test]
    fn each_error_is_named() {
        let planks = state("minecraft:oak_planks", &[]);
        let ok = template(vec![planks.clone()], vec![block([0, 0, 0], 0, None)]);
        let freeze = |t: &Template| t.freeze(&id(), &resolve).unwrap_err();

        let t = Template {
            data_version: TEMPLATE_DATA_VERSION + 1,
            ..ok.clone()
        };
        assert_eq!(
            freeze(&t),
            TemplateError::DataVersion {
                id: id(),
                found: TEMPLATE_DATA_VERSION + 1
            }
        );

        let t = Template {
            palettes: Some(vec![vec![planks.clone()]]),
            ..ok.clone()
        };
        assert_eq!(freeze(&t), TemplateError::BothPalettes { id: id() });

        let t = Template {
            palette: None,
            ..ok.clone()
        };
        assert_eq!(freeze(&t), TemplateError::NoPalette { id: id() });
        let t = Template {
            palette: None,
            palettes: Some(vec![]),
            ..ok.clone()
        };
        assert_eq!(freeze(&t), TemplateError::NoPalette { id: id() });

        let t = Template {
            size: [2, 0, 2],
            ..ok.clone()
        };
        assert_eq!(
            freeze(&t),
            TemplateError::Size {
                id: id(),
                size: [2, 0, 2]
            }
        );
        let t = Template {
            size: [2, 2, 70000],
            ..ok.clone()
        };
        assert_eq!(
            freeze(&t),
            TemplateError::Size {
                id: id(),
                size: [2, 2, 70000]
            }
        );

        let t = Template {
            palette: None,
            palettes: Some(vec![vec![planks.clone()], vec![]]),
            ..ok.clone()
        };
        assert_eq!(
            freeze(&t),
            TemplateError::PaletteLength {
                id: id(),
                palette: 1,
                len: 0,
                expected: 1
            }
        );

        let t = Template {
            blocks: vec![block([0, 0, 0], 1, None)],
            ..ok.clone()
        };
        assert_eq!(
            freeze(&t),
            TemplateError::StateIndex {
                id: id(),
                index: 0,
                state: 1,
                len: 1
            }
        );
        let t = Template {
            blocks: vec![block([0, 0, 0], -1, None)],
            ..ok.clone()
        };
        assert_eq!(
            freeze(&t),
            TemplateError::StateIndex {
                id: id(),
                index: 0,
                state: -1,
                len: 1
            }
        );

        let t = Template {
            blocks: vec![block([0, 2, 0], 0, None)],
            ..ok.clone()
        };
        assert_eq!(
            freeze(&t),
            TemplateError::OutOfBounds {
                id: id(),
                index: 0,
                pos: [0, 2, 0],
                size: [2, 2, 2]
            }
        );

        let t = template(
            vec![state("minecraft:unknown_block", &[("lit", "true")])],
            vec![block([0, 0, 0], 0, None)],
        );
        assert_eq!(
            freeze(&t),
            TemplateError::UnknownState {
                id: id(),
                state: "minecraft:unknown_block[lit=true]".into()
            }
        );

        let jigsaw = |nbt: Option<NbtCompound>, orientation: &[(&str, &str)]| {
            template(
                vec![state("minecraft:jigsaw", orientation)],
                vec![block([0, 0, 0], 0, nbt)],
            )
        };
        let what = |t: &Template| match freeze(t) {
            TemplateError::Jigsaw { pos, what, .. } => {
                assert_eq!(pos, [0, 0, 0]);
                what
            }
            other => panic!("{other}"),
        };
        assert_eq!(
            what(&jigsaw(None, &[("orientation", "north_up")])),
            "missing nbt"
        );
        assert_eq!(
            what(&jigsaw(Some(jigsaw_nbt(&[])), &[])),
            "missing orientation"
        );
        assert_eq!(
            what(&jigsaw(
                Some(jigsaw_nbt(&[])),
                &[("orientation", "sideways")]
            )),
            "unknown orientation `sideways`"
        );
        assert_eq!(
            what(&jigsaw(
                Some(jigsaw_nbt(&[("joint", s("loose"))])),
                &[("orientation", "north_up")]
            )),
            "unknown joint `loose`"
        );
        assert!(
            what(&jigsaw(
                Some(jigsaw_nbt(&[("final_state", s("minecraft:stone[lit"))])),
                &[("orientation", "north_up")]
            ))
            .starts_with("final_state ")
        );
        assert_eq!(
            what(&jigsaw(
                Some(jigsaw_nbt(&[(
                    "final_state",
                    s("minecraft:stone[lit=bogus]")
                )])),
                &[("orientation", "north_up")]
            )),
            "unknown final_state minecraft:stone[lit=bogus]"
        );
        assert_eq!(
            what(&jigsaw(
                Some(jigsaw_nbt(&[("pool", s("nocolon"))])),
                &[("orientation", "north_up")]
            )),
            "pool: missing ':' separator in ResourceLocation: \"nocolon\""
        );
        assert_eq!(
            what(&jigsaw(
                Some(jigsaw_nbt(&[("selection_priority", s("7"))])),
                &[("orientation", "north_up")]
            )),
            "selection_priority: not a number"
        );
    }

    #[test]
    fn final_state_grammar() {
        let parsed = parse_final_state(" minecraft:stone [ a = 1 , b = two ] ] trailing").unwrap();
        assert_eq!(
            parsed,
            state("minecraft:stone", &[("a", "1"), ("b", "two")])
        );
        assert_eq!(
            parse_final_state("minecraft:stone[]").unwrap(),
            PaletteState {
                id: ResourceLocation::minecraft("stone"),
                properties: Some(BTreeMap::new())
            }
        );
        assert!(parse_final_state("minecraft:stone[a=1,a=2]").is_err());
        assert!(parse_final_state("minecraft:stone[a]").is_err());
        assert!(parse_final_state("stone").is_err());
    }

    #[test]
    fn transform_follows_the_rotation_formulas() {
        let pos = IVec3::new(3, 5, 7);
        let pivot = IVec3::new(2, 0, 4);
        let at = |mirror, rotation| transform(pos, mirror, rotation, pivot);
        assert_eq!(at(Mirror::None, Rotation::None), pos);
        assert_eq!(
            at(Mirror::None, Rotation::Counterclockwise90),
            IVec3::new(2 - 4 + 7, 5, 2 + 4 - 3)
        );
        assert_eq!(
            at(Mirror::None, Rotation::Clockwise90),
            IVec3::new(2 + 4 - 7, 5, 4 - 2 + 3)
        );
        assert_eq!(
            at(Mirror::None, Rotation::Clockwise180),
            IVec3::new(4 - 3, 5, 8 - 7)
        );
    }

    #[test]
    fn transform_mirrors_before_it_rotates() {
        let pos = IVec3::new(3, 5, 7);
        let pivot = IVec3::new(2, 0, 4);
        let at = |mirror, rotation| transform(pos, mirror, rotation, pivot);
        assert_eq!(at(Mirror::LeftRight, Rotation::None), IVec3::new(3, 5, -7));
        assert_eq!(at(Mirror::FrontBack, Rotation::None), IVec3::new(-3, 5, 7));
        assert_eq!(
            at(Mirror::LeftRight, Rotation::Clockwise90),
            IVec3::new(2 + 4 + 7, 5, 4 - 2 + 3)
        );
        assert_eq!(
            at(Mirror::FrontBack, Rotation::Counterclockwise90),
            IVec3::new(2 - 4 + 7, 5, 2 + 4 + 3)
        );
        assert_eq!(
            at(Mirror::FrontBack, Rotation::Clockwise180),
            IVec3::new(4 + 3, 5, 8 - 7)
        );
    }

    #[test]
    fn bounding_box_covers_the_rotated_footprint() {
        let at = IVec3::new(10, 20, 30);
        let boxed = |r| bounding_box([3, 4, 5], at, r, Mirror::None, IVec3::ZERO);
        assert_eq!(
            boxed(Rotation::None),
            BoundingBox {
                min: (at).into(),
                max: (at + IVec3::new(2, 3, 4)).into()
            }
        );
        assert_eq!(
            boxed(Rotation::Clockwise90),
            BoundingBox {
                min: (at + IVec3::new(-4, 0, 0)).into(),
                max: (at + IVec3::new(0, 3, 2)).into()
            }
        );
        assert_eq!(
            boxed(Rotation::Counterclockwise90),
            BoundingBox {
                min: (at + IVec3::new(0, 0, -2)).into(),
                max: (at + IVec3::new(4, 3, 0)).into()
            }
        );
        assert_eq!(
            boxed(Rotation::Clockwise180),
            BoundingBox {
                min: (at + IVec3::new(-2, 0, -4)).into(),
                max: (at + IVec3::new(0, 3, 0)).into()
            }
        );
    }

    #[test]
    fn bounding_box_takes_the_mirror_and_the_pivot_into_account() {
        let at = IVec3::new(10, 20, 30);
        let pivot = IVec3::new(1, 0, 2);
        assert_eq!(
            bounding_box(
                [3, 4, 5],
                at,
                Rotation::Clockwise90,
                Mirror::FrontBack,
                pivot
            ),
            BoundingBox {
                min: (at + IVec3::new(-1, 0, -1)).into(),
                max: (at + IVec3::new(3, 3, 1)).into()
            }
        );
        assert_eq!(
            bounding_box(
                [3, 4, 5],
                at,
                Rotation::None,
                Mirror::LeftRight,
                IVec3::ZERO
            ),
            BoundingBox {
                min: (at + IVec3::new(0, 0, -4)).into(),
                max: (at + IVec3::new(2, 3, 0)).into()
            }
        );
    }

    #[test]
    fn the_zero_position_keeps_the_box_minimum_where_the_piece_was() {
        let zero = IVec3::new(-5, 3, 11);
        let (size_x, size_z) = (3, 5);
        let shift = |m, r| zero_position_with_transform(zero, m, r, size_x, size_z) - zero;
        assert_eq!(shift(Mirror::None, Rotation::None), IVec3::ZERO);
        assert_eq!(
            shift(Mirror::FrontBack, Rotation::None),
            IVec3::new(2, 0, 0)
        );
        assert_eq!(
            shift(Mirror::LeftRight, Rotation::None),
            IVec3::new(0, 0, 4)
        );
        assert_eq!(
            shift(Mirror::None, Rotation::Clockwise90),
            IVec3::new(4, 0, 0)
        );
        assert_eq!(shift(Mirror::LeftRight, Rotation::Clockwise90), IVec3::ZERO);
        assert_eq!(
            shift(Mirror::None, Rotation::Counterclockwise90),
            IVec3::new(0, 0, 2)
        );
        assert_eq!(
            shift(Mirror::FrontBack, Rotation::Counterclockwise90),
            IVec3::ZERO
        );
        assert_eq!(
            shift(Mirror::None, Rotation::Clockwise180),
            IVec3::new(2, 0, 4)
        );
        assert_eq!(
            shift(Mirror::LeftRight, Rotation::Clockwise180),
            IVec3::new(2, 0, 0)
        );
        for mirror in Mirror::ALL {
            for rotation in Rotation::ALL {
                let position = zero_position_with_transform(zero, mirror, rotation, size_x, size_z);
                let bounds = bounding_box([3, 4, 5], position, rotation, mirror, IVec3::ZERO);
                assert_eq!(bounds.min, BlockPos::from(zero), "{mirror:?} {rotation:?}");
            }
        }
    }
    #[test]
    fn box_arithmetic_is_inclusive() {
        let a = BoundingBox::from_corners(BlockPos::new(0, 0, 0), BlockPos::new(4, 2, 4));
        assert_eq!(a.y_span(), 3);
        assert_eq!(a.moved(IVec3::new(1, -1, 0)).min, BlockPos::new(1, -1, 0));
        assert_eq!(a.inflated(12).max, BlockPos::new(16, 14, 16));
        assert!(a.intersects(BoundingBox::from_corners(
            BlockPos::new(4, 2, 4),
            IVec3::splat(9).into()
        )));
        assert!(!a.intersects(BoundingBox::from_corners(
            BlockPos::new(5, 0, 0),
            IVec3::splat(9).into()
        )));
    }

    #[test]
    fn a_rotation_turns_the_horizontal_faces_and_keeps_the_vertical_ones() {
        assert_eq!(
            Rotation::Clockwise90.rotate(Direction::North),
            Direction::East
        );
        assert_eq!(
            Rotation::Clockwise180.rotate(Direction::North),
            Direction::South
        );
        assert_eq!(
            Rotation::Counterclockwise90.rotate(Direction::North),
            Direction::West
        );
        assert_eq!(Rotation::None.rotate(Direction::West), Direction::West);
        assert_eq!(Rotation::Clockwise90.rotate(Direction::Up), Direction::Up);
        assert_eq!(
            Rotation::ALL[Rotation::ALL.len() - 1],
            Rotation::Counterclockwise90
        );
    }
}
