use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::ObjectInfo;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GlyphFont<'a> {
    Named(&'a ResourceLocationBuf),
    Object(&'a ObjectInfo),
}
