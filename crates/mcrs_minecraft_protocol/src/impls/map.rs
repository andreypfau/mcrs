use std::collections::BTreeMap;
use std::io::Write;

use anyhow::ensure;

use crate::{Decode, Encode, VarInt};

impl<K, V> Encode for BTreeMap<K, V>
where
    K: Encode,
    V: Encode,
{
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        let len = self.len();

        ensure!(
            len <= i32::MAX as usize,
            "length of B-tree map ({len}) exceeds i32::MAX"
        );

        VarInt(len as i32).encode(&mut w)?;

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
    fn decode(r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let len = VarInt::decode(r)?.0;
        ensure!(
            len >= 0,
            "attempt to decode B-tree map with negative length"
        );
        let len = len as usize;

        let mut map = BTreeMap::new();

        for _ in 0..len {
            ensure!(
                map.insert(K::decode(r)?, V::decode(r)?).is_none(),
                "encountered duplicate key while decoding B-tree map"
            );
        }

        Ok(map)
    }
}
