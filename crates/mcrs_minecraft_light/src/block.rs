use mcrs_voxel_math::Direction;
use mcrs_voxel_math::voxel_shape::VoxelShape;
use mcrs_voxel_storage::VoxelId;

use crate::level::LightLevel;

/// Everything the light calculation reads about a block state.
#[derive(Clone, Debug)]
pub struct LightProperties {
    /// How much light this block costs to enter. In vanilla this only ever
    /// takes the values 0, 1 or 15, but the engine accepts `0..=15`.
    ///
    /// Beware: 0 and 1 are indistinguishable when propagating, because the cost
    /// is `max(1, dampening)`. They differ only in [`LightRegistry::breaks_sky_column`].
    pub dampening: u8,
    /// Light this block emits on its own.
    pub emission: LightLevel,
    /// `None` when the state does not use its shape for light occlusion, which
    /// is the overwhelming majority: a full stone cube stops light through
    /// `dampening == 15` instead.
    pub occlusion: Option<&'static VoxelShape>,
}

impl LightProperties {
    pub const AIR: Self = Self {
        dampening: 0,
        emission: LightLevel::ZERO,
        occlusion: None,
    };

    /// A full opaque cube: stone, dirt, and the filler used for unloaded space.
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

/// Which of the two independent light layers is being computed.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Layer {
    /// Emitted by blocks. Sources are block emission.
    Block,
    /// Comes from the sky. Sources are whole vertical runs of unoccluded
    /// column, never block emission.
    Sky,
}

/// What stands in for a block the world cannot supply.
///
/// The two cases are genuinely different and must not share an id. Space that
/// is merely not loaded has to stop light, or an edit near the loading frontier
/// would flood the dark with whatever happens to be beyond it. Space above and
/// below the world has to pass light, because that is where sky light enters.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SpecialBlocks {
    /// Inside the world's vertical extent but not loaded. Must be fully opaque.
    pub unloaded: VoxelId,
    /// Above or below the world's vertical extent. Must be fully transparent.
    pub outside: VoxelId,
}

/// The light table. Immutable during a light calculation, so it can be shared
/// across worker threads without synchronisation.
#[derive(Clone, Debug)]
pub struct LightRegistry {
    properties: Vec<LightProperties>,
    special: SpecialBlocks,
}

impl LightRegistry {
    /// `properties[i]` describes `VoxelId(i)`.
    pub fn new(properties: Vec<LightProperties>, special: SpecialBlocks) -> Self {
        let check = |id: VoxelId, what: &str| {
            assert!(
                (id.0 as usize) < properties.len(),
                "{what} block id is not in the table"
            );
        };
        check(special.unloaded, "unloaded");
        check(special.outside, "outside");
        assert_eq!(
            properties[special.unloaded.0 as usize].dampening, 15,
            "the unloaded filler must stop light"
        );
        assert_eq!(
            properties[special.outside.0 as usize].dampening, 0,
            "the outside filler must pass light"
        );
        Self {
            properties,
            special,
        }
    }

    pub fn get(&self, id: VoxelId) -> &LightProperties {
        // Falling back to opaque keeps an out-of-range id from leaking light.
        self.properties
            .get(id.0 as usize)
            .unwrap_or(&LightProperties::SOLID)
    }

    /// Stands in for space inside the world that is not loaded.
    pub fn unloaded(&self) -> VoxelId {
        self.special.unloaded
    }

    /// Stands in for space above and below the world.
    pub fn outside(&self) -> VoxelId {
        self.special.outside
    }

    /// Light emitted by a block into the given layer.
    ///
    /// Sky light has no per-block emitters; its sources come from
    /// [`Self::breaks_sky_column`] applied down a whole column.
    pub fn emission(&self, id: VoxelId, layer: Layer) -> LightLevel {
        match layer {
            Layer::Block => self.get(id).emission,
            Layer::Sky => LightLevel::ZERO,
        }
    }

    /// Cost of moving light from `from` into `to` across `dir`. `None` means
    /// the transition is forbidden outright.
    ///
    /// Two separate mechanisms, and only one of them is pairwise: the
    /// *magnitude* depends solely on the destination block, while the *veto* is
    /// a boolean test on the pair of facing shapes.
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

    /// Does the seam between a block and the one directly below it end a run of
    /// sky sources?
    ///
    /// NOT the same predicate as [`Self::attenuation`], and reusing one for the
    /// other is the mistake this pair of methods exists to prevent. Water breaks
    /// the column here yet still lets light through at cost 1 there; if this
    /// test were used for propagation, water would be a wall.
    ///
    /// Note both terms are load-bearing. A slab has `dampening == 0` exactly
    /// like glass, and breaks the column purely through its face shape.
    pub fn breaks_sky_column(&self, above: VoxelId, below: VoxelId) -> bool {
        let below_props = self.get(below);
        if below_props.dampening != 0 {
            return true;
        }
        self.get(above)
            .shape()
            .face_occludes(below_props.shape(), Direction::Down)
    }

    /// Whether replacing `old` with `new` can change any light at all.
    ///
    /// The shape terms are deliberately conservative: two different shaped
    /// states can share a dampening value and still occlude differently.
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
