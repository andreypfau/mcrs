package mcrs.data;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.BitSet;
import java.util.Comparator;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.concurrent.ConcurrentHashMap;
import java.util.stream.Collectors;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.tags.TagKey;
import net.minecraft.tags.TagLoader;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.ScheduledTickAccess;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.SupportType;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.shapes.BooleanOp;
import net.minecraft.world.phys.shapes.CollisionContext;
import net.minecraft.world.phys.shapes.Shapes;
import net.minecraft.world.phys.shapes.VoxelShape;

final class SupportDeriver {
    static final BlockPos P = new BlockPos(0, 128, 0);
    private static final Direction[] DIRECTIONS = Direction.values();
    private static final Direction[] FACES = {Direction.UP, Direction.NORTH, Direction.SOUTH, Direction.WEST, Direction.EAST, Direction.DOWN};
    private static final int FULL = 0, CENTER = 1, RING = 2, COLLISION_FULL = 3;
    private static final int MARKER_ROUNDS = 8;
    private static final int MAX_SOLUTIONS = 100_000;

    private final List<BlockState> states = new ArrayList<>();
    private final int air;
    private final BitSet all;
    private final BitSet none = new BitSet();
    private final BitSet[][] sturdy = new BitSet[6][4];
    private final BitSet solid = new BitSet();
    private final List<TagKey<Block>> tags;
    private final Map<TagKey<Block>, List<Holder<Block>>> resolved = new TreeMap<>(Comparator.comparing(tag -> tag.location().toString()));
    private final Map<TagKey<Block>, BitSet> tagBits = new HashMap<>();
    private final List<Template> templates = new ArrayList<>();

    record Result(Map<BlockState, JsonObject> filters, Map<String, Map<String, Set<String>>> carriers) {}

    private record Template(String label, Node root, int holes, boolean shortForm) {}

    private record Match(Template template, List<int[]> solutions) {}

    private SupportDeriver() {
        for (BlockState state : Block.BLOCK_STATE_REGISTRY) {
            states.add(state);
        }
        air = Block.getId(Blocks.AIR.defaultBlockState());
        all = new BitSet();
        all.set(0, states.size());
        for (Direction face : DIRECTIONS) {
            BitSet[] row = sturdy[face.ordinal()];
            for (int i = 0; i < 4; i++) {
                row[i] = new BitSet();
            }
            for (int id = 0; id < states.size(); id++) {
                BlockState state = states.get(id);
                BlockGetter level = EmptyBlockGetter.INSTANCE;
                row[FULL].set(id, state.isFaceSturdy(level, P, face, SupportType.FULL));
                row[CENTER].set(id, state.isFaceSturdy(level, P, face, SupportType.CENTER));
                row[RING].set(id, state.isFaceSturdy(level, P, face, SupportType.RIGID));
                row[COLLISION_FULL].set(id, Block.isFaceFull(state.getCollisionShape(level, P), face));
            }
        }
        for (int id = 0; id < states.size(); id++) {
            solid.set(id, states.get(id).isSolid());
        }
        BuiltInRegistries.BLOCK.getTags().forEach(named -> resolved.put(named.key(), named.stream().toList()));
        tags = List.copyOf(resolved.keySet());

        Direction down = Direction.DOWN;
        templates.add(new Template("`matching_block_tag`", new TagLeaf(down, 0), 1, true));
        for (Direction face : FACES) {
            templates.add(new Template("`has_sturdy_face` `center`", new Sturdy(face.getOpposite(), CENTER), 0, false));
        }
        for (Direction face : FACES) {
            templates.add(new Template("`has_sturdy_face` `full`", new Sturdy(face.getOpposite(), FULL), 0, false));
        }
        templates.add(new Template("snow layer (SUP-SNOW-001)", new AllOf(List.of(
            new Not(new TagLeaf(down, 0)),
            new AnyOf(List.of(new TagLeaf(down, 1), new Sturdy(down, COLLISION_FULL), fullSnow(down)))
        )), 2, false));
        templates.add(new Template("`not` `matching_block_tag`", new Not(new TagLeaf(down, 0)), 1, false));
        for (Direction face : FACES) {
            templates.add(new Template("`has_sturdy_face` `ring` or `center`",
                new AnyOf(List.of(new Sturdy(face.getOpposite(), RING), new Sturdy(face.getOpposite(), CENTER))), 0, false));
        }
        for (Direction face : FACES) {
            templates.add(new Template("`solid`", new Solid(face.getOpposite()), 0, false));
        }
    }

