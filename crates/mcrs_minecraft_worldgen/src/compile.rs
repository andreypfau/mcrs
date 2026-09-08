use crate::beta::seed::{BetaClimateNoises, BetaTerrainNoises};
use crate::bounds::{self, Bounds};
use crate::interval::Interval;
use crate::jmath;
use crate::material::compile::{MaterialInputs, compile_material};
use crate::node::distance::DistanceParams;
use crate::node::end_island::EndIslandParams;
use crate::node::gradient::GradientParams;
use crate::node::noise::NoiseFunctionParams;
use crate::node::spline::{CompiledSpline, Multipoint, SplineValue};
use crate::noise::blended::{BlendedNoise, NOISE_SEED};
use crate::noise::normal::NormalNoise;
use crate::noise::stack::{NoiseStack, Octave};
use crate::program::{
    BinaryOp, Node, NodeId, Program, RoundKind, UnaryOp, drops_offset, node_axes,
};
use crate::proto::{
    DensityFunctionHolder, NoiseHolder, NoiseParam, ProtoDensityFunction, ProtoSpline,
};
use crate::router::{NoiseGeneratorSettings, NoiseRouter, ROOT_NAMES};
use crate::strata::{Axes, NO_AXES};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, RandomSource};
use mcrs_voxel_storage::VoxelId;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// A kind that needs re-entrant evaluation at a substituted position, which
    /// the flat program cannot express yet.
    Unsupported(&'static str),
    UnknownFunction(String),
    UnknownNoise(String),
    ReferenceCycle(String),
    UnknownRule(String),
    UnknownCondition(String),
    UnknownBlockState(String),
    UnknownBiome(String),
    /// A condition kind this build parses but cannot evaluate.
    UnsupportedCondition(&'static str),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::Unsupported(kind) => write!(f, "unsupported density function: {kind}"),
            CompileError::UnknownFunction(id) => write!(f, "unknown density function: {id}"),
            CompileError::UnknownNoise(id) => write!(f, "unknown noise: {id}"),
            CompileError::ReferenceCycle(id) => write!(f, "reference cycle through {id}"),
            CompileError::UnknownRule(id) => write!(f, "unknown material rule: {id}"),
            CompileError::UnknownCondition(id) => write!(f, "unknown material condition: {id}"),
            CompileError::UnknownBlockState(id) => write!(f, "unknown block state: {id}"),
            CompileError::UnknownBiome(id) => write!(f, "unknown biome: {id}"),
            CompileError::UnsupportedCondition(kind) => write!(
                f,
                "material condition minecraft:{kind} cannot be evaluated by this build"
            ),
        }
    }
}

impl std::error::Error for CompileError {}

pub fn build_router(
    settings: &NoiseGeneratorSettings,
    registry: &BTreeMap<ResourceLocation, DensityFunctionHolder>,
    noises: &BTreeMap<ResourceLocation, NoiseParam>,
    seed: u64,
    default_block: VoxelId,
    default_fluid: VoxelId,
    material: Option<&MaterialInputs<'_>>,
) -> Result<NoiseRouter, CompileError> {
    let mut compiler = Compiler::new(registry, noises, seed, settings.legacy_random_source);
    let mut nodes = Vec::with_capacity(8);
    let mut failed = Vec::new();

    for (name, holder) in ROOT_NAMES.iter().zip(settings.noise_router.roots()) {
        match compiler.compile_root(holder) {
            Ok(id) => nodes.push(id),
            Err(error) => {
                tracing::warn!(root = name, %error, "density root did not compile");
                failed.push((*name, error));
                nodes.push(compiler.constant(0.0));
            }
        }
    }

    // A density root that fails degrades to a constant so the rest of the
    // terrain still generates; a material rule that fails does not, because
    // every column in the world would come out as bare stone.
    let material = match material {
        Some(inputs) => Some(compile_material(&mut compiler, &mut nodes, settings, inputs)?),
        None => None,
    };

    let program = compiler.into_program(nodes);
    Ok(NoiseRouter::new(
        program,
        failed,
        material,
        settings,
        seed,
        default_block,
        default_fluid,
    ))
}

pub struct Compiler<'a> {
    registry: &'a BTreeMap<ResourceLocation, DensityFunctionHolder>,
    noises: &'a BTreeMap<ResourceLocation, NoiseParam>,
    seed: u64,
    legacy_random: bool,
    random: RandomSource,
    nodes: Vec<Node>,
    axes: Vec<Axes>,
    node_ranges: Vec<Interval>,
    interned: HashMap<Key, NodeId>,
    compiled: HashMap<DensityFunctionHolder, NodeId>,
    inlined: HashMap<ResourceLocation, DensityFunctionHolder>,
    inlining: Vec<ResourceLocation>,
    samplers: HashMap<NoiseHolder, Arc<NoiseStack<Octave>>>,
    noise_params: HashMap<(usize, u64, u64), Arc<NoiseFunctionParams>>,
    splines: HashMap<ProtoSpline, Arc<CompiledSpline>>,
    blended: HashMap<[u64; 5], Arc<BlendedNoise>>,
    end_islands: Option<Arc<EndIslandParams>>,
    /// Both are an 82- and a 10-octave `LegacyRandom` walk; the ids that resolve
    /// to them are separate assets, so without this each one reseeds from zero.
    beta_terrain: Option<Arc<BetaTerrainNoises>>,
    beta_climate: Option<Arc<BetaClimateNoises>>,
    cells: usize,
}

