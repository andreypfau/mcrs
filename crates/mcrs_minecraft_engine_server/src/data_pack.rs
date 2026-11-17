use std::path::PathBuf;

#[cfg(feature = "client-jar")]
use mcrs_minecraft_client_jar::ClientJar;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataPack {
    Directory(PathBuf),
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
