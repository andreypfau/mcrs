use mcrs_random::{Random, RandomSource};

const FLAT_SIMPLEX_GRAD: [f32; 64] = [
    1.0, 1.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0, 1.0, 0.0,
    1.0, 0.0, -1.0, 0.0, 1.0, 0.0, 1.0, 0.0, -1.0, 0.0, -1.0, 0.0, -1.0, 0.0, 0.0, 1.0, 1.0, 0.0,
    0.0, -1.0, 1.0, 0.0, 0.0, 1.0, -1.0, 0.0, 0.0, -1.0, -1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, -1.0,
    1.0, 0.0, -1.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0,
];

#[derive(Debug, Clone, PartialEq)]
pub struct ImprovedNoise {
    permutation: [u8; 256],
    pub origin_x: f32,
    pub origin_y: f32,
    pub origin_z: f32,
}

impl Default for ImprovedNoise {
    fn default() -> Self {
        Self::from_random(&mut RandomSource::new(0, true))
    }
}

impl ImprovedNoise {
    pub fn from_random<T>(random: &mut T) -> Self
    where
        T: Random,
    {
        let origin_x = random.next_f32() * 256.0;
        let origin_y = random.next_f32() * 256.0;
        let origin_z = random.next_f32() * 256.0;
        let mut permutation = [0u8; 256];
        for i in 0..256 {
            permutation[i] = i as u8;
        }
        for i in 0..256 {
            let j = random.next_u32_bound(256 - i);
            permutation.swap(i as usize, (i + j) as usize);
        }
        Self {
            permutation,
            origin_x,
            origin_y,
            origin_z,
        }
    }

    pub fn sample(&self, x: f32, y: f32, z: f32, y_scale: f32, y_max: f32) -> f32 {
        let shifted_x = x + self.origin_x;
        let shifted_y = y + self.origin_y;
        let shifted_z = z + self.origin_z;
        let section_x = shifted_x.floor() as i32;
        let section_y = shifted_y.floor() as i32;
        let section_z = shifted_z.floor() as i32;
        let local_x = shifted_x - section_x as f32;
        let local_y = shifted_y - section_y as f32;
        let local_z = shifted_z - section_z as f32;
        let mut fade = 0.0;
        if y_scale != 0.0 {
            let t = if y_max >= 0.0 && y_max < local_y {
                y_max
            } else {
                local_y
            };
            fade = (t / y_scale + 1.0E-7f32).floor() * y_scale
        }
        self.sample_and_lerp(
            section_x,
            section_y,
            section_z,
            local_x,
            local_y - fade,
            local_z,
            local_y,
        )
    }

