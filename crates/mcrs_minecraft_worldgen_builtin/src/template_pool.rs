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
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_registry::Built;
use mcrs_minecraft_worldgen_feature::keys::TEMPLATE_POOL;
use mcrs_minecraft_worldgen_feature::pool::{PoolElement, PoolEntry, SingleElement, TemplatePool};
use mcrs_minecraft_worldgen_feature::proto::StructureProcessorList;
use mcrs_minecraft_worldgen_feature::template::Projection;
use std::collections::{BTreeMap, BTreeSet};

type ProcessorListKey = ResourceKey<StructureProcessorList, &'static str>;

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

const fn template(legacy: bool, name: &'static str) -> Piece {
    Piece::Template(Template {
        legacy,
        name,
        numbers: None,
        processors: None,
    })
}

pub const fn legacy(name: &'static str) -> Piece {
    template(true, name)
}

pub const fn single(name: &'static str) -> Piece {
    template(false, name)
}

impl Piece {
    const fn template(self) -> Template {
        match self {
            Piece::Template(template) => template,
            _ => panic!("only a template row takes processors or numbers"),
        }
    }

    pub const fn with(self, processors: ProcessorListKey) -> Self {
        let mut template = self.template();
        template.processors = Some(processors);
        Piece::Template(template)
    }

    pub const fn numbered(self, lo: u32, hi: u32) -> Self {
        self.padded(lo, hi, 0)
    }

    pub const fn padded(self, lo: u32, hi: u32, width: usize) -> Self {
        let mut template = self.template();
        template.numbers = Some((lo, hi, width));
        Piece::Template(template)
    }
}

/// A pool whose fallback is `empty`, whose pieces are rigid, sit at the root
/// and take no processors, until a builder call says otherwise.
pub struct Pool {
    name: &'static str,
    fallback: &'static str,
    projection: Projection,
    dir: &'static str,
    processors: Option<ProcessorListKey>,
    pieces: &'static [(Piece, i32)],
}

pub const fn pool(name: &'static str) -> Pool {
    Pool {
        name,
        fallback: "empty",
        projection: Projection::Rigid,
        dir: "",
        processors: None,
        pieces: &[],
    }
}

impl Pool {
    pub const fn fallback(mut self, fallback: &'static str) -> Self {
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

    fn elements(&self, piece: Piece, out: &mut Vec<PoolElement>) {
        let projection = self.projection;
        match piece {
            Piece::Template(template) => {
                let mut push = |name: std::fmt::Arguments| {
                    let location = ResourceLocation::minecraft(&format!("{}{name}", self.dir))
                        .expect("a hardcoded name");
                    let processors = template.processors.or(self.processors);
                    let element =
                        SingleElement::new(location, processors.map(Into::into), projection);
                    out.push(if template.legacy {
                        PoolElement::LegacySingle(element)
                    } else {
                        PoolElement::Single(element)
                    });
                };
                match template.numbers {
                    None => push(format_args!("{}", template.name)),
                    Some((lo, hi, width)) => {
                        for n in lo..=hi {
                            push(format_args!("{}{n:0width$}", template.name));
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
                    self.elements(*piece, &mut elements);
                }
                out.push(PoolElement::List {
                    elements,
                    projection,
                });
            }
            Piece::Empty => out.push(PoolElement::Empty {}),
        }
    }

    fn build(&self) -> TemplatePool {
        let mut entries = Vec::new();
        for (piece, weight) in self.pieces {
            let mut elements = Vec::new();
            self.elements(*piece, &mut elements);
            entries.extend(elements.into_iter().map(|element| (element, *weight)));
        }
        entries_pool(self.fallback, entries)
    }
}

const EMPTY: &[Pool] = &[pool("empty")];

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

fn entries_pool(
    fallback: &str,
    entries: impl IntoIterator<Item = (PoolElement, i32)>,
) -> TemplatePool {
    let entry = |(element, weight)| PoolEntry {
        element,
        weight: Bounded::new(weight).expect("a pool weight is within 1..=150"),
    };
    TemplatePool {
        fallback: ResourceLocation::minecraft(fallback).expect("a hardcoded name"),
        elements: entries.into_iter().map(entry).collect(),
    }
}

fn listed() -> impl Iterator<Item = &'static Pool> {
    TABLES.iter().flat_map(|table| table.iter())
}

fn keys() -> impl Iterator<Item = ResourceLocation> {
    listed()
        .map(|pool| ResourceLocation::minecraft(pool.name).expect("a hardcoded name"))
        .chain(abandoned_camp::keys())
}

pub fn all() -> impl Iterator<Item = (ResourceLocation, TemplatePool)> {
    listed()
        .map(|pool| {
            (
                ResourceLocation::minecraft(pool.name).expect("a hardcoded name"),
                pool.build(),
            )
        })
        .chain(abandoned_camp::all())
}

pub fn built() -> Built {
    Built::new(
        TEMPLATE_POOL.location(),
        keys().collect::<BTreeSet<_>>().into_iter().collect(),
        |_| Ok(all().collect::<BTreeMap<_, _>>().into_values().collect()),
    )
}
