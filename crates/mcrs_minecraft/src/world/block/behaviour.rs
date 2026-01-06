use crate::sound::SoundType;
use crate::world::block::minecraft::note_block::NoteBlockInstrument;
use crate::world::material::PushReaction;
use crate::world::material::map::MapColor;
use mcrs_engine::world::block::BlockPos;

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
        self
    }

    pub const fn no_collision(mut self) -> Self {
        self.has_collision = false;
        self.can_occlude = false;
        self
    }

    pub const fn replacable(mut self) -> Self {
        self.replaceable = true;
        self
    }

    pub const fn with_no_loot_table(mut self) -> Self {
        //todo: loot table
        self
    }

    pub const fn with_is_valid_spawn(mut self, value: bool) -> Self {
        self.can_occlude = value;
        self
    }
}

impl Default for Properties {
    fn default() -> Self {
        Self::new()
    }
}

pub trait BlockBehaviour: Sync + Send {
    fn properties(&self) -> &Properties;

    fn get_destroy_speed(&self, pos: BlockPos) -> f32 {
        self.properties().hardness
    }
}
