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

    #[inline(always)]
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

    #[inline(always)]
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
        // SAFETY: All permutation indices are masked with & 0xFF, guaranteeing [0, 255].
        // All gradient indices are (perm & 15) << 2 = [0, 60], accessed with offsets +0/+1/+2,
        // so max index is 62, within FLAT_SIMPLEX_GRAD's 64 elements.
        unsafe {
            let perm = &self.permutation;

            // Hash lookups — first level (X)
            let var0 = (section_x & 0xFF) as usize;
            let var1 = (section_x.wrapping_add(1) & 0xFF) as usize;
            let p0 = *perm.get_unchecked(var0) as usize;
            let p1 = *perm.get_unchecked(var1) as usize;

            // Second level (X+Y)
            let sy = section_y as usize;
            let var4 = (p0.wrapping_add(sy)) & 0xFF;
            let var5 = (p1.wrapping_add(sy)) & 0xFF;
            let var6 = (p0.wrapping_add(sy).wrapping_add(1)) & 0xFF;
            let var7 = (p1.wrapping_add(sy).wrapping_add(1)) & 0xFF;
            let p4 = *perm.get_unchecked(var4) as usize;
            let p5 = *perm.get_unchecked(var5) as usize;
            let p6 = *perm.get_unchecked(var6) as usize;
            let p7 = *perm.get_unchecked(var7) as usize;

            // Third level (X+Y+Z) — 8 corner gradient indices
            let sz = section_z as usize;
            let h000 = ((*perm.get_unchecked((p4.wrapping_add(sz)) & 0xFF) & 15) as usize) << 2;
            let h100 = ((*perm.get_unchecked((p5.wrapping_add(sz)) & 0xFF) & 15) as usize) << 2;
            let h010 = ((*perm.get_unchecked((p6.wrapping_add(sz)) & 0xFF) & 15) as usize) << 2;
            let h110 = ((*perm.get_unchecked((p7.wrapping_add(sz)) & 0xFF) & 15) as usize) << 2;
            let h001 = ((*perm.get_unchecked((p4.wrapping_add(sz).wrapping_add(1)) & 0xFF) & 15) as usize) << 2;
            let h101 = ((*perm.get_unchecked((p5.wrapping_add(sz).wrapping_add(1)) & 0xFF) & 15) as usize) << 2;
            let h011 = ((*perm.get_unchecked((p6.wrapping_add(sz).wrapping_add(1)) & 0xFF) & 15) as usize) << 2;
            let h111 = ((*perm.get_unchecked((p7.wrapping_add(sz).wrapping_add(1)) & 0xFF) & 15) as usize) << 2;

            // Relative offsets for the far corner
            let x1 = local_x - 1.0;
            let y1 = local_y - 1.0;
            let z1 = local_z - 1.0;

            // Gradient dot products using FMA (grad · offset)
            let g = &FLAT_SIMPLEX_GRAD;
            let d000 = g.get_unchecked(h000 + 2).mul_add(local_z, g.get_unchecked(h000 + 1).mul_add(local_y, *g.get_unchecked(h000) * local_x));
            let d100 = g.get_unchecked(h100 + 2).mul_add(local_z, g.get_unchecked(h100 + 1).mul_add(local_y, *g.get_unchecked(h100) * x1));
            let d010 = g.get_unchecked(h010 + 2).mul_add(local_z, g.get_unchecked(h010 + 1).mul_add(y1, *g.get_unchecked(h010) * local_x));
            let d110 = g.get_unchecked(h110 + 2).mul_add(local_z, g.get_unchecked(h110 + 1).mul_add(y1, *g.get_unchecked(h110) * x1));
            let d001 = g.get_unchecked(h001 + 2).mul_add(z1, g.get_unchecked(h001 + 1).mul_add(local_y, *g.get_unchecked(h001) * local_x));
            let d101 = g.get_unchecked(h101 + 2).mul_add(z1, g.get_unchecked(h101 + 1).mul_add(local_y, *g.get_unchecked(h101) * x1));
            let d011 = g.get_unchecked(h011 + 2).mul_add(z1, g.get_unchecked(h011 + 1).mul_add(y1, *g.get_unchecked(h011) * local_x));
            let d111 = g.get_unchecked(h111 + 2).mul_add(z1, g.get_unchecked(h111 + 1).mul_add(y1, *g.get_unchecked(h111) * x1));

            // Fade curves: t³(6t² - 15t + 10)
            let fade_x = local_x * local_x * local_x * local_x.mul_add(local_x.mul_add(6.0, -15.0), 10.0);
            let fade_y = fade_local_x * fade_local_x * fade_local_x * fade_local_x.mul_add(fade_local_x.mul_add(6.0, -15.0), 10.0);
            let fade_z = local_z * local_z * local_z * local_z.mul_add(local_z.mul_add(6.0, -15.0), 10.0);

            // Trilinear interpolation using FMA
            let l00 = (d100 - d000).mul_add(fade_x, d000);
            let l10 = (d110 - d010).mul_add(fade_x, d010);
            let l01 = (d101 - d001).mul_add(fade_x, d001);
            let l11 = (d111 - d011).mul_add(fade_x, d011);
            let ll0 = (l10 - l00).mul_add(fade_y, l00);
            let ll1 = (l11 - l01).mul_add(fade_y, l01);
            (ll1 - ll0).mul_add(fade_z, ll0)
        }
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
