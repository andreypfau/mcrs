use crate::block::minecraft::note_block::NoteBlockInstrument;
use crate::block::Block;
use crate::material::map::MapColor;
use crate::material::PushReaction;
use crate::sound::SoundType;
use mcrs_core::voxel_shape::VoxelShape;
use mcrs_protocol::BlockStateId;

#[derive(Clone, Copy, Debug)]
pub enum LightSpec {
    Const(u8),
    PerState(fn(&Block, BlockStateId) -> u8),
}

impl LightSpec {
    pub fn eval(&self, block: &Block, state: BlockStateId) -> u8 {
        match self {
            LightSpec::Const(v) => *v,
            LightSpec::PerState(f) => f(block, state),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum OcclusionSpec {
    Empty,
    FullCube,
    PerState(fn(&Block, BlockStateId) -> &'static VoxelShape),
}

impl OcclusionSpec {
    pub fn eval(&self, block: &Block, state: BlockStateId) -> &'static VoxelShape {
        match self {
            OcclusionSpec::Empty => VoxelShape::empty(),
            OcclusionSpec::FullCube => VoxelShape::block(),
            OcclusionSpec::PerState(f) => f(block, state),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Properties {
    pub hardness: f32,
    pub explosion_resistance: f32,
    pub ignited_by_lava: bool,
    pub replaceable: bool,
    pub is_air: bool,
    pub has_collision: bool,
    pub can_occlude: bool,
    pub requires_correct_tool_for_drops: bool,
    pub is_valid_spawn: bool,
    pub is_randomly_ticking: bool,
    pub map_color: MapColor,
    pub sound_type: &'static SoundType,
    pub push_reaction: PushReaction,
    pub instrument: NoteBlockInstrument,
    pub xp_range: Option<(u32, u32)>,
    pub light_emission: LightSpec,
    pub light_dampening: LightSpec,
    pub occlusion: OcclusionSpec,
}

impl Properties {
    pub const fn new() -> Self {
        Properties {
            hardness: 0.0,
            explosion_resistance: 0.0,
            ignited_by_lava: false,
            is_air: false,
            replaceable: false,
            has_collision: true,
            can_occlude: true,
            requires_correct_tool_for_drops: false,
            is_valid_spawn: true,
            is_randomly_ticking: false,
            map_color: MapColor::NONE,
            sound_type: &SoundType::STONE,
            push_reaction: PushReaction::Normal,
            instrument: NoteBlockInstrument::Harp,
            xp_range: None,
            light_emission: LightSpec::Const(0),
            light_dampening: LightSpec::Const(15),
            occlusion: OcclusionSpec::FullCube,
        }
    }

    pub const fn with_strength(mut self, value: f32) -> Self {
        self.hardness = value;
        self.explosion_resistance = value;
        self
    }

    pub const fn with_hardness(mut self, value: f32) -> Self {
        self.hardness = value;
        self
    }

    pub const fn with_explosion_resistance(mut self, value: f32) -> Self {
        self.explosion_resistance = value.max(0.0);
        self
    }

    pub const fn instant_break(self) -> Self {
        self.with_hardness(0.0).with_explosion_resistance(0.0)
    }

    pub const fn ignited_by_lava(mut self) -> Self {
        self.ignited_by_lava = true;
        self
    }

    pub const fn with_random_ticks(mut self) -> Self {
        self.is_randomly_ticking = true;
        self
    }

    pub const fn with_map_color(mut self, value: MapColor) -> Self {
        self.map_color = value;
        self
    }

    pub const fn with_note_block_instrument(mut self, value: NoteBlockInstrument) -> Self {
        self.instrument = value;
        self
    }

    pub const fn with_sound(mut self, value: &'static SoundType) -> Self {
        self.sound_type = value;
        self
    }

    pub const fn with_push_reaction(mut self, value: PushReaction) -> Self {
        self.push_reaction = value;
        self
    }

    pub const fn requires_correct_tool_for_drops(mut self) -> Self {
        self.requires_correct_tool_for_drops = true;
        self
    }

    pub const fn air(mut self) -> Self {
        self.is_air = true;
        self.light_emission = LightSpec::Const(0);
        self.light_dampening = LightSpec::Const(0);
        self.occlusion = OcclusionSpec::Empty;
        self
    }

    pub const fn no_collision(mut self) -> Self {
        self.has_collision = false;
        self.can_occlude = false;
        self.light_dampening = LightSpec::Const(1);
        self.occlusion = OcclusionSpec::Empty;
        self
    }

    pub const fn with_light_emission(mut self, value: LightSpec) -> Self {
        self.light_emission = value;
        self
    }

    pub const fn with_light_dampening(mut self, value: LightSpec) -> Self {
        self.light_dampening = value;
        self
    }

    pub const fn with_occlusion(mut self, value: OcclusionSpec) -> Self {
        self.occlusion = value;
        self
    }

    pub const fn replacable(mut self) -> Self {
        self.replaceable = true;
        self
    }

    pub const fn with_xp_range(mut self, min: u32, max: u32) -> Self {
        self.xp_range = Some((min, max));
        self
    }

    pub const fn with_no_loot_table(mut self) -> Self {
        //todo: loot table
        self
    }

    pub const fn with_is_valid_spawn(mut self, value: bool) -> Self {
        self.is_valid_spawn = value;
        self
    }
}

impl Default for Properties {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_default_has_solid_lighting_defaults() {
        let p = Properties::new();
        assert!(matches!(p.light_emission, LightSpec::Const(0)));
        assert!(matches!(p.light_dampening, LightSpec::Const(15)));
        assert!(matches!(p.occlusion, OcclusionSpec::FullCube));
    }

    #[test]
    fn air_builder_sets_transparent_lighting_defaults() {
        let p = Properties::new().air();
        assert!(p.is_air);
        assert!(matches!(p.light_emission, LightSpec::Const(0)));
        assert!(matches!(p.light_dampening, LightSpec::Const(0)));
        assert!(matches!(p.occlusion, OcclusionSpec::Empty));
    }

    #[test]
    fn no_collision_builder_sets_partial_dampening() {
        let p = Properties::new().no_collision();
        assert!(!p.has_collision);
        assert!(!p.can_occlude);
        assert!(matches!(p.light_dampening, LightSpec::Const(1)));
        assert!(matches!(p.occlusion, OcclusionSpec::Empty));
        assert!(matches!(p.light_emission, LightSpec::Const(0)));
    }

    #[test]
    fn properties_is_copy() {
        let p1 = Properties::new();
        let _p2 = p1;
        let _p3 = p1;
    }

    #[test]
    fn with_light_emission_const() {
        let p = Properties::new().with_light_emission(LightSpec::Const(15));
        assert!(matches!(p.light_emission, LightSpec::Const(15)));
    }

    #[test]
    fn with_light_dampening_const() {
        let p = Properties::new().with_light_dampening(LightSpec::Const(5));
        assert!(matches!(p.light_dampening, LightSpec::Const(5)));
    }

    #[test]
    fn with_occlusion_empty() {
        let p = Properties::new().with_occlusion(OcclusionSpec::Empty);
        assert!(matches!(p.occlusion, OcclusionSpec::Empty));
    }
}