    static Result derive() {
        return new SupportDeriver().run();
    }

    static JsonObject supportShape(BlockState state, BlockGetter level, BlockPos pos, java.util.function.Function<VoxelShape, JsonArray> boxes) {
        VoxelShape support = state.getBlockSupportShape(level, pos);
        if (!Shapes.joinIsNotEmpty(support, state.getCollisionShape(level, pos, CollisionContext.empty()), BooleanOp.NOT_SAME)) {
            return null;
        }
        JsonObject component = new JsonObject();
        component.add("boxes", boxes.apply(support));
        return component;
    }

    static JsonPrimitive solidValue(BlockState state, BlockGetter level, BlockPos pos) {
        VoxelShape collision = state.getCollisionShape(level, pos, CollisionContext.empty());
        boolean derived;
        if (collision.isEmpty()) {
            derived = false;
        } else {
            AABB bounds = collision.bounds();
            double height = bounds.getYsize();
            derived = (bounds.getXsize() + height + bounds.getZsize()) / 3.0 >= 0.7291666666666666 || height >= 1.0;
        }
        return state.isSolid() == derived ? null : new JsonPrimitive(state.isSolid());
    }

    private Result run() {
        List<Block> subjects = BuiltInRegistries.BLOCK.stream().filter(SupportDeriver::overridesSurvivalTest).toList();
        Map<BlockState, BitSet[]> accepted = new ConcurrentHashMap<>();
        Map<Block, Set<String>> reads = new ConcurrentHashMap<>();
        long start = System.nanoTime();
        subjects.parallelStream().forEach(block -> {
            Probe probe = new Probe();
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                try {
                    accepted.put(state, acceptedSets(probe, state));
                } catch (LevelRead read) {
                    reads.computeIfAbsent(block, unused -> ConcurrentHashMap.newKeySet()).add(read.getMessage());
                }
            }
        });
        System.out.println("support probe: " + accepted.size() + " subject states in " + (System.nanoTime() - start) / 1_000_000 + " ms");

