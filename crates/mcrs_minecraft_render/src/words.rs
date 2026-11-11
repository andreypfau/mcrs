use bevy::render::render_resource::binding_types::{
    storage_buffer_read_only_sized, texture_2d_array,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use wgpu::util::StagingBelt;

use super::render_path::RenderPath;

/// A row of a word texture holds two to this power words, so a shader finds a word with two
/// masks and two shifts. WebGL2 promises no texture wider than this.
pub const WORD_ROW_BITS: u32 = 11;
const ROW: u64 = 1 << WORD_ROW_BITS;
/// Rows of a layer: half a row's words. wgpu's GL backend takes a square array for a cube map
/// when its layers come in sixes, and a layer this shape is never square.
const LAYER_ROWS: u64 = ROW / 2;
/// The same backend binds an array of one layer as a plain texture, which the shader's array
/// sampler then rejects.
const FEWEST_LAYERS: u64 = 2;
const WORD: u64 = size_of::<u32>() as u64;

/// An array of `u32` a shader reads by index: a storage buffer on the full path, and on the
/// minimal one an integer texture array of one word a texel, which every device can bind.
pub struct Words(Array);

enum Array {
    Buffer(Buffer),
    Texture(WordTexture),
}

struct WordTexture {
    texture: Texture,
    view: TextureView,
}

/// What a write recorded in the frame's encoder needs: the belt for a buffer, and for a texture
/// the buffer its words wait in.
pub(super) struct Staged<'a> {
    pub encoder: &'a mut CommandEncoder,
    pub belt: &'a mut StagingBelt,
    pub texels: &'a mut TexelStaging,
    pub device: &'a RenderDevice,
}

/// Where words wait for their copy into a word texture. Belt memory cannot be the source of a
/// texture copy, so a write goes belt, then here, then into the texture, each a command of the
/// frame's encoder: it lands after the copy that carried a grown texture's words across, which a
/// write through the queue, landing ahead of every command of the frame, would not.
#[derive(Default)]
pub(super) struct TexelStaging {
    buffer: Option<Buffer>,
    used: u64,
}

impl TexelStaging {
    /// The copies out of the buffer recorded so far come first in the encoder, so the buffer is
    /// filled again from its start.
    pub fn reset(&mut self) {
        self.used = 0;
    }

