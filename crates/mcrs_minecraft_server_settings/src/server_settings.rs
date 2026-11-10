use alloc::borrow::ToOwned;
use alloc::string::String;
use core::num::NonZeroUsize;

use mcrs_minecraft_game_mode::GameMode;

use crate::setting_error::SettingError;

/// The settings of a server run, each named by its `MCRS_` variable without the prefix. An unset
/// variable leaves the default.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ServerSettings {
    /// `PORT`: the port to listen on; `None` keeps the port of the network plugin.
    pub port: Option<u16>,
    /// `COMPUTE_THREADS`: the threads of the compute pool; `None` keeps the default size.
    pub compute_threads: Option<NonZeroUsize>,
    /// `WORLD_SEED`: written signed, as the Java long it is, or unsigned; the router hashes the
    /// same bits either way.
    pub world_seed: u64,
    /// `WORLD_PRESET`: a short name (`normal`) or a namespaced one (`minecraft:normal`), in
    /// lower case.
    pub world_preset: String,
    /// `DEFAULT_GAMEMODE`: `survival`, `creative`, `adventure` or `spectator`, in any case.
    pub default_game_mode: GameMode,
    /// `MOB_AI`: whether the mobs think.
    pub mob_ai: bool,
    /// `LAN_ANNOUNCE`: whether the server announces itself on the local network.
    pub lan_announce: bool,
    /// `COMPRESSION_THRESHOLD`: the packet size in bytes from which packets are compressed; a
    /// negative value turns compression off. `None` keeps the default.
    pub compression_threshold: Option<i32>,
    /// `ACCEPT_TRANSFERS`: whether the server accepts players that another server transfers.
    pub accept_transfers: bool,
    /// `ENABLE_STATUS`: whether the server answers status requests.
    pub enable_status: bool,
    /// `HIDE_ONLINE_PLAYERS`: whether the status hides the names of the players.
    pub hide_online_players: bool,
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
        }
    }
}

impl ServerSettings {
    /// Reads each setting through `lookup`, which takes a name without the `MCRS_` prefix.
    /// `report` gets each value that does not read; that setting keeps its default.
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
        }
    }
}

struct Reader<'a, L, R> {
    lookup: &'a L,
    report: &'a mut R,
}

impl<L: Fn(&str) -> Option<String>, R: FnMut(SettingError)> Reader<'_, L, R> {
    /// The value of `name`, trimmed, through `parse`; `None` when it is unset or does not read.
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

    /// `on`, `1`, `true` and `yes` turn a setting on; `off`, `0`, `false` and `no` turn it off,
    /// in any case.
    fn flag(&mut self, name: &'static str, default: bool) -> bool {
        self.read(name, "on or off", |raw| {
            match raw.to_ascii_lowercase().as_str() {
                "on" | "1" | "true" | "yes" => Some(true),
                "off" | "0" | "false" | "no" => Some(false),
                _ => None,
            }
        })
        .unwrap_or(default)
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
            }
        );
        assert_eq!(rejected, ["ENABLE_STATUS"]);
    }

    #[test]
    fn values_out_of_range_are_reported() {
        let (settings, rejected) = read(&[
            ("PORT", "0"),
            ("COMPUTE_THREADS", "0"),
            ("WORLD_SEED", "18446744073709551615"),
            ("DEFAULT_GAMEMODE", "hardcore"),
            ("MOB_AI", "2"),
        ]);
        assert_eq!(settings.world_seed, u64::MAX, "an unsigned seed reads");
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
            ["PORT", "COMPUTE_THREADS", "DEFAULT_GAMEMODE", "MOB_AI"]
        );
    }
}
