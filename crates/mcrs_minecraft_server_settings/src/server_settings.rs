use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;
use core::net::IpAddr;
use core::num::NonZeroUsize;

use mcrs_minecraft_game_mode::GameMode;

use crate::setting_error::SettingError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RedstonePreset {
    #[default]
    Java,
    Bedrock,
}

impl RedstonePreset {
    fn parse(raw: &str) -> Option<Self> {
        match raw.to_ascii_lowercase().as_str() {
            "java" => Some(Self::Java),
            "bedrock" => Some(Self::Bedrock),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RedstoneSettings {
    pub preset: RedstonePreset,
    pub quasi_connectivity: Option<bool>,
    pub sticky_piston_short_pulse: Option<bool>,
    pub pistons_move_block_entities: Option<bool>,
    pub dust_connects_to_pistons: Option<bool>,
    pub experimental_wire: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ServerSettings {
    pub port: Option<u16>,
    pub compute_threads: Option<NonZeroUsize>,
    pub world_seed: u64,
    pub world_preset: String,
    pub default_game_mode: GameMode,
    pub mob_ai: bool,
    pub lan_announce: bool,
    pub compression_threshold: Option<i32>,
    pub accept_transfers: bool,
    pub enable_status: bool,
    pub hide_online_players: bool,
    pub forwarding: bool,
    pub forwarding_secret: String,
    pub proxy_upstreams: Vec<IpAddr>,
    pub redstone: RedstoneSettings,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            port: None,
            compute_threads: None,
            world_seed: 0,
            world_preset: "normal".to_owned(),
            default_game_mode: GameMode::Creative,
            mob_ai: true,
            lan_announce: true,
            compression_threshold: None,
            accept_transfers: false,
            enable_status: true,
            hide_online_players: false,
            forwarding: false,
            forwarding_secret: String::new(),
            proxy_upstreams: Vec::new(),
            redstone: RedstoneSettings::default(),
        }
    }
}

impl ServerSettings {
    pub fn read(
        lookup: impl Fn(&str) -> Option<String>,
        mut report: impl FnMut(SettingError),
    ) -> Self {
        let defaults = Self::default();
        let mut reader = Reader {
            lookup: &lookup,
            report: &mut report,
        };
        Self {
            port: reader.read("PORT", "a port from 1 to 65535", positive::<u16>),
            compute_threads: reader.read("COMPUTE_THREADS", "a positive count", |raw| {
                raw.parse().ok()
            }),
            world_seed: reader
                .read("WORLD_SEED", "a signed or an unsigned 64-bit number", seed)
                .unwrap_or(defaults.world_seed),
            world_preset: reader
                .read("WORLD_PRESET", "a preset name", |raw| {
                    (!raw.is_empty()).then(|| raw.to_lowercase())
                })
                .unwrap_or(defaults.world_preset),
            default_game_mode: reader
                .read("DEFAULT_GAMEMODE", "a game mode", |raw| {
                    raw.to_ascii_lowercase().parse().ok()
                })
                .unwrap_or(defaults.default_game_mode),
            mob_ai: reader.flag("MOB_AI", defaults.mob_ai),
            lan_announce: reader.flag("LAN_ANNOUNCE", defaults.lan_announce),
            compression_threshold: reader.read("COMPRESSION_THRESHOLD", "a whole number", |raw| {
                raw.parse().ok()
            }),
            accept_transfers: reader.flag("ACCEPT_TRANSFERS", defaults.accept_transfers),
            enable_status: reader.flag("ENABLE_STATUS", defaults.enable_status),
            hide_online_players: reader.flag("HIDE_ONLINE_PLAYERS", defaults.hide_online_players),
            forwarding: reader.flag("FORWARDING", defaults.forwarding),
            forwarding_secret: lookup("FORWARDING_SECRET").unwrap_or(defaults.forwarding_secret),
            proxy_upstreams: reader
                .read(
                    "PROXY_UPSTREAMS",
                    "IP addresses separated by commas",
                    |raw| raw.split(',').map(|ip| ip.trim().parse().ok()).collect(),
                )
                .unwrap_or(defaults.proxy_upstreams),
            redstone: RedstoneSettings {
                preset: reader
                    .read("REDSTONE_PRESET", "java or bedrock", RedstonePreset::parse)
                    .unwrap_or(defaults.redstone.preset),
                quasi_connectivity: reader.switch("REDSTONE_QUASI_CONNECTIVITY"),
                sticky_piston_short_pulse: reader.switch("REDSTONE_STICKY_PISTON_SHORT_PULSE"),
                pistons_move_block_entities: reader.switch("REDSTONE_PISTONS_MOVE_BLOCK_ENTITIES"),
                dust_connects_to_pistons: reader.switch("REDSTONE_DUST_CONNECTS_TO_PISTONS"),
                experimental_wire: reader.switch("REDSTONE_EXPERIMENTAL_WIRE"),
            },
        }
    }
}

struct Reader<'a, L, R> {
    lookup: &'a L,
    report: &'a mut R,
}

impl<L: Fn(&str) -> Option<String>, R: FnMut(SettingError)> Reader<'_, L, R> {
    fn read<T>(
        &mut self,
        name: &'static str,
        expected: &'static str,
        parse: impl FnOnce(&str) -> Option<T>,
    ) -> Option<T> {
        let value = (self.lookup)(name)?;
        let parsed = parse(value.trim());
        if parsed.is_none() {
            (self.report)(SettingError {
                name,
                value,
                expected,
            });
        }
        parsed
    }

