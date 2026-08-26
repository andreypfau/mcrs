//! The biome layer: which biomes surround a position and how their attribute
//! maps are weighted into one.

use std::sync::Arc;

use bevy_math::DVec3;

use crate::attribute::{
    AttributeError, AttributeSpec, AttributeValue, ENVIRONMENT_ATTRIBUTES, EnvironmentAttributeMap,
    Operation, apply,
};

/// `GaussianSampler`: a 6³ neighbourhood weighted by a separable binomial
/// kernel, sampled at quart resolution.
const KERNEL: [f64; 7] = [0.0, 1.0, 4.0, 6.0, 4.0, 1.0, 0.0];
const BREADTH: i32 = 6;
const RADIUS: i32 = 2;

pub fn gaussian_sample<V>(
    position: DVec3,
    sampler: impl Fn(i32, i32, i32) -> V,
    mut accumulate: impl FnMut(f64, V),
) {
    let position = position - DVec3::splat(0.5);
    let integral = position.floor();
    let relative = position - integral;
    let axis = |step: i32, origin: f64, relative: f64| {
        let i = step as usize;
        (
            lerp(relative, KERNEL[i + 1], KERNEL[i]),
            origin as i32 - RADIUS + step,
        )
    };

    for z in 0..BREADTH {
        let (weight_z, sample_z) = axis(z, integral.z, relative.z);
        for x in 0..BREADTH {
            let (weight_x, sample_x) = axis(x, integral.x, relative.x);
            for y in 0..BREADTH {
                let (weight_y, sample_y) = axis(y, integral.y, relative.y);
                accumulate(
                    weight_x * weight_y * weight_z,
                    sampler(sample_x, sample_y, sample_z),
                );
            }
        }
    }
}

fn lerp(alpha: f64, from: f64, to: f64) -> f64 {
    from + alpha * (to - from)
}

/// One biome's attribute map, keyed by the attribute's registry position
/// rather than by its id.
///
/// Baked when the biome loads so the frame path neither hashes a string nor
/// re-parses the JSON argument of an entry it walks every frame.
#[derive(Debug, Clone)]
pub struct BiomeAttributes {
    entries: Vec<Option<(Operation, AttributeValue)>>,
}

impl Default for BiomeAttributes {
    fn default() -> Self {
        BiomeAttributes {
            entries: vec![None; ENVIRONMENT_ATTRIBUTES.len()],
        }
    }
}

impl BiomeAttributes {
    pub fn bake(attributes: &EnvironmentAttributeMap) -> Result<Self, AttributeError> {
        let mut baked = BiomeAttributes::default();
        for (position, spec) in ENVIRONMENT_ATTRIBUTES.values().enumerate() {
            if let Some(entry) = attributes.get(spec.id) {
                baked.entries[position] = Some((entry.modifier, entry.value(spec)?));
            }
        }
        Ok(baked)
    }

    /// `EnvironmentAttributeMap.applyModifier`: a biome that says nothing about
    /// this attribute leaves the layer beneath it alone.
    fn apply(&self, index: usize, spec: &AttributeSpec, base: &AttributeValue) -> AttributeValue {
        match &self.entries[index] {
            Some((op, argument)) => {
                apply(spec.ty, *op, base, argument).unwrap_or_else(|_| base.clone())
            }
            None => base.clone(),
        }
    }
}

/// Where the biome attributes at a quart-resolution position come from.
///
/// Quart resolution is the biome grid: one entry per 4×4×4 blocks.
pub trait BiomeAttributeSource {
    fn at_quart(&self, x: i32, y: i32, z: i32) -> &Arc<BiomeAttributes>;
}

/// Every position is the same biome.
///
/// Stands in until loaded chunks can answer a biome lookup. Swapping in the
/// real source changes nothing above this trait.
#[derive(Debug, Clone, Default)]
pub struct UniformBiomes(pub Arc<BiomeAttributes>);

impl BiomeAttributeSource for UniformBiomes {
    fn at_quart(&self, _x: i32, _y: i32, _z: i32) -> &Arc<BiomeAttributes> {
        &self.0
    }
}

/// `SpatialAttributeInterpolator`: the biome attribute maps around one
/// position, each with the weight the Gaussian kernel gave it.
///
/// Sampled once per frame for the camera and read by every spatially
/// interpolated attribute; an attribute that is not spatially interpolated
/// reads [`SpatialAttributeInterpolator::exact`] instead.
#[derive(Debug, Clone, Default)]
pub struct SpatialAttributeInterpolator {
    weights: Vec<(Arc<BiomeAttributes>, f64)>,
    exact: Option<Arc<BiomeAttributes>>,
}

impl SpatialAttributeInterpolator {
    pub fn clear(&mut self) {
        self.weights.clear();
        self.exact = None;
    }

    pub fn accumulate(&mut self, weight: f64, attributes: &Arc<BiomeAttributes>) {
        match self
            .weights
            .iter_mut()
            .find(|(source, _)| Arc::ptr_eq(source, attributes))
        {
            Some((_, accumulated)) => *accumulated += weight,
            None => self.weights.push((attributes.clone(), weight)),
        }
    }

    pub fn exact(&self) -> Option<&BiomeAttributes> {
        self.exact.as_deref()
    }

