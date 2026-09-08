use crate::interval::Interval;
use crate::jmath::mul_add;
use crate::noise::Noise;
use crate::noise::gradient::GradientNoise;
use crate::noise::perlin::{LegacyPerlin2dNoise, PerlinNoise};
use crate::noise::simplex::SimplexNoise;
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

/// Which sampler a layer of a mixed stack carries. Vanilla holds its layers
/// behind the `Noise` interface; the enum is the same choice made once per
/// layer instead of once per virtual call.
#[derive(Clone, Debug, PartialEq)]
pub enum Octave {
    Perlin(PerlinNoise),
    /// Beta's `ySize == 1` branch, which is a different noise over the same
    /// lattice rather than the 3D one evaluated at y = 0.
    Perlin2d(LegacyPerlin2dNoise),
    Simplex(SimplexNoise),
}

impl Noise for Octave {
    fn range(&self) -> Interval {
        match self {
            Self::Perlin(n) => n.range(),
            Self::Perlin2d(n) => n.range(),
            Self::Simplex(n) => n.range(),
        }
    }

    #[inline]
    fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        match self {
            Self::Perlin(n) => n.get(x, y, z),
            Self::Perlin2d(n) => n.get(x, y, z),
            Self::Simplex(n) => n.get(x, y, z),
        }
    }

    #[inline]
    fn get_column(&self, x: f64, z: f64, ys: &[f64], out: &mut [f32]) {
        match self {
            Self::Perlin(n) => n.get_column(x, z, ys, out),
            Self::Perlin2d(n) => n.get_column(x, z, ys, out),
            Self::Simplex(n) => n.get_column(x, z, ys, out),
        }
    }

    fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &Volume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        match self {
            Self::Perlin(n) => n.add_to_volume(out, volume, xz_scale, y_scale, amplitude),
            Self::Perlin2d(n) => n.add_to_volume(out, volume, xz_scale, y_scale, amplitude),
            Self::Simplex(n) => n.add_to_volume(out, volume, xz_scale, y_scale, amplitude),
        }
    }
}

impl NoiseStack<PerlinNoise> {
    /// [`PerlinNoise::legacy_fill`] summed over the layers, which is how Beta
    /// reads its beach and surface octaves.
    pub fn fill_legacy_grid(
        &self,
        out: &mut [f32],
        offset: [f64; 3],
        size: [usize; 3],
        scale: [f64; 3],
    ) {
        out.fill(0.0);
        for layer in &self.layers {
            let f = layer.frequency;
            layer.noise.legacy_fill(
                out,
                offset,
                size,
                [scale[0] * f, scale[1] * f, scale[2] * f],
                layer.amplitude,
            );
        }
    }
}

/// Beta's fbm: the octaves are walked from the first drawn to the last, the
/// frequency halves each step and the contribution is divided by that frequency
/// rather than normalised. As a stack that is a layer per octave weighing
/// `1 / frequency`, summed in Beta's own order.
pub fn beta_fbm<N: Noise>(
    lattices: Vec<Option<GradientNoise>>,
    layer: impl Fn(GradientNoise) -> N,
) -> NoiseStack<N> {
    let mut stack = NoiseStack::builder();
    let mut frequency = 1.0f64;
    for lattice in lattices.into_iter().rev() {
        if let Some(lattice) = lattice {
            stack.add(layer(lattice), frequency, (1.0 / frequency) as f32);
        }
        frequency *= 0.5;
    }
    stack.build()
}

/// Reused across the layers of one column fill.
#[derive(Default)]
pub struct ColumnScratch {
    scaled: Vec<f64>,
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
            value = mul_add(layer.amplitude, layer.noise.get(x * f, y * f, z * f), value);
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
        scratch.resize(ys.len());
        for layer in &self.layers {
            let f = layer.frequency;
            for (slot, &y) in scratch.scaled.iter_mut().zip(ys) {
                *slot = y * f;
            }
            layer
                .noise
                .get_column(x * f, z * f, &scratch.scaled, &mut scratch.layer);
            for (slot, &sampled) in out.iter_mut().zip(scratch.layer.iter()) {
                *slot = mul_add(layer.amplitude, sampled, *slot);
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
        scratch.resize(out.len());
        for layer in &self.layers {
            let f = layer.frequency;
            let layer_xz = xz_scale * f;
            let layer_y = y_scale * f;
            for (i, slot) in scratch.scaled.iter_mut().enumerate() {
                *slot = volume.block_y(i as i32) as f64 * layer_y;
            }
            layer.noise.get_column(
                block_x as f64 * layer_xz,
                block_z as f64 * layer_xz,
                &scratch.scaled,
                &mut scratch.layer,
            );
            for (slot, &sampled) in out.iter_mut().zip(scratch.layer.iter()) {
                *slot = mul_add(layer.amplitude, sampled, *slot);
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
    fn resize(&mut self, len: usize) {
        self.scaled.clear();
        self.scaled.resize(len, 0.0);
        self.layer.clear();
        self.layer.resize(len, 0.0);
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
