use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

/// A file that a pack operation could not add or read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackError {
    /// A pack layer supplies a file that the pack already holds.
    Duplicate { path: String },
    /// The pack holds no file at the path.
    Missing { path: String },
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackError::Duplicate { path } => {
                write!(
                    f,
                    "{path}: a pack layer supplies a file the client pack holds"
                )
            }
            PackError::Missing { path } => write!(f, "the resource pack has no {path}"),
        }
    }
}

impl Error for PackError {}

/// The files of a resource pack, each keyed by its path below `assets/`, such as
/// `minecraft/models/block/stone.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackFiles {
    files: BTreeMap<String, Vec<u8>>,
}

impl PackFiles {
    /// Adds a file at its path below `assets/`. A file of an extra pack layer never replaces a
    /// file that the set holds.
    pub fn add_file(&mut self, path: &str, bytes: Vec<u8>) -> Result<(), PackError> {
        if self.files.contains_key(path) {
            return Err(PackError::Duplicate {
                path: path.to_owned(),
            });
        }
        self.files.insert(path.to_owned(), bytes);
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The bytes of the file at `path` below `assets/`.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    /// The bytes of the file at `path`, or an error that names the path.
    pub fn read(&self, path: &str) -> Result<&[u8], PackError> {
        self.get(path).ok_or_else(|| PackError::Missing {
            path: path.to_owned(),
        })
    }

    /// Each file `<namespace>/<folder>/<path>.<extension>`, as the identifier
    /// `<namespace>:<path>` and its bytes, in path order.
    pub fn entries<'a>(
        &'a self,
        folder: &'a str,
        extension: &'a str,
    ) -> impl Iterator<Item = (String, &'a [u8])> + 'a {
        self.files.iter().filter_map(move |(path, bytes)| {
            let (namespace, rest) = path.split_once('/')?;
            let rest = rest.strip_prefix(folder)?.strip_prefix('/')?;
            let id = rest.strip_suffix(extension)?.strip_suffix('.')?;
            Some((format!("{namespace}:{id}"), bytes.as_slice()))
        })
    }
}
