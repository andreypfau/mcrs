use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::{Context, bail};
use mcrs_minecraft_dimension::dimension_keys::OVERWORLD;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_voxel::ColumnPos;

pub const USAGE: &str = "usage: mcrs_minecraft_worldgen_preview \
    --assets <dir> --vanilla <dir> [--pack <dir>]... --seed <n> [--dimension <id>] \
    --from <chunk x>,<chunk z> --to <chunk x>,<chunk z> --out <dir> [--slice x=<n>|z=<n>]...";

#[derive(Clone, Copy)]
pub enum Slice {
    X(i32),
    Z(i32),
}

pub struct Arguments {
    pub assets: PathBuf,
    pub vanilla: PathBuf,
    pub packs: Vec<PathBuf>,
    pub seed: u64,
    pub dimension: ResourceLocationBuf,
    pub from: ColumnPos,
    pub to: ColumnPos,
    pub out: PathBuf,
    pub slices: Vec<Slice>,
}

impl Arguments {
    pub fn parse(arguments: impl IntoIterator<Item = OsString>) -> anyhow::Result<Self> {
        let mut assets = None;
        let mut vanilla = None;
        let mut packs = Vec::new();
        let mut seed = None;
        let mut dimension = None;
        let mut from = None;
        let mut to = None;
        let mut out = None;
        let mut slices = Vec::new();
        let mut arguments = arguments.into_iter();
        while let Some(flag) = arguments.next() {
            let flag = flag.to_string_lossy().into_owned();
            let value = arguments
                .next()
                .with_context(|| format!("{flag} needs a value\n{USAGE}"))?;
            let text = || value.to_string_lossy().into_owned();
            match flag.as_str() {
                "--assets" => assets = Some(PathBuf::from(&value)),
                "--vanilla" => vanilla = Some(PathBuf::from(&value)),
                "--pack" => packs.push(PathBuf::from(&value)),
                "--seed" => seed = Some(parse_seed(&text())?),
                "--dimension" => dimension = Some(ResourceLocationBuf::read(&text())?),
                "--from" => from = Some(parse_chunk(&text())?),
                "--to" => to = Some(parse_chunk(&text())?),
                "--out" => out = Some(PathBuf::from(&value)),
                "--slice" => slices.push(parse_slice(&text())?),
                _ => bail!("unknown argument {flag}\n{USAGE}"),
            }
        }
        let from: ColumnPos = from.context(USAGE)?;
        let to: ColumnPos = to.context(USAGE)?;
        Ok(Self {
            assets: assets.context(USAGE)?,
            vanilla: vanilla.context(USAGE)?,
            packs,
            seed: seed.context(USAGE)?,
            dimension: dimension.unwrap_or_else(|| OVERWORLD.location().into()),
            from: ColumnPos::new(from.x.min(to.x), from.z.min(to.z)),
            to: ColumnPos::new(from.x.max(to.x), from.z.max(to.z)),
            out: out.context(USAGE)?,
            slices,
        })
    }
}

fn parse_seed(text: &str) -> anyhow::Result<u64> {
    let signed: Result<i64, _> = text.parse();
    match signed {
        Ok(signed) => Ok(signed.cast_unsigned()),
        Err(_) => text
            .parse()
            .with_context(|| format!("the seed {text} is not a 64-bit integer")),
    }
}

fn parse_chunk(text: &str) -> anyhow::Result<ColumnPos> {
    let (x, z) = text
        .split_once(',')
        .with_context(|| format!("the chunk {text} is not <x>,<z>"))?;
    Ok(ColumnPos::new(x.trim().parse()?, z.trim().parse()?))
}

fn parse_slice(text: &str) -> anyhow::Result<Slice> {
    match text.split_once('=') {
        Some(("x", x)) => Ok(Slice::X(x.parse()?)),
        Some(("z", z)) => Ok(Slice::Z(z.parse()?)),
        _ => bail!("the slice {text} is not x=<block x> or z=<block z>"),
    }
}
