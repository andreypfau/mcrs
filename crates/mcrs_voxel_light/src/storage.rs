use crate::nibble::LightNibbles;

#[derive(Clone, Debug, Default)]
pub enum LightStorage {
    #[default]
    Empty,
    Uniform(u8),
    Dense(Box<LightNibbles>),
}

impl LightStorage {
    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        match self {
            LightStorage::Empty => 0,
            LightStorage::Uniform(v) => *v,
            LightStorage::Dense(arr) => arr.get(x, y, z),
        }
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, val: u8) {
        debug_assert!(val < 16);
        match self {
            LightStorage::Empty => {
                if val != 0 {
                    let mut arr = LightNibbles::zeros();
                    arr.set(x, y, z, val);
                    *self = LightStorage::Dense(Box::new(arr));
                }
            }
            LightStorage::Uniform(current) => {
                if *current == val {
                    return;
                }
                let mut arr = LightNibbles::filled(*current);
                arr.set(x, y, z, val);
                *self = LightStorage::Dense(Box::new(arr));
            }
            LightStorage::Dense(arr) => {
                arr.set(x, y, z, val);
            }
        }
    }

    pub fn compact(self) -> Self {
        match self {
            LightStorage::Dense(arr) => {
                let bytes = &arr.0;
                if bytes.iter().all(|&b| b == 0x00) {
                    return LightStorage::Empty;
                }
                let first = bytes[0];
                let low = first & 0x0F;
                let high = (first >> 4) & 0x0F;
                if low == high {
                    let packed = first;
                    if bytes.iter().all(|&b| b == packed) {
                        return LightStorage::Uniform(low);
                    }
                }
                LightStorage::Dense(arr)
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_null() {
        let s = LightStorage::default();
        assert!(matches!(s, LightStorage::Empty));
    }

    #[test]
    fn null_set_zero_stays_null() {
        let mut s = LightStorage::Empty;
        s.set(0, 0, 0, 0);
        assert!(matches!(s, LightStorage::Empty));
        s.set(7, 3, 9, 0);
        assert!(matches!(s, LightStorage::Empty));
    }

    #[test]
    fn null_set_nonzero_becomes_mixed_with_single_cell() {
        let mut s = LightStorage::Empty;
        s.set(3, 7, 11, 9);
        assert!(matches!(s, LightStorage::Dense(_)));
        assert_eq!(s.get(3, 7, 11), 9, "written cell holds the value");
        assert_eq!(s.get(0, 0, 0), 0, "other cells remain zero");
        assert_eq!(s.get(15, 15, 15), 0, "other cells remain zero");
    }

    #[test]
    fn uniform_set_same_stays_uniform() {
        let mut s = LightStorage::Uniform(5);
        s.set(0, 0, 0, 5);
        assert!(matches!(s, LightStorage::Uniform(5)));
        s.set(10, 10, 10, 5);
        assert!(matches!(s, LightStorage::Uniform(5)));
    }

    #[test]
    fn uniform_set_different_becomes_mixed_correctly() {
        let mut s = LightStorage::Uniform(5);
        s.set(0, 0, 0, 9);
        assert!(matches!(s, LightStorage::Dense(_)));
        assert_eq!(s.get(0, 0, 0), 9);
        assert_eq!(s.get(1, 2, 3), 5);
        assert_eq!(s.get(15, 15, 15), 5);
    }

    #[test]
    fn mixed_set_writes_through() {
        let mut s = LightStorage::Uniform(5);
        s.set(0, 0, 0, 9);
        s.set(4, 4, 4, 2);
        assert!(matches!(s, LightStorage::Dense(_)));
        assert_eq!(s.get(4, 4, 4), 2);
        assert_eq!(s.get(0, 0, 0), 9);
        assert_eq!(s.get(7, 8, 9), 5);
    }

    #[test]
    fn compact_null_passthrough() {
        let s = LightStorage::Empty.compact();
        assert!(matches!(s, LightStorage::Empty));
    }

    #[test]
    fn compact_uniform_passthrough() {
        let s = LightStorage::Uniform(11).compact();
        assert!(matches!(s, LightStorage::Uniform(11)));
    }

    #[test]
    fn compact_mixed_all_zero_becomes_null() {
        let arr = LightNibbles::zeros();
        let s = LightStorage::Dense(Box::new(arr)).compact();
        assert!(matches!(s, LightStorage::Empty));
    }

    #[test]
    fn compact_mixed_all_uniform_becomes_uniform_n() {
        let arr = LightNibbles::filled(8);
        let s = LightStorage::Dense(Box::new(arr)).compact();
        assert!(matches!(s, LightStorage::Uniform(8)));
    }

    #[test]
    fn compact_mixed_heterogeneous_stays_mixed() {
        let mut arr = LightNibbles::filled(3);
        arr.set(5, 5, 5, 12);
        arr.set(10, 1, 2, 7);
        let s = LightStorage::Dense(Box::new(arr)).compact();
        match s {
            LightStorage::Dense(a) => {
                assert_eq!(a.get(5, 5, 5), 12);
                assert_eq!(a.get(10, 1, 2), 7);
                assert_eq!(a.get(0, 0, 0), 3);
            }
            _ => panic!("expected Mixed"),
        }
    }

    #[test]
    fn get_returns_zero_on_null() {
        let s = LightStorage::Empty;
        assert_eq!(s.get(0, 0, 0), 0);
        assert_eq!(s.get(15, 15, 15), 0);
        assert_eq!(s.get(7, 3, 11), 0);
    }

    #[test]
    fn uniform_get_returns_constant_for_all_coords() {
        let s = LightStorage::Uniform(6);
        assert_eq!(s.get(0, 0, 0), 6);
        assert_eq!(s.get(15, 15, 15), 6);
        assert_eq!(s.get(1, 7, 3), 6);
    }
}
