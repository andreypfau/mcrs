use crate::interval::Interval;
use crate::noise::Noise;
use crate::noise::gradient::wrap;
use crate::noise::perlin::{PerlinNoise, SmearedPerlinNoise};
use crate::volume::Volume;

/// One octave's contribution: a lattice, the frequency its coordinates are
/// scaled by, and the weight its sample carries.
#[derive(Clone, Debug, PartialEq)]
struct Layer<N> {
    noise: N,
    frequency: f64,
    amplitude: f32,
}

/// A flat sum of scaled octaves. Vanilla's `NoiseStack`: no nesting, no octave
/// index arithmetic at sample time, just a list walked in order.
#[derive(Clone, Debug, PartialEq)]
pub struct NoiseStack<N> {
    layers: Box<[Layer<N>]>,
    range: Interval,
}

impl Noise for PerlinNoise {
    const NEEDS_UNWRAPPED: bool = false;

    fn range(&self) -> Interval {
        Interval::symmetric(2.0)
    }

    #[inline(always)]
    fn sample(&self, x: f64, y: f64, z: f64) -> f32 {
        PerlinNoise::sample(self, x, y, z)
    }

    #[inline]
    fn sample_column(&self, x: f64, z: f64, ys: &[f64], _unwrapped_ys: &[f64], out: &mut [f32]) {
        PerlinNoise::sample_column(self, x, z, ys, out);
    }

    fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &Volume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        PerlinNoise::add_to_volume(self, out, volume, xz_scale, y_scale, amplitude);
    }
}

impl Noise for SmearedPerlinNoise {
    const NEEDS_UNWRAPPED: bool = true;

    fn range(&self) -> Interval {
        Interval::symmetric((self.fudge_y_scale().abs() + 2.0) as f32)
    }

    #[inline(always)]
    fn sample(&self, x: f64, y: f64, z: f64) -> f32 {
        let mut out = [0.0f32; 1];
        self.sample_column(x, z, &[y], &[y], &mut out);
        out[0]
    }

    #[inline]
    fn sample_column(&self, x: f64, z: f64, ys: &[f64], unwrapped_ys: &[f64], out: &mut [f32]) {
        SmearedPerlinNoise::sample_column(self, x, z, ys, unwrapped_ys, out);
    }

    fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &Volume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        SmearedPerlinNoise::add_to_volume(self, out, volume, xz_scale, y_scale, amplitude);
    }
}

/// Reused across the layers of one column fill.
#[derive(Default)]
pub struct ColumnScratch {
    scaled: Vec<f64>,
    unwrapped: Vec<f64>,
    layer: Vec<f32>,
}

impl<N: Noise> NoiseStack<N> {
    pub fn builder() -> Builder<N> {
        Builder(Vec::new())
    }

    pub fn range(&self) -> Interval {
        self.range
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Forms each layer's coordinate as `coordinate * frequency`. The volume
    /// paths below fold the scale into the frequency instead, and at the
    /// `1.0181268882175227` ratio of every second sub-noise the two products
    /// differ by an f64 ulp — vanilla ships that difference, so the two
    /// associations must not be unified.
    pub fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        let mut value = 0.0f32;
        for layer in &self.layers {
            let f = layer.frequency;
            value += layer.amplitude * layer.noise.sample(wrap(x * f), wrap(y * f), wrap(z * f));
        }
        value
    }

    /// [`Self::get`] over a run of positions sharing `x` and `z`, which lets each
    /// layer hoist its lattice hashes across the run. Same association as `get`.
    pub fn fill_column(
        &self,
        out: &mut [f32],
        x: f64,
        z: f64,
        ys: &[f64],
        scratch: &mut ColumnScratch,
    ) {
        out.fill(0.0);
        scratch.resize(ys.len(), N::NEEDS_UNWRAPPED);
        for layer in &self.layers {
            let f = layer.frequency;
            for (i, &y) in ys.iter().enumerate() {
                let scaled = y * f;
                if N::NEEDS_UNWRAPPED {
                    scratch.unwrapped[i] = scaled;
                }
                scratch.scaled[i] = wrap(scaled);
            }
            layer.noise.sample_column(
                wrap(x * f),
                wrap(z * f),
                &scratch.scaled,
                &scratch.unwrapped,
                &mut scratch.layer,
            );
            for (slot, &sampled) in out.iter_mut().zip(scratch.layer.iter()) {
                *slot += layer.amplitude * sampled;
            }
        }
    }

