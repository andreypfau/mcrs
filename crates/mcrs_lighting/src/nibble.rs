pub struct NibbleArray(pub Box<[u8; 2048]>);

impl Clone for NibbleArray {
    fn clone(&self) -> Self {
        Self(Box::new(*self.0))
    }
}

impl NibbleArray {
    #[inline]
    pub fn zeros() -> Self {
        Self(Box::new([0u8; 2048]))
    }

    #[inline]
    pub fn filled(val: u8) -> Self {
        debug_assert!(val < 16);
        let packed = val | (val << 4);
        Self(Box::new([packed; 2048]))
    }

    #[inline]
    pub const fn index(x: usize, y: usize, z: usize) -> usize {
        debug_assert!(x < 16 && y < 16 && z < 16);
        (y << 8) | (z << 4) | x
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        let idx = Self::index(x, y, z);
        (self.0[idx >> 1] >> ((idx & 1) * 4)) & 0x0F
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, val: u8) {
        debug_assert!(val < 16);
        let idx = Self::index(x, y, z);
        let byte = &mut self.0[idx >> 1];
        let shift = (idx & 1) * 4;
        *byte = (*byte & !(0x0F << shift)) | ((val & 0x0F) << shift);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nibble_low_byte_index_zero() {
        let mut arr = NibbleArray::zeros();
        arr.set(0, 0, 0, 0x0F);
        assert_eq!(arr.0[0], 0x0F);
    }

    #[test]
    fn nibble_high_byte_index_one_preserves_low() {
        let mut arr = NibbleArray::zeros();
        arr.set(0, 0, 0, 0x0F);
        arr.set(1, 0, 0, 0x0A);
        assert_eq!(arr.0[0], 0xAF);
    }

    #[test]
    fn nibble_y_stride_256() {
        let mut arr = NibbleArray::zeros();
        arr.set(0, 1, 0, 5);
        let linear = NibbleArray::index(0, 1, 0);
        assert_eq!(linear, 256);
        let byte_index = linear >> 1;
        assert_eq!(byte_index, 128);
        assert_eq!(arr.0[128], 0x05);
        assert_eq!(arr.get(0, 1, 0), 5);
    }

    #[test]
    fn nibble_z_stride_16() {
        let mut arr = NibbleArray::zeros();
        arr.set(0, 0, 1, 3);
        let linear = NibbleArray::index(0, 0, 1);
        assert_eq!(linear, 16);
        let byte_index = linear >> 1;
        assert_eq!(byte_index, 8);
        assert_eq!(arr.0[8], 0x03);
        assert_eq!(arr.get(0, 0, 1), 3);
    }

    #[test]
    fn nibble_max_coord_x_15_y_15_z_15() {
        let mut arr = NibbleArray::zeros();
        arr.set(15, 15, 15, 0xC);
        let linear = NibbleArray::index(15, 15, 15);
        assert_eq!(linear, 0xFFF);
        let byte_index = linear >> 1;
        assert_eq!(byte_index, 0x7FF);
        assert_eq!(byte_index, 2047);
        assert_eq!(linear & 1, 1);
        assert_eq!(arr.0[2047], 0xC0);
        assert_eq!(arr.get(15, 15, 15), 0xC);
    }

    #[test]
    fn nibble_filled_constructor() {
        let arr = NibbleArray::filled(0x07);
        for i in 0..2048 {
            assert_eq!(arr.0[i], 0x77, "byte {i} should be 0x77");
        }
        assert_eq!(arr.get(0, 0, 0), 0x07);
        assert_eq!(arr.get(15, 15, 15), 0x07);
    }

    #[test]
    fn nibble_round_trip_random_coordinates() {
        let cells: [(usize, usize, usize, u8); 6] = [
            (3, 7, 11, 0x5),
            (15, 0, 8, 0xC),
            (1, 14, 2, 0x9),
            (10, 10, 10, 0xF),
            (7, 3, 6, 0x1),
            (8, 8, 8, 0x4),
        ];
        let mut arr = NibbleArray::zeros();
        for &(x, y, z, v) in &cells {
            arr.set(x, y, z, v);
        }
        for &(x, y, z, v) in &cells {
            assert_eq!(arr.get(x, y, z), v, "mismatch at ({x}, {y}, {z})");
        }
    }

    #[test]
    fn nibble_independent_bytes() {
        let mut arr = NibbleArray::zeros();
        arr.set(0, 0, 0, 0x6);
        arr.set(2, 0, 0, 0xB);
        assert_eq!(arr.get(0, 0, 0), 0x6);
        assert_eq!(arr.get(2, 0, 0), 0xB);
        assert_eq!(arr.get(1, 0, 0), 0x0);
        assert_eq!(arr.get(3, 0, 0), 0x0);
    }

    #[test]
    fn nibble_clone_is_deep() {
        let mut a = NibbleArray::zeros();
        a.set(5, 5, 5, 0xE);
        let b = a.clone();
        a.set(5, 5, 5, 0x1);
        assert_eq!(a.get(5, 5, 5), 0x1);
        assert_eq!(b.get(5, 5, 5), 0xE);
    }
}