impl<'a> Compiler<'a> {
    pub fn new(
        registry: &'a BTreeMap<ResourceLocation, DensityFunctionHolder>,
        noises: &'a BTreeMap<ResourceLocation, NoiseParam>,
        seed: u64,
        legacy_random: bool,
    ) -> Self {
        Self {
            registry,
            noises,
            seed,
            legacy_random,
            random: RandomSource::new(seed, legacy_random),
            nodes: Vec::new(),
            axes: Vec::new(),
            node_ranges: Vec::new(),
            interned: HashMap::new(),
            compiled: HashMap::new(),
            inlined: HashMap::new(),
            inlining: Vec::new(),
            samplers: HashMap::new(),
            noise_params: HashMap::new(),
            splines: HashMap::new(),
            blended: HashMap::new(),
            end_islands: None,
            beta_terrain: None,
            beta_climate: None,
            cells: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn compile_root(&mut self, holder: &DensityFunctionHolder) -> Result<NodeId, CompileError> {
        let inlined = self.inline(holder)?;
        self.compile(&inlined)
    }

    /// Drops every node no root can reach — a root that failed halfway leaves
    /// its partial subgraph behind, and `Program::fill` evaluates the whole
    /// array rather than a reachable subset.
    pub fn into_program(self, roots: Vec<NodeId>) -> Program {
        let (nodes, axes, ranges, roots) = prune(self.nodes, self.axes, self.node_ranges, roots);
        Program::new(nodes, axes, ranges, roots)
    }

    /// A node's stratum and its declared bound are both functions of its own
    /// kind and of its inputs, which are interned before it. Deriving them here
    /// is what keeps them off a second walk over the tree the node came from.
    fn intern(&mut self, key: Key, node: Node) -> NodeId {
        if let Some(&id) = self.interned.get(&key) {
            return id;
        }
        let axes = node_axes(&node, &self.axes);
        let range = self.declared_range(&node);
        self.push(key, node, axes, range)
    }

    fn declared_range(&self, node: &Node) -> Interval {
        let ranges = &self.node_ranges;
        bounds::node_bounds(node, Bounds::Declared, |id| ranges[id as usize])
            .expect("every kind declares a bound")
    }

    fn push(&mut self, key: Key, node: Node, axes: Axes, range: Interval) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(node);
        self.axes.push(axes);
        self.node_ranges.push(range);
        self.interned.insert(key, id);
        id
    }

    pub fn constant(&mut self, value: f32) -> NodeId {
        self.intern(Key::Constant(value.to_bits()), Node::Constant(value))
    }

    /// The value vanilla's fallback returns where this build has no blender and
    /// no beardifier, carrying the wider bound the real function declares. A key
    /// of its own keeps it off the plain constant of the same value, whose bound
    /// is exact and would delete branches vanilla keeps.
    fn declared_constant(&mut self, value: f32, declared: Interval) -> NodeId {
        let key = Key::DeclaredConstant(
            value.to_bits(),
            declared.min().to_bits(),
            declared.max().to_bits(),
        );
        if let Some(&id) = self.interned.get(&key) {
            return id;
        }
        self.push(key, Node::Constant(value), NO_AXES, declared)
    }

    fn as_constant(&self, id: NodeId) -> Option<f32> {
        match self.nodes[id as usize] {
            Node::Constant(value) => Some(value),
            _ => None,
        }
    }

    // --- reference inlining -------------------------------------------------

    fn inline(
        &mut self,
        holder: &DensityFunctionHolder,
    ) -> Result<DensityFunctionHolder, CompileError> {
        match holder {
            DensityFunctionHolder::Value(_) => Ok(holder.clone()),
            DensityFunctionHolder::Reference(id) => {
                if let Some(done) = self.inlined.get(id) {
                    return Ok(done.clone());
                }
                if self.inlining.contains(id) {
                    return Err(CompileError::ReferenceCycle(id.as_str().to_string()));
                }
                let registry = self.registry;
                let target = registry
                    .get(id)
                    .ok_or_else(|| CompileError::UnknownFunction(id.as_str().to_string()))?;
                self.inlining.push(id.clone());
                let result = self.inline(target);
                self.inlining.pop();
                let inlined = result?;
                self.inlined.insert(id.clone(), inlined.clone());
                Ok(inlined)
            }
            DensityFunctionHolder::Owned(function) => {
                // A cache memoizes for an engine that re-walks the graph per
                // position; the flat array evaluates every node once per fill,
                // so the marker is its input and is dropped here.
                if let ProtoDensityFunction::Cache(inner) = &**function {
                    return self.inline(&inner.input);
                }
                let mut failure = None;
                let rewritten = function.rewrite_children(&mut |child| match self.inline(child) {
                    Ok(inlined) => inlined,
                    Err(error) => {
                        failure.get_or_insert(error);
                        child.clone()
                    }
                });
                match failure {
                    Some(error) => Err(error),
                    None => Ok(DensityFunctionHolder::Owned(Box::new(rewritten))),
                }
            }
        }
    }

    // --- compilation --------------------------------------------------------

    fn compile(&mut self, holder: &DensityFunctionHolder) -> Result<NodeId, CompileError> {
        if let Some(&id) = self.compiled.get(holder) {
            return Ok(id);
        }
        let id = self.compile_uncached(holder)?;
        self.compiled.insert(holder.clone(), id);
        Ok(id)
    }

    fn compile_uncached(&mut self, holder: &DensityFunctionHolder) -> Result<NodeId, CompileError> {
        let function = match holder {
            DensityFunctionHolder::Value(value) => return Ok(self.constant(value.value.0 as f32)),
            DensityFunctionHolder::Reference(id) => {
                unreachable!("reference {id} survived inlining")
            }
            DensityFunctionHolder::Owned(function) => function,
        };
        use ProtoDensityFunction as P;
        match &**function {
            P::Constant(value) => Ok(self.constant(value.value.0 as f32)),

            // No blender and no beardifier here, so each is the value vanilla's
            // fallback returns. Their declared ranges stay conservative, which
            // is what keeps branch elimination from deleting a branch vanilla
            // would keep.
            P::BlendAlpha => Ok(self.declared_constant(1.0, Interval::of(0.0, 1.0))),
            P::BlendOffset => Ok(self.declared_constant(0.0, Interval::INFINITE)),
            P::Beardifier => Ok(self.declared_constant(0.0, Interval::INFINITE)),
            P::BlendDensity(x) => self.compile(&x.input),

            P::Cache(_) => unreachable!("cache survived inlining"),

            P::Gradient(g) => {
                let params = GradientParams {
                    axis: g.axis,
                    tiling: g.tiling,
                    from: g.from_coordinate as f32,
                    to: g.to_coordinate as f32,
                    from_value: g.from_value.0 as f32,
                    to_value: g.to_value.0 as f32,
                };
                let key = Key::Gradient(
                    g.axis.bit(),
                    tiling_tag(g.tiling),
                    params.from.to_bits(),
                    params.to.to_bits(),
                    params.from_value.to_bits(),
                    params.to_value.to_bits(),
                );
                Ok(self.intern(key, Node::Gradient(params)))
            }

            P::DistanceToPoint { point, metric } => {
                let params = DistanceParams::new(point[0], point[1], point[2], *metric);
                Ok(self.intern(Key::DistanceToPoint(params), Node::DistanceToPoint(params)))
            }

            P::EndOuterIslands => {
                let params = match &self.end_islands {
                    Some(params) => Arc::clone(params),
                    None => {
                        let params = Arc::new(EndIslandParams::new(self.seed));
                        self.end_islands = Some(Arc::clone(&params));
                        params
                    }
                };
                let key = Key::EndOuterIslands(Arc::as_ptr(&params) as usize);
                Ok(self.intern(key, Node::EndOuterIslands(params)))
            }

            P::OldBlendedNoise(x) => {
                let params = self.blended_params(
                    x.xz_scale.0,
                    x.y_scale.0,
                    x.xz_factor.0,
                    x.y_factor.0,
                    x.smear_scale_multiplier.0,
                );
                let key = Key::OldBlendedNoise(Arc::as_ptr(&params) as usize);
                Ok(self.intern(key, Node::OldBlendedNoise(params)))
            }

            P::Noise {
                noise,
                xz_scale,
                y_scale,
                shift_x,
                shift_y,
                shift_z,
            } => {
                let sampler = self.noise_sampler(noise)?;
                let params = self.noise_params(&sampler, xz_scale.0, y_scale.0);
                let pointer = Arc::as_ptr(&params) as usize;
                if shift_x.is_zero_constant()
                    && shift_y.is_zero_constant()
                    && shift_z.is_zero_constant()
                {
                    return Ok(self.intern(Key::Noise(pointer), Node::Noise { params }));
                }
                let x = self.compile(shift_x)?;
                let y = self.compile(shift_y)?;
                let z = self.compile(shift_z)?;
                Ok(self.intern(
                    Key::ShiftedNoise(pointer, x, y, z),
                    Node::ShiftedNoise { params, x, y, z },
                ))
            }

            // `shift` and `shift_a` are a plain noise scaled by four, so they
            // lower onto nodes a bare `noise` of the same parameters shares.
            P::ShiftA { noise } => self.compile_shift(noise, 0.0),
            P::Shift { noise } => self.compile_shift(noise, 0.25),
            P::ShiftB { noise } => {
                let sampler = self.noise_sampler(noise)?;
                let params = self.shift_b_params(&sampler);
                let key = Key::ShiftB(Arc::as_ptr(&params) as usize);
                Ok(self.intern(key, Node::ShiftB { params }))
            }

            P::Abs(x) => self.compile_unary(UnaryOp::Abs, &x.input),
            P::Square(x) => self.compile_unary(UnaryOp::Square, &x.input),
            P::Cube(x) => self.compile_unary(UnaryOp::Cube, &x.input),
            P::Sqrt(x) => self.compile_unary(UnaryOp::Sqrt, &x.input),
            P::Reciprocal(x) => self.compile_unary(UnaryOp::Reciprocal, &x.input),
            P::Negate(x) => self.compile_unary(UnaryOp::Negate, &x.input),
            P::Squeeze(x) => self.compile_unary(UnaryOp::Squeeze, &x.input),
            P::Log(x) => self.compile_unary(UnaryOp::Log, &x.input),
            P::Sign(x) => self.compile_unary(UnaryOp::Sign, &x.input),

            P::HalfNegative(x) => {
                let input = self.compile(&x.input)?;
                Ok(self.leaky_relu(input, 0.5))
            }
            P::QuarterNegative(x) => {
                let input = self.compile(&x.input)?;
                Ok(self.leaky_relu(input, 0.25))
            }

            P::Clamp(x) => {
                let input = self.compile(&x.input)?;
                let (min, max) = (x.min.0 as f32, x.max.0 as f32);
                if let Some(value) = self.as_constant(input) {
                    return Ok(self.constant(jmath::clampf(value, min, max)));
                }
                Ok(self.intern(
                    Key::Clamp(input, min.to_bits(), max.to_bits()),
                    Node::Clamp { input, min, max },
                ))
            }

            P::Floor(x) => self.compile_round(RoundKind::Floor, &x.input, &x.multiple),
            P::Round(x) => self.compile_round(RoundKind::Round, &x.input, &x.multiple),
            P::Ceil(x) => self.compile_round(RoundKind::Ceil, &x.input, &x.multiple),
            P::Truncate(x) => self.compile_round(RoundKind::Truncate, &x.input, &x.multiple),

            P::Add(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                Ok(self.add(left, right))
            }
            P::Sub(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                Ok(self.sub(left, right))
            }
            P::Mul(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                Ok(self.mul(left, right))
            }
            P::Div(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                Ok(self.div(left, right))
            }
            P::Min(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                Ok(self.extremum(BinaryOp::Min, left, right))
            }
            P::Max(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                Ok(self.extremum(BinaryOp::Max, left, right))
            }

            P::Pow(x) => {
                let (base, exponent) = self.operands(&x.base, &x.exponent)?;
                Ok(self.pow(base, exponent))
            }

            P::Lerp {
                alpha,
                first,
                second,
            } => {
                let alpha = self.compile(alpha)?;
                let first = self.compile(first)?;
                let second = self.compile(second)?;
                Ok(self.lerp(alpha, first, second))
            }

            P::RangeChoice {
                input,
                min_inclusive,
                max_exclusive,
                when_in_range,
                when_out_of_range,
            } => {
                let input = self.compile(input)?;
                let when_in = self.compile(when_in_range)?;
                let when_out = self.compile(when_out_of_range)?;
                let min_inclusive = min_inclusive.0 as f32;
                let max_exclusive = max_exclusive.0 as f32;
                match (self.as_constant(when_in), self.as_constant(when_out)) {
                    (Some(when_in), Some(when_out)) => Ok(self.intern(
                        Key::ConstRangeChoice(
                            input,
                            min_inclusive.to_bits(),
                            max_exclusive.to_bits(),
                            when_in.to_bits(),
                            when_out.to_bits(),
                        ),
                        Node::ConstRangeChoice {
                            input,
                            min_inclusive,
                            max_exclusive,
                            when_in,
                            when_out,
                        },
                    )),
                    _ => Ok(self.intern(
                        Key::RangeChoice(
                            input,
                            min_inclusive.to_bits(),
                            max_exclusive.to_bits(),
                            when_in,
                            when_out,
                        ),
                        Node::RangeChoice {
                            input,
                            min_inclusive,
                            max_exclusive,
                            when_in,
                            when_out,
                        },
                    )),
                }
            }

            P::IntervalSelect(x) => {
                let input = self.compile(&x.input)?;
                let mut arms = Vec::with_capacity(x.functions.len());
                for function in &x.functions {
                    arms.push(self.compile(function)?);
                }
                let thresholds: Vec<f32> = x.thresholds.iter().map(|t| t.0 as f32).collect();
                if let [threshold] = thresholds[..] {
                    let below = arms[0];
                    let above = arms[arms.len() - 1];
                    return Ok(self.intern(
                        Key::SingleThreshold(input, threshold.to_bits(), below, above),
                        Node::SingleThreshold {
                            input,
                            threshold,
                            below,
                            above,
                        },
                    ));
                }
                let key = Key::IntervalSelect(
                    input,
                    thresholds.iter().map(|t| t.to_bits()).collect(),
                    arms.clone(),
                );
                Ok(self.intern(
                    key,
                    Node::IntervalSelect {
                        input,
                        thresholds: thresholds.into(),
                        arms: arms.into(),
                    },
                ))
            }

            P::Spline { spline } => self.compile_spline(spline),

            // A slice whose input already ignores the sliced axis is the
            // identity; anything else needs the input evaluated at a
            // substituted coordinate, which the flat program cannot do.
            P::Slice { axis, input, .. } => {
                let input = self.compile(input)?;
                if self.axes[input as usize] & axis.bit() == 0 {
                    return Ok(input);
                }
                Err(CompileError::Unsupported("slice"))
            }
            P::Interpolated {
                input,
                cell_size_xz,
                cell_size_y,
            } => {
                let input = self.compile(input)?;
                let key = Key::Interpolated(input, cell_size_xz.get(), cell_size_y.get());
                if let Some(&id) = self.interned.get(&key) {
                    return Ok(id);
                }
                let cell = self.cells;
                self.cells += 1;
                // The lattice is addressed by the column being filled, which the
                // per-fill phase has none of, so this never lands there however
                // little its input varies.
                Ok(self.intern(
                    key,
                    Node::Interpolated {
                        input,
                        cell_xz: cell_size_xz.get() as i32,
                        cell_y: cell_size_y.get() as i32,
                        cell,
                    },
                ))
            }

            P::FindTopSurface(x) => {
                let density = self.compile(&x.density)?;
                let upper_bound = self.compile(&x.upper_bound)?;
                let lower_bound = x.lower_bound;
                let cell_height = x.cell_height.get();
                Ok(self.intern(
                    Key::FindTopSurface(density, upper_bound, lower_bound, cell_height),
                    Node::FindTopSurface {
                        density,
                        upper_bound,
                        lower_bound,
                        cell_height: cell_height as i32,
                    },
                ))
            }
        }
    }

    fn operands(
        &mut self,
        left: &DensityFunctionHolder,
        right: &DensityFunctionHolder,
    ) -> Result<(NodeId, NodeId), CompileError> {
        Ok((self.compile(left)?, self.compile(right)?))
    }

    fn compile_unary(
        &mut self,
        op: UnaryOp,
        input: &DensityFunctionHolder,
    ) -> Result<NodeId, CompileError> {
        let input = self.compile(input)?;
        Ok(self.unary(op, input))
    }

    fn compile_round(
        &mut self,
        kind: RoundKind,
        input: &DensityFunctionHolder,
        multiple: &DensityFunctionHolder,
    ) -> Result<NodeId, CompileError> {
        let input = self.compile(input)?;
        let multiple = self.compile(multiple)?;
        Ok(self.round(kind, input, multiple))
    }

    fn compile_shift(&mut self, noise: &NoiseHolder, y_scale: f64) -> Result<NodeId, CompileError> {
        let sampler = self.noise_sampler(noise)?;
        let params = self.noise_params(&sampler, 0.25, y_scale);
        let key = Key::Noise(Arc::as_ptr(&params) as usize);
        let inner = self.intern(key, Node::Noise { params });
        Ok(self.affine(inner, 4.0, 0.0))
    }

    fn compile_spline(&mut self, spline: &ProtoSpline) -> Result<NodeId, CompileError> {
        if let ProtoSpline::Constant(value) = spline {
            return Ok(self.constant(*value));
        }
        let mut order: Vec<DensityFunctionHolder> = Vec::new();
        spline.visit_coordinates(&mut |coordinate| {
            if !order.contains(coordinate) {
                order.push(coordinate.clone());
            }
        });
        let mut coords = Vec::with_capacity(order.len());
        for coordinate in &order {
            coords.push(self.compile(coordinate)?);
        }
        let compiled = match self.splines.get(spline) {
            Some(compiled) => Arc::clone(compiled),
            None => {
                let compiled = Arc::new(CompiledSpline {
                    root: spline_value(spline, &order),
                });
                self.splines.insert(spline.clone(), Arc::clone(&compiled));
                compiled
            }
        };
        let key = Key::Spline(Arc::as_ptr(&compiled) as *const u8 as usize, coords.clone());
        Ok(self.intern(
            key,
            Node::Spline {
                spline: compiled,
                coords: coords.into(),
            },
        ))
    }

    // --- the specialization ladder -----------------------------------------

    fn unary(&mut self, op: UnaryOp, input: NodeId) -> NodeId {
        if let Some(value) = self.as_constant(input) {
            return self.constant(op.apply(value));
        }
        self.intern(Key::Unary(unary_tag(op), input), Node::Unary { op, input })
    }

    /// The rectifier vanilla emits for `half_negative` and `quarter_negative`:
    /// unit slope above zero, `negative_scale` below.
    fn leaky_relu(&mut self, input: NodeId, negative_scale: f32) -> NodeId {
        self.piecewise_affine(input, negative_scale, 1.0, 0.0)
    }

    /// `input * scale + offset`, with the two chained forms vanilla emits as
    /// separate samplers folded into one node. Only the scale-then-offset order
    /// fuses: `(v * s + 0) + o` and `v * s + o` are the same two roundings,
    /// while `(v * s1) * s2` and `v * (s1 * s2)` are not.
    fn affine(&mut self, input: NodeId, scale: f32, offset: f32) -> NodeId {
        if let Some(value) = self.as_constant(input) {
            return self.constant(jmath::mul_add(value, scale, offset));
        }
        if scale == 1.0 && offset == 0.0 {
            return input;
        }
        let fusion = match &self.nodes[input as usize] {
            Node::Affine {
                input: inner,
                scale: inner_scale,
                offset: inner_offset,
            } if scale == 1.0 && *inner_offset == 0.0 => Fusion::Affine(*inner, *inner_scale),
            // Multiplying by a power of two is exact, so `(v*m)*s` and
            // `v*(m*s)` round once on the same real number and the two slopes
            // may absorb the outer scale. The rectifier's are 0.5 and 0.25.
            Node::PiecewiseAffine {
                input: inner,
                neg_scale,
                pos_scale,
                offset: inner_offset,
            } if *inner_offset == 0.0
                && (scale == 1.0
                    || (is_power_of_two(*neg_scale) && is_power_of_two(*pos_scale))) =>
            {
                Fusion::PiecewiseAffine(*inner, *neg_scale * scale, *pos_scale * scale)
            }
            _ => Fusion::None,
        };
        match fusion {
            Fusion::Affine(inner, inner_scale) => self.affine(inner, inner_scale, offset),
            Fusion::PiecewiseAffine(inner, neg_scale, pos_scale) => {
                self.piecewise_affine(inner, neg_scale, pos_scale, offset)
            }
            Fusion::None => self.intern(
                Key::Affine(input, scale.to_bits(), offset.to_bits()),
                Node::Affine {
                    input,
                    scale,
                    offset,
                },
            ),
        }
    }

    fn piecewise_affine(
        &mut self,
        input: NodeId,
        neg_scale: f32,
        pos_scale: f32,
        offset: f32,
    ) -> NodeId {
        if let Some(value) = self.as_constant(input) {
            let scale = if value < 0.0 { neg_scale } else { pos_scale };
            return self.constant(if drops_offset(offset) {
                value * scale
            } else {
                jmath::mul_add(value, scale, offset)
            });
        }
        self.intern(
            Key::PiecewiseAffine(
                input,
                neg_scale.to_bits(),
                pos_scale.to_bits(),
                offset.to_bits(),
            ),
            Node::PiecewiseAffine {
                input,
                neg_scale,
                pos_scale,
                offset,
            },
        )
    }

    fn binary(&mut self, op: BinaryOp, a: NodeId, b: NodeId) -> NodeId {
        self.intern(Key::Binary(binary_tag(op), a, b), Node::Binary { op, a, b })
    }

    fn add(&mut self, left: NodeId, right: NodeId) -> NodeId {
        match (self.as_constant(left), self.as_constant(right)) {
            (Some(a), Some(b)) => self.constant(a + b),
            (Some(c), None) => self.affine(right, 1.0, c),
            (None, Some(c)) => self.affine(left, 1.0, c),
            (None, None) => self.binary(BinaryOp::Add, left, right),
        }
    }

    fn sub(&mut self, left: NodeId, right: NodeId) -> NodeId {
        match (self.as_constant(left), self.as_constant(right)) {
            (Some(a), Some(b)) => self.constant(a - b),
            (Some(value), None) => self.intern(
                Key::ConstSub(right, value.to_bits()),
                Node::ConstSub {
                    input: right,
                    value,
                },
            ),
            // `x - c` is `x + (-c)`: negation is exact in binary floating point.
            (None, Some(c)) => self.affine(left, 1.0, -c),
            (None, None) => self.binary(BinaryOp::Sub, left, right),
        }
    }

    fn mul(&mut self, left: NodeId, right: NodeId) -> NodeId {
        match (self.as_constant(left), self.as_constant(right)) {
            (Some(a), Some(b)) => self.constant(a * b),
            (Some(c), None) => self.affine(right, c, 0.0),
            (None, Some(c)) => self.affine(left, c, 0.0),
            (None, None) => self.binary(BinaryOp::Mul, left, right),
        }
    }

    fn div(&mut self, left: NodeId, right: NodeId) -> NodeId {
        match (self.as_constant(left), self.as_constant(right)) {
            (Some(a), Some(b)) => self.constant(a / b),
            (Some(value), None) => self.intern(
                Key::ConstDiv(right, value.to_bits()),
                Node::ConstDiv {
                    input: right,
                    value,
                },
            ),
            // Vanilla compiles `x / c` to the reciprocal multiply, not to a
            // division, and the two disagree in the last bit for a divisor that
            // is not a power of two.
            (None, Some(c)) => self.affine(left, 1.0 / c, 0.0),
            (None, None) => self.binary(BinaryOp::Div, left, right),
        }
    }

    fn extremum(&mut self, op: BinaryOp, left: NodeId, right: NodeId) -> NodeId {
        let left_range = self.node_ranges[left as usize];
        let right_range = self.node_ranges[right as usize];
        let constants = (self.as_constant(left), self.as_constant(right));
        // The accumulator is the operand that is not the baked-in constant, so
        // the fold takes the operands in the order the sampler would.
        if let (Some(a), Some(b)) = constants {
            let folded = match op {
                BinaryOp::Min => jmath::vmin(b, a),
                _ => jmath::vmax(b, a),
            };
            return self.constant(folded);
        }
        let (left_wins, right_wins) = match op {
            BinaryOp::Min => (
                left_range.max() < right_range.min(),
                right_range.max() < left_range.min(),
            ),
            _ => (
                left_range.min() > right_range.max(),
                right_range.min() > left_range.max(),
            ),
        };
        if left_wins || right_wins {
            tracing::warn!(
                operation = ?op,
                left = ?(left_range.min(), left_range.max()),
                right = ?(right_range.min(), right_range.max()),
                "compiling an extremum between two non-overlapping inputs"
            );
            return if left_wins { left } else { right };
        }
        match constants {
            (Some(value), None) => self.const_extremum(op, right, value),
            (None, Some(value)) => self.const_extremum(op, left, value),
            _ => self.binary(op, left, right),
        }
    }

    fn const_extremum(&mut self, op: BinaryOp, input: NodeId, value: f32) -> NodeId {
        let bits = value.to_bits();
        match op {
            BinaryOp::Min => {
                self.intern(Key::ConstMin(input, bits), Node::ConstMin { input, value })
            }
            _ => self.intern(Key::ConstMax(input, bits), Node::ConstMax { input, value }),
        }
    }

    fn pow(&mut self, base: NodeId, exponent: NodeId) -> NodeId {
        match (self.as_constant(base), self.as_constant(exponent)) {
            (Some(a), Some(b)) => self.constant(jmath::pow(a, b)),
            (Some(value), None) => self.intern(
                Key::ConstBasePow(value.to_bits(), exponent),
                Node::ConstBasePow {
                    base: value,
                    exponent,
                },
            ),
            (None, Some(value)) => self.const_exponent_pow(base, value),
            (None, None) => self.intern(Key::Pow(base, exponent), Node::Pow { base, exponent }),
        }
    }

    fn const_exponent_pow(&mut self, base: NodeId, exponent: f32) -> NodeId {
        let magnitude = exponent.abs();
        let special = if magnitude == 0.5 {
            Some(self.unary(UnaryOp::Sqrt, base))
        } else if magnitude == 1.0 {
            Some(base)
        } else if magnitude == 2.0 {
            Some(self.unary(UnaryOp::Square, base))
        } else if magnitude == 3.0 {
            Some(self.unary(UnaryOp::Cube, base))
        } else {
            None
        };
        match special {
            Some(id) if exponent >= 0.0 => id,
            Some(id) => self.unary(UnaryOp::Reciprocal, id),
            None => self.intern(
                Key::ConstExponentPow(base, exponent.to_bits()),
                Node::ConstExponentPow {
                    input: base,
                    exponent,
                },
            ),
        }
    }

    fn round(&mut self, kind: RoundKind, input: NodeId, multiple: NodeId) -> NodeId {
        if let Some(m) = self.as_constant(multiple) {
            if let Some(value) = self.as_constant(input) {
                let folded = if m == 0.0 {
                    value
                } else {
                    kind.apply(value / m) * m
                };
                return self.constant(folded);
            }
            // A zero multiple returns the input untouched, which is the one case
            // `IntegerMultipleRound` cannot express.
            if m == 0.0 {
                return input;
            }
            return self.intern(
                Key::IntegerMultipleRound(input, m.to_bits(), round_tag(kind)),
                Node::IntegerMultipleRound {
                    input,
                    multiple: m,
                    kind,
                },
            );
        }
        self.intern(
            Key::Round(input, multiple, round_tag(kind)),
            Node::Round {
                value: input,
                multiple,
                kind,
            },
        )
    }

    fn lerp(&mut self, alpha: NodeId, first: NodeId, second: NodeId) -> NodeId {
        let constants = (
            self.as_constant(alpha),
            self.as_constant(first),
            self.as_constant(second),
        );
        if let (Some(a), Some(f), Some(s)) = constants {
            return self.constant(jmath::sampler_lerp(a, f, s));
        }
        if let Some(value) = constants.1 {
            return self.intern(
                Key::ConstFirstLerp(alpha, value.to_bits(), second),
                Node::ConstFirstLerp {
                    alpha,
                    first: value,
                    second,
                },
            );
        }
        if let Some(value) = constants.2 {
            return self.intern(
                Key::ConstSecondLerp(alpha, first, value.to_bits()),
                Node::ConstSecondLerp {
                    alpha,
                    first,
                    second: value,
                },
            );
        }
        self.intern(
            Key::Lerp(alpha, first, second),
            Node::Lerp {
                alpha,
                first,
                second,
            },
        )
    }

    // --- noise construction -------------------------------------------------

    fn noise_params(
        &mut self,
        sampler: &Arc<NoiseStack<Octave>>,
        xz_scale: f64,
        y_scale: f64,
    ) -> Arc<NoiseFunctionParams> {
        let key = (
            Arc::as_ptr(sampler) as usize,
            xz_scale.to_bits(),
            y_scale.to_bits(),
        );
        if let Some(params) = self.noise_params.get(&key) {
            return Arc::clone(params);
        }
        let params = Arc::new(NoiseFunctionParams::new(
            Arc::clone(sampler),
            xz_scale,
            y_scale,
        ));
        self.noise_params.insert(key, Arc::clone(&params));
        params
    }

    fn shift_b_params(&mut self, sampler: &Arc<NoiseStack<Octave>>) -> Arc<NoiseFunctionParams> {
        let key = (Arc::as_ptr(sampler) as usize, u64::MAX, u64::MAX);
        if let Some(params) = self.noise_params.get(&key) {
            return Arc::clone(params);
        }
        let params = Arc::new(NoiseFunctionParams::shift_b(Arc::clone(sampler)));
        self.noise_params.insert(key, Arc::clone(&params));
        params
    }

    fn blended_params(
        &mut self,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Arc<BlendedNoise> {
        let key = [
            xz_scale.to_bits(),
            y_scale.to_bits(),
            xz_factor.to_bits(),
            y_factor.to_bits(),
            smear_scale_multiplier.to_bits(),
        ];
        if let Some(params) = self.blended.get(&key) {
            return Arc::clone(params);
        }
        // Beta seeds the three stacks straight off the world seed instead of
        // forking the terrain hash, and the two streams are unrelated.
        let mut random = if self.legacy_random {
            RandomSource::new(self.seed, true)
        } else {
            let mut root = self.random.clone();
            root.fork_hash(NOISE_SEED)
        };
        let params = Arc::new(BlendedNoise::new(
            &mut random,
            xz_scale,
            y_scale,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
        ));
        self.blended.insert(key, Arc::clone(&params));
        params
    }

    pub(crate) fn noise_sampler(
        &mut self,
        holder: &NoiseHolder,
    ) -> Result<Arc<NoiseStack<Octave>>, CompileError> {
        if let Some(sampler) = self.samplers.get(holder) {
            return Ok(Arc::clone(sampler));
        }
        let sampler = Arc::new(self.create_noise(holder)?);
        self.samplers.insert(holder.clone(), Arc::clone(&sampler));
        Ok(sampler)
    }

    fn create_noise(&mut self, holder: &NoiseHolder) -> Result<NoiseStack<Octave>, CompileError> {
        match holder {
            NoiseHolder::Owned(param) => {
                let mut random = self.random.clone();
                Ok(NormalNoise::new(param.clone()).create(&mut random))
            }
            NoiseHolder::Reference(id) => self.create_named_noise(id),
        }
    }

    fn create_named_noise(
        &mut self,
        id: &ResourceLocation,
    ) -> Result<NoiseStack<Octave>, CompileError> {
        // The two nether climate noises are seeded from the raw world seed, not
        // from the hashed fork every other noise takes.
        match id.as_str() {
            "minecraft:nether/temperature" => {
                return Ok(NormalNoise::create_parity(-7, &[1.0, 1.0])
                    .create(&mut LegacyRandom::new(self.seed)));
            }
            "minecraft:nether/vegetation" => {
                return Ok(NormalNoise::create_parity(-7, &[1.0, 1.0])
                    .create(&mut LegacyRandom::new(self.seed.wrapping_add(1))));
            }
            _ => {}
        }

        if self.legacy_random
            && let Some(sampler) = self.create_legacy_noise(id)
        {
            return Ok(sampler);
        }

        let noises = self.noises;
        let param = noises
            .get(id)
            .ok_or_else(|| CompileError::UnknownNoise(id.as_str().to_string()))?;
        let mut root = self.random.clone();
        let mut random = root.fork_hash(id.as_str());
        Ok(NormalNoise::new(param.clone()).create(&mut random))
    }

    /// The unnamed `PositionalRandomFactory` the whole generator forks from,
    /// which the surface depth draw and the iceberg draw use directly.
    pub(crate) fn positional_random(&self) -> RandomSource {
        self.random.clone()
    }

    /// `PositionalRandomFactory` for a named stream: the returned source is the
    /// factory itself, and `fork_at` on a clone of it is one `at(x, y, z)`.
    pub(crate) fn hashed_random(&self, name: &str) -> RandomSource {
        let mut root = self.random.clone();
        root.fork_hash(name)
    }

    fn beta_terrain(&mut self) -> Arc<BetaTerrainNoises> {
        Arc::clone(
            self.beta_terrain
                .get_or_insert_with(|| Arc::new(BetaTerrainNoises::new(self.seed))),
        )
    }

    fn beta_climate(&mut self) -> Arc<BetaClimateNoises> {
        Arc::clone(
            self.beta_climate
                .get_or_insert_with(|| Arc::new(BetaClimateNoises::new(self.seed))),
        )
    }

    /// The Beta noises are drawn from the pre-26.3 `LegacyRandom` streams and
    /// have no `worldgen/noise` entry, so the ids are resolved here instead.
    fn create_legacy_noise(&mut self, id: &ResourceLocation) -> Option<NoiseStack<Octave>> {
        match id.as_str() {
            "minecraft:offset" => {
                let mut root = self.random.clone();
                let mut random = root.fork_hash("minecraft:offset");
                Some(NormalNoise::create_parity(0, &[0.0]).create(&mut random))
            }
            "mcrs:beta/scale" => Some(self.beta_terrain().scale.clone()),
            "mcrs:beta/depth" => Some(self.beta_terrain().depth.clone()),
            "mcrs:beta/temperature" => Some(self.beta_climate().temperature.clone()),
            "mcrs:beta/vegetation" => Some(self.beta_climate().vegetation.clone()),
            "mcrs:beta/climate_detail" => Some(self.beta_climate().detail.clone()),
            _ => None,
        }
    }
}

/// Drops every node no root can reach — a root that failed halfway leaves its
/// partial subgraph behind, and `Program::fill` evaluates the whole array rather
/// than a reachable subset. Compaction preserves order, so the result is still
/// topologically sorted.
fn prune(
    mut nodes: Vec<Node>,
    mut axes: Vec<Axes>,
    mut ranges: Vec<Interval>,
    roots: Vec<NodeId>,
) -> (Vec<Node>, Vec<Axes>, Vec<Interval>, Vec<NodeId>) {
    let mut live = vec![false; nodes.len()];
    for &root in &roots {
        live[root as usize] = true;
    }
    for i in (0..nodes.len()).rev() {
        if live[i] {
            nodes[i].visit_inputs(&mut |j| live[j as usize] = true);
        }
    }

    let mut map = vec![0 as NodeId; nodes.len()];
    let mut next = 0usize;
    for i in 0..nodes.len() {
        map[i] = next as NodeId;
        if live[i] {
            nodes.swap(next, i);
            axes.swap(next, i);
            ranges.swap(next, i);
            next += 1;
        }
    }
    nodes.truncate(next);
    axes.truncate(next);
    ranges.truncate(next);
    for node in nodes.iter_mut() {
        remap_inputs(node, &map);
    }
    let roots = roots.into_iter().map(|root| map[root as usize]).collect();
    (nodes, axes, ranges, roots)
}

enum Fusion {
    None,
    Affine(NodeId, f32),
    PiecewiseAffine(NodeId, f32, f32),
}

/// Excludes zero and the infinities, both of which share a power of two's zero
/// mantissa.
fn is_power_of_two(value: f32) -> bool {
    value != 0.0 && value.is_finite() && value.to_bits() & 0x007f_ffff == 0
}

fn spline_value(spline: &ProtoSpline, order: &[DensityFunctionHolder]) -> SplineValue {
    match spline {
        ProtoSpline::Constant(value) => SplineValue::Constant(*value),
        ProtoSpline::Multipoint(multipoint) => {
            let coord = order
                .iter()
                .position(|holder| holder == &multipoint.coordinate)
                .expect("every coordinate was collected by the same pre-order walk");
            SplineValue::Multipoint(Box::new(Multipoint {
                coord,
                locations: multipoint.locations.clone(),
                derivatives: multipoint.derivatives.clone(),
                values: multipoint
                    .values
                    .iter()
                    .map(|value| spline_value(value, order))
                    .collect(),
            }))
        }
    }
}

fn remap_inputs(node: &mut Node, map: &[NodeId]) {
    let rewrite = |id: &mut NodeId| *id = map[*id as usize];
    match node {
        Node::Constant(_)
        | Node::Gradient(_)
        | Node::Noise { .. }
        | Node::ShiftB { .. }
        | Node::DistanceToPoint(_)
        | Node::EndOuterIslands(_)
        | Node::OldBlendedNoise(_) => {}

        Node::Affine { input, .. }
        | Node::PiecewiseAffine { input, .. }
        | Node::Unary { input, .. }
        | Node::Clamp { input, .. }
        | Node::ConstMin { input, .. }
        | Node::ConstMax { input, .. }
        | Node::ConstSub { input, .. }
        | Node::ConstDiv { input, .. }
        | Node::ConstExponentPow { input, .. }
        | Node::IntegerMultipleRound { input, .. }
        | Node::ConstRangeChoice { input, .. } => rewrite(input),
        Node::ConstBasePow { exponent, .. } => rewrite(exponent),

        Node::Binary { a, b, .. } => {
            rewrite(a);
            rewrite(b);
        }
        Node::Pow { base, exponent } => {
            rewrite(base);
            rewrite(exponent);
        }
        Node::Round {
            value, multiple, ..
        } => {
            rewrite(value);
            rewrite(multiple);
        }
        Node::Lerp {
            alpha,
            first,
            second,
        } => {
            rewrite(alpha);
            rewrite(first);
            rewrite(second);
        }
        Node::ConstFirstLerp { alpha, second, .. } => {
            rewrite(alpha);
            rewrite(second);
        }
        Node::ConstSecondLerp { alpha, first, .. } => {
            rewrite(alpha);
            rewrite(first);
        }
        Node::ShiftedNoise { x, y, z, .. } => {
            rewrite(x);
            rewrite(y);
            rewrite(z);
        }
        Node::RangeChoice {
            input,
            when_in,
            when_out,
            ..
        } => {
            rewrite(input);
            rewrite(when_in);
            rewrite(when_out);
        }
        Node::SingleThreshold {
            input,
            below,
            above,
            ..
        } => {
            rewrite(input);
            rewrite(below);
            rewrite(above);
        }
        Node::IntervalSelect { input, arms, .. } => {
            rewrite(input);
            *arms = arms.iter().map(|&arm| map[arm as usize]).collect();
        }
        Node::Spline { coords, .. } => {
            *coords = coords.iter().map(|&coord| map[coord as usize]).collect();
        }
        Node::Interpolated { input, .. } => rewrite(input),
        Node::FindTopSurface {
            density,
            upper_bound,
            ..
        } => {
            rewrite(density);
            rewrite(upper_bound);
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Constant(u32),
    DeclaredConstant(u32, u32, u32),
    Gradient(u8, u8, u32, u32, u32, u32),
    Noise(usize),
    ShiftB(usize),
    DistanceToPoint(DistanceParams),
    EndOuterIslands(usize),
    OldBlendedNoise(usize),
    Affine(NodeId, u32, u32),
    PiecewiseAffine(NodeId, u32, u32, u32),
    Unary(u8, NodeId),
    Clamp(NodeId, u32, u32),
    ConstMin(NodeId, u32),
    ConstMax(NodeId, u32),
    ConstSub(NodeId, u32),
    ConstDiv(NodeId, u32),
    ConstBasePow(u32, NodeId),
    ConstExponentPow(NodeId, u32),
    IntegerMultipleRound(NodeId, u32, u8),
    Binary(u8, NodeId, NodeId),
    Pow(NodeId, NodeId),
    Round(NodeId, NodeId, u8),
    Lerp(NodeId, NodeId, NodeId),
    ConstFirstLerp(NodeId, u32, NodeId),
    ConstSecondLerp(NodeId, NodeId, u32),
    ShiftedNoise(usize, NodeId, NodeId, NodeId),
    RangeChoice(NodeId, u32, u32, NodeId, NodeId),
    ConstRangeChoice(NodeId, u32, u32, u32, u32),
    SingleThreshold(NodeId, u32, NodeId, NodeId),
    IntervalSelect(NodeId, Vec<u32>, Vec<NodeId>),
    Spline(usize, Vec<NodeId>),
    Interpolated(NodeId, u32, u32),
    FindTopSurface(NodeId, NodeId, i32, u32),
}

fn unary_tag(op: UnaryOp) -> u8 {
    match op {
        UnaryOp::Abs => 0,
        UnaryOp::Square => 1,
        UnaryOp::Cube => 2,
        UnaryOp::Sqrt => 3,
        UnaryOp::Reciprocal => 4,
        UnaryOp::Negate => 5,
        UnaryOp::Squeeze => 6,
        UnaryOp::Log => 7,
        UnaryOp::Sign => 8,
    }
}

fn binary_tag(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Add => 0,
        BinaryOp::Sub => 1,
        BinaryOp::Mul => 2,
        BinaryOp::Div => 3,
        BinaryOp::Min => 4,
        BinaryOp::Max => 5,
    }
}

fn round_tag(kind: RoundKind) -> u8 {
    match kind {
        RoundKind::Floor => 0,
        RoundKind::Round => 1,
        RoundKind::Ceil => 2,
        RoundKind::Truncate => 3,
    }
}

fn tiling_tag(tiling: crate::node::gradient::Tiling) -> u8 {
    use crate::node::gradient::Tiling;
    match tiling {
        Tiling::ClampToEdge => 0,
        Tiling::Repeat => 1,
        Tiling::MirroredRepeat => 2,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::program::Workspace;
    use crate::router::{CONTINENTS, DEPTH, EROSION, RIDGES, TEMPERATURE, VEGETATION};
    use crate::strata::{AXIS_X, AXIS_Y, AXIS_Z};
    use crate::volume::Volume;
    use bevy_math::IVec3;
    use std::path::{Path, PathBuf};

    fn no_functions() -> BTreeMap<ResourceLocation, DensityFunctionHolder> {
        BTreeMap::new()
    }

    fn no_noises() -> BTreeMap<ResourceLocation, NoiseParam> {
        BTreeMap::new()
    }

    struct Built {
        nodes: Vec<Node>,
        axes: Vec<Axes>,
        ranges: Vec<Interval>,
        root: NodeId,
    }

    impl Built {
        fn node(&self) -> &Node {
            &self.nodes[self.root as usize]
        }

        fn axes(&self) -> Axes {
            self.axes[self.root as usize]
        }

        fn range(&self) -> Interval {
            self.ranges[self.root as usize]
        }
    }

    fn build(json: &str) -> Built {
        let holder: DensityFunctionHolder =
            serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
        let functions = no_functions();
        let noises = no_noises();
        let mut compiler = Compiler::new(&functions, &noises, 0, false);
        let root = compiler.compile_root(&holder).expect("compiles");
        let (nodes, axes, ranges, roots) = prune(
            compiler.nodes,
            compiler.axes,
            compiler.node_ranges,
            vec![root],
        );
        Built {
            nodes,
            axes,
            ranges,
            root: roots[0],
        }
    }

    fn sample(json: &str, at: IVec3) -> f32 {
        let holder: DensityFunctionHolder = serde_json::from_str(json).unwrap();
        let functions = no_functions();
        let noises = no_noises();
        let mut compiler = Compiler::new(&functions, &noises, 0, false);
        let root = compiler.compile_root(&holder).expect("compiles");
        let program = compiler.into_program(vec![root]);
        let volume = Volume::point(at);
        let mut out = vec![0.0; volume.len()];
        program.fill(&mut Workspace::new(), &volume, 0, &mut out);
        out[0]
    }

    const Y_GRADIENT: &str = r#"{"type":"gradient","axis":"y","from_coordinate":0,"to_coordinate":16,"from_value":0.0,"to_value":16.0}"#;
    /// Straddles zero, so the rectifier's negative half is reachable.
    const SIGNED_Y_GRADIENT: &str = r#"{"type":"gradient","axis":"y","from_coordinate":-16,"to_coordinate":16,"from_value":-16.0,"to_value":16.0}"#;

    /// The input's range would confine this to the out-of-range branch, and
    /// vanilla still reports the hull of both.
    #[test]
    fn range_choice_keeps_the_unreachable_branch() {
        let bounds = build(
            r#"{"type":"minecraft:range_choice","input":100.0,"min_inclusive":0.0,"max_exclusive":1.0,
                "when_in_range":-5.0,"when_out_of_range":5.0}"#,
        )
        .range();
        assert_eq!((bounds.min(), bounds.max()), (-5.0, 5.0));
    }

    /// `interval_select` is the mirror image: the input is excluded from the
    /// range and included in the axes.
    #[test]
    fn interval_select_ignores_its_input() {
        let bounds = build(
            r#"{"type":"minecraft:interval_select","input":-1000.0,"thresholds":[0.0],"functions":[1.0,2.0]}"#,
        )
        .range();
        assert_eq!((bounds.min(), bounds.max()), (1.0, 2.0));
    }

    /// A flat segment must not pick up the ±0.25 overshoot term, and the end
    /// probes are strict, so a coordinate inside the span adds nothing either.
    #[test]
    fn a_flat_spline_is_exactly_its_values() {
        let bounds = build(
            r#"{"type":"minecraft:spline","spline":{"coordinate":0.5,"points":[
                {"location":0.0,"value":1.0,"derivative":0.0},
                {"location":1.0,"value":3.0,"derivative":0.0}]}}"#,
        )
        .range();
        assert_eq!((bounds.min(), bounds.max()), (1.0, 3.0));

        let sloped = build(
            r#"{"type":"minecraft:spline","spline":{"coordinate":0.5,"points":[
                {"location":0.0,"value":1.0,"derivative":1.0},
                {"location":1.0,"value":3.0,"derivative":0.0}]}}"#,
        )
        .range();
        assert!(sloped.max() > 3.0, "the overshoot term widens it");
    }

    #[test]
    fn a_wholly_negative_sqrt_is_unknown_rather_than_zero() {
        assert!(
            build(r#"{"type":"minecraft:sqrt","input":-4.0}"#)
                .range()
                .is_nai()
        );
    }

    /// The clamp-alpha lerp cannot leave the two limit noises' common interval,
    /// so the declared bound is one fbm's.
    #[test]
    fn the_overworld_blended_noise_matches_its_published_bound() {
        let bounds = build(
            r#"{"type":"minecraft:old_blended_noise","xz_scale":0.25,"y_scale":0.125,
                "xz_factor":80.0,"y_factor":160.0,"smear_scale_multiplier":8.0}"#,
        )
        .range();
        assert_eq!(bounds.max(), 2.1670623);
        assert_eq!(bounds.min(), -2.1670623);
    }

    /// A zero scale is the only way a `noise` sheds an axis, and the shifts add
    /// theirs back on top of whatever survives.
    #[test]
    fn a_zero_scale_drops_the_axes_it_flattens() {
        let flat_y = build(
            r#"{"type":"minecraft:noise","noise":{"base_octave":-7,"octave_count":2},"xz_scale":1.0,"y_scale":0.0}"#,
        );
        assert_eq!(flat_y.axes(), AXIS_X | AXIS_Z);

        let flat_xz = build(
            r#"{"type":"minecraft:noise","noise":{"base_octave":-7,"octave_count":2},"xz_scale":0.0,"y_scale":0.0}"#,
        );
        assert_eq!(flat_xz.axes(), NO_AXES);

        let shifted = build(
            r#"{"type":"minecraft:noise","noise":{"base_octave":-7,"octave_count":2},"xz_scale":0.0,"y_scale":0.0,
                "shift_x":{"type":"minecraft:gradient","axis":"y","from_coordinate":0,"to_coordinate":1,"from_value":0.0,"to_value":1.0}}"#,
        );
        assert_eq!(shifted.axes(), AXIS_Y);
    }

    #[test]
    fn slice_and_find_top_surface_subtract_their_axis() {
        let sliced = build(
            r#"{"type":"minecraft:slice","axis":"x","coordinate":0,
                "input":{"type":"minecraft:noise","noise":{"base_octave":-7,"octave_count":2},"xz_scale":0.0,"y_scale":1.0}}"#,
        );
        assert_eq!(sliced.axes(), AXIS_Y);

        let surface = build(
            r#"{"type":"minecraft:find_top_surface","lower_bound":-64,"cell_height":8,
                "density":{"type":"minecraft:noise","noise":{"base_octave":-7,"octave_count":2},"xz_scale":1.0,"y_scale":1.0},
                "upper_bound":0.0}"#,
        );
        assert_eq!(surface.axes(), AXIS_X | AXIS_Z);
    }

    #[test]
    fn two_constant_operands_leave_one_node() {
        let built = build(r#"{"type":"add","left":2.0,"right":3.0}"#);
        assert_eq!(built.nodes.len(), 1);
        assert!(matches!(built.node(), Node::Constant(v) if *v == 5.0));

        let built =
            build(r#"{"type":"mul","left":{"type":"sub","left":10.0,"right":4.0},"right":0.5}"#);
        assert!(matches!(built.node(), Node::Constant(v) if *v == 3.0));
    }

    #[test]
    fn a_constant_folds_through_a_unary_and_a_round() {
        let built = build(r#"{"type":"square","input":-3.0}"#);
        assert!(matches!(built.node(), Node::Constant(v) if *v == 9.0));

        let built = build(r#"{"type":"floor","input":7.5,"multiple":2.0}"#);
        assert!(matches!(built.node(), Node::Constant(v) if *v == 6.0));
    }

    /// A zero multiple is the one case `IntegerMultipleRound` cannot express,
    /// because the generic sampler returns the input untouched there.
    #[test]
    fn a_zero_multiple_round_is_the_identity() {
        let json = format!(r#"{{"type":"floor","input":{Y_GRADIENT},"multiple":0.0}}"#);
        let built = build(&json);
        assert!(matches!(built.node(), Node::Gradient(_)));
    }

    #[test]
    fn a_scale_and_an_offset_fuse_into_one_affine() {
        let json = format!(
            r#"{{"type":"add","left":{{"type":"mul","left":{Y_GRADIENT},"right":3.0}},"right":-1.0}}"#
        );
        let built = build(&json);
        assert_eq!(built.nodes.len(), 2, "the gradient and one affine");
        match built.node() {
            Node::Affine { scale, offset, .. } => {
                assert_eq!(*scale, 3.0);
                assert_eq!(*offset, -1.0);
            }
            other => panic!("expected a fused affine, got {other:?}"),
        }
        assert_eq!(sample(&json, IVec3::new(0, 4, 0)), 11.0);
    }

    #[test]
    fn a_unit_affine_is_the_identity_and_a_division_becomes_a_reciprocal_multiply() {
        let json = format!(r#"{{"type":"mul","left":{Y_GRADIENT},"right":1.0}}"#);
        assert!(matches!(build(&json).node(), Node::Gradient(_)));

        let json = format!(r#"{{"type":"div","left":{Y_GRADIENT},"right":4.0}}"#);
        match build(&json).node() {
            Node::Affine { scale, offset, .. } => {
                assert_eq!(*scale, 0.25);
                assert_eq!(*offset, 0.0);
            }
            other => panic!("expected an affine, got {other:?}"),
        }
    }

    #[test]
    fn a_scaled_rectifier_becomes_one_piecewise_affine() {
        let json = format!(
            r#"{{"type":"add","left":{{"type":"mul","left":{{"type":"half_negative","input":{SIGNED_Y_GRADIENT}}},"right":2.0}},"right":1.0}}"#
        );
        let built = build(&json);
        assert_eq!(
            built.nodes.len(),
            2,
            "the gradient and one piecewise affine"
        );
        match built.node() {
            Node::PiecewiseAffine {
                neg_scale,
                pos_scale,
                offset,
                ..
            } => {
                assert_eq!(*neg_scale, 1.0);
                assert_eq!(*pos_scale, 2.0);
                assert_eq!(*offset, 1.0);
            }
            other => panic!("expected a piecewise affine, got {other:?}"),
        }
        // The gradient reads -8 at y = -8, halved by the rectifier.
        assert_eq!(sample(&json, IVec3::new(0, -8, 0)), -7.0);
        assert_eq!(sample(&json, IVec3::new(0, 8, 0)), 17.0);
    }

    #[test]
    fn a_non_overlapping_extremum_drops_the_operand_that_cannot_win() {
        let json = format!(r#"{{"type":"min","left":{Y_GRADIENT},"right":100.0}}"#);
        let built = build(&json);
        assert_eq!(built.nodes.len(), 1);
        assert!(matches!(built.node(), Node::Gradient(_)));

        let json = format!(r#"{{"type":"max","left":{Y_GRADIENT},"right":100.0}}"#);
        let built = build(&json);
        assert!(matches!(built.node(), Node::Constant(v) if *v == 100.0));
    }

    #[test]
    fn an_overlapping_extremum_keeps_the_constant_operand_baked_in() {
        let json = format!(r#"{{"type":"min","left":{Y_GRADIENT},"right":8.0}}"#);
        match build(&json).node() {
            Node::ConstMin { value, .. } => assert_eq!(*value, 8.0),
            other => panic!("expected a const min, got {other:?}"),
        }
        assert_eq!(sample(&json, IVec3::new(0, 12, 0)), 8.0);
        assert_eq!(sample(&json, IVec3::new(0, 4, 0)), 4.0);
    }

    #[test]
    fn a_single_threshold_select_and_a_two_constant_range_choice_specialize() {
        let json = format!(
            r#"{{"type":"interval_select","input":{Y_GRADIENT},"thresholds":[4.0],"functions":[-1.0,1.0]}}"#
        );
        assert!(matches!(build(&json).node(), Node::SingleThreshold { .. }));
        assert_eq!(sample(&json, IVec3::new(0, 0, 0)), -1.0);
        assert_eq!(sample(&json, IVec3::new(0, 8, 0)), 1.0);

        let json = format!(
            r#"{{"type":"range_choice","input":{Y_GRADIENT},"min_inclusive":0.0,"max_exclusive":4.0,"when_in_range":1.0,"when_out_of_range":-1.0}}"#
        );
        assert!(matches!(build(&json).node(), Node::ConstRangeChoice { .. }));
        assert_eq!(sample(&json, IVec3::new(0, 2, 0)), 1.0);
        assert_eq!(sample(&json, IVec3::new(0, 6, 0)), -1.0);
    }

    #[test]
    fn a_constant_exponent_lowers_to_the_unary_it_names() {
        let json = format!(r#"{{"type":"pow","base":{Y_GRADIENT},"exponent":2.0}}"#);
        assert!(matches!(
            build(&json).node(),
            Node::Unary {
                op: UnaryOp::Square,
                ..
            }
        ));

        let json = format!(r#"{{"type":"pow","base":{Y_GRADIENT},"exponent":-1.0}}"#);
        assert!(matches!(
            build(&json).node(),
            Node::Unary {
                op: UnaryOp::Reciprocal,
                ..
            }
        ));
    }

    #[test]
    fn a_shared_subtree_is_one_node() {
        let json = format!(
            r#"{{"type":"add","left":{{"type":"square","input":{Y_GRADIENT}}},"right":{{"type":"square","input":{Y_GRADIENT}}}}}"#
        );
        let built = build(&json);
        assert_eq!(built.nodes.len(), 3, "gradient, square, add");
    }

    #[test]
    fn an_unsupported_kind_names_itself() {
        let holder: DensityFunctionHolder = serde_json::from_str(&format!(
            r#"{{"type":"slice","axis":"y","coordinate":0,"input":{Y_GRADIENT}}}"#
        ))
        .unwrap();
        let functions = no_functions();
        let noises = no_noises();
        let mut compiler = Compiler::new(&functions, &noises, 0, false);
        assert_eq!(
            compiler.compile_root(&holder),
            Err(CompileError::Unsupported("slice"))
        );
    }

    // --- the shipped corpus -------------------------------------------------

    pub(crate) fn assets() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/minecraft/worldgen")
    }

    pub(crate) fn load_dir<T: serde::de::DeserializeOwned>(
        root: &Path,
        dir: &Path,
        out: &mut BTreeMap<ResourceLocation, T>,
    ) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.unwrap().path();
            if path.is_dir() {
                load_dir(root, &path, out);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let relative = path.strip_prefix(root).unwrap().with_extension("");
            let id = ResourceLocation::minecraft(&relative.to_string_lossy().replace('\\', "/"));
            let bytes = std::fs::read(&path).unwrap();
            let value = serde_json::from_slice(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            out.insert(id, value);
        }
    }

    pub(crate) fn corpus() -> (
        BTreeMap<ResourceLocation, DensityFunctionHolder>,
        BTreeMap<ResourceLocation, NoiseParam>,
    ) {
        let assets = assets();
        let mut functions = BTreeMap::new();
        let functions_root = assets.join("density_function");
        load_dir(&functions_root, &functions_root, &mut functions);
        let mut noises = BTreeMap::new();
        let noises_root = assets.join("noise");
        load_dir(&noises_root, &noises_root, &mut noises);
        (functions, noises)
    }

    fn tree_size(
        registry: &BTreeMap<ResourceLocation, DensityFunctionHolder>,
        holder: &DensityFunctionHolder,
        memo: &mut HashMap<ResourceLocation, usize>,
    ) -> usize {
        match holder {
            DensityFunctionHolder::Value(_) => 1,
            DensityFunctionHolder::Reference(id) => {
                if let Some(&size) = memo.get(id) {
                    return size;
                }
                let size = tree_size(registry, &registry[id], memo);
                memo.insert(id.clone(), size);
                size
            }
            DensityFunctionHolder::Owned(function) => {
                let mut size = 1;
                function.visit_children(&mut |child| size += tree_size(registry, child, memo));
                size
            }
        }
    }

    #[test]
    fn the_overworld_router_compiles_to_far_fewer_nodes_than_its_tree_has() {
        let (functions, noises) = corpus();
        let settings: NoiseGeneratorSettings = serde_json::from_slice(
            &std::fs::read(assets().join("noise_settings/overworld.json")).unwrap(),
        )
        .unwrap();
        let router = build_router(
            &settings,
            &functions,
            &noises,
            42,
            VoxelId(1),
            VoxelId(2),
            None,
        )
        .unwrap();

        let failed: Vec<&str> = router
            .failed_roots()
            .iter()
            .map(|(name, _)| *name)
            .collect();
        assert!(
            failed.is_empty(),
            "every overworld root compiles: {failed:?}"
        );

        let mut memo = HashMap::new();
        let naive: usize = ROOT_NAMES
            .iter()
            .zip(settings.noise_router.roots())
            .filter(|(name, _)| !failed.contains(name))
            .map(|(_, holder)| tree_size(&functions, holder, &mut memo))
            .sum();
        let compiled = router.program().len();
        assert!(
            compiled < naive,
            "a tree walk would build {naive} nodes; the graph has {compiled}"
        );
        assert!(
            compiled * 3 < naive,
            "dedup should be worth far more than a few nodes"
        );
    }

    #[test]
    fn the_overworld_climate_roots_evaluate_over_a_chunk() {
        let (functions, noises) = corpus();
        let settings: NoiseGeneratorSettings = serde_json::from_slice(
            &std::fs::read(assets().join("noise_settings/overworld.json")).unwrap(),
        )
        .unwrap();
        let router = build_router(
            &settings,
            &functions,
            &noises,
            42,
            VoxelId(1),
            VoxelId(2),
            None,
        )
        .unwrap();

        let volume = Volume::new(
            IVec3::new(5, 3, 5),
            IVec3::new(0, -64, 0),
            IVec3::new(4, 8, 4),
        );
        let mut workspace = Workspace::new();
        let mut out = vec![0.0f32; volume.len()];
        for root in [TEMPERATURE, VEGETATION, CONTINENTS, EROSION, DEPTH, RIDGES] {
            router
                .program()
                .fill(&mut workspace, &volume, root, &mut out);
            assert!(
                out.iter().all(|v| v.is_finite()),
                "root {root} left a non-finite value"
            );
            assert!(
                out.iter().any(|v| *v != out[0]),
                "root {root} is constant over a whole chunk"
            );
        }
    }

    #[test]
    fn every_shipped_noise_settings_builds_a_router() {
        let (functions, noises) = corpus();
        for name in [
            "amplified",
            "beta",
            "caves",
            "end",
            "floating_islands",
            "large_biomes",
            "nether",
            "overworld",
        ] {
            let path = assets().join(format!("noise_settings/{name}.json"));
            let settings: NoiseGeneratorSettings =
                serde_json::from_slice(&std::fs::read(&path).unwrap())
                    .unwrap_or_else(|e| panic!("{name}: {e}"));
            let router = build_router(
                &settings,
                &functions,
                &noises,
                42,
                VoxelId(1),
                VoxelId(2),
                None,
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            for (root, error) in router.failed_roots() {
                assert!(
                    matches!(error, CompileError::Unsupported(_)),
                    "{name}/{root}: {error}"
                );
            }
        }
    }
}
