use std::path::PathBuf;

#[cfg(feature = "client-jar")]
use mcrs_minecraft_client_jar::ClientJar;

/// Where the server reads the game's data pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataPack {
    /// A directory laid out as the client jar's `data/minecraft` tree with its `version.json`.
    /// A relative path is resolved as the asset root is.
    Directory(PathBuf),
    /// The pinned release's client jar, unpacked once below `cache`, or the user's cache
    /// directory when it is `None`.
    #[cfg(feature = "client-jar")]
    ClientJar {
        jar: ClientJar,
        cache: Option<PathBuf>,
    },
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DataPackError {
    #[cfg(feature = "client-jar")]
    #[error("the vanilla data pack: {0}")]
    ClientJar(#[from] mcrs_minecraft_client_jar::VanillaDataError),
}

impl DataPack {
    /// The directory, unpacking the client jar first when that is the source.
    pub fn resolve(&self) -> Result<PathBuf, DataPackError> {
        match self {
            DataPack::Directory(dir) => Ok(dir.clone()),
            #[cfg(feature = "client-jar")]
            DataPack::ClientJar { jar, cache } => Ok(mcrs_minecraft_client_jar::vanilla_data(
                jar,
                cache.as_deref(),
                &mcrs_minecraft_client_jar::Progress::default(),
            )?),
        }
    }
}

impl Default for DataPack {
    #[cfg(feature = "client-jar")]
    fn default() -> Self {
        DataPack::ClientJar {
            jar: ClientJar::InstalledOrDownloaded,
            cache: None,
        }
    }

    #[cfg(not(feature = "client-jar"))]
    fn default() -> Self {
        DataPack::Directory(PathBuf::from("assets/minecraft"))
    }
}
