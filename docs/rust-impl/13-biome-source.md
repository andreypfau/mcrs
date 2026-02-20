# MultiNoiseBiomeSource & Climate R-tree — Spec

Covers the 7-dimensional climate parameter space, quantization, R-tree construction,
nearest-neighbor search, and how `MultiNoiseBiomeSource` uses it.
Sources: `Climate.java`, `MultiNoiseBiomeSource.java`, `MultiNoiseBiomeSourceParameterList.java`.

---

## 1. Quantization

All climate coordinates are stored as `i64` by multiplying `f32` values by **10,000**:

```rust
const QUANTIZATION_FACTOR: f32 = 10_000.0;

pub fn quantize(coord: f32) -> i64 {
    (coord * QUANTIZATION_FACTOR) as i64
}

pub fn unquantize(coord: i64) -> f32 {
    coord as f32 / QUANTIZATION_FACTOR
}
```

Valid float range: `[-2.0, 2.0]` → quantized range: `[-20_000, 20_000]`.

This avoids floating-point precision issues and enables exact integer distance calculations.

---

## 2. TargetPoint

Represents the sampled climate state at a specific world position:

```rust
pub struct TargetPoint {
    pub temperature:     i64,
    pub humidity:        i64,
    pub continentalness: i64,
    pub erosion:         i64,
    pub depth:           i64,
    pub weirdness:       i64,
}

impl TargetPoint {
    /// Convert to 7-element array for RTree search.
    /// Index 6 (offset) is always 0 — offset is only in ParameterPoints.
    pub fn to_parameter_array(&self) -> [i64; 7] {
        [self.temperature, self.humidity, self.continentalness,
         self.erosion, self.depth, self.weirdness, 0]
    }
}
```

Factory (quantizes from floats):
```rust
pub fn target(temperature: f32, humidity: f32, continentalness: f32,
              erosion: f32, depth: f32, weirdness: f32) -> TargetPoint {
    TargetPoint {
        temperature:     quantize(temperature),
        humidity:        quantize(humidity),
        continentalness: quantize(continentalness),
        erosion:         quantize(erosion),
        depth:           quantize(depth),
        weirdness:       quantize(weirdness),
    }
}
```

---

## 3. Parameter (Range Type)

Each climate dimension in a `ParameterPoint` is represented as a **closed range**:

```rust
#[derive(Clone, Copy, Debug)]
pub struct Parameter {
    pub min: i64,
    pub max: i64,
}

impl Parameter {
    /// Point: min == max (used for single-value parameters).
    pub fn point(value: f32) -> Self {
        let q = quantize(value);
        Parameter { min: q, max: q }
    }

    /// Range: [min, max].
    pub fn span(min: f32, max: f32) -> Self {
        Parameter { min: quantize(min), max: quantize(max) }
    }

    /// Distance from this range to a target value.
    /// Returns 0 if target is inside [min, max].
    pub fn distance_to_value(&self, target: i64) -> i64 {
        let above = target - self.max;
        let below = self.min - target;
        if above > 0 { above } else { below.max(0) }
    }

    /// Distance between two ranges.
    /// Returns 0 if they overlap.
    pub fn distance_to_range(&self, other: &Parameter) -> i64 {
        let above = other.min - self.max;
        let below = self.min - other.max;
        if above > 0 { above } else { below.max(0) }
    }
}
```

JSON codec field names: `"min"`, `"max"` (as unquantized floats).

---

## 4. ParameterPoint

A biome's climate niche — 6 parameter ranges + offset tiebreaker:

```rust
pub struct ParameterPoint {
    pub temperature:     Parameter,
    pub humidity:        Parameter,
    pub continentalness: Parameter,
    pub erosion:         Parameter,
    pub depth:           Parameter,
    pub weirdness:       Parameter,
    pub offset:          i64,  // Single value, NOT a range
}

impl ParameterPoint {
    /// Squared L2 distance from this biome's niche to a target point.
    pub fn fitness(&self, target: &TargetPoint) -> i64 {
        sq(self.temperature.distance_to_value(target.temperature))
            + sq(self.humidity.distance_to_value(target.humidity))
            + sq(self.continentalness.distance_to_value(target.continentalness))
            + sq(self.erosion.distance_to_value(target.erosion))
            + sq(self.depth.distance_to_value(target.depth))
            + sq(self.weirdness.distance_to_value(target.weirdness))
            + sq(self.offset)
    }

    /// Convert to 7-element parameter-space representation for the RTree node.
    pub fn to_parameter_space(&self) -> [Parameter; 7] {
        [self.temperature, self.humidity, self.continentalness,
         self.erosion, self.depth, self.weirdness,
         Parameter { min: self.offset, max: self.offset }]
    }
}

fn sq(x: i64) -> i64 { x * x }
```

**Important**: `offset` is a *single i64* (not a range), but is treated as a point range in
the 7-element parameter space used by the RTree node bounds.

