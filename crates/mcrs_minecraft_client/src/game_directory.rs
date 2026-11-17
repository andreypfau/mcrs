use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameDirectory {
    pub root: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Unix,
    MacOs,
    Windows,
}

impl Platform {
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Unix
        }
    }
}

impl GameDirectory {
    pub fn pick(platform: Platform, variable: impl Fn(&str) -> Option<OsString>) -> Option<Self> {
        let set = |name| {
            variable(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        let root = match set("MCRS_GAME_DIR") {
            Some(root) => root,
            None => match platform {
                Platform::Unix => set("XDG_DATA_HOME")
                    .filter(|data| data.is_absolute())
                    .or_else(|| set("HOME").map(|home| home.join(".local/share")))?
                    .join("mcrs"),
                Platform::MacOs => set("HOME")?.join("Library/Application Support/mcrs"),
                Platform::Windows => set("APPDATA")?.join("mcrs"),
            },
        };
        Some(Self { root })
    }

    pub fn options_file(&self) -> PathBuf {
        self.root.join("options.json")
    }

    pub fn command_history(&self) -> PathBuf {
        self.root.join("command_history.txt")
    }

    pub fn saves(&self) -> PathBuf {
        self.root.join("saves")
    }

    pub fn ops_file(&self) -> PathBuf {
        self.root.join("ops.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABSOLUTE_DATA: &str = if cfg!(windows) { "C:/data" } else { "/data" };
    const ABSOLUTE_DATA_ROOT: &str = if cfg!(windows) {
        "C:/data/mcrs"
    } else {
        "/data/mcrs"
    };

    #[test]
    fn the_root_is_the_override_or_the_platform_data_folder() {
        let cases: [(Platform, &[(&str, &str)], Option<&str>); 9] = [
            (
                Platform::MacOs,
                &[("MCRS_GAME_DIR", "/games/a"), ("HOME", "/u")],
                Some("/games/a"),
            ),
            (
                Platform::MacOs,
                &[("MCRS_GAME_DIR", ""), ("HOME", "/u")],
                Some("/u/Library/Application Support/mcrs"),
            ),
            (Platform::MacOs, &[], None),
            (
                Platform::Unix,
                &[("XDG_DATA_HOME", ABSOLUTE_DATA), ("HOME", "/u")],
                Some(ABSOLUTE_DATA_ROOT),
            ),
            (
                Platform::Unix,
                &[("XDG_DATA_HOME", "data"), ("HOME", "/u")],
                Some("/u/.local/share/mcrs"),
            ),
            (
                Platform::Unix,
                &[("HOME", "/u")],
                Some("/u/.local/share/mcrs"),
            ),
            (Platform::Unix, &[("MCRS_GAME_DIR", "/g")], Some("/g")),
            (
                Platform::Windows,
                &[("APPDATA", "/roaming"), ("HOME", "/u")],
                Some("/roaming/mcrs"),
            ),
            (Platform::Windows, &[("HOME", "/u")], None),
        ];
        for (platform, variables, root) in cases {
            let picked = GameDirectory::pick(platform, |name| {
                variables
                    .iter()
                    .find(|(set, _)| *set == name)
                    .map(|(_, value)| OsString::from(value))
            });
            assert_eq!(
                picked.map(|directory| directory.root),
                root.map(PathBuf::from),
                "{platform:?} {variables:?}"
            );
        }
    }
}
