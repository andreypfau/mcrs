use crate::GaussianBank;

const F32_MULTIPLIER: f32 = 1.0 / (1u64 << 24) as f32;
const F64_MULTIPLIER: f64 = 1.0 / (1u64 << 53) as f64;

/// `BitRandomSource`: every draw assembled from `next(bits)` the way
/// `java.util.Random` assembles it, whatever produces the bits.
pub(crate) trait BitSource {
    fn next_bits(&mut self, bits: usize) -> u64;

    fn gaussian_bank(&mut self) -> &mut GaussianBank;

    fn bits_u64(&mut self) -> u64 {
        (self.next_bits(32) << 32) + self.next_bits(32)
    }

    /// `nextLong`: both 32-bit halves are sign-extended before combining, so
    /// a lower half with its high bit set comes out 2^32 below `bits_u64`.
    fn bits_java_long(&mut self) -> i64 {
        let hi = self.next_bits(32) as i32 as i64;
        let lo = self.next_bits(32) as i32 as i64;
        (hi << 32).wrapping_add(lo)
    }

    fn bits_bool(&mut self) -> bool {
        self.next_bits(1) != 0
    }

    fn bits_u32_bound(&mut self, bound: u32) -> u32 {
        if (bound & (bound - 1)) == 0 {
            let n = self.next_bits(31);
            return ((bound as u64).wrapping_mul(n) >> 31) as u32;
        }
        let bound = bound as i32;
        loop {
            let sample = self.next_bits(31) as i32;
            let modulo = sample % bound;
            if sample.wrapping_sub(modulo).wrapping_add(bound - 1) >= 0 {
                return modulo as u32;
            }
        }
    }

    fn bits_f32(&mut self) -> f32 {
        self.next_bits(24) as f32 * F32_MULTIPLIER
    }

    /// `BitRandomSource.DOUBLE_MULTIPLIER` is declared from the float literal
    /// `1.110223E-16F`, which rounds exactly onto `2^-53`, so the modern source and
    /// `java.util.Random` agree bit for bit here.
    fn bits_f64(&mut self) -> f64 {
        let hi = self.next_bits(26);
        let lo = self.next_bits(27);
        ((hi << 27) + lo) as f64 * F64_MULTIPLIER
    }

    fn bits_gaussian(&mut self) -> f64 {
        let mut bank = *self.gaussian_bank();
        let value = bank.next(|| self.bits_f64());
        *self.gaussian_bank() = bank;
        value
    }
}
