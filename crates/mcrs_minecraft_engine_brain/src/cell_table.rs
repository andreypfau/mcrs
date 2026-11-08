use std::collections::BTreeMap;

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::block_tag_keys;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::BlockStateFlags;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_registries::Tags;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::TagId;
use mcrs_minecraft_resource::TagKey;
use mcrs_voxel::BlockStateId;

use crate::PathType;

/// What the goals and the controls of a mob read of each block state: the render and solid
/// flags, the fluid, the state path type and the collision shape.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CellTable {
    states: Vec<StateCell>,
    boxes: Vec<CellBox>,
}

/// The data of one block state in a [`CellTable`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StateCell {
    pub flags: StateFlags,
    /// The state path type of PATH-TYP-002.
    pub path: PathType,
    pub collision: Collision,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct StateFlags(u8);

/// The collision shape of a block state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Collision {
    Empty,
    FullCube,
    /// The boxes `start..end` of the table.
    Boxes {
        start: u32,
        end: u32,
    },
}

/// A box of a collision shape, relative to its block position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellBox {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl StateFlags {
    pub const NONE: Self = Self(0);
    /// The state has a solid render state: a floor that a mob stands on (GOAL-POS-002).
    pub const SOLID_RENDER: Self = Self(1);
    pub const LEGACY_SOLID: Self = Self(1 << 1);
    pub const WATER: Self = Self(1 << 2);
    pub const GRASS_BLOCK: Self = Self(1 << 3);
    /// The block is in the tag `doors` or `fences`: the move control ignores its top.
    pub const DOOR_OR_FENCE: Self = Self(1 << 4);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl StateCell {
    pub const AIR: Self = Self {
        flags: StateFlags::NONE,
        path: PathType::Open,
        collision: Collision::Empty,
    };

    /// A block of a section that is not loaded: no mob walks, stands or sees through it.
    pub const UNLOADED: Self = Self {
        flags: StateFlags::LEGACY_SOLID,
        path: PathType::Blocked,
        collision: Collision::FullCube,
    };
}

/// The state path types that a block tag gives, in the order of PATH-TYP-002, step 1.
const PATH_TAGS: [(TagKey<'static, Block>, PathType); 9] = [
    (block_tag_keys::PATHFINDING_OPEN, PathType::Open),
    (block_tag_keys::PATHFINDING_DROP_DOWN, PathType::DropDown),
    (
        block_tag_keys::PATHFINDING_POWDER_SNOW,
        PathType::PowderSnow,
    ),
    (block_tag_keys::PATHFINDING_RAILS, PathType::Rail),
    (block_tag_keys::PATHFINDING_DAMAGING, PathType::Damaging),
    (block_tag_keys::PATHFINDING_LEAVES, PathType::Leaves),
    (block_tag_keys::PATHFINDING_STICKY, PathType::Sticky),
    (
        block_tag_keys::PATHFINDING_AVOID_IN_AIR,
        PathType::AvoidInAir,
    ),
    (
        block_tag_keys::PATHFINDING_DAMAGE_CAUTIOUS,
        PathType::DamageCautious,
    ),
];

const FIRE_BLOCKS: [&str; 3] = [
    "minecraft:lava",
    "minecraft:magma_block",
    "minecraft:lava_cauldron",
];

impl CellTable {
    /// A table of `states` in the order of the state ids, whose [`Collision::Boxes`] index
    /// `boxes`.
    #[must_use]
    pub fn new(states: Vec<StateCell>, boxes: Vec<CellBox>) -> Self {
        Self { states, boxes }
    }

    /// The cells of every state of `definitions`, with the block tags `tags`.
    ///
    /// A state is pathfindable on land when its collision shape is empty, since the corpus does
    /// not hold the navigation component of PATH section 12.3 yet.
    #[must_use]
    pub fn from_definitions(definitions: &BlockDefinitions, tags: &Tags<Block>) -> Self {
        let kinds = BlockKinds::new(definitions, tags);
        let mut shapes = BTreeMap::new();
        let mut boxes = Vec::new();
        let mut states = Vec::with_capacity(definitions.state_count());
        for id in definitions.states() {
            let data = definitions.state(id);
            let shape = definitions.shape(data.collision_shape);
            let collision = if shape.is_empty() {
                Collision::Empty
            } else if data
                .flags
                .contains(BlockStateFlags::IS_COLLISION_SHAPE_FULL_BLOCK)
            {
                Collision::FullCube
            } else {
                *shapes.entry(data.collision_shape.0).or_insert_with(|| {
                    let start = u32::try_from(boxes.len()).unwrap_or(u32::MAX);
                    boxes.extend(shape.iter().map(|aabb| CellBox {
                        min: aabb.min.as_dvec3().to_array(),
                        max: aabb.max.as_dvec3().to_array(),
                    }));
                    Collision::Boxes {
                        start,
                        end: u32::try_from(boxes.len()).unwrap_or(u32::MAX),
                    }
                })
            };
            states.push(kinds.cell(definitions, id, collision));
        }
        Self { states, boxes }
    }

    /// The cell of `state`; a state out of the table is air.
    #[must_use]
    pub fn state(&self, state: BlockStateId) -> StateCell {
        self.states
            .get(state.index())
            .copied()
            .unwrap_or(StateCell::AIR)
    }

    /// The boxes of `collision`, relative to the block position.
    #[must_use]
    pub fn boxes(&self, collision: Collision) -> &[CellBox] {
        const FULL: [CellBox; 1] = [CellBox {
            min: [0.0; 3],
            max: [1.0; 3],
        }];
        match collision {
            Collision::Empty => &[],
            Collision::FullCube => &FULL,
            Collision::Boxes { start, end } => {
                self.boxes.get(start as usize..end as usize).unwrap_or(&[])
            }
        }
    }

    /// The top of the collision shape of `cell` above its block, for the move control; `None`
    /// for an empty shape or a door or a fence (PATH-MOV-002).
    #[must_use]
    pub fn top(&self, cell: StateCell) -> Option<f64> {
        if cell.flags.contains(StateFlags::DOOR_OR_FENCE) {
            return None;
        }
        self.boxes(cell.collision)
            .iter()
            .map(|cell_box| cell_box.max[1])
            .reduce(f64::max)
    }
}

/// The tags and the blocks that decide the state path type and the flags of a state.
struct BlockKinds<'a> {
    tags: &'a Tags<Block>,
    path_tags: Vec<(TagId<Block>, PathType)>,
    fire: Option<TagId<Block>>,
    campfires: Option<TagId<Block>>,
    doors: Option<TagId<Block>>,
    fences: Option<TagId<Block>>,
    walls: Option<TagId<Block>>,
    fence_gates: Option<TagId<Block>>,
    iron_door: Option<Id<Block>>,
    grass_block: Option<Id<Block>>,
    fire_blocks: Vec<Id<Block>>,
}

