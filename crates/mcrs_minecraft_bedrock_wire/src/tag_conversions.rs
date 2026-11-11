use crate::be_i32::BeI32;
use crate::var_int::{VarI32, VarU32};

macro_rules! i64_conversions {
    ($($name:ident($int:ty)),*) => {$(
        impl From<$name> for i64 {
            fn from(value: $name) -> Self {
                i64::from(value.0)
            }
        }

        impl TryFrom<i64> for $name {
            type Error = core::num::TryFromIntError;

            fn try_from(value: i64) -> Result<Self, Self::Error> {
                <$int>::try_from(value).map(Self)
            }
        }
    )*};
}

i64_conversions!(VarI32(i32), VarU32(u32), BeI32(i32));
