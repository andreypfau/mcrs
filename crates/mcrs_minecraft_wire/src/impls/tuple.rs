use std::io::Write;

use crate::{Decode, Encode, Result};

macro_rules! impl_tuple {
    ($($ty:ident $idx:tt)*) => {
        impl<$($ty: Encode,)*> Encode for ($($ty,)*) {
            fn encode(&self, mut _w: impl Write) -> Result<()> {
                $(
                    self.$idx.encode(&mut _w)?;
                )*
                Ok(())
            }
        }

        impl<'a, $($ty: Decode<'a>,)*> Decode<'a> for ($($ty,)*) {
            fn decode(_r: &mut &'a [u8]) -> Result<Self> {
                Ok(($($ty::decode(_r)?,)*))
            }
        }
    }
}

impl_tuple!();
impl_tuple!(A 0);
impl_tuple!(A 0 B 1);
impl_tuple!(A 0 B 1 C 2);
impl_tuple!(A 0 B 1 C 2 D 3);