    /// One column of `volume`, in the volume association: each layer receives
    /// `scale * frequency` *before* the block multiply.
    pub fn fill_column_at(
        &self,
        out: &mut [f32],
        block_x: i32,
        block_z: i32,
        volume: &Volume,
        xz_scale: f64,
        y_scale: f64,
        scratch: &mut ColumnScratch,
    ) {
        out.fill(0.0);
        scratch.resize(out.len(), N::NEEDS_UNWRAPPED);
        for layer in &self.layers {
            let f = layer.frequency;
            let layer_xz = xz_scale * f;
            let layer_y = y_scale * f;
            for i in 0..out.len() {
                // The smear compares against the scaled y before it is wrapped.
                let scaled = volume.block_y(i as i32) as f64 * layer_y;
                if N::NEEDS_UNWRAPPED {
                    scratch.unwrapped[i] = scaled;
                }
                scratch.scaled[i] = wrap(scaled);
            }
            layer.noise.sample_column(
                wrap(block_x as f64 * layer_xz),
                wrap(block_z as f64 * layer_xz),
                &scratch.scaled,
                &scratch.unwrapped,
                &mut scratch.layer,
            );
            for (slot, &sampled) in out.iter_mut().zip(scratch.layer.iter()) {
                *slot += layer.amplitude * sampled;
            }
        }
    }

    /// Accumulates `amplitude * get(...)` over `volume`, in the volume association.
    pub fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &Volume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        for layer in &self.layers {
            layer.noise.add_to_volume(
                out,
                volume,
                xz_scale * layer.frequency,
                y_scale * layer.frequency,
                amplitude * layer.amplitude,
            );
        }
    }
}

impl ColumnScratch {
    fn resize(&mut self, len: usize, needs_unwrapped: bool) {
        self.scaled.clear();
        self.scaled.resize(len, 0.0);
        self.layer.clear();
        self.layer.resize(len, 0.0);
        self.unwrapped.clear();
        self.unwrapped.resize(if needs_unwrapped { len } else { 0 }, 0.0);
    }
}

pub struct Builder<N>(Vec<Layer<N>>);

impl<N: Noise> Builder<N> {
    pub fn add(&mut self, noise: N, frequency: f64, amplitude: f32) -> &mut Self {
        self.0.push(Layer {
            noise,
            frequency,
            amplitude,
        });
        self
    }

    pub fn build(self) -> NoiseStack<N> {
        let mut range = Interval::exact(0.0);
        for layer in &self.0 {
            range = range + layer.noise.range() * Interval::exact(layer.amplitude);
        }
        NoiseStack {
            layers: self.0.into_boxed_slice(),
            range,
        }
    }

    /// Overrides the range the layers imply. `NormalNoise` declares a six-sigma
    /// statistical bound on the summed octaves rather than their analytic hull,
    /// and branch elimination consumes the declared one.
    pub fn build_with_range(self, range: Interval) -> NoiseStack<N> {
        NoiseStack {
            layers: self.0.into_boxed_slice(),
            range,
        }
    }
}

impl<N> NoiseStack<N> {
    /// The per-layer weights, in build order. A drift guard for the
    /// normalization modes rather than something generation reads.
    #[cfg(test)]
    pub(crate) fn amplitudes(&self) -> Vec<f32> {
        self.layers.iter().map(|l| l.amplitude).collect()
    }

    #[cfg(test)]
    pub(crate) fn layer(&self, index: usize) -> (&N, f64, f32) {
        let layer = &self.layers[index];
        (&layer.noise, layer.frequency, layer.amplitude)
    }
}
