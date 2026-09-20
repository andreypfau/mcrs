use std::io::Write;

use anyhow::ensure;
use bytes::Bytes;
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_registry::RegistryLookup;

use crate::item::component::ItemReg;
use crate::item::ctx::{DecodeCtx, EncodeCtx, Opaque};
use crate::item::patch::ComponentMap;
use crate::item::stack::Slot;
use crate::{Decode, Encode, VarInt};

/// An item a trade takes, matched by exact component values.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemCost {
    pub item: ResourceKey<ItemReg>,
    pub count: i32,
    pub components: ComponentMap,
}

impl EncodeCtx for ItemCost {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        self.item.encode_ctx(ctx, &mut w)?;
        VarInt(self.count).encode(&mut w)?;
        self.components.encode_ctx(ctx, w)
    }
}

impl<'a> DecodeCtx<'a> for ItemCost {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        Ok(Self {
            item: ResourceKey::decode_ctx(ctx, r)?,
            count: VarInt::decode(r)?.0,
            components: ComponentMap::decode_ctx(ctx, r)?,
        })
    }
}

/// Out of stock is `uses >= max_uses`; the wire carries the flag and the
/// reader clamps `uses` to it, as vanilla does.
#[derive(Clone, Debug, PartialEq)]
pub struct MerchantOffer {
    pub cost_a: ItemCost,
    pub result: Slot,
    pub cost_b: Option<ItemCost>,
    pub uses: i32,
    pub max_uses: i32,
    pub xp: i32,
    pub special_price_diff: i32,
    pub price_multiplier: f32,
    pub demand: i32,
}

impl MerchantOffer {
    pub fn is_out_of_stock(&self) -> bool {
        self.uses >= self.max_uses
    }
}

impl EncodeCtx for MerchantOffer {
    fn encode_ctx(&self, ctx: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        ensure!(!self.result.is_empty(), "Empty ItemStack not allowed");
        self.cost_a.encode_ctx(ctx, &mut w)?;
        self.result.encode_ctx(ctx, &mut w)?;
        self.cost_b.encode_ctx(ctx, &mut w)?;
        self.is_out_of_stock().encode(&mut w)?;
        self.uses.encode(&mut w)?;
        self.max_uses.encode(&mut w)?;
        self.xp.encode(&mut w)?;
        self.special_price_diff.encode(&mut w)?;
        self.price_multiplier.encode(&mut w)?;
        self.demand.encode(w)
    }
}

impl<'a> DecodeCtx<'a> for MerchantOffer {
    fn decode_ctx(ctx: &dyn RegistryLookup, r: &mut &'a [u8]) -> anyhow::Result<Self> {
        let cost_a = ItemCost::decode_ctx(ctx, r)?;
        let result = Slot::decode_ctx(ctx, r)?;
        ensure!(!result.is_empty(), "Empty ItemStack not allowed");
        let cost_b = Option::decode_ctx(ctx, r)?;
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

/// The exact bytes of one offer, carried the way `RawStack` carries a stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawMerchantOffer(pub Bytes);

impl RawMerchantOffer {
    pub fn resolve(&self, ctx: &dyn RegistryLookup) -> anyhow::Result<MerchantOffer> {
        let mut r = &self.0[..];
        let offer = MerchantOffer::decode_ctx(ctx, &mut r)?;
        ensure!(r.is_empty(), "{} trailing bytes after an offer", r.len());
        Ok(offer)
    }

    pub fn from_offer(offer: &MerchantOffer, ctx: &dyn RegistryLookup) -> anyhow::Result<Self> {
        let mut bytes = Vec::new();
        offer.encode_ctx(ctx, &mut bytes)?;
        Ok(RawMerchantOffer(bytes.into()))
    }
}

impl Encode for RawMerchantOffer {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        Ok(w.write_all(&self.0)?)
    }
}

impl Decode<'_> for RawMerchantOffer {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let start = *r;
        MerchantOffer::decode_ctx(&Opaque, r)?;
        Ok(RawMerchantOffer(Bytes::copy_from_slice(
            &start[..start.len() - r.len()],
        )))
    }
}