    pub fn is_empty(&self) -> bool {
        self.weights.is_empty() && self.exact.is_none()
    }

    /// Fill in the biomes around `position`, in block coordinates.
    pub fn sample(&mut self, position: DVec3, biomes: &dyn BiomeAttributeSource) {
        self.clear();
        let quart = position * 0.25;
        gaussian_sample(
            quart,
            |x, y, z| biomes.at_quart(x, y, z),
            |weight, attributes| self.accumulate(weight, attributes),
        );
        self.exact = Some(
            biomes
                .at_quart(quart.x as i32, quart.y as i32, quart.z as i32)
                .clone(),
        );
    }

    /// Compose this layer onto `base`. `index` is the attribute's registry
    /// position, resolved when the layer stack was built.
    pub fn apply(
        &self,
        index: usize,
        spec: &AttributeSpec,
        base: &AttributeValue,
    ) -> AttributeValue {
        if !spec.spatially_interpolated || self.weights.is_empty() {
            return match self.exact() {
                Some(attributes) => attributes.apply(index, spec, base),
                None => base.clone(),
            };
        }
        if self.weights.len() == 1 {
            return self.weights[0].0.apply(index, spec, base);
        }

        let lerp = spec.ty.spatial_lerp();
        let mut result: Option<AttributeValue> = None;
        let mut accumulated = 0.0;
        for (attributes, weight) in &self.weights {
            let value = attributes.apply(index, spec, base);
            accumulated += weight;
            result = Some(match result {
                None => value,
                Some(previous) => lerp.apply((weight / accumulated) as f32, &previous, &value),
            });
        }
        result.unwrap_or_else(|| base.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute::attribute;
    use serde_json::json;

    fn map(json: serde_json::Value) -> Arc<BiomeAttributes> {
        let attributes: EnvironmentAttributeMap = serde_json::from_value(json).unwrap();
        Arc::new(BiomeAttributes::bake(&attributes).unwrap())
    }

    fn index(id: &str) -> usize {
        ENVIRONMENT_ATTRIBUTES
            .keys()
            .position(|key| *key == id)
            .unwrap()
    }

    #[test]
    fn the_kernel_weights_sum_to_the_same_total_wherever_it_lands() {
        for offset in [0.0, 0.25, 0.5, 0.9] {
            let mut total = 0.0;
            gaussian_sample(
                DVec3::splat(offset),
                |_, _, _| (),
                |weight, ()| total += weight,
            );
            assert!(
                (total - 16.0f64.powi(3)).abs() < 1e-9,
                "offset {offset} gave {total}"
            );
        }
    }

    #[test]
    fn a_single_biome_applies_its_own_modifier() {
        let swamp = map(json!({"minecraft:visual/sky_color": "#6a7039"}));
        let mut interpolator = SpatialAttributeInterpolator::default();
        interpolator.sample(DVec3::ZERO, &UniformBiomes(swamp));

        let spec = attribute("minecraft:visual/sky_color").unwrap();
        let index = index(spec.id);
        assert_eq!(
            interpolator.apply(index, spec, &AttributeValue::Color(0xFF78_A7FF)),
            AttributeValue::Color(0xFF6A_7039)
        );
    }

    #[test]
    fn an_undeclared_attribute_falls_through() {
        let plains = map(json!({"minecraft:visual/sky_color": "#78a7ff"}));
        let mut interpolator = SpatialAttributeInterpolator::default();
        interpolator.sample(DVec3::ZERO, &UniformBiomes(plains));

        let spec = attribute("minecraft:visual/water_fog_color").unwrap();
        let index = index(spec.id);
        assert_eq!(
            interpolator.apply(index, spec, &AttributeValue::Color(0xFF05_0533)),
            AttributeValue::Color(0xFF05_0533)
        );
    }

    #[test]
    fn two_biomes_blend_by_their_weights() {
        let (black, white) = (
            map(json!({"minecraft:visual/sky_color": "#000000"})),
            map(json!({"minecraft:visual/sky_color": "#ffffff"})),
        );
        let mut interpolator = SpatialAttributeInterpolator::default();
        interpolator.accumulate(1.0, &black);
        interpolator.accumulate(1.0, &white);

        let spec = attribute("minecraft:visual/sky_color").unwrap();
        let index = index(spec.id);
        let AttributeValue::Color(blended) =
            interpolator.apply(index, spec, &AttributeValue::Color(0xFF00_0000))
        else {
            panic!("a colour attribute blends to a colour");
        };
        assert_eq!(blended, 0xFF7F_7F7F);
    }

    #[test]
    fn an_attribute_that_is_not_spatially_interpolated_takes_the_biome_at_the_position() {
        let dripping = map(json!({
            "minecraft:visual/default_dripstone_particle": {"type": "minecraft:dripping_lava"},
        }));
        let mut interpolator = SpatialAttributeInterpolator::default();
        interpolator.sample(DVec3::ZERO, &UniformBiomes(dripping));

        let spec = attribute("minecraft:visual/default_dripstone_particle").unwrap();
        let index = index(spec.id);
        assert!(!spec.spatially_interpolated);
        assert_eq!(
            interpolator.apply(index, spec, &spec.default),
            AttributeValue::Opaque(json!({"type": "minecraft:dripping_lava"}))
        );
    }
}
