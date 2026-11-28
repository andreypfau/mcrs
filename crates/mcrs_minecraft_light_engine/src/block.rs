use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::voxel_shape::VoxelShape;

use crate::level::LightLevel;

#[derive(Clone, Debug)]
pub struct LightProperties {
    pub dampening: u8,
    pub emission: LightLevel,
    pub occlusion: Option<&'static VoxelShape>,
}

impl LightProperties {
    pub const AIR: Self = Self {
        dampening: 0,
        emission: LightLevel::ZERO,
        occlusion: None,
    };

    pub const SOLID: Self = Self {
        dampening: 15,
        emission: LightLevel::ZERO,
        occlusion: None,
    };

    pub const fn transparent(dampening: u8) -> Self {
        Self {
            dampening,
            emission: LightLevel::ZERO,
            occlusion: None,
        }
    }

    pub const fn emitter(emission: u8) -> Self {
        Self {
            dampening: 0,
            emission: LightLevel::new(emission),
            occlusion: None,
        }
    }

    pub const fn shaped(dampening: u8, shape: &'static VoxelShape) -> Self {
        Self {
            dampening,
            emission: LightLevel::ZERO,
            occlusion: Some(shape),
        }
    }

    fn shape(&self) -> &'static VoxelShape {
        self.occlusion.unwrap_or_else(VoxelShape::empty)
    }

    fn is_shaped(&self) -> bool {
        self.occlusion.is_some()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SpecialBlocks {
    pub unloaded: VoxelId,
    pub outside: VoxelId,
}

#[derive(Clone, Debug)]
pub struct LightRegistry {
    properties: Vec<LightProperties>,
    special: SpecialBlocks,
}

impl LightRegistry {
    pub fn new(properties: Vec<LightProperties>, special: SpecialBlocks) -> Self {
        let check = |id: VoxelId, what: &str| {
            assert!(
                (id.0 as usize) < properties.len(),
                "{what} block id is not in the table"
            );
        };
        check(special.unloaded, "unloaded");
        check(special.outside, "outside");
        let dampening = |id: VoxelId| properties.get(id.0 as usize).map(|p| p.dampening);
        assert_eq!(
            dampening(special.unloaded),
            Some(15),
            "the unloaded filler must stop light"
        );
        assert_eq!(
            dampening(special.outside),
            Some(0),
            "the outside filler must pass light"
        );
        Self {
            properties,
            special,
        }
    }

    pub fn get(&self, id: VoxelId) -> &LightProperties {
        self.properties
            .get(id.0 as usize)
            .unwrap_or(&LightProperties::SOLID)
    }

    pub fn unloaded(&self) -> VoxelId {
        self.special.unloaded
    }

    pub fn outside(&self) -> VoxelId {
        self.special.outside
    }

    pub fn emission(&self, id: VoxelId) -> LightLevel {
        self.get(id).emission
    }

    pub fn attenuation(&self, from: VoxelId, to: VoxelId, dir: Direction) -> Option<u8> {
        let from_props = self.get(from);
        let to_props = self.get(to);

        if (from_props.is_shaped() || to_props.is_shaped())
            && from_props.shape().face_occludes(to_props.shape(), dir)
        {
            return None;
        }

        Some(to_props.dampening.max(1))
    }

    pub fn breaks_sky_column(&self, above: VoxelId, below: VoxelId) -> bool {
        let below_props = self.get(below);
        if below_props.dampening != 0 {
            return true;
        }
        self.get(above)
            .shape()
            .face_occludes(below_props.shape(), Direction::Down)
    }

    pub fn light_properties_differ(&self, old: VoxelId, new: VoxelId) -> bool {
        if old == new {
            return false;
        }
        let old_props = self.get(old);
        let new_props = self.get(new);
        old_props.dampening != new_props.dampening
            || old_props.emission != new_props.emission
            || old_props.is_shaped()
            || new_props.is_shaped()
    }
}
