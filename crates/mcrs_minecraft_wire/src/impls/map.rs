use std::collections::BTreeMap;
use std::io::Write;

use super::{decode_len, encode_len};
use crate::{Decode, Encode, ErrorKind, Result};

impl<K, V> Encode for BTreeMap<K, V>
where
    K: Encode,
    V: Encode,
{
    fn encode(&self, mut w: impl Write) -> Result<()> {
        encode_len(self.len(), &mut w)?;

        for pair in self.iter() {
            pair.encode(&mut w)?;
        }

        Ok(())
    }
}

impl<'a, K, V> Decode<'a> for BTreeMap<K, V>
where
    K: Ord + Decode<'a>,
    V: Decode<'a>,
{
    fn decode(r: &mut &'a [u8]) -> Result<Self> {
        let len = decode_len(r)?;

        let mut map = BTreeMap::new();

        for _ in 0..len {
            if map.insert(K::decode(r)?, V::decode(r)?).is_some() {
                return Err(ErrorKind::DuplicateKey.into());
            }
        }

        Ok(map)
    }
}
