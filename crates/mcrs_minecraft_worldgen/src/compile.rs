use crate::beta::seed::{seed_beta_climate, seed_beta_terrain};
use crate::interval::Interval;
use crate::jmath;
use crate::node::blended::{BlendedParams, NOISE_SEED};
use crate::node::distance::DistanceParams;
use crate::node::end_island::EndIslandParams;
use crate::node::gradient::GradientParams;
use crate::node::noise::NoiseParams;
use crate::node::spline::{CompiledSpline, Multipoint, SplineValue};
use crate::noise::normal_noise::NoiseSampler;
use crate::program::{BinaryOp, Node, NodeId, Program, RoundKind, UnaryOp};
use crate::proto::{
    self, DensityFunctionHolder, NoiseHolder, NoiseParam, ProtoDensityFunction, ProtoSpline,
};
use crate::router::{GeneratorSettings, NoiseRouter, ROOT_NAMES};
use crate::strata::{AXIS_X, AXIS_Z, Axes, NO_AXES};
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
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::Unsupported(kind) => write!(f, "unsupported density function: {kind}"),
            CompileError::UnknownFunction(id) => write!(f, "unknown density function: {id}"),
            CompileError::UnknownNoise(id) => write!(f, "unknown noise: {id}"),
            CompileError::ReferenceCycle(id) => write!(f, "density function cycle through {id}"),
        }
    }
}

impl std::error::Error for CompileError {}