        Map<List<Object>, Match> bySignature = new HashMap<>();
        Map<List<Object>, BlockState> representative = new HashMap<>();
        for (Block block : subjects) {
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                BitSet[] acc = accepted.get(state);
                if (acc != null) {
                    List<Object> signature = signature(block, acc);
                    representative.putIfAbsent(signature, state);
                    bySignature.computeIfAbsent(signature, unused -> match(acc));
                }
            }
        }
        try {
            bySignature.replaceAll((signature, match) -> match == null || match.solutions().size() == 1
                ? match : disambiguate(match, representative.get(signature)));
        } finally {
            rebind(resolved);
        }
        resolved.forEach((tag, members) -> {
            Set<Block> live = new HashSet<>();
            BuiltInRegistries.BLOCK.getTagOrEmpty(tag).forEach(holder -> live.add(holder.value()));
            if (!live.equals(members.stream().map(Holder::value).collect(Collectors.toSet()))) {
                throw new IllegalStateException("block tag " + tag.location() + " was not restored after the marker probe");
            }
        });

        Map<BlockState, JsonObject> filters = new IdentityHashMap<>();
        Map<String, Map<String, Set<String>>> carriers = new TreeMap<>();
        int partial = 0;
        Probe probe = new Probe();
        for (Block block : subjects) {
            String id = BuiltInRegistries.BLOCK.getKey(block).toString();
            List<BlockState> blockStates = block.getStateDefinition().getPossibleStates();
            int matched = 0;
            for (BlockState state : blockStates) {
                BitSet[] acc = accepted.get(state);
                Match match = acc == null ? null : bySignature.get(signature(block, acc));
                if (match == null) {
                    continue;
                }
                Set<Direction> tested;
                try {
                    tested = testedDirections(probe, state);
                } catch (LevelRead read) {
                    System.out.println("  " + id + ": the shape update reads " + read.getMessage() + ", no component");
                    continue;
                }
                int[] assignment = match.solutions().get(0);
                Template template = match.template();
                Set<Direction> offsets = EnumSet.noneOf(Direction.class);
                template.root().offsets(offsets);
                if (tested.isEmpty()) {
                    System.out.println("  " + id + ": no shape update removes it, no component");
                    continue;
                }
                JsonObject filter = template.shortForm() ? shortForm(tagsOf(assignment).get(0)) : predicateForm(template, assignment);
                String testedText = "default";
                if (!tested.equals(offsets)) {
                    JsonElement value = testedJson(tested);
                    filter.add("tested_directions", value);
                    testedText = value.isJsonPrimitive() ? value.getAsString() : names(tested);
                }
                filters.put(state, filter);
                matched++;
                String label = template.label() + (template.holes() == 0 ? ""
                    : " " + tagsOf(assignment).stream().map(tag -> "`#" + tag.location() + "`").collect(Collectors.joining(", ")));
                Map<String, Set<String>> row = carriers.computeIfAbsent(id, unused -> new TreeMap<>());
                row.computeIfAbsent(label, unused -> new TreeSet<>()).add(names(offsets) + "|" + testedText);
            }
            if (matched > 0 && matched < blockStates.size()) {
                partial++;
                System.out.println("  " + id + ": " + matched + " of " + blockStates.size() + " states carry a filter");
            }
        }
        reads.entrySet().stream()
            .sorted(Comparator.comparing(entry -> BuiltInRegistries.BLOCK.getKey(entry.getKey()).toString()))
            .forEach(entry -> System.out.println("  " + BuiltInRegistries.BLOCK.getKey(entry.getKey()) + " reads " + new TreeSet<>(entry.getValue())));
        System.out.println("placement filters: " + carriers.size() + " blocks carry one (" + partial + " on some states only), "
            + subjects.size() + " override the survival test, " + reads.size() + " read the level beyond the neighbours");
        return new Result(filters, carriers);
    }

    private static List<Object> signature(Block block, BitSet[] acc) {
        List<Object> key = new ArrayList<>(List.of(acc));
        key.add(block);
        return key;
    }

    private BitSet[] acceptedSets(Probe probe, BlockState subject) {
        BitSet[] acc = new BitSet[6];
        for (Direction q : DIRECTIONS) {
            BitSet set = new BitSet();
            for (int id = 0; id < states.size(); id++) {
                if (probe.survives(subject, q, states.get(id))) {
                    set.set(id);
                }
            }
            acc[q.ordinal()] = set;
        }
        return acc;
    }

    private Match match(BitSet[] acc) {
        if (Arrays.stream(acc).allMatch(BitSet::isEmpty) || Arrays.stream(acc).allMatch(all::equals)) {
            return null;
        }
        for (Template template : templates) {
            List<int[]> solutions = new ArrayList<>();
            int[] assignment = new int[template.holes()];
            Arrays.fill(assignment, -1);
            search(template, acc, assignment, 0, solutions);
            if (!solutions.isEmpty()) {
                return new Match(template, solutions);
            }
        }
        return null;
    }

    private void search(Template template, BitSet[] acc, int[] assignment, int hole, List<int[]> out) {
        if (!within(template, acc, assignment)) {
            return;
        }
        if (hole == assignment.length) {
            if (out.size() == MAX_SOLUTIONS) {
                throw new IllegalStateException("more than " + MAX_SOLUTIONS + " tag assignments for " + template.label());
            }
            out.add(assignment.clone());
            return;
        }
        for (int i = 0; i < tags.size(); i++) {
            assignment[hole] = i;
            search(template, acc, assignment, hole + 1, out);
        }
        assignment[hole] = -1;
    }

    private boolean within(Template template, BitSet[] acc, int[] assignment) {
        int[] polarity = new int[assignment.length];
        template.root().polarity(1, polarity);
        BitSet[] lower = new BitSet[assignment.length];
        BitSet[] upper = new BitSet[assignment.length];
        for (int h = 0; h < assignment.length; h++) {
            if (assignment[h] >= 0) {
                lower[h] = upper[h] = bitsOf(tags.get(assignment[h]));
            } else if (polarity[h] > 0) {
                lower[h] = none;
                upper[h] = all;
            } else {
                lower[h] = all;
                upper[h] = none;
            }
        }
        for (Direction q : DIRECTIONS) {
            BitSet target = acc[q.ordinal()];
            BitSet low = (BitSet) template.root().eval(q, lower).clone();
            low.andNot(target);
            if (!low.isEmpty()) {
                return false;
            }
            BitSet missing = (BitSet) target.clone();
            missing.andNot(template.root().eval(q, upper));
            if (!missing.isEmpty()) {
                return false;
            }
        }
        return true;
    }

    private Match disambiguate(Match match, BlockState subject) {
        Template template = match.template();
        String subjectId = BuiltInRegistries.BLOCK.getKey(subject.getBlock()).toString();
        Probe probe = new Probe();
        List<int[]> left = new ArrayList<>(match.solutions());
        for (int hole = 0; hole < template.holes(); hole++) {
            Map<Integer, Boolean> read = new TreeMap<>();
            int h = hole;
            for (int[] solution : left) {
                read.computeIfAbsent(solution[h], tag -> gameReads(template, probe, subject, solution, h));
            }
            if (read.containsValue(true)) {
                left.removeIf(solution -> !read.get(solution[h]));
            } else {
                System.out.println("  " + subjectId + ": the game follows none of the candidate tags "
                    + read.keySet().stream().map(tag -> tags.get(tag).location().toString()).toList());
                left.removeIf(solution -> !isAirTag(tags.get(solution[h])));
            }
        }
        if (left.size() != 1) {
            System.out.println("  " + subjectId + ": " + left.size() + " tag assignments remain after the marker probe, no component");
            return null;
        }
        System.out.println("  " + subjectId + ": marker probe picked " + tagsOf(left.get(0)).stream().map(tag -> tag.location().toString()).toList()
            + " out of " + match.solutions().size() + " assignments");
        return new Match(template, left);
    }

    private boolean isAirTag(TagKey<Block> tag) {
        Set<Block> members = new HashSet<>();
        resolved.get(tag).forEach(holder -> members.add(holder.value()));
        Set<Block> air = BuiltInRegistries.BLOCK.stream()
            .filter(block -> block.getStateDefinition().getPossibleStates().stream().allMatch(BlockState::isAir))
            .collect(Collectors.toSet());
        return tag.location().toString().equals("minecraft:air") && members.equals(air);
    }

    private boolean gameReads(Template template, Probe probe, BlockState subject, int[] solution, int hole) {
        TagKey<Block> tag = tags.get(solution[hole]);
        BitSet[] holes = new BitSet[solution.length];
        for (int h = 0; h < solution.length; h++) {
            holes[h] = bitsOf(tags.get(solution[h]));
        }
        Set<Block> members = new HashSet<>();
        resolved.get(tag).forEach(holder -> members.add(holder.value()));
        for (Block marker : BuiltInRegistries.BLOCK) {
            if (members.contains(marker) || marker == subject.getBlock()) {
                continue;
            }
            BitSet widened = (BitSet) holes[hole].clone();
            marker.getStateDefinition().getPossibleStates().forEach(state -> widened.set(Block.getId(state)));
            BitSet[] overlaid = holes.clone();
            overlaid[hole] = widened;
            boolean sensitive = false;
            for (Direction q : DIRECTIONS) {
                sensitive |= !template.root().eval(q, holes).equals(template.root().eval(q, overlaid));
            }
            if (!sensitive) {
                continue;
            }
            Map<TagKey<Block>, List<Holder<Block>>> overlay = new HashMap<>(resolved);
            List<Holder<Block>> withMarker = new ArrayList<>(resolved.get(tag));
            withMarker.add(BuiltInRegistries.BLOCK.wrapAsHolder(marker));
            overlay.put(tag, withMarker);
            try {
                rebind(overlay);
                return within(template, acceptedSets(probe, subject), solution);
            } finally {
                rebind(resolved);
            }
        }
        return true;
    }

    private Set<Direction> testedDirections(Probe probe, BlockState subject) {
        Set<Direction> tested = EnumSet.noneOf(Direction.class);
        BlockState air = Blocks.AIR.defaultBlockState();
        for (Direction from : DIRECTIONS) {
            if (!probe.update(subject, from, air).is(subject.getBlock())) {
                tested.add(from);
            }
        }
        return tested;
    }

    private BitSet bitsOf(TagKey<Block> tag) {
        return tagBits.computeIfAbsent(tag, key -> {
            BitSet bits = new BitSet();
            for (int id = 0; id < states.size(); id++) {
                if (states.get(id).is(key)) {
                    bits.set(id);
                }
            }
            return bits;
        });
    }

    private void rebind(Map<TagKey<Block>, List<Holder<Block>>> overlay) {
        BuiltInRegistries.BLOCK.prepareTagReload(new TagLoader.LoadResult<>(Registries.BLOCK, overlay)).apply();
        tagBits.clear();
    }

    private List<TagKey<Block>> tagsOf(int[] assignment) {
        return Arrays.stream(assignment).mapToObj(tags::get).toList();
    }

    private JsonObject predicateForm(Template template, int[] assignment) {
        JsonObject filter = new JsonObject();
        filter.add("predicate", template.root().json(tagsOf(assignment)));
        return filter;
    }

    private static JsonObject shortForm(TagKey<Block> tag) {
        JsonArray faces = new JsonArray();
        faces.add("up");
        JsonArray blocks = new JsonArray();
        blocks.add("#" + tag.location());
        JsonObject condition = new JsonObject();
        condition.add("allowed_faces", faces);
        condition.add("block_filter", blocks);
        JsonArray conditions = new JsonArray();
        conditions.add(condition);
        JsonObject filter = new JsonObject();
        filter.add("conditions", conditions);
        return filter;
    }

    private static JsonElement testedJson(Set<Direction> tested) {
        if (tested.size() == DIRECTIONS.length) {
            return new JsonPrimitive("all");
        }
        JsonArray list = new JsonArray();
        tested.forEach(direction -> list.add(direction.getSerializedName()));
        return list;
    }

    private static String names(Set<Direction> directions) {
        return directions.stream().map(Direction::getSerializedName).collect(Collectors.joining(", "));
    }

    private static boolean overridesSurvivalTest(Block block) {
        for (Class<?> type = block.getClass(); type != BlockBehaviour.class; type = type.getSuperclass()) {
            try {
                type.getDeclaredMethod("canSurvive", BlockState.class, LevelReader.class, BlockPos.class);
                return true;
            } catch (NoSuchMethodException none) {
            }
        }
        return false;
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private Node fullSnow(Direction offset) {
        Property layers = Blocks.SNOW.getStateDefinition().getProperty("layers");
        Comparable eight = (Comparable) layers.getValue("8").orElseThrow();
        BitSet bits = new BitSet();
        bits.set(Block.getId(Blocks.SNOW.defaultBlockState().setValue(layers, eight)));
        return new Leaf(offset) {
            BitSet bits(BitSet[] holes) {
                return bits;
            }

            JsonObject body(List<TagKey<Block>> tags) {
                JsonObject state = new JsonObject();
                state.addProperty("layers", "8");
                JsonObject json = typed("matching_blocks");
                json.addProperty("blocks", "minecraft:snow");
                json.add("state", state);
                return json;
            }
        };
    }

    private static JsonObject typed(String type) {
        JsonObject json = new JsonObject();
        json.addProperty("type", "minecraft:" + type);
        return json;
    }

    private static JsonArray offsetOf(Direction direction) {
        JsonArray offset = new JsonArray();
        offset.add(direction.getStepX());
        offset.add(direction.getStepY());
        offset.add(direction.getStepZ());
        return offset;
    }

    private interface Node {
        BitSet eval(Direction q, BitSet[] holes);

        JsonObject json(List<TagKey<Block>> tags);

        void polarity(int sign, int[] out);

        void offsets(Set<Direction> out);
    }

    private abstract class Leaf implements Node {
        final Direction offset;

        Leaf(Direction offset) {
            this.offset = offset;
        }

        abstract BitSet bits(BitSet[] holes);

        abstract JsonObject body(List<TagKey<Block>> tags);

        public BitSet eval(Direction q, BitSet[] holes) {
            BitSet bits = bits(holes);
            return q == offset ? bits : bits.get(air) ? all : none;
        }

        public JsonObject json(List<TagKey<Block>> tags) {
            JsonObject body = body(tags);
            JsonObject json = new JsonObject();
            json.add("type", body.remove("type"));
            json.add("offset", offsetOf(offset));
            body.entrySet().forEach(entry -> json.add(entry.getKey(), entry.getValue()));
            return json;
        }

        public void polarity(int sign, int[] out) {}

        public void offsets(Set<Direction> out) {
            out.add(offset);
        }
    }

    private final class TagLeaf extends Leaf {
        final int hole;

        TagLeaf(Direction offset, int hole) {
            super(offset);
            this.hole = hole;
        }

        BitSet bits(BitSet[] holes) {
            return holes[hole];
        }

        JsonObject body(List<TagKey<Block>> tags) {
            JsonObject json = typed("matching_block_tag");
            json.addProperty("tag", tags.get(hole).location().toString());
            return json;
        }

        @Override
        public void polarity(int sign, int[] out) {
            if (out[hole] != 0 && out[hole] != sign) {
                throw new IllegalStateException("a tag hole under both signs");
            }
            out[hole] = sign;
        }
    }

    private final class Sturdy extends Leaf {
        final int test;

        Sturdy(Direction offset, int test) {
            super(offset);
            this.test = test;
        }

        BitSet bits(BitSet[] holes) {
            return sturdy[offset.getOpposite().ordinal()][test];
        }

        JsonObject body(List<TagKey<Block>> tags) {
            JsonObject json = typed("has_sturdy_face");
            json.addProperty("direction", offset.getOpposite().getSerializedName());
            switch (test) {
                case CENTER -> json.addProperty("face_test", "center");
                case RING -> json.addProperty("face_test", "ring");
                case COLLISION_FULL -> json.addProperty("shape", "collision");
                default -> {}
            }
            return json;
        }
    }

    private final class Solid extends Leaf {
        Solid(Direction offset) {
            super(offset);
        }

        BitSet bits(BitSet[] holes) {
            return solid;
        }

        JsonObject body(List<TagKey<Block>> tags) {
            return typed("solid");
        }
    }

    private final class Not implements Node {
        final Node inner;

        Not(Node inner) {
            this.inner = inner;
        }

        public BitSet eval(Direction q, BitSet[] holes) {
            BitSet out = (BitSet) all.clone();
            out.andNot(inner.eval(q, holes));
            return out;
        }

        public JsonObject json(List<TagKey<Block>> tags) {
            JsonObject json = typed("not");
            json.add("predicate", inner.json(tags));
            return json;
        }

        public void polarity(int sign, int[] out) {
            inner.polarity(-sign, out);
        }

        public void offsets(Set<Direction> out) {
            inner.offsets(out);
        }
    }

    private class AnyOf implements Node {
        final List<Node> parts;

        AnyOf(List<Node> parts) {
            this.parts = parts;
        }

        String type() {
            return "any_of";
        }

        public BitSet eval(Direction q, BitSet[] holes) {
            BitSet out = new BitSet();
            parts.forEach(part -> out.or(part.eval(q, holes)));
            return out;
        }

        public JsonObject json(List<TagKey<Block>> tags) {
            JsonObject json = typed(type());
            JsonArray predicates = new JsonArray();
            parts.forEach(part -> predicates.add(part.json(tags)));
            json.add("predicates", predicates);
            return json;
        }

        public void polarity(int sign, int[] out) {
            parts.forEach(part -> part.polarity(sign, out));
        }

        public void offsets(Set<Direction> out) {
            parts.forEach(part -> part.offsets(out));
        }
    }

    private final class AllOf extends AnyOf {
        AllOf(List<Node> parts) {
            super(parts);
        }

        @Override
        String type() {
            return "all_of";
        }

        @Override
        public BitSet eval(Direction q, BitSet[] holes) {
            BitSet out = (BitSet) all.clone();
            parts.forEach(part -> out.and(part.eval(q, holes)));
            return out;
        }
    }

    private static final class LevelRead extends RuntimeException {
        LevelRead(String what) {
            super(what, null, false, false);
        }
    }

    private static final class Probe implements InvocationHandler {
        private final LevelReader level = (LevelReader) Proxy.newProxyInstance(
            LevelReader.class.getClassLoader(), new Class<?>[] {LevelReader.class}, this
        );
        private final ScheduledTickAccess ticks = (ScheduledTickAccess) Proxy.newProxyInstance(
            ScheduledTickAccess.class.getClassLoader(), new Class<?>[] {ScheduledTickAccess.class}, (proxy, method, args) -> null
        );
        private final RandomSource random = RandomSource.create(0);
        private BlockState subject;
        private Direction at;
        private BlockState there;

        boolean survives(BlockState subject, Direction at, BlockState there) {
            this.subject = subject;
            this.at = at;
            this.there = there;
            return subject.canSurvive(level, P);
        }

        BlockState update(BlockState subject, Direction from, BlockState neighbour) {
            this.subject = subject;
            this.at = from;
            this.there = neighbour;
            return subject.updateShape(level, ticks, P, from, P.relative(from), neighbour, random);
        }

        private BlockState stateAt(BlockPos pos, String method) {
            if (pos.equals(P)) {
                return subject;
            }
            for (Direction direction : DIRECTIONS) {
                if (pos.equals(P.relative(direction))) {
                    return direction == at ? there : Blocks.AIR.defaultBlockState();
                }
            }
            throw new LevelRead(method + " beyond the neighbours");
        }

        @Override
        public Object invoke(Object proxy, Method method, Object[] args) throws Throwable {
            if (method.getDeclaringClass() == Object.class) {
                return switch (method.getName()) {
                    case "hashCode" -> System.identityHashCode(proxy);
                    case "equals" -> proxy == args[0];
                    default -> "support probe level";
                };
            }
            if (method.getName().equals("getBlockState") && args.length == 1 && args[0] instanceof BlockPos pos) {
                return stateAt(pos, "getBlockState");
            }
            if (method.getName().equals("getFluidState") && args.length == 1 && args[0] instanceof BlockPos pos) {
                return stateAt(pos, "getFluidState").getFluidState();
            }
            if (method.getName().equals("getBlockEntity") && args.length == 1 && args[0] instanceof BlockPos pos) {
                stateAt(pos, "getBlockEntity");
                return null;
            }
            if (method.isDefault()) {
                return InvocationHandler.invokeDefault(proxy, method, args);
            }
            throw new LevelRead(method.getName());
        }
    }
}
