/// What a face's colour is multiplied by: nothing, one of the biome colour maps sampled where
/// the face stands, or a colour fixed by the block state.
#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub enum Tint {
    #[default]
    None,
    Grass,
    Foliage,
    DryFoliage,
    Water,
    Fixed(u8),
}

/// The biome colour maps, in the order of their layers in the tint texture.
pub const BIOME_TINTS: [Tint; 4] = [Tint::Grass, Tint::Foliage, Tint::DryFoliage, Tint::Water];

const SPRUCE_LEAVES: u8 = 0;
const BIRCH_LEAVES: u8 = 1;
const ATTACHED_STEM: u8 = 2;
const LILY_PAD: u8 = 3;
const STEM: u8 = 4;
const STEM_AGES: u8 = 8;
const REDSTONE: u8 = STEM + STEM_AGES;
const REDSTONE_POWERS: u8 = 16;

pub const FIXED_TINTS: usize = (REDSTONE + REDSTONE_POWERS) as usize;

impl Tint {
    pub const SPRUCE_LEAVES: Tint = Tint::Fixed(SPRUCE_LEAVES);
    pub const BIRCH_LEAVES: Tint = Tint::Fixed(BIRCH_LEAVES);
    pub const ATTACHED_STEM: Tint = Tint::Fixed(ATTACHED_STEM);
    pub const LILY_PAD: Tint = Tint::Fixed(LILY_PAD);

    pub fn stem(age: u8) -> Tint {
        Tint::Fixed(STEM + age.min(STEM_AGES - 1))
    }

    pub fn redstone(power: u8) -> Tint {
        Tint::Fixed(REDSTONE + power.min(REDSTONE_POWERS - 1))
    }

    /// The number the shader reads: 0 for none, then the biome maps, then the fixed colours.
    pub const fn index(self) -> u32 {
        match self {
            Tint::None => 0,
            Tint::Grass => 1,
            Tint::Foliage => 2,
            Tint::DryFoliage => 3,
            Tint::Water => 4,
            Tint::Fixed(index) => 1 + BIOME_TINTS.len() as u32 + index as u32,
        }
    }
}

/// The fixed colours as `0xRRGGBB`, indexed like `Tint::Fixed`.
pub fn fixed_colors() -> [u32; FIXED_TINTS] {
    let mut colors = [0u32; FIXED_TINTS];
    colors[SPRUCE_LEAVES as usize] = 0x619961;
    colors[BIRCH_LEAVES as usize] = 0x80a755;
    colors[ATTACHED_STEM as usize] = 0xe0c71c;
    colors[LILY_PAD as usize] = 0x208030;
    for age in 0..STEM_AGES as u32 {
        colors[(STEM as u32 + age) as usize] = (age * 32) << 16 | (255 - age * 8) << 8 | age * 4;
    }
    for power in 0..REDSTONE_POWERS {
        colors[(REDSTONE + power) as usize] = redstone_color(power);
    }
    colors
}

fn redstone_color(power: u8) -> u32 {
    let power = power as f32 / 15.0;
    let red = power * 0.6 + if power > 0.0 { 0.4 } else { 0.3 };
    let green = (power * power * 0.7 - 0.5).clamp(0.0, 1.0);
    let blue = (power * power * 0.6 - 0.7).clamp(0.0, 1.0);
    let byte = |channel: f32| (channel * 255.0).floor() as u32;
    byte(red) << 16 | byte(green) << 8 | byte(blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixed_colours_are_the_ones_vanilla_registers() {
        let colors = fixed_colors();
        let argb = |value: i32| value as u32 & 0xff_ffff;
        assert_eq!(colors[SPRUCE_LEAVES as usize], argb(-10380959));
        assert_eq!(colors[BIRCH_LEAVES as usize], argb(-8345771));
        assert_eq!(colors[ATTACHED_STEM as usize], argb(-2046180));
        assert_eq!(colors[LILY_PAD as usize], argb(-14647248));
        assert_eq!(colors[STEM as usize], 0x00ff00);
        assert_eq!(colors[(STEM + 7) as usize], 0xe0c71c);
        assert_eq!(colors[REDSTONE as usize], 0x4c0000);
        assert_eq!(colors[(REDSTONE + 15) as usize], 0xff3200);
    }

    #[test]
    fn every_index_is_distinct_and_fits_the_fields() {
        let mut seen = std::collections::BTreeSet::new();
        let all = [Tint::None]
            .into_iter()
            .chain(BIOME_TINTS)
            .chain((0..FIXED_TINTS as u8).map(Tint::Fixed));
        for tint in all {
            assert!(seen.insert(tint.index()), "{tint:?}");
        }
        assert!(
            (*seen.last().unwrap() as u64) <= crate::pack::FACE_TINT.max(),
            "the face tint field is too narrow"
        );
    }
}