pub fn build_router(
    settings: &GeneratorSettings,
    registry: &BTreeMap<ResourceLocation, DensityFunctionHolder>,
    noises: &BTreeMap<ResourceLocation, NoiseParam>,
    seed: u64,
    default_block: VoxelId,
    default_fluid: VoxelId,
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

    let roots = [0, 1, 2, 3, 4, 5, 6, 7];
    let program = compiler.into_program(nodes);
    Ok(NoiseRouter::new(
        program,
        roots,
        failed,
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
    /// The bound every holder that compiled to a node declared, widened over all
    /// of them. `None` is a node no holder landed on directly, which the program
    /// must treat as unbounded.
    node_ranges: Vec<Option<Interval>>,
    interned: HashMap<Key, NodeId>,
    compiled: HashMap<DensityFunctionHolder, NodeId>,
    inlined: HashMap<ResourceLocation, DensityFunctionHolder>,
    inlining: Vec<ResourceLocation>,
    ranges: HashMap<DensityFunctionHolder, Interval>,
    samplers: HashMap<NoiseHolder, Arc<NoiseSampler>>,
    noise_params: HashMap<(usize, u64, u64), Arc<NoiseParams>>,
    splines: HashMap<ProtoSpline, Arc<CompiledSpline>>,
    blended: HashMap<[u64; 5], Arc<BlendedParams>>,
    end_islands: Option<Arc<EndIslandParams>>,
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
            ranges: HashMap::new(),
            samplers: HashMap::new(),
            noise_params: HashMap::new(),
            splines: HashMap::new(),
            blended: HashMap::new(),
            end_islands: None,
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
        let ranges = self
            .node_ranges
            .into_iter()
            .map(|range| range.unwrap_or(Interval::INFINITE))
            .collect();
        let (nodes, axes, ranges, roots) = prune(self.nodes, self.axes, ranges, roots);
        Program::new(nodes, axes, ranges, roots)
    }

    fn intern(&mut self, key: Key, node: Node, axes: Axes) -> NodeId {
        if let Some(&id) = self.interned.get(&key) {
            return id;
        }
        let id = self.nodes.len() as NodeId;
        self.nodes.push(node);
        self.axes.push(axes);
        self.node_ranges.push(None);
        self.interned.insert(key, id);
        id
    }

    pub fn constant(&mut self, value: f32) -> NodeId {
        self.intern(
            Key::Constant(value.to_bits()),
            Node::Constant(value),
            NO_AXES,
        )
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
                // so the marker is its input and is dropped here rather than at
                // compile time, where `domain_axes` and `range` would meet it.
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

    // --- ranges -------------------------------------------------------------

    /// `Interval::INFINITE` where a noise the corpus never declared is in play:
    /// `proto::range` panics on one, and a bound nobody can compute must not
    /// delete a branch.
    fn range_of(&mut self, holder: &DensityFunctionHolder) -> Interval {
        if let Some(&range) = self.ranges.get(holder) {
            return range;
        }
        let range = if self.noises_declared(holder) {
            proto::holder_range(self.noises, holder)
        } else {
            Interval::INFINITE
        };
        self.ranges.insert(holder.clone(), range);
        range
    }

    fn noises_declared(&self, holder: &DensityFunctionHolder) -> bool {
        let DensityFunctionHolder::Owned(function) = holder else {
            return true;
        };
        let declared = |noise: &NoiseHolder| match noise {
            NoiseHolder::Owned(_) => true,
            NoiseHolder::Reference(id) => self.noises.contains_key(id),
        };
        let here = match &**function {
            ProtoDensityFunction::Noise { noise, .. }
            | ProtoDensityFunction::Shift { noise }
            | ProtoDensityFunction::ShiftA { noise }
            | ProtoDensityFunction::ShiftB { noise } => declared(noise),
            _ => true,
        };
        if !here {
            return false;
        }
        let mut all = true;
        function.visit_children(&mut |child| all &= self.noises_declared(child));
        all
    }

    // --- compilation --------------------------------------------------------

    fn compile(&mut self, holder: &DensityFunctionHolder) -> Result<NodeId, CompileError> {
        if let Some(&id) = self.compiled.get(holder) {
            return Ok(id);
        }
        let id = self.compile_uncached(holder)?;
        let range = self.range_of(holder);
        let slot = &mut self.node_ranges[id as usize];
        *slot = Some(match *slot {
            Some(known) => known.union(range),
            None => range,
        });
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
        let axes = proto::domain_axes(function);
        use ProtoDensityFunction as P;
        match &**function {
            P::Constant(value) => Ok(self.constant(value.value.0 as f32)),

            // No blender and no beardifier here, so each is the value vanilla's
            // fallback returns. Their declared ranges stay conservative, which
            // is what keeps branch elimination from deleting a branch vanilla
            // would keep.
            P::BlendAlpha => Ok(self.constant(1.0)),
            P::BlendOffset => Ok(self.constant(0.0)),
            P::Beardifier => Ok(self.constant(0.0)),
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
                Ok(self.intern(key, Node::Gradient(params), axes))
            }

            P::DistanceToPoint { point, metric } => {
                let params = DistanceParams::new(point[0], point[1], point[2], *metric);
                Ok(self.intern(
                    Key::DistanceToPoint(params),
                    Node::DistanceToPoint(params),
                    axes,
                ))
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
                Ok(self.intern(key, Node::EndOuterIslands(params), axes))
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
                Ok(self.intern(key, Node::OldBlendedNoise(params), axes))
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
                    return Ok(self.intern(Key::Noise(pointer), Node::Noise { params }, axes));
                }
                let x = self.compile(shift_x)?;
                let y = self.compile(shift_y)?;
                let z = self.compile(shift_z)?;
                Ok(self.intern(
                    Key::ShiftedNoise(pointer, x, y, z),
                    Node::ShiftedNoise { params, x, y, z },
                    axes,
                ))
            }

            // `shift` and `shift_a` are a plain noise scaled by four, so they
            // lower onto nodes a bare `noise` of the same parameters shares.
            P::ShiftA { noise } => self.compile_shift(noise, 0.0, axes),
            P::Shift { noise } => self.compile_shift(noise, 0.25, axes),
            P::ShiftB { noise } => {
                let sampler = self.noise_sampler(noise)?;
                let params = self.shift_b_params(&sampler);
                let key = Key::ShiftB(Arc::as_ptr(&params) as usize);
                Ok(self.intern(key, Node::ShiftB { params }, axes))
            }

            P::Abs(x) => self.compile_unary(UnaryOp::Abs, &x.input, axes),
            P::Square(x) => self.compile_unary(UnaryOp::Square, &x.input, axes),
            P::Cube(x) => self.compile_unary(UnaryOp::Cube, &x.input, axes),
            P::Sqrt(x) => self.compile_unary(UnaryOp::Sqrt, &x.input, axes),
            P::Reciprocal(x) => self.compile_unary(UnaryOp::Reciprocal, &x.input, axes),
            P::Negate(x) => self.compile_unary(UnaryOp::Negate, &x.input, axes),
            P::Squeeze(x) => self.compile_unary(UnaryOp::Squeeze, &x.input, axes),
            P::Log(x) => self.compile_unary(UnaryOp::Log, &x.input, axes),
            P::Sign(x) => self.compile_unary(UnaryOp::Sign, &x.input, axes),

            P::HalfNegative(x) => {
                let input = self.compile(&x.input)?;
                Ok(self.leaky_relu(input, 0.5, axes))
            }
            P::QuarterNegative(x) => {
                let input = self.compile(&x.input)?;
                Ok(self.leaky_relu(input, 0.25, axes))
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
                    axes,
                ))
            }

            P::Floor(x) => self.compile_round(RoundKind::Floor, &x.input, &x.multiple, axes),
            P::Round(x) => self.compile_round(RoundKind::Round, &x.input, &x.multiple, axes),
            P::Ceil(x) => self.compile_round(RoundKind::Ceil, &x.input, &x.multiple, axes),
            P::Truncate(x) => self.compile_round(RoundKind::Truncate, &x.input, &x.multiple, axes),

            P::Add(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                Ok(self.add(left, right, axes))
            }
            P::Sub(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                Ok(self.sub(left, right, axes))
            }
            P::Mul(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                Ok(self.mul(left, right, axes))
            }
            P::Div(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                Ok(self.div(left, right, axes))
            }
            P::Min(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                let bounds = (self.range_of(&x.left), self.range_of(&x.right));
                Ok(self.extremum(BinaryOp::Min, left, right, bounds, axes))
            }
            P::Max(x) => {
                let (left, right) = self.operands(&x.left, &x.right)?;
                let bounds = (self.range_of(&x.left), self.range_of(&x.right));
                Ok(self.extremum(BinaryOp::Max, left, right, bounds, axes))
            }

            P::Pow(x) => {
                let (base, exponent) = self.operands(&x.base, &x.exponent)?;
                Ok(self.pow(base, exponent, axes))
            }

            P::Lerp {
                alpha,
                first,
                second,
            } => {
                let alpha = self.compile(alpha)?;
                let first = self.compile(first)?;
                let second = self.compile(second)?;
                Ok(self.lerp(alpha, first, second, axes))
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
                        axes,
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
                        axes,
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
                        axes,
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
                    axes,
                ))
            }

            P::Spline { spline } => self.compile_spline(spline, axes),

            // A slice whose input already ignores the sliced axis is the
            // identity; anything else needs the input evaluated at a
            // substituted coordinate, which the flat program cannot do.
            P::Slice { axis, input, .. } => {
                if proto::holder_axes(input) & proto::axes_from(*axis) == 0 {
                    return self.compile(input);
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
                    axes | AXIS_X | AXIS_Z,
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
                    axes,
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
        axes: Axes,
    ) -> Result<NodeId, CompileError> {
        let input = self.compile(input)?;
        Ok(self.unary(op, input, axes))
    }

    fn compile_round(
        &mut self,
        kind: RoundKind,
        input: &DensityFunctionHolder,
        multiple: &DensityFunctionHolder,
        axes: Axes,
    ) -> Result<NodeId, CompileError> {
        let input = self.compile(input)?;
        let multiple = self.compile(multiple)?;
        Ok(self.round(kind, input, multiple, axes))
    }

    fn compile_shift(
        &mut self,
        noise: &NoiseHolder,
        y_scale: f64,
        axes: Axes,
    ) -> Result<NodeId, CompileError> {
        let sampler = self.noise_sampler(noise)?;
        let params = self.noise_params(&sampler, 0.25, y_scale);
        let key = Key::Noise(Arc::as_ptr(&params) as usize);
        let inner = self.intern(key, Node::Noise { params }, axes);
        Ok(self.affine(inner, 4.0, 0.0, axes))
    }

    fn compile_spline(&mut self, spline: &ProtoSpline, axes: Axes) -> Result<NodeId, CompileError> {
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
            axes,
        ))
    }

    // --- the specialization ladder -----------------------------------------

    fn unary(&mut self, op: UnaryOp, input: NodeId, axes: Axes) -> NodeId {
        if let Some(value) = self.as_constant(input) {
            return self.constant(op.apply(value));
        }
        self.intern(
            Key::Unary(unary_tag(op), input),
            Node::Unary { op, input },
            axes,
        )
    }

    fn leaky_relu(&mut self, input: NodeId, negative_scale: f32, axes: Axes) -> NodeId {
        if let Some(value) = self.as_constant(input) {
            let folded = if value > 0.0 {
                value
            } else {
                value * negative_scale
            };
            return self.constant(folded);
        }
        self.intern(
            Key::LeakyRelu(input, negative_scale.to_bits()),
            Node::LeakyRelu {
                input,
                negative_scale,
            },
            axes,
        )
    }

    /// `input * scale + offset`, with the two chained forms vanilla emits as
    /// separate samplers folded into one node. Only the scale-then-offset order
    /// fuses: `(v * s + 0) + o` and `v * s + o` are the same two roundings,
    /// while `(v * s1) * s2` and `v * (s1 * s2)` are not.
    fn affine(&mut self, input: NodeId, scale: f32, offset: f32, axes: Axes) -> NodeId {
        if let Some(value) = self.as_constant(input) {
            return self.constant(value * scale + offset);
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
            Node::LeakyRelu {
                input: inner,
                negative_scale,
            } => Fusion::LeakyRelu(*inner, *negative_scale),
            Node::PiecewiseAffine {
                input: inner,
                neg_scale,
                pos_scale,
                offset: inner_offset,
            } if scale == 1.0 && *inner_offset == 0.0 => {
                Fusion::PiecewiseAffine(*inner, *neg_scale, *pos_scale)
            }
            _ => Fusion::None,
        };
        match fusion {
            Fusion::Affine(inner, inner_scale) => self.affine(inner, inner_scale, offset, axes),
            // The rectifier's negative slope is a power of two, so folding it
            // into the scale is exact and the piecewise node still rounds twice.
            Fusion::LeakyRelu(inner, negative_scale) => {
                self.piecewise_affine(inner, negative_scale * scale, scale, offset, axes)
            }
            Fusion::PiecewiseAffine(inner, neg_scale, pos_scale) => {
                self.piecewise_affine(inner, neg_scale, pos_scale, offset, axes)
            }
            Fusion::None => self.intern(
                Key::Affine(input, scale.to_bits(), offset.to_bits()),
                Node::Affine {
                    input,
                    scale,
                    offset,
                },
                axes,
            ),
        }
    }

    fn piecewise_affine(
        &mut self,
        input: NodeId,
        neg_scale: f32,
        pos_scale: f32,
        offset: f32,
        axes: Axes,
    ) -> NodeId {
        if let Some(value) = self.as_constant(input) {
            let scale = if value < 0.0 { neg_scale } else { pos_scale };
            return self.constant(value * scale + offset);
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
            axes,
        )
    }

    fn binary(&mut self, op: BinaryOp, a: NodeId, b: NodeId, axes: Axes) -> NodeId {
        self.intern(
            Key::Binary(binary_tag(op), a, b),
            Node::Binary { op, a, b },
            axes,
        )
    }

    fn add(&mut self, left: NodeId, right: NodeId, axes: Axes) -> NodeId {
        match (self.as_constant(left), self.as_constant(right)) {
            (Some(a), Some(b)) => self.constant(a + b),
            (Some(c), None) => self.affine(right, 1.0, c, axes),
            (None, Some(c)) => self.affine(left, 1.0, c, axes),
            (None, None) => self.binary(BinaryOp::Add, left, right, axes),
        }
    }

    fn sub(&mut self, left: NodeId, right: NodeId, axes: Axes) -> NodeId {
        match (self.as_constant(left), self.as_constant(right)) {
            (Some(a), Some(b)) => self.constant(a - b),
            (Some(value), None) => self.intern(
                Key::ConstSub(right, value.to_bits()),
                Node::ConstSub {
                    input: right,
                    value,
                },
                axes,
            ),
            // `x - c` is `x + (-c)`: negation is exact in binary floating point.
            (None, Some(c)) => self.affine(left, 1.0, -c, axes),
            (None, None) => self.binary(BinaryOp::Sub, left, right, axes),
        }
    }

    fn mul(&mut self, left: NodeId, right: NodeId, axes: Axes) -> NodeId {
        match (self.as_constant(left), self.as_constant(right)) {
            (Some(a), Some(b)) => self.constant(a * b),
            (Some(c), None) => self.affine(right, c, 0.0, axes),
            (None, Some(c)) => self.affine(left, c, 0.0, axes),
            (None, None) => self.binary(BinaryOp::Mul, left, right, axes),
        }
    }

    fn div(&mut self, left: NodeId, right: NodeId, axes: Axes) -> NodeId {
        match (self.as_constant(left), self.as_constant(right)) {
            (Some(a), Some(b)) => self.constant(a / b),
            (Some(value), None) => self.intern(
                Key::ConstDiv(right, value.to_bits()),
                Node::ConstDiv {
                    input: right,
                    value,
                },
                axes,
            ),
            // Vanilla compiles `x / c` to the reciprocal multiply, not to a
            // division, and the two disagree in the last bit for a divisor that
            // is not a power of two.
            (None, Some(c)) => self.affine(left, 1.0 / c, 0.0, axes),
            (None, None) => self.binary(BinaryOp::Div, left, right, axes),
        }
    }

    fn extremum(
        &mut self,
        op: BinaryOp,
        left: NodeId,
        right: NodeId,
        bounds: (Interval, Interval),
        axes: Axes,
    ) -> NodeId {
        let (left_range, right_range) = bounds;
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
            (Some(value), None) => self.const_extremum(op, right, value, axes),
            (None, Some(value)) => self.const_extremum(op, left, value, axes),
            _ => self.binary(op, left, right, axes),
        }
    }

    fn const_extremum(&mut self, op: BinaryOp, input: NodeId, value: f32, axes: Axes) -> NodeId {
        let bits = value.to_bits();
        match op {
            BinaryOp::Min => self.intern(
                Key::ConstMin(input, bits),
                Node::ConstMin { input, value },
                axes,
            ),
            _ => self.intern(
                Key::ConstMax(input, bits),
                Node::ConstMax { input, value },
                axes,
            ),
        }
    }

    fn pow(&mut self, base: NodeId, exponent: NodeId, axes: Axes) -> NodeId {
        match (self.as_constant(base), self.as_constant(exponent)) {
            (Some(a), Some(b)) => self.constant(jmath::pow(a, b)),
            (Some(value), None) => self.intern(
                Key::ConstBasePow(value.to_bits(), exponent),
                Node::ConstBasePow {
                    base: value,
                    exponent,
                },
                axes,
            ),
            (None, Some(value)) => self.const_exponent_pow(base, value, axes),
            (None, None) => {
                self.intern(Key::Pow(base, exponent), Node::Pow { base, exponent }, axes)
            }
        }
    }

    fn const_exponent_pow(&mut self, base: NodeId, exponent: f32, axes: Axes) -> NodeId {
        let magnitude = exponent.abs();
        let special = if magnitude == 0.5 {
            Some(self.unary(UnaryOp::Sqrt, base, axes))
        } else if magnitude == 1.0 {
            Some(base)
        } else if magnitude == 2.0 {
            Some(self.unary(UnaryOp::Square, base, axes))
        } else if magnitude == 3.0 {
            Some(self.unary(UnaryOp::Cube, base, axes))
        } else {
            None
        };
        match special {
            Some(id) if exponent >= 0.0 => id,
            Some(id) => self.unary(UnaryOp::Reciprocal, id, axes),
            None => self.intern(
                Key::ConstExponentPow(base, exponent.to_bits()),
                Node::ConstExponentPow {
                    input: base,
                    exponent,
                },
                axes,
            ),
        }
    }

    fn round(&mut self, kind: RoundKind, input: NodeId, multiple: NodeId, axes: Axes) -> NodeId {
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
                axes,
            );
        }
        self.intern(
            Key::Round(input, multiple, round_tag(kind)),
            Node::Round {
                value: input,
                multiple,
                kind,
            },
            axes,
        )
    }

    fn lerp(&mut self, alpha: NodeId, first: NodeId, second: NodeId, axes: Axes) -> NodeId {
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
                axes,
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
                axes,
            );
        }
        self.intern(
            Key::Lerp(alpha, first, second),
            Node::Lerp {
                alpha,
                first,
                second,
            },
            axes,
        )
    }

    // --- noise construction -------------------------------------------------

    fn noise_params(
        &mut self,
        sampler: &Arc<NoiseSampler>,
        xz_scale: f64,
        y_scale: f64,
    ) -> Arc<NoiseParams> {
        let key = (
            Arc::as_ptr(sampler) as usize,
            xz_scale.to_bits(),
            y_scale.to_bits(),
        );
        if let Some(params) = self.noise_params.get(&key) {
            return Arc::clone(params);
        }
        let params = Arc::new(NoiseParams::new(Arc::clone(sampler), xz_scale, y_scale));
        self.noise_params.insert(key, Arc::clone(&params));
        params
    }

    fn shift_b_params(&mut self, sampler: &Arc<NoiseSampler>) -> Arc<NoiseParams> {
        let key = (Arc::as_ptr(sampler) as usize, u64::MAX, u64::MAX);
        if let Some(params) = self.noise_params.get(&key) {
            return Arc::clone(params);
        }
        let params = Arc::new(NoiseParams::shift_b(Arc::clone(sampler)));
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
    ) -> Arc<BlendedParams> {
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
        let params = Arc::new(BlendedParams::new(
            &mut random,
            xz_scale,
            y_scale,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
            if self.legacy_random { 128.0 } else { 1.0 },
        ));
        self.blended.insert(key, Arc::clone(&params));
        params
    }

    fn noise_sampler(&mut self, holder: &NoiseHolder) -> Result<Arc<NoiseSampler>, CompileError> {
        if let Some(sampler) = self.samplers.get(holder) {
            return Ok(Arc::clone(sampler));
        }
        let sampler = Arc::new(self.create_noise(holder)?);
        self.samplers.insert(holder.clone(), Arc::clone(&sampler));
        Ok(sampler)
    }

    fn create_noise(&mut self, holder: &NoiseHolder) -> Result<NoiseSampler, CompileError> {
        match holder {
            NoiseHolder::Owned(param) => {
                let mut random = self.random.clone();
                Ok(NoiseSampler::from_params(
                    &mut random,
                    param.base_octave,
                    param.octave_amplitudes(),
                    param.base_amplitude.0,
                    param.normalize,
                ))
            }
            NoiseHolder::Reference(id) => self.create_named_noise(id),
        }
    }

    fn create_named_noise(&mut self, id: &ResourceLocation) -> Result<NoiseSampler, CompileError> {
        // The two nether climate noises are seeded from the raw world seed, not
        // from the hashed fork every other noise takes.
        match id.as_str() {
            "minecraft:nether/temperature" => {
                return Ok(NoiseSampler::new(
                    &mut LegacyRandom::new(self.seed),
                    -7,
                    vec![1.0, 1.0],
                ));
            }
            "minecraft:nether/vegetation" => {
                return Ok(NoiseSampler::new(
                    &mut LegacyRandom::new(self.seed.wrapping_add(1)),
                    -7,
                    vec![1.0, 1.0],
                ));
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
        Ok(NoiseSampler::from_params(
            &mut random,
            param.base_octave,
            param.octave_amplitudes(),
            param.base_amplitude.0,
            param.normalize,
        ))
    }

    /// The Beta noises are drawn from the pre-26.3 `LegacyRandom` streams and
    /// have no `worldgen/noise` entry, so the ids are resolved here instead.
    fn create_legacy_noise(&mut self, id: &ResourceLocation) -> Option<NoiseSampler> {
        match id.as_str() {
            "minecraft:offset" => {
                let mut root = self.random.clone();
                let mut random = root.fork_hash("minecraft:offset");
                Some(NoiseSampler::new(&mut random, 0, vec![0.0]))
            }
            "mcrs:beta/scale" => {
                let (_, _, _, _, _, scale, _) = seed_beta_terrain(self.seed);
                Some(NoiseSampler::beta_octave_2d(scale, 1.121, 2048.0))
            }
            "mcrs:beta/depth" => {
                let (_, _, _, _, _, _, depth) = seed_beta_terrain(self.seed);
                Some(NoiseSampler::beta_octave_2d(depth, 200.0, 131072.0))
            }
            "mcrs:beta/temperature" => {
                let (temperature, _, _) = seed_beta_climate(self.seed);
                Some(NoiseSampler::beta_simplex_2d(
                    temperature,
                    0.025,
                    0.25,
                    16.0,
                ))
            }
            "mcrs:beta/vegetation" => {
                let (_, rain, _) = seed_beta_climate(self.seed);
                Some(NoiseSampler::beta_simplex_2d(rain, 0.05, 1.0 / 3.0, 16.0))
            }
            "mcrs:beta/climate_detail" => {
                let (_, _, detail) = seed_beta_climate(self.seed);
                Some(NoiseSampler::beta_simplex_2d(detail, 0.25, 1.0 / 1.7, 4.0))
            }
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
    LeakyRelu(NodeId, f32),
    PiecewiseAffine(NodeId, f32, f32),
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
        | Node::LeakyRelu { input, .. }
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
    Gradient(u8, u8, u32, u32, u32, u32),
    Noise(usize),
    ShiftB(usize),
    DistanceToPoint(DistanceParams),
    EndOuterIslands(usize),
    OldBlendedNoise(usize),
    Affine(NodeId, u32, u32),
    PiecewiseAffine(NodeId, u32, u32, u32),
    Unary(u8, NodeId),
    LeakyRelu(NodeId, u32),
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
mod tests {
    use super::*;
    use crate::program::Workspace;
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
        root: NodeId,
    }

    impl Built {
        fn node(&self) -> &Node {
            &self.nodes[self.root as usize]
        }
    }

    fn build(json: &str) -> Built {
        let holder: DensityFunctionHolder =
            serde_json::from_str(json).unwrap_or_else(|e| panic!("{json}: {e}"));
        let functions = no_functions();
        let noises = no_noises();
        let mut compiler = Compiler::new(&functions, &noises, 0, false);
        let root = compiler.compile_root(&holder).expect("compiles");
        let ranges = vec![Interval::INFINITE; compiler.nodes.len()];
        let (nodes, _, _, roots) = prune(compiler.nodes, compiler.axes, ranges, vec![root]);
        Built {
            nodes,
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

    fn assets() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/minecraft/worldgen")
    }

    fn load_dir<T: serde::de::DeserializeOwned>(
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

    fn corpus() -> (
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
        let settings: GeneratorSettings = serde_json::from_slice(
            &std::fs::read(assets().join("noise_settings/overworld.json")).unwrap(),
        )
        .unwrap();
        let router =
            build_router(&settings, &functions, &noises, 42, VoxelId(1), VoxelId(2)).unwrap();

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
        let settings: GeneratorSettings = serde_json::from_slice(
            &std::fs::read(assets().join("noise_settings/overworld.json")).unwrap(),
        )
        .unwrap();
        let router =
            build_router(&settings, &functions, &noises, 42, VoxelId(1), VoxelId(2)).unwrap();

        let volume = Volume::new(
            IVec3::new(5, 3, 5),
            IVec3::new(0, -64, 0),
            IVec3::new(4, 8, 4),
        );
        let mut workspace = Workspace::new();
        let mut out = vec![0.0f32; volume.len()];
        for root in [
            router.temperature(),
            router.vegetation(),
            router.continents(),
            router.erosion(),
            router.depth(),
            router.ridges(),
        ] {
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
            let settings: GeneratorSettings =
                serde_json::from_slice(&std::fs::read(&path).unwrap())
                    .unwrap_or_else(|e| panic!("{name}: {e}"));
            let router = build_router(&settings, &functions, &noises, 42, VoxelId(1), VoxelId(2))
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