    /// Room for `bytes` in the buffer and where it starts. A buffer too small is replaced by one
    /// that would have held the frame so far: the copies already recorded keep the old one alive.
    fn place(&mut self, bytes: u64, device: &RenderDevice) -> (&Buffer, u64) {
        let end = self.used + bytes;
        if self
            .buffer
            .as_ref()
            .is_none_or(|buffer| end > buffer.size())
        {
            self.buffer = None;
            self.used = 0;
        }
        let at = self.used;
        self.used += bytes;
        let buffer = self.buffer.get_or_insert_with(|| {
            device.create_buffer(&BufferDescriptor {
                label: Some("terrain word staging"),
                size: end.next_power_of_two().max(ROW * WORD),
                usage: BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        });
        (buffer, at)
    }
}

impl Words {
    pub(super) fn new(path: RenderPath, label: &str, bytes: u64, device: &RenderDevice) -> Self {
        match path {
            RenderPath::Full => Self::records(label, bytes, BufferUsages::STORAGE, device),
            RenderPath::Minimal => Self(Array::Texture(WordTexture::new(label, bytes, device))),
        }
    }

    /// A buffer on either path, for words that reach a shader as `usage` rather than by index.
    pub(super) fn records(
        label: &str,
        bytes: u64,
        usage: BufferUsages,
        device: &RenderDevice,
    ) -> Self {
        Self(Array::Buffer(device.create_buffer(&BufferDescriptor {
            label: Some(label),
            size: bytes.max(WORD).next_multiple_of(COPY_BUFFER_ALIGNMENT),
            usage: usage | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })))
    }

    pub(super) fn buffer(&self) -> Option<&Buffer> {
        match &self.0 {
            Array::Buffer(buffer) => Some(buffer),
            Array::Texture(_) => None,
        }
    }

    /// Bytes the array holds.
    pub(super) fn bytes(&self) -> u64 {
        match &self.0 {
            Array::Buffer(buffer) => buffer.size(),
            Array::Texture(words) => {
                let size = words.texture.size();
                let rows = if size.height as u64 == LAYER_ROWS {
                    LAYER_ROWS * u64::from(size.depth_or_array_layers)
                } else {
                    u64::from(size.height)
                };
                ROW * rows * WORD
            }
        }
    }

    /// An array of `bytes` holding what this one holds, by a copy recorded ahead of whatever the
    /// encoder is given next.
    pub(super) fn grown(
        &self,
        label: &str,
        bytes: u64,
        device: &RenderDevice,
        encoder: &mut CommandEncoder,
    ) -> Self {
        match &self.0 {
            Array::Buffer(buffer) => {
                let grown = Self::records(label, bytes, buffer.usage(), device);
                if let Some(target) = grown.buffer() {
                    encoder.copy_buffer_to_buffer(buffer, 0, target, 0, buffer.size());
                }
                grown
            }
            Array::Texture(words) => {
                let grown = WordTexture::new(label, bytes, device);
                encoder.copy_texture_to_texture(
                    words.texture.as_image_copy(),
                    grown.texture.as_image_copy(),
                    words.texture.size(),
                );
                Self(Array::Texture(grown))
            }
        }
    }

    /// Writes `data`, whole words, at byte `offset`, ahead of every command of the frame. An
    /// array that grows is written through [`Self::stage`], in step with the copy of a growth.
    pub fn write(&self, offset: u64, data: &[u8], queue: &RenderQueue) {
        match &self.0 {
            Array::Buffer(buffer) => queue.write_buffer(buffer, offset, data),
            Array::Texture(words) => words.write(offset, data, queue),
        }
    }

    /// Writes `data`, whole words, at byte `offset`, as a command of the frame's encoder.
    pub(super) fn stage(&self, offset: u64, data: &[u8], staged: &mut Staged) {
        match &self.0 {
            Array::Buffer(buffer) => {
                if let Some(size) = BufferSize::new(data.len() as u64) {
                    staged
                        .belt
                        .write_buffer(staged.encoder, buffer, offset, size)
                        .copy_from_slice(data);
                }
            }
            Array::Texture(words) => words.stage(offset, data, staged),
        }
    }

    /// The layout entry of a binding of this kind of array on `path`.
    #[must_use]
    pub fn layout(path: RenderPath) -> BindGroupLayoutEntryBuilder {
        match path {
            RenderPath::Full => storage_buffer_read_only_sized(false, None),
            RenderPath::Minimal => texture_2d_array(TextureSampleType::Uint),
        }
    }

    #[must_use]
    pub fn binding(&self) -> BindingResource<'_> {
        match &self.0 {
            Array::Buffer(buffer) => buffer.as_entire_binding(),
            Array::Texture(words) => BindingResource::TextureView(&words.view),
        }
    }
}

/// The layers and rows that hold `words`: layers as tall as one of them needs to be, or whole
/// layers.
fn extent(words: u64) -> Extent3d {
    let rows = words.div_ceil(ROW).max(1);
    Extent3d {
        width: ROW as u32,
        height: rows.min(LAYER_ROWS) as u32,
        depth_or_array_layers: rows.div_ceil(LAYER_ROWS).max(FEWEST_LAYERS) as u32,
    }
}

/// The rectangles of one layer that the words from `word` on fall into, each with how many
/// words it takes: the rest of a row begun, then whole rows, then the start of a last one.
fn rectangles(mut word: u64, mut count: u64) -> impl Iterator<Item = (Origin3d, Extent3d)> {
    std::iter::from_fn(move || {
        if count == 0 {
            return None;
        }
        let (x, row) = (word % ROW, word / ROW);
        let (y, layer) = (row % LAYER_ROWS, row / LAYER_ROWS);
        let (width, height) = if x == 0 && count >= ROW {
            (ROW, (count / ROW).min(LAYER_ROWS - y))
        } else {
            ((ROW - x).min(count), 1)
        };
        word += width * height;
        count -= width * height;
        Some((
            Origin3d {
                x: x as u32,
                y: y as u32,
                z: layer as u32,
            },
            Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
        ))
    })
}

impl WordTexture {
    // chisle: an array holds at most `max_texture_array_layers` layers of 8 MiB, 2 GiB on
    // WebGL2's 256, and nothing clamps the terrain budget to it: the browser's arenas stay
    // under 210 MB, and a larger budget on a device of this path fails to create the texture.
    // Clamping the budget to the device before the arenas are sized lifts it.
    fn new(label: &str, bytes: u64, device: &RenderDevice) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(label),
            size: extent(bytes.div_ceil(WORD)),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Uint,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor {
            label: Some(label),
            dimension: Some(TextureViewDimension::D2Array),
            ..TextureViewDescriptor::default()
        });
        Self { texture, view }
    }

    /// The rectangles `data` falls into from byte `offset` on, each with its words.
    fn split(offset: u64, data: &[u8]) -> impl Iterator<Item = (Origin3d, Extent3d, &[u8])> {
        let mut rest = data;
        rectangles(offset / WORD, data.len() as u64 / WORD).map_while(move |(origin, size)| {
            let bytes = (u64::from(size.width) * u64::from(size.height) * WORD) as usize;
            let (rectangle, tail) = rest.split_at_checked(bytes)?;
            rest = tail;
            Some((origin, size, rectangle))
        })
    }

    fn stage(&self, offset: u64, data: &[u8], staged: &mut Staged) {
        for (origin, size, rectangle) in Self::split(offset, data) {
            let Some(bytes) = BufferSize::new(rectangle.len() as u64) else {
                continue;
            };
            let (buffer, at) = staged.texels.place(bytes.get(), staged.device);
            staged
                .belt
                .write_buffer(staged.encoder, buffer, at, bytes)
                .copy_from_slice(rectangle);
            staged.encoder.copy_buffer_to_texture(
                TexelCopyBufferInfo {
                    buffer,
                    layout: TexelCopyBufferLayout {
                        offset: at,
                        // A row of a copy of several is a whole row of the texture, which is the
                        // multiple of 256 bytes such a copy has to state.
                        bytes_per_row: (size.height > 1).then_some(size.width * WORD as u32),
                        rows_per_image: None,
                    },
                },
                TexelCopyTextureInfo {
                    origin,
                    ..self.texture.as_image_copy()
                },
                size,
            );
        }
    }

    fn write(&self, offset: u64, data: &[u8], queue: &RenderQueue) {
        for (origin, size, rectangle) in Self::split(offset, data) {
            queue.write_texture(
                TexelCopyTextureInfo {
                    origin,
                    ..self.texture.as_image_copy()
                },
                rectangle,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size.width * WORD as u32),
                    rows_per_image: None,
                },
                size,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every word lands on the texel the shader reads it from, once.
    #[test]
    fn a_run_of_words_covers_its_texels_once_in_order() {
        let layer = ROW * LAYER_ROWS;
        let cases = [
            (0, 1),
            (5, ROW - 5),
            (ROW - 1, 2),
            (3, 3 * ROW + 7),
            (layer - ROW - 2, 2 * ROW + 5),
            (0, 2 * layer + 1),
        ];
        for (first, count) in cases {
            let mut next = first;
            for (origin, size) in rectangles(first, count) {
                assert_eq!(size.depth_or_array_layers, 1);
                let row = u64::from(origin.z) * LAYER_ROWS + u64::from(origin.y);
                let start = row * ROW + u64::from(origin.x);
                assert_eq!(start, next, "{first}+{count}");
                assert!(u64::from(origin.x) + u64::from(size.width) <= ROW);
                assert!(u64::from(origin.y) + u64::from(size.height) <= LAYER_ROWS);
                assert!(size.height == 1 || (origin.x == 0 && u64::from(size.width) == ROW));
                next += u64::from(size.width) * u64::from(size.height);
            }
            assert_eq!(next, first + count, "{first}+{count}");
        }
    }

    #[test]
    fn a_texture_grows_by_rows_then_by_layers() {
        let layer = ROW * LAYER_ROWS;
        let cases = [
            (0, (1, 2)),
            (ROW, (1, 2)),
            (ROW + 1, (2, 2)),
            (2 * layer, (LAYER_ROWS as u32, 2)),
            (2 * layer + 1, (LAYER_ROWS as u32, 3)),
        ];
        for (words, (height, layers)) in cases {
            let size = extent(words);
            assert_eq!((size.height, size.depth_or_array_layers), (height, layers));
        }
    }
}
