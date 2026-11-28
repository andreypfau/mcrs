use mcrs_minecraft_client_jar::Directory;

use crate::inflated_file::InflatedFile;

pub fn inflate_zip(archive: &[u8]) -> Result<Vec<InflatedFile>, String> {
    let directory = Directory::of(archive)?;
    directory
        .inflated(archive, |name| name.ends_with(".hex"))
        .map(|file| {
            let (name, bytes) = file?;
            Ok(InflatedFile {
                name: name.to_owned(),
                bytes,
            })
        })
        .collect()
}
