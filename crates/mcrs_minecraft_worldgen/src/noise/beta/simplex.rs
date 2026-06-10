use mcrs_random::Random;

pub struct SimplexNoise {
    permutation: [u8; 256],
    pub origin_x: f64,
    pub origin_y: f64,
    pub origin_z: f64,
}

impl SimplexNoise {
    pub fn from_random<T: Random>(random: &mut T) -> Self {
        let origin_x = random.next_f64() * 256.0;
        let origin_y = random.next_f64() * 256.0;
        let origin_z = random.next_f64() * 256.0;
        let mut permutation = [0u8; 256];
        for i in 0..256 {
            permutation[i] = i as u8;
        }
        for i in 0..256u32 {
            let j = random.next_u32_bound(256 - i);
            permutation.swap(i as usize, (i + j) as usize);
        }
        Self { permutation, origin_x, origin_y, origin_z }
    }

    pub fn sample(&self, x: f64, z: f64, scale_x: f64, scale_z: f64) -> f64 {
        const SKEW_2D: f64 = 0.3660254037844386;
        const UNSKEW_2D: f64 = 0.2113248654051871;

        const GRADIENTS: [[i32; 2]; 12] = [
            [ 1,  1], [-1,  1], [ 1, -1], [-1, -1],
            [ 1,  0], [-1,  0], [ 1,  0], [-1,  0],
            [ 0,  1], [ 0, -1], [ 0,  1], [ 0, -1],
        ];

        let px = x * scale_x + self.origin_x;
        let py = z * scale_z + self.origin_y;

        let skew = (px + py) * SKEW_2D;
        let i = (px + skew).floor() as i32;
        let j = (py + skew).floor() as i32;

        let unskew = (i + j) as f64 * UNSKEW_2D;
        let x0 = px - (i as f64 - unskew);
        let y0 = py - (j as f64 - unskew);

        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

        let x1 = x0 - i1 as f64 + UNSKEW_2D;
        let y1 = y0 - j1 as f64 + UNSKEW_2D;
        let x2 = x0 - 1.0 + 2.0 * UNSKEW_2D;
        let y2 = y0 - 1.0 + 2.0 * UNSKEW_2D;

        let gi0 = self.permutation[((i & 0xFF) as u8).wrapping_add(self.permutation[(j & 0xFF) as usize]) as usize] % 12;
        let gi1 = self.permutation[((i.wrapping_add(i1) & 0xFF) as u8).wrapping_add(self.permutation[(j.wrapping_add(j1) & 0xFF) as usize]) as usize] % 12;
        let gi2 = self.permutation[((i.wrapping_add(1) & 0xFF) as u8).wrapping_add(self.permutation[(j.wrapping_add(1) & 0xFF) as usize]) as usize] % 12;

        let contrib = |t2_base: f64, tx: f64, ty: f64, gi: u8| -> f64 {
            let t = t2_base - tx * tx - ty * ty;
            if t < 0.0 {
                0.0
            } else {
                let t2 = t * t;
                let g = GRADIENTS[gi as usize];
                t2 * t2 * (g[0] as f64 * tx + g[1] as f64 * ty)
            }
        };

        let n0 = contrib(0.5, x0, y0, gi0);
        let n1 = contrib(0.5, x1, y1, gi1);
        let n2 = contrib(0.5, x2, y2, gi2);

        70.0 * (n0 + n1 + n2)
    }
}

#[cfg(test)]
mod test {
    use super::SimplexNoise;
    use mcrs_random::legacy::LegacyRandom;

    #[test]
    fn simplex_reachable() {
        let noise = SimplexNoise::from_random(&mut LegacyRandom::new(845));
        let v = noise.sample(0.5, 0.5, 1.0, 1.0);
        assert!(v.is_finite(), "sample must return a finite f64");
    }
}
