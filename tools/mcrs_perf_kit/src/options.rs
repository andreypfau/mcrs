use std::path::PathBuf;

use anyhow::{Context, bail};
use serde::Serialize;

use crate::profile::Profile;
use crate::section::Section;

pub const USAGE: &str = "usage: perf-kit run|mob|client|server|worldgen --out <dir> [--profile quick|full] \
[--only mob,client,server,worldgen] [--repo <root>] [--window WxH] [--heap 8g] [--java <path>] [--accept-eula]";

#[derive(Clone, Debug, Serialize)]
pub struct Options {
    pub sections: Vec<Section>,
    pub out: PathBuf,
    pub profile: Profile,
    pub repo: PathBuf,
    pub window: Window,
    pub heap: String,
    pub java: String,
    pub accept_eula: bool,
    /// One run and short windows, to check the kit itself.
    pub smoke: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Window {
    pub width: u32,
    pub height: u32,
}

impl Options {
    pub fn parse(arguments: &[String]) -> anyhow::Result<Self> {
        let Some((command, rest)) = arguments.split_first() else {
            bail!("{USAGE}");
        };
        let mut sections: Vec<Section> = match command.as_str() {
            "run" => Section::ALL.to_vec(),
            name => vec![name.parse().context(USAGE)?],
        };
        let mut out = None;
        let mut profile = Profile::Quick;
        let mut repo = None;
        let mut window = Window {
            width: 1920,
            height: 1080,
        };
        let mut heap = "16g".to_owned();
        let mut java = "java".to_owned();
        let mut accept_eula = false;
        let mut smoke = false;
        let mut options = rest.iter();
        while let Some(option) = options.next() {
            match option.as_str() {
                "--accept-eula" => accept_eula = true,
                "--smoke" => smoke = true,
                flag => {
                    let Some(value) = options.next() else {
                        bail!("{flag} needs a value; {USAGE}");
                    };
                    match flag {
                        "--out" => out = Some(PathBuf::from(value)),
                        "--profile" => profile = value.parse()?,
                        "--only" => {
                            let only: Vec<Section> = value
                                .split(',')
                                .map(str::parse)
                                .collect::<anyhow::Result<_>>()?;
                            sections.retain(|section| only.contains(section));
                        }
                        "--repo" => repo = Some(PathBuf::from(value)),
                        "--window" => window = Window::parse(value)?,
                        "--heap" => heap.clone_from(value),
                        "--java" => java.clone_from(value),
                        other => bail!("unknown option {other}; {USAGE}"),
                    }
                }
            }
        }
        let Some(out) = out else {
            bail!("--out is required; {USAGE}");
        };
        let repo = match repo {
            Some(repo) => repo,
            None => std::env::current_dir()?,
        };
        let manifest = std::fs::read_to_string(repo.join("Cargo.toml")).unwrap_or_default();
        if !manifest.contains("[workspace]") {
            bail!(
                "{} is not the mcrs workspace; run from the repository root or pass --repo",
                repo.display()
            );
        }
        Ok(Self {
            sections,
            out,
            profile,
            repo,
            window,
            heap,
            java,
            accept_eula,
            smoke,
        })
    }
}

impl Window {
    fn parse(text: &str) -> anyhow::Result<Self> {
        let parsed = text
            .split_once('x')
            .and_then(|(width, height)| Some((width.parse().ok()?, height.parse().ok()?)));
        match parsed {
            Some((width, height)) => Ok(Self { width, height }),
            None => bail!("--window takes <width>x<height>, got {text}"),
        }
    }

    #[must_use]
    pub fn resolution(self) -> String {
        format!("{}x{}", self.width, self.height)
    }
}