    fn switch(&mut self, name: &'static str) -> Option<bool> {
        self.read(name, "on or off", |raw| {
            match raw.to_ascii_lowercase().as_str() {
                "on" | "1" | "true" | "yes" => Some(true),
                "off" | "0" | "false" | "no" => Some(false),
                _ => None,
            }
        })
    }

    fn flag(&mut self, name: &'static str, default: bool) -> bool {
        self.switch(name).unwrap_or(default)
    }
}

fn positive<T: core::str::FromStr + Default + PartialOrd>(raw: &str) -> Option<T> {
    raw.parse().ok().filter(|count| *count > T::default())
}

#[expect(
    clippy::cast_sign_loss,
    reason = "a negative seed keeps its bits, as the Java long does"
)]
fn seed(raw: &str) -> Option<u64> {
    raw.parse::<i64>()
        .map(|seed| seed as u64)
        .or_else(|_| raw.parse::<u64>())
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    fn read(pairs: &[(&str, &str)]) -> (ServerSettings, Vec<&'static str>) {
        let mut rejected = Vec::new();
        let settings = ServerSettings::read(
            |name| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            },
            |error| rejected.push(error.name),
        );
        (settings, rejected)
    }

    #[test]
    fn unset_settings_keep_their_defaults() {
        assert_eq!(read(&[]), (ServerSettings::default(), vec![]));
    }

    #[test]
    fn each_setting_reads_its_spellings_and_a_bad_value_keeps_the_default() {
        let (settings, rejected) = read(&[
            ("PORT", " 25570 "),
            ("COMPUTE_THREADS", "8"),
            ("WORLD_SEED", "-1"),
            ("WORLD_PRESET", " Flat "),
            ("DEFAULT_GAMEMODE", " Survival "),
            ("MOB_AI", "OFF"),
            ("LAN_ANNOUNCE", " No "),
            ("COMPRESSION_THRESHOLD", "-1"),
            ("ACCEPT_TRANSFERS", "Yes"),
            ("ENABLE_STATUS", "of f"),
            ("HIDE_ONLINE_PLAYERS", "1"),
            ("FORWARDING", "on"),
            ("FORWARDING_SECRET", " secret "),
            ("PROXY_UPSTREAMS", " 192.0.2.1, ::1 "),
            ("REDSTONE_PRESET", " Bedrock "),
            ("REDSTONE_QUASI_CONNECTIVITY", "on"),
            ("REDSTONE_STICKY_PISTON_SHORT_PULSE", "maybe"),
            ("REDSTONE_PISTONS_MOVE_BLOCK_ENTITIES", "0"),
            ("REDSTONE_EXPERIMENTAL_WIRE", "yes"),
        ]);
        assert_eq!(
            settings,
            ServerSettings {
                port: Some(25570),
                compute_threads: NonZeroUsize::new(8),
                world_seed: u64::MAX,
                world_preset: "flat".to_owned(),
                default_game_mode: GameMode::Survival,
                mob_ai: false,
                lan_announce: false,
                compression_threshold: Some(-1),
                accept_transfers: true,
                enable_status: true,
                hide_online_players: true,
                forwarding: true,
                forwarding_secret: " secret ".to_owned(),
                proxy_upstreams: vec![[192, 0, 2, 1].into(), core::net::Ipv6Addr::LOCALHOST.into()],
                redstone: RedstoneSettings {
                    preset: RedstonePreset::Bedrock,
                    quasi_connectivity: Some(true),
                    sticky_piston_short_pulse: None,
                    pistons_move_block_entities: Some(false),
                    dust_connects_to_pistons: None,
                    experimental_wire: Some(true),
                },
            }
        );
        assert_eq!(
            rejected,
            ["ENABLE_STATUS", "REDSTONE_STICKY_PISTON_SHORT_PULSE"]
        );
    }

    #[test]
    fn values_out_of_range_are_reported() {
        let (settings, rejected) = read(&[
            ("PORT", "0"),
            ("COMPUTE_THREADS", "0"),
            ("WORLD_SEED", "18446744073709551615"),
            ("DEFAULT_GAMEMODE", "hardcore"),
            ("MOB_AI", "2"),
            ("FORWARDING", "enabled"),
            ("PROXY_UPSTREAMS", "192.0.2.1,localhost"),
            ("REDSTONE_PRESET", "pocket"),
        ]);
        assert_eq!(settings.world_seed, u64::MAX, "an unsigned seed reads");
        assert_eq!(settings.redstone.preset, RedstonePreset::Java);
        assert_eq!(
            (
                settings.port,
                settings.compute_threads,
                settings.default_game_mode,
                settings.mob_ai
            ),
            (None, None, GameMode::Creative, true)
        );
        assert_eq!(
            rejected,
            [
                "PORT",
                "COMPUTE_THREADS",
                "DEFAULT_GAMEMODE",
                "MOB_AI",
                "FORWARDING",
                "PROXY_UPSTREAMS",
                "REDSTONE_PRESET"
            ]
        );
    }
}
