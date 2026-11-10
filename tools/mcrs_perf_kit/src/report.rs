use serde::Serialize;

use crate::client::{self, ClientReport};
use crate::clock::utc_now;
use crate::mob::{self, MobRun};
use crate::options::Options;
use crate::profile::Profile;
use crate::repo;
use crate::section::Section;
use crate::server_start::{self, ServerStartReport};
use crate::worldgen::{self, WorldgenBench};

#[derive(Debug, Serialize)]
pub struct Report {
    pub kit_version: &'static str,
    pub git_commit: Option<String>,
    pub profile: Profile,
    pub started_utc: String,
    pub finished_utc: String,
    pub options: Options,
    pub mob: Option<Vec<MobRun>>,
    pub client: Option<ClientReport>,
    pub server_start: Option<ServerStartReport>,
    pub worldgen: Option<Vec<WorldgenBench>>,
    pub errors: Vec<SectionError>,
}

#[derive(Debug, Serialize)]
pub struct SectionError {
    pub section: Section,
    pub message: String,
}

impl Report {
    /// Runs every selected section; a failed section is an entry of `errors` and the rest run.
    #[must_use]
    pub fn run(options: Options) -> Self {
        let mut report = Self {
            kit_version: env!("CARGO_PKG_VERSION"),
            git_commit: repo::git_commit(&options.repo),
            profile: options.profile,
            started_utc: utc_now(),
            finished_utc: String::new(),
            options,
            mob: None,
            client: None,
            server_start: None,
            worldgen: None,
            errors: Vec::new(),
        };
        let options = report.options.clone();
        for section in options.sections.iter().copied() {
            let mut messages = Vec::new();
            let outcome = match section {
                Section::Mob => mob::run(&options, &mut messages).map(|mob| report.mob = Some(mob)),
                Section::Client => {
                    client::run(&options, &mut messages).map(|client| report.client = Some(client))
                }
                Section::ServerStart => server_start::run(&options, &mut messages)
                    .map(|server| report.server_start = Some(server)),
                Section::Worldgen => {
                    report.worldgen = Some(worldgen::run(&options, &mut messages));
                    Ok(())
                }
            };
            if let Err(error) = outcome {
                messages.push(format!("{error:#}"));
            }
            for message in messages {
                println!("[{section}] error: {message}");
                report.errors.push(SectionError { section, message });
            }
        }
        report.finished_utc = utc_now();
        report
    }
}
