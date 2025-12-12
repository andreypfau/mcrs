use mcrs_engine::world::block::BlockPos;

#[derive(Clone, Copy, Debug)]
pub struct Properties {
    destroy_time: f32,
    explosion_resistance: f32,
    ignited_by_lava: bool,
    replaceable: bool,
    is_air: bool,
    has_collision: bool,
    can_occlude: bool,
    requires_correct_tool_for_drops: bool,
}

impl Properties {
    pub const fn new() -> Self {
        Properties {
            destroy_time: 0.0,
            explosion_resistance: 0.0,
            ignited_by_lava: false,
            is_air: false,
            replaceable: false,
            has_collision: true,
            can_occlude: true,
            requires_correct_tool_for_drops: false,
        }
    }

    pub const fn destroy_time(mut self, value: f32) -> Self {
        self.destroy_time = value;
        self
    }

    pub const fn explosion_resistance(mut self, value: f32) -> Self {
        self.explosion_resistance = value.max(0.0);
        self
    }

    pub const fn instant_break(self) -> Self {
        self.destroy_time(0.0).explosion_resistance(0.0)
    }

    pub const fn ignited_by_lava(mut self) -> Self {
        self.ignited_by_lava = true;
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

    pub const fn no_loot_table(mut self) -> Self {
        //todo: loot table
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
        self.properties().destroy_time
    }
}
