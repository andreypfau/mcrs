use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes;

use crate::template::Template;

#[derive(Debug, thiserror::Error)]
pub enum TemplateFileError {
    #[error(transparent)]
    Nbt(#[from] mcrs_minecraft_nbt::Error),
    #[error("DataVersion {found}, expected {expected}")]
    DataVersion { found: i32, expected: i32 },
}

pub fn decode_template(bytes: &[u8]) -> Result<Template, TemplateFileError> {
    let template: Template = from_gzip_bytes(bytes)?;
    if template.data_version != VERSION.world_version {
        return Err(TemplateFileError::DataVersion {
            found: template.data_version,
            expected: VERSION.world_version,
        });
    }
    Ok(template)
}