    pub fn sample_and_lerp(
        &self,
        section_x: i32,
        section_y: i32,
        section_z: i32,
        local_x: f32,
        local_y: f32,
        local_z: f32,
        fade_local_x: f32,
    ) -> f32 {
        let var0 = section_x & 0xFF;
        let var1 = (section_x.wrapping_add(1)) & 0xFF;
        let var2 = self.permutation[var0 as usize] as i32;
        let var3 = self.permutation[var1 as usize] as i32;
        let var4 = (var2.wrapping_add(section_y)) & 0xFF;
        let var5 = (var3.wrapping_add(section_y)) & 0xFF;
        let var6 = (var2.wrapping_add(section_y).wrapping_add(1)) & 0xFF;
        let var7 = (var3.wrapping_add(section_y).wrapping_add(1)) & 0xFF;
        let var8 = self.permutation[var4 as usize] as i32;
        let var9 = self.permutation[var5 as usize] as i32;
        let var10 = self.permutation[var6 as usize] as i32;
        let var11 = self.permutation[var7 as usize] as i32;

        let var12 = (var8.wrapping_add(section_z)) & 0xFF;
        let var13 = (var9.wrapping_add(section_z)) & 0xFF;
        let var14 = (var10.wrapping_add(section_z)) & 0xFF;
        let var15 = (var11.wrapping_add(section_z)) & 0xFF;
        let var16 = (var8.wrapping_add(section_z).wrapping_add(1)) & 0xFF;
        let var17 = (var9.wrapping_add(section_z).wrapping_add(1)) & 0xFF;
        let var18 = (var10.wrapping_add(section_z).wrapping_add(1)) & 0xFF;
        let var19 = (var11.wrapping_add(section_z).wrapping_add(1)) & 0xFF;

        let var20 = (self.permutation[var12 as usize] & 15) as usize * 4;
        let var21 = (self.permutation[var13 as usize] & 15) as usize * 4;
        let var22 = (self.permutation[var14 as usize] & 15) as usize * 4;
        let var23 = (self.permutation[var15 as usize] & 15) as usize * 4;
        let var24 = (self.permutation[var16 as usize] & 15) as usize * 4;
        let var25 = (self.permutation[var17 as usize] & 15) as usize * 4;
        let var26 = (self.permutation[var18 as usize] & 15) as usize * 4;
        let var27 = (self.permutation[var19 as usize] & 15) as usize * 4;

        let var60 = local_x - 1.0;
        let var61 = local_y - 1.0;
        let var62 = local_z - 1.0;

        let var87 = FLAT_SIMPLEX_GRAD[var20] * local_x
            + FLAT_SIMPLEX_GRAD[var20 + 1] * local_y
            + FLAT_SIMPLEX_GRAD[var20 + 2] * local_z;
        let var88 = FLAT_SIMPLEX_GRAD[var21] * var60
            + FLAT_SIMPLEX_GRAD[var21 + 1] * local_y
            + FLAT_SIMPLEX_GRAD[var21 + 2] * local_z;
        let var89 = FLAT_SIMPLEX_GRAD[var22] * local_x
            + FLAT_SIMPLEX_GRAD[var22 + 1] * var61
            + FLAT_SIMPLEX_GRAD[var22 + 2] * local_z;
        let var90 = FLAT_SIMPLEX_GRAD[var23] * var60
            + FLAT_SIMPLEX_GRAD[var23 + 1] * var61
            + FLAT_SIMPLEX_GRAD[var23 + 2] * local_z;
        let var91 = FLAT_SIMPLEX_GRAD[var24] * local_x
            + FLAT_SIMPLEX_GRAD[var24 + 1] * local_y
            + FLAT_SIMPLEX_GRAD[var24 + 2] * var62;
        let var92 = FLAT_SIMPLEX_GRAD[var25] * var60
            + FLAT_SIMPLEX_GRAD[var25 + 1] * local_y
            + FLAT_SIMPLEX_GRAD[var25 + 2] * var62;
        let var93 = FLAT_SIMPLEX_GRAD[var26] * local_x
            + FLAT_SIMPLEX_GRAD[var26 + 1] * var61
            + FLAT_SIMPLEX_GRAD[var26 + 2] * var62;
        let var94 = FLAT_SIMPLEX_GRAD[var27] * var60
            + FLAT_SIMPLEX_GRAD[var27 + 1] * var61
            + FLAT_SIMPLEX_GRAD[var27 + 2] * var62;

        let var95 = local_x * 6.0 - 15.0;
        let var96 = fade_local_x * 6.0 - 15.0;
        let var97 = local_z * 6.0 - 15.0;
        let var98 = local_x * var95 + 10.0;
        let var99 = fade_local_x * var96 + 10.0;
        let var100 = local_z * var97 + 10.0;
        let var101 = local_x * local_x * local_x * var98;
        let var102 = fade_local_x * fade_local_x * fade_local_x * var99;
        let var103 = local_z * local_z * local_z * var100;

        let var113 = var87 + var101 * (var88 - var87);
        let var114 = var93 + var101 * (var94 - var93);
        let var115 = var91 + var101 * (var92 - var91);
        let var116 = var89 + var101 * (var90 - var89);
        let var117 = var114 - var115;
        let var118 = var102 * (var116 - var113);
        let var119 = var102 * var117;
        let var120 = var113 + var118;
        let var121 = var115 + var119;

        var120 + (var103 * (var121 - var120))
    }
}

#[cfg(test)]
mod test {
    use crate::noise::improved_noise::ImprovedNoise;
    use mcrs_random::legacy::LegacyRandom;

    #[test]
    fn create() {
        let noise = ImprovedNoise::from_random(&mut LegacyRandom::new(845));
        assert_eq!(
            format!("{:.4}", noise.origin_x),
            format!("{:.4}", 179.49112098377014)
        );
        assert_eq!(
            format!("{:.4}", noise.origin_y),
            format!("{:.4}", 178.89801548324886)
        );
        assert_eq!(
            format!("{:.4}", noise.origin_z),
            format!("{:.4}", 139.89344963681773)
        );
        assert_eq!(
            noise.permutation[0..10],
            [12, 160, 244, 220, 152, 102, 106, 117, 151, 137]
        );
    }

    #[test]
    fn sample() {
        let noise = ImprovedNoise::from_random(&mut LegacyRandom::new(845));
        assert_eq!(
            format!("{:.4}", noise.sample(0.0, 0.0, 0.0, 0.0, 0.0)),
            format!("{:.4}", 0.009862268437005883)
        );
        assert_eq!(
            format!("{:.4}", noise.sample(0.5, 4.0, -2.0, 0.0, 0.0)),
            format!("{:.4}", -0.11885865493740287)
        );
        assert_eq!(
            format!("{:.4}", noise.sample(-204.0, 28.0, 12.0, 0.0, 0.0)),
            format!("{:.4}", -0.589681280485348)
        );
    }
}
