use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_item::ProtoStack;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_registries::{FromWire, Refs, Resolved, Wire};

use crate::item::patch::ComponentMap;
use crate::{Decode, Encode, VarInt};

/// An item a trade takes, matched by exact component values.
#[derive(Clone, Debug, PartialEq, FromWire)]
pub struct ItemCost<M: Refs = Resolved> {
    pub item: M::Id<Item>,
    pub count: i32,
    pub components: ComponentMap<M>,
}

impl Encode for ItemCost<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        self.item.encode(&mut w)?;
        VarInt(self.count).encode(&mut w)?;
        self.components.encode(w)
    }
}

impl Decode<'_> for ItemCost<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        Ok(Self {
            item: Decode::decode(r)?,
            count: VarInt::decode(r)?.0,
            components: ComponentMap::decode(r)?,
        })
    }
}

/// Out of stock is `uses >= max_uses`; the wire carries the flag and the
/// reader clamps `uses` to it, as vanilla does.
#[derive(Clone, Debug, PartialEq, FromWire)]
pub struct MerchantOffer<M: Refs = Resolved> {
    pub cost_a: ItemCost<M>,
    pub result: ProtoStack<M>,
    pub cost_b: Option<ItemCost<M>>,
    pub uses: i32,
    pub max_uses: i32,
    pub xp: i32,
    pub special_price_diff: i32,
    pub price_multiplier: f32,
    pub demand: i32,
}

impl<M: Refs> MerchantOffer<M> {
    pub fn is_out_of_stock(&self) -> bool {
        self.uses >= self.max_uses
    }
}

impl Encode for MerchantOffer<Wire> {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        ensure!(!self.result.is_empty(), "Empty ItemStack not allowed");
        self.cost_a.encode(&mut w)?;
        self.result.encode(&mut w)?;
        self.cost_b.encode(&mut w)?;
        self.is_out_of_stock().encode(&mut w)?;
        self.uses.encode(&mut w)?;
        self.max_uses.encode(&mut w)?;
        self.xp.encode(&mut w)?;
        self.special_price_diff.encode(&mut w)?;
        self.price_multiplier.encode(&mut w)?;
        self.demand.encode(w)
    }
}

impl Decode<'_> for MerchantOffer<Wire> {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let cost_a = ItemCost::decode(r)?;
        let result = ProtoStack::decode(r)?;
        ensure!(!result.is_empty(), "Empty ItemStack not allowed");
        let cost_b = Option::decode(r)?;
        let out_of_stock = bool::decode(r)?;
        let uses = i32::decode(r)?;
        let max_uses = i32::decode(r)?;
        Ok(Self {
            cost_a,
            result,
            cost_b,
            uses: if out_of_stock { max_uses } else { uses },
            max_uses,
            xp: i32::decode(r)?,
            special_price_diff: i32::decode(r)?,
            price_multiplier: f32::decode(r)?,
            demand: i32::decode(r)?,
        })
    }
}