impl<'a> BlockKinds<'a> {
    fn new(definitions: &BlockDefinitions, tags: &'a Tags<Block>) -> Self {
        Self {
            tags,
            path_tags: PATH_TAGS
                .iter()
                .filter_map(|(key, path)| Some((tags.id(*key)?, *path)))
                .collect(),
            fire: tags.id(block_tag_keys::FIRE),
            campfires: tags.id(block_tag_keys::CAMPFIRES),
            doors: tags.id(block_tag_keys::DOORS),
            fences: tags.id(block_tag_keys::FENCES),
            walls: tags.id(block_tag_keys::WALLS),
            fence_gates: tags.id(block_tag_keys::FENCE_GATES),
            iron_door: definitions.id_of("minecraft:iron_door"),
            grass_block: definitions.id_of("minecraft:grass_block"),
            fire_blocks: FIRE_BLOCKS
                .iter()
                .filter_map(|name| definitions.id_of(name))
                .collect(),
        }
    }

    /// The cell of the state `id` with the collision shape `collision`.
    fn cell(
        &self,
        definitions: &BlockDefinitions,
        id: BlockStateId,
        collision: Collision,
    ) -> StateCell {
        let data = definitions.state(id);
        let owner = definitions.owner(id);
        let block = definitions.block_index(id);
        let in_tag = |tag: Option<TagId<Block>>| {
            block
                .zip(tag)
                .is_some_and(|(block, tag)| self.tags.contains(tag, block))
        };
        let flag_on = |property: &str| {
            owner.and_then(|entry| entry.value_of(id, property)) == Some(&PropertyValue::Bool(true))
        };
        let is_block = |other: Option<Id<Block>>| block.is_some() && block == other;
        let fluid = data.fluid.map(|fluid| fluid.fluid);
        let tagged = self
            .path_tags
            .iter()
            .find(|(tag, _)| in_tag(Some(*tag)))
            .map(|(_, path)| *path);
        let is_fire = in_tag(self.fire)
            || block.is_some_and(|block| self.fire_blocks.contains(&block))
            || (in_tag(self.campfires) && flag_on("lit"));
        let path = if let Some(path) = tagged {
            path
        } else if fluid == Some(Fluid::Lava.id()) {
            PathType::Lava
        } else if is_fire {
            PathType::Fire
        } else if in_tag(self.doors) {
            if flag_on("open") {
                PathType::DoorOpen
            } else if is_block(self.iron_door) {
                PathType::DoorIronClosed
            } else {
                PathType::DoorWoodClosed
            }
        } else if in_tag(self.fences)
            || in_tag(self.walls)
            || (in_tag(self.fence_gates) && !flag_on("open"))
        {
            PathType::Fence
        } else if collision != Collision::Empty {
            PathType::Blocked
        } else if fluid == Some(Fluid::Water.id()) {
            PathType::Water
        } else {
            PathType::Open
        };
        let marks = [
            (
                data.flags.contains(BlockStateFlags::IS_SOLID_RENDER),
                StateFlags::SOLID_RENDER,
            ),
            (
                data.flags.contains(BlockStateFlags::LEGACY_SOLID),
                StateFlags::LEGACY_SOLID,
            ),
            (fluid == Some(Fluid::Water.id()), StateFlags::WATER),
            (is_block(self.grass_block), StateFlags::GRASS_BLOCK),
            (
                in_tag(self.doors) || in_tag(self.fences),
                StateFlags::DOOR_OR_FENCE,
            ),
        ];
        let flags = marks
            .into_iter()
            .filter(|(set, _)| *set)
            .fold(StateFlags::NONE, |flags, (_, flag)| flags.union(flag));
        StateCell {
            flags,
            path,
            collision,
        }
    }
}
