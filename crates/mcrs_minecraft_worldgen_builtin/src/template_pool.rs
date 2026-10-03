mod abandoned_camp;
mod ancient_city;
mod bastion;
mod pillager_outpost;
mod trail_ruins;
mod trial_chambers;
mod village_common;
mod village_desert;
mod village_plains;
mod village_savanna;
mod village_snowy;
mod village_taiga;

use crate::keys::{Id, PlacedKey, ProcessorsKey};
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ResourceLocation, rl};
use mcrs_minecraft_worldgen_feature::template::Projection;
use mcrs_minecraft_worldgen_structure::{PoolElement, PoolEntry, SingleElement, TemplatePool};

#[derive(Clone, Copy)]
pub enum Piece {
    Legacy(Id),
    LegacyWith(Id, ProcessorsKey),
    Single(Id),
    SingleWith(Id, ProcessorsKey),
    Feature(PlacedKey),
    List(&'static [Piece]),
    Empty,
}

pub struct Pool {
    name: Id,
    fallback: Id,
    projection: Projection,
    pieces: &'static [(Piece, i32)],
}

const EMPTY: &[Pool] = &[Pool {
    name: rl!("minecraft:empty"),
    fallback: rl!("minecraft:empty"),
    projection: Projection::Rigid,
    pieces: &[],
}];

const TABLES: [&[Pool]; 12] = [
    EMPTY,
    ancient_city::POOLS,
    bastion::POOLS,
    pillager_outpost::POOLS,
    trail_ruins::POOLS,
    trial_chambers::POOLS,
    village_common::POOLS,
    village_desert::POOLS,
    village_plains::POOLS,
    village_savanna::POOLS,
    village_snowy::POOLS,
    village_taiga::POOLS,
];

fn single(
    location: impl Into<ResourceLocation>,
    processors: Option<ProcessorsKey>,
    projection: Projection,
) -> SingleElement {
    SingleElement::new(location, processors.map(Into::into), projection)
}

fn element(piece: Piece, projection: Projection) -> PoolElement {
    match piece {
        Piece::Legacy(location) => PoolElement::LegacySingle(single(location, None, projection)),
        Piece::LegacyWith(location, processors) => {
            PoolElement::LegacySingle(single(location, Some(processors), projection))
        }
        Piece::Single(location) => PoolElement::Single(single(location, None, projection)),
        Piece::SingleWith(location, processors) => {
            PoolElement::Single(single(location, Some(processors), projection))
        }
        Piece::Feature(feature) => PoolElement::Feature {
            feature: feature.into(),
            projection,
        },
        Piece::List(pieces) => PoolElement::List {
            elements: pieces
                .iter()
                .map(|piece| element(*piece, projection))
                .collect(),
            projection,
        },
        Piece::Empty => PoolElement::Empty {},
    }
}

fn pool(fallback: Id, entries: impl IntoIterator<Item = (PoolElement, i32)>) -> TemplatePool {
    let entry = |(element, weight)| PoolEntry {
        element,
        weight: Bounded(weight),
    };
    TemplatePool {
        fallback: fallback.into(),
        elements: entries.into_iter().map(entry).collect(),
    }
}

fn listed() -> impl Iterator<Item = &'static Pool> {
    TABLES.iter().flat_map(|table| table.iter())
}

pub fn keys() -> impl Iterator<Item = ResourceLocation> {
    listed()
        .map(|pool| pool.name.into())
        .chain(abandoned_camp::keys())
}

pub fn build(id: &ResourceLocation) -> Option<TemplatePool> {
    let Some(found) = listed().find(|pool| pool.name == *id) else {
        return abandoned_camp::build(id);
    };
    let entries = found
        .pieces
        .iter()
        .map(|(piece, weight)| (element(*piece, found.projection), *weight));
    Some(pool(found.fallback, entries))
}
