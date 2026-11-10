use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::block_tag_keys;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::PathType;
use mcrs_minecraft_registries::Tags;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::TagId;
use mcrs_minecraft_resource::TagKey;
use mcrs_voxel::BlockStateId;

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

/// The tags and the blocks that decide the state path type of a state (PATH-TYP-002).
///
/// A state is pathfindable on land when its collision shape is empty, since the corpus does not
/// hold the navigation component of PATH section 12.3 yet.
pub(crate) struct PathTypes<'a> {
    tags: &'a Tags<Block>,
    path_tags: Vec<(TagId<Block>, PathType)>,
    fire: Option<TagId<Block>>,
    campfires: Option<TagId<Block>>,
    doors: Option<TagId<Block>>,
    fences: Option<TagId<Block>>,
    walls: Option<TagId<Block>>,
    fence_gates: Option<TagId<Block>>,
    iron_door: Option<Id<Block>>,
    fire_blocks: Vec<Id<Block>>,
}

impl<'a> PathTypes<'a> {
    pub(crate) fn new(definitions: &BlockDefinitions, tags: &'a Tags<Block>) -> Self {
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
            fire_blocks: FIRE_BLOCKS
                .iter()
                .filter_map(|name| definitions.id_of(name))
                .collect(),
        }
    }

    /// The state path type of the state `id`, whose collision shape is `boxes`.
    pub(crate) fn of(
        &self,
        definitions: &BlockDefinitions,
        id: BlockStateId,
        boxes: &[Aabb],
    ) -> PathType {
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
        let fluid = definitions.state(id).fluid.map(|fluid| fluid.fluid);
        let tagged = self
            .path_tags
            .iter()
            .find(|(tag, _)| in_tag(Some(*tag)))
            .map(|(_, path)| *path);
        let is_fire = in_tag(self.fire)
            || block.is_some_and(|block| self.fire_blocks.contains(&block))
            || (in_tag(self.campfires) && flag_on("lit"));
        if let Some(path) = tagged {
            path
        } else if fluid == Some(Fluid::Lava.id()) {
            PathType::Lava
        } else if is_fire {
            PathType::Fire
        } else if in_tag(self.doors) {
            if flag_on("open") {
                PathType::DoorOpen
            } else if block.is_some() && block == self.iron_door {
                PathType::DoorIronClosed
            } else {
                PathType::DoorWoodClosed
            }
        } else if in_tag(self.fences)
            || in_tag(self.walls)
            || (in_tag(self.fence_gates) && !flag_on("open"))
        {
            PathType::Fence
        } else if !boxes.is_empty() {
            PathType::Blocked
        } else if fluid == Some(Fluid::Water.id()) {
            PathType::Water
        } else {
            PathType::Open
        }
    }
}
