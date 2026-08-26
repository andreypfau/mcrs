/// What a block carries that the definition corpus does not. Every value the
/// corpus states — hardness, light, shapes, map colour, piston reaction — is
/// read from it instead, so a block's own data is only what no component
/// carries.
#[derive(Clone, Copy, Debug)]
pub struct Properties {
    pub xp_range: Option<(u32, u32)>,
}

impl Properties {
    pub const fn new() -> Self {
        Properties { xp_range: None }
    }

    pub const fn with_xp_range(mut self, min: u32, max: u32) -> Self {
        self.xp_range = Some((min, max));
        self
    }
}

impl Default for Properties {
    fn default() -> Self {
        Self::new()
    }
}
