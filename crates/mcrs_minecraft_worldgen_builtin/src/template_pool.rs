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

use mcrs_minecraft_biome_file::PlacedFeatureKey;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_resource::{
    InvalidResourceLocation, ResourceKey, ResourceLocation, ResourceLocationBuf, rl,
};
use mcrs_minecraft_worldgen_feature::pool::{PoolElement, PoolEntry, SingleElement, TemplatePool};
use mcrs_minecraft_worldgen_feature::proto::StructureProcessorList;
use mcrs_minecraft_worldgen_feature::template::Projection;

type ProcessorListKey = ResourceKey<'static, StructureProcessorList>;
type Weight = Bounded<1, 150>;

/// A template named relative to its pool's directory. `numbers` makes one row
/// stand for the templates `name<lo>` to `name<hi>`, zero-padded to a width.
#[derive(Clone, Copy)]
pub struct Template {
    legacy: bool,
    name: &'static str,
    numbers: Option<(u32, u32, usize)>,
    processors: Option<ProcessorListKey>,
}

#[derive(Clone, Copy)]
pub enum Piece {
    Template(Template),
    Feature(PlacedFeatureKey),
    List(&'static [Piece]),
    Empty,
}

const fn template(legacy: bool, name: &'static str) -> Template {
    Template {
        legacy,
        name,
        numbers: None,
        processors: None,
    }
}

pub const fn legacy(name: &'static str) -> Template {
    template(true, name)
}

pub const fn single(name: &'static str) -> Template {
    template(false, name)
}

impl Template {
    pub const fn with(mut self, processors: ProcessorListKey) -> Self {
        self.processors = Some(processors);
        self
    }

    pub const fn numbered(self, lo: u32, hi: u32) -> Self {
        self.padded(lo, hi, 0)
    }

    pub const fn padded(mut self, lo: u32, hi: u32, width: usize) -> Self {
        self.numbers = Some((lo, hi, width));
        self
    }
}

/// A pool whose fallback is `empty`, whose pieces are rigid, sit at the root
/// and take no processors, until a builder call says otherwise.
pub struct Pool {
    name: ResourceLocation<'static>,
    fallback: ResourceLocation<'static>,
    projection: Projection,
    dir: &'static str,
    processors: Option<ProcessorListKey>,
    pieces: &'static [(Piece, i32)],
}

pub const fn pool(name: ResourceLocation<'static>) -> Pool {
    Pool {
        name,
        fallback: rl!("minecraft:empty"),
        projection: Projection::Rigid,
        dir: "",
        processors: None,
        pieces: &[],
    }
}

impl Pool {
    pub const fn fallback(mut self, fallback: ResourceLocation<'static>) -> Self {
        self.fallback = fallback;
        self
    }

    pub const fn terrain_matching(mut self) -> Self {
        self.projection = Projection::TerrainMatching;
        self
    }

    pub const fn dir(mut self, dir: &'static str) -> Self {
        self.dir = dir;
        self
    }

    pub const fn processors(mut self, processors: ProcessorListKey) -> Self {
        self.processors = Some(processors);
        self
    }

    pub const fn pieces(mut self, pieces: &'static [(Piece, i32)]) -> Self {
        self.pieces = pieces;
        self
    }

    fn elements(
        &self,
        piece: Piece,
        out: &mut Vec<PoolElement>,
    ) -> Result<(), InvalidResourceLocation> {
        let projection = self.projection;
        match piece {
            Piece::Template(template) => {
                let mut push = |name: std::fmt::Arguments| {
                    let location = ResourceLocationBuf::minecraft(&format!("{}{name}", self.dir))?;
                    let processors = template.processors.or(self.processors);
                    let element =
                        SingleElement::new(location, processors.map(Into::into), projection);
                    out.push(if template.legacy {
                        PoolElement::LegacySingle(element)
                    } else {
                        PoolElement::Single(element)
                    });
                    Ok(())
                };
                match template.numbers {
                    None => push(format_args!("{}", template.name))?,
                    Some((lo, hi, width)) => {
                        for n in lo..=hi {
                            push(format_args!("{}{n:0width$}", template.name))?;
                        }
                    }
                }
            }
            Piece::Feature(feature) => out.push(PoolElement::Feature {
                feature: feature.into(),
                projection,
            }),
            Piece::List(pieces) => {
                let mut elements = Vec::new();
                for piece in pieces {
                    self.elements(*piece, &mut elements)?;
                }
                out.push(PoolElement::List {
                    elements,
                    projection,
                });
            }
            Piece::Empty => out.push(PoolElement::Empty {}),
        }
        Ok(())
    }

    fn build(&self) -> Result<TemplatePool, InvalidResourceLocation> {
        let mut entries = Vec::new();
        for (piece, weight) in self.pieces {
            let mut elements = Vec::new();
            self.elements(*piece, &mut elements)?;
            // `weights_in_range` checks every weight of a table when the crate compiles.
            let weight = Bounded(*weight);
            entries.extend(elements.into_iter().map(|element| (element, weight)));
        }
        Ok(entries_pool(self.fallback, entries))
    }
}

const EMPTY: &[Pool] = &[pool(rl!("minecraft:empty"))];

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

const fn weights_in_range(mut tables: &[&[Pool]]) -> bool {
    while let [pools, rest @ ..] = tables {
        let mut pools: &[Pool] = pools;
        while let [pool, rest @ ..] = pools {
            let mut pieces = pool.pieces;
            while let [(_, weight), rest @ ..] = pieces {
                if *weight < 1 || *weight > 150 {
                    return false;
                }
                pieces = rest;
            }
            pools = rest;
        }
        tables = rest;
    }
    true
}

const _: () = assert!(weights_in_range(&TABLES), "a pool weight is within 1..=150");

fn entries_pool(
    fallback: ResourceLocation<'static>,
    entries: impl IntoIterator<Item = (PoolElement, Weight)>,
) -> TemplatePool {
    TemplatePool {
        fallback: fallback.into(),
        elements: entries
            .into_iter()
            .map(|(element, weight)| PoolEntry { element, weight })
            .collect(),
    }
}

fn listed() -> impl Iterator<Item = &'static Pool> {
    TABLES.iter().flat_map(|table| table.iter())
}

pub fn all()
-> impl Iterator<Item = Result<(ResourceLocationBuf, TemplatePool), InvalidResourceLocation>> {
    listed()
        .map(|pool| Ok((pool.name.into(), pool.build()?)))
        .chain(abandoned_camp::all())
}
