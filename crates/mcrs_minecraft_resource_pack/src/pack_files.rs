use std::collections::BTreeMap;

use crate::pack_error::PackError;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackFiles {
    files: BTreeMap<String, Vec<u8>>,
}

impl PackFiles {
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

    #[must_use]
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    pub fn read(&self, path: &str) -> Result<&[u8], PackError> {
        self.get(path).ok_or_else(|| PackError::Missing {
            path: path.to_owned(),
        })
    }

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
