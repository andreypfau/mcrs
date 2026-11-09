use glam::IVec3;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, block_pos_seed};

/// Where the selected parts of a state come from, which decides how the seed reaches each part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartSource {
    Variants,
    Multipart,
}

/// How a block state at a block position chooses one variant of each of its selected parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariantPick {
    /// Added to the block position before the position seed, so the two halves of a door, a
    /// tall plant or a bed choose alike.
    pub seed_offset: [i32; 3],
    pub source: PartSource,
    /// The weights of the variants of each selected part, in part order.
    pub weights: Vec<Vec<u32>>,
}

impl VariantPick {
    #[must_use]
    pub fn seed(&self, position: [i32; 3]) -> i64 {
        let [x, y, z] = position;
        let [dx, dy, dz] = self.seed_offset;
        block_pos_seed(IVec3::new(
            x.wrapping_add(dx),
            y.wrapping_add(dy),
            z.wrapping_add(dz),
        )) as i64
    }

    /// The index of the chosen variant of each part.
    #[must_use]
    pub fn chosen(&self, position: [i32; 3]) -> Vec<usize> {
        let mut chosen = Vec::with_capacity(self.weights.len());
        self.each_choice(position, |index| chosen.push(index));
        chosen
    }

    /// The chosen variants as one number: the index of each part's variant, in mixed radix
    /// over the part sizes, the first part lowest.
    #[must_use]
    pub fn choice(&self, position: [i32; 3]) -> u32 {
        let mut choice = 0u32;
        let mut scale = 1u32;
        let mut parts = self.weights.iter();
        self.each_choice(position, |index| {
            let size = parts.next().map_or(1, |part| part.len() as u32);
            choice = choice.saturating_add(scale.saturating_mul(index as u32));
            scale = scale.saturating_mul(size);
        });
        choice
    }

    /// The number of distinct values of [`VariantPick::choice`], or `None` past `u32`.
    #[must_use]
    pub fn choices(&self) -> Option<u32> {
        self.weights
            .iter()
            .try_fold(1u32, |count, part| count.checked_mul(part.len() as u32))
    }

    fn each_choice(&self, position: [i32; 3], mut take: impl FnMut(usize)) {
        let mut random = LegacyRandom::new(self.seed(position) as u64);
        match self.source {
            PartSource::Variants => {
                for part in &self.weights {
                    take(pick(part, &mut random));
                }
            }
            PartSource::Multipart => {
                let seed = random.next_java_long();
                for part in &self.weights {
                    if part.len() < 2 {
                        take(0);
                        continue;
                    }
                    let mut random = LegacyRandom::new(seed as u64);
                    take(pick(part, &mut random));
                }
            }
        }
    }
}

fn pick(weights: &[u32], random: &mut LegacyRandom) -> usize {
    if weights.len() < 2 {
        return 0;
    }
    let total: u32 = weights
        .iter()
        .fold(0u32, |sum, weight| sum.saturating_add(*weight));
    let Ok(total) = i32::try_from(total) else {
        return 0;
    };
    if total == 0 {
        return 0;
    }
    let mut left = i64::from(random.next_i32_bound(total));
    for (index, weight) in weights.iter().enumerate() {
        left -= i64::from(*weight);
        if left < 0 {
            return index;
        }
    }
    0
}