JSON codec (`"parameters"` field in biome source):
```json
{
  "temperature": {"min": -0.45, "max": -0.15},
  "humidity": {"min": -0.35, "max": 0.1},
  "continentalness": {"min": 0.03, "max": 1.0},
  "erosion": {"min": -0.375, "max": 0.05},
  "depth": {"min": 0.2, "max": 0.9},
  "weirdness": {"min": -0.56666, "max": -0.4},
  "offset": 0.0
}
```

---

## 5. RTree — 7-Dimensional Nearest-Neighbor Index

**Source**: `Climate.java` inner class `RTree` (lines 272–514).

### Node Types

```rust
enum Node<T> {
    Leaf {
        bounds: [Parameter; 7],  // Bounding box of this point
        value: T,
    },
    SubTree {
        bounds: [Parameter; 7],  // Bounding box of all children
        children: Vec<Node<T>>,  // Up to CHILDREN_PER_NODE = 6
    },
}
```

`bounds` for a leaf is exactly the 7-element parameter space of its `ParameterPoint`.
`bounds` for a subtree is the axis-aligned bounding box of all descendants.

### Distance from Node Bounds to Target

```rust
fn node_distance(bounds: &[Parameter; 7], target: &[i64; 7]) -> i64 {
    let mut dist = 0i64;
    for i in 0..7 {
        dist += sq(bounds[i].distance_to_value(target[i]));
    }
    dist
}
```

This is the **minimum possible** squared distance from any point within the node's bounding
box to the target. Used for pruning.

### Nearest-Neighbor Search (Branch-and-Bound)

```rust
fn search<T>(
    node: &Node<T>,
    target: &[i64; 7],
    best: Option<(&T, i64)>,  // (best_value, best_distance)
    fitness_fn: impl Fn(&T, &[i64; 7]) -> i64,
) -> (&T, i64) {
    match node {
        Node::Leaf { value, .. } => {
            let dist = fitness_fn(value, target);
            match best {
                None => (value, dist),
                Some((_, best_dist)) if dist < best_dist => (value, dist),
                Some(b) => b,
            }
        },
        Node::SubTree { bounds, children } => {
            let mut current_best = best;
            for child in children {
                let child_min_dist = node_distance(&child.bounds(), target);
                // PRUNING: skip if child bounds can't improve on current best
                if current_best.map_or(true, |(_, d)| child_min_dist < d) {
                    current_best = Some(search(child, target, current_best, &fitness_fn));
                }
            }
            current_best.unwrap()
        }
    }
}
```

**Time complexity**: O(log₆ N) average case due to pruning; O(N) worst case.

**Thread-local last-result cache**: The Java implementation stores the last leaf result per
thread. Subsequent nearby lookups often hit the same biome → skip most of the tree.

### Tree Construction

```rust
const CHILDREN_PER_NODE: usize = 6;

pub fn build<T: Clone>(entries: Vec<(ParameterPoint, T)>) -> RTree<T> {
    let leaves: Vec<Node<T>> = entries.into_iter()
        .map(|(pp, v)| Node::Leaf { bounds: pp.to_parameter_space(), value: v })
        .collect();

    let root = build_nodes(leaves);
    RTree { root }
}

fn build_nodes<T: Clone>(mut nodes: Vec<Node<T>>) -> Node<T> {
    if nodes.len() == 1 {
        return nodes.remove(0);
    }
    if nodes.len() <= CHILDREN_PER_NODE {
        let bounds = merge_bounds(nodes.iter().map(|n| n.bounds()));
        return Node::SubTree { bounds, children: nodes };
    }

    // Find the split dimension with minimum bounding-box cost
    let best_dim = (0..7).min_by_key(|&dim| {
        let sorted = sort_and_bucketize(&nodes, dim);
        sorted.iter().map(|bucket| cost(&merge_bounds(bucket.iter().map(|n| n.bounds())))).sum::<i64>()
    }).unwrap();

    // Pass 1 (cost estimation): sort by signed center along best dimension
    // Pass 2 (final grouping): sort by |center| (absolute) along best dimension
    nodes.sort_by_key(|n| {
        let b = &n.bounds()[best_dim];
        ((b.min + b.max) / 2).abs()  // absolute midpoint for final ordering
    });

    let bucket_size = expected_children_per_bucket(nodes.len());
    let children: Vec<Node<T>> = nodes
        .chunks(bucket_size)
        .map(|chunk| build_nodes(chunk.to_vec()))
        .collect();

    let bounds = merge_bounds(children.iter().map(|n| n.bounds()));
    Node::SubTree { bounds, children }
}

fn cost(bounds: &[Parameter; 7]) -> i64 {
    bounds.iter().map(|p| (p.max - p.min).abs()).sum()
}

fn expected_children_per_bucket(n: usize) -> usize {
    // Approximates: ceil(n / 6^floor(log6(n)))
    (6.0f64).powi(((n as f64 - 0.01).ln() / 6.0f64.ln()).floor() as i32) as usize
}
```

**Construction strategy**:
1. If ≤ 1 node: base case
2. If ≤ 6 nodes: create leaf subtree directly
3. Otherwise: try all 7 dimensions, pick the one that minimizes total bounding-box span
   after bucketizing → recursively build subtrees

---

## 6. Climate Sampler

Samples all 6 climate density functions at a quart position to produce a `TargetPoint`:

```rust
pub struct Sampler {
    pub temperature:     DensityFunction,
    pub humidity:        DensityFunction,
    pub continentalness: DensityFunction,
    pub erosion:         DensityFunction,
    pub depth:           DensityFunction,
    pub weirdness:       DensityFunction,
    pub spawn_target:    Vec<ParameterPoint>,
}

impl Sampler {
    pub fn sample(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> TargetPoint {
        let block_x = quart_x * 4;
        let block_y = quart_y * 4;
        let block_z = quart_z * 4;
        let ctx = SinglePointContext { x: block_x, y: block_y, z: block_z };
        target(
            self.temperature.compute(&ctx) as f32,
            self.humidity.compute(&ctx) as f32,
            self.continentalness.compute(&ctx) as f32,
            self.erosion.compute(&ctx) as f32,
            self.depth.compute(&ctx) as f32,
            self.weirdness.compute(&ctx) as f32,
        )
    }
}
```

Created from `NoiseRouter`'s 6 climate fields in `RandomState` initialization.

---

## 7. MultiNoiseBiomeSource

```rust
pub struct MultiNoiseBiomeSource {
    parameters: Either<ParameterList<Handle<Biome>>, Handle<MultiNoiseBiomeSourceParameterList>>,
}

impl MultiNoiseBiomeSource {
    pub fn get_noise_biome(
        &self,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
        sampler: &Sampler,
    ) -> Handle<Biome> {
        let target = sampler.sample(quart_x, quart_y, quart_z);
        self.parameters().find_value(&target)
    }
}
```

**JSON codec** (two forms):

**Form 1 — Direct biome list**:
```json
{
  "type": "minecraft:multi_noise",
  "biomes": [
    {
      "parameters": { "temperature": ..., "humidity": ..., ... },
      "biome": "minecraft:plains"
    }
  ]
}
```

**Form 2 — Preset reference**:
```json
{
  "type": "minecraft:multi_noise",
  "preset": "minecraft:overworld"
}
```

---

## 8. ParameterList

Holds the biome entries and the RTree index:

```rust
pub struct ParameterList<T> {
    pub values: Vec<(ParameterPoint, T)>,
    pub index:  RTree<T>,
}

impl<T: Clone> ParameterList<T> {
    pub fn new(values: Vec<(ParameterPoint, T)>) -> Self {
        let index = RTree::build(values.clone());
        ParameterList { values, index }
    }

    pub fn find_value(&self, target: &TargetPoint) -> &T {
        self.index.search(target)
    }
}
```

---

## 9. Presets

### Nether (5 entries, hardcoded)

| Biome | Temp | Humidity | Cont | Erosion | Depth | Weird | Offset |
|-------|------|----------|------|---------|-------|-------|--------|
| nether_wastes | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| soul_sand_valley | 0.0 | -0.5 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| crimson_forest | 0.4 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| warped_forest | 0.0 | 0.5 | 0.0 | 0.0 | 0.0 | 0.0 | 0.375 |
| basalt_deltas | -0.5 | 0.0 | 0.0 | 0.0 | 0.0 | 0.0 | 0.175 |

All Nether parameters are **point values** (min == max), not ranges.

### Overworld (100+ entries)

Generated by `OverworldBiomeBuilder.addBiomes()` — creates complex range-based entries
covering different combinations of temperature/humidity/continentalness/erosion/depth/weirdness
for all ~50 overworld biomes.

---

## 10. Key Source Locations

| File | Purpose |
|------|---------|
| `world/level/biome/Climate.java` (622 lines) | TargetPoint, Parameter, ParameterPoint, Sampler, RTree |
| `world/level/biome/MultiNoiseBiomeSource.java` | BiomeSource implementation, codec |
| `world/level/biome/MultiNoiseBiomeSourceParameterList.java` | NETHER/OVERWORLD preset definitions |
| `world/level/biome/OverworldBiomeBuilder.java` | Overworld biome parameter construction |
| `util/Mth.java` | `square(long)` helper |

## See also

- [../worldgen/05-biome-sources.md](../worldgen/05-biome-sources.md) — Java biome source spec (MultiNoise, TheEnd, Fixed)
- [../worldgen/04-biomes.md](../worldgen/04-biomes.md) — biome climate parameters used in R-tree search
- [12-chunk-pipeline.md](12-chunk-pipeline.md) — chunk pipeline calling biome source in the BIOMES stage
