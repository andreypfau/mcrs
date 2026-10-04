package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import com.mojang.serialization.JsonOps;
import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.BitSet;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.TreeMap;
import java.util.regex.Pattern;
import java.util.stream.Stream;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.HolderSet;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.ItemTags;
import net.minecraft.tags.TagKey;
import net.minecraft.tags.TagLoader;
import net.minecraft.util.Util;
import net.minecraft.util.valueproviders.IntProvider;
import net.minecraft.util.valueproviders.IntProviders;
import net.minecraft.world.Container;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.DropExperienceBlock;
import net.minecraft.world.level.block.DropExperienceEntityBlock;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.StateDefinition;
import net.minecraft.world.level.block.state.properties.BooleanProperty;
import net.minecraft.world.level.block.state.properties.IntegerProperty;
import net.minecraft.world.level.block.state.properties.NoteBlockInstrument;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.PushReaction;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.shapes.CollisionContext;
import net.minecraft.world.phys.shapes.VoxelShape;

public final class BlockDefinitionDumper {
    private static final String FORMAT_VERSION = "1.21.130";
    private static final BlockPos PROBE_POS = new BlockPos(0, 128, 0);
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
    private static final Pattern COORDINATE_TRIPLE = Pattern.compile("\\[\\s+(-?[\\d.]+),\\s+(-?[\\d.]+),\\s+(-?[\\d.]+)\\s+]");
    private static final Pattern BOX = Pattern.compile("\\{\\s+(\"origin\": \\[[^]]*]),\\s+(\"size\": \\[[^]]*])\\s+}");
    private static final List<TagKey<Item>> TOOL_FAMILIES =
        List.of(ItemTags.PICKAXES, ItemTags.AXES, ItemTags.SHOVELS, ItemTags.HOES, ItemTags.SWORDS);
    private static final Map<Item, ItemStack> TOOL_STACKS = new IdentityHashMap<>();

    private BlockDefinitionDumper() {}

    public static void main(final String[] args) {
        int status;
        try {
            status = run(Path.of(args[0]), Integer.parseInt(args[1]));
        } catch (Throwable failure) {
            failure.printStackTrace();
            status = 1;
        }
        System.exit(status);
    }

    private static int run(final Path outDir, final int expectedWorldVersion) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        int observed = SharedConstants.getCurrentVersion().dataVersion().version();
        if (observed != expectedWorldVersion) {
            throw new IllegalStateException(
                "the game has data version " + observed + " but the corpus version file states world_version " + expectedWorldVersion
            );
        }

        try (MultiPackResourceManager resources = new MultiPackResourceManager(
                PackType.SERVER_DATA, List.of(ServerPacksSource.createVanillaPackSource().fullResources())
            )) {
            LayeredRegistryAccess<RegistryLayer> layers = RegistryLayer.createRegistryAccess();
            RegistryAccess.Frozen loaded = loadWorldRegistries(resources, layers);
            Map<Block, TagKey<Block>> placementFilters = derivePlacementFilters();
            RegistryAccess.Frozen access = layers.replaceFrom(RegistryLayer.WORLD, loaded).compositeAccess();
            BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(access).forEach(pending -> pending.apply());

            List<String> failed = new ArrayList<>(dump(outDir.resolve("block_definition"), placementFilters));
            failed.addAll(ItemDefinitionDumper.dump(access, outDir.resolve("item_definition")));
            if (!failed.isEmpty()) {
                System.err.println("failed: " + failed.size() + " " + failed);
                return 1;
            }
        }
        return 0;
    }

    static RegistryAccess.Frozen loadWorldRegistries(
        final MultiPackResourceManager resources, final LayeredRegistryAccess<RegistryLayer> initialLayers
    ) {
        List<Registry.PendingTags<?>> staticLayerTags = TagLoader.loadTagsForExistingRegistries(
            resources, initialLayers.getLayer(RegistryLayer.STATIC)
        );
        RegistryAccess.Frozen worldLoadContext = initialLayers.getAccessForLoading(RegistryLayer.WORLD);
        List<HolderLookup.RegistryLookup<?>> worldContextRegistries = TagLoader.buildUpdatedLookups(worldLoadContext, staticLayerTags);
        RegistryAccess.Frozen loaded = RegistryDataLoader.load(
            resources, worldContextRegistries, RegistryDataLoader.WORLD_REGISTRIES, Util.backgroundExecutor()
        ).join();
        staticLayerTags.forEach(Registry.PendingTags::apply);
        return loaded;
    }

    private static List<String> dump(final Path root, final Map<Block, TagKey<Block>> placementFilters) throws IOException {
        Files.createDirectories(root);
        BlockGetter level = EmptyBlockGetter.INSTANCE;

        BitSet coveredStateIds = new BitSet();
        List<String> failedBlocks = new ArrayList<>();

        for (Block block : BuiltInRegistries.BLOCK) {
            Identifier id = BuiltInRegistries.BLOCK.getKey(block);
            try {
                List<BlockState> states = block.getStateDefinition().getPossibleStates();
                List<Map<String, JsonElement>> perState = new ArrayList<>(states.size());
                for (BlockState state : states) {
                    perState.add(componentsOf(state, level, PROBE_POS, placementFilters.get(block)));
                }

                int baseStateId = Block.getId(states.get(0));
                for (int i = 0; i < states.size(); i++) {
                    int stateId = Block.getId(states.get(i));
                    if (stateId != baseStateId + i) {
                        throw new IllegalStateException(id + " state ids are not contiguous at index " + i);
                    }
                    if (coveredStateIds.get(stateId)) {
                        throw new IllegalStateException("state id " + stateId + " claimed twice, second claim by " + id);
                    }
                    coveredStateIds.set(stateId);
                }

                JsonObject definition = definitionOf(block, id, states, perState, baseStateId);
                Files.writeString(root.resolve(id.getPath() + ".json"), toJson(definition) + "\n", StandardCharsets.UTF_8);
            } catch (Throwable failure) {
                failedBlocks.add(id.toString());
                failure.printStackTrace();
            }
        }

        Files.writeString(root.resolve("README.md"), readme(placementFilters), StandardCharsets.UTF_8);

        int distinctStateIds = Block.BLOCK_STATE_REGISTRY.size();
        boolean contiguous = coveredStateIds.cardinality() == distinctStateIds && coveredStateIds.nextClearBit(0) == distinctStateIds;
        if (!contiguous && failedBlocks.isEmpty()) {
            throw new IllegalStateException(
                "state id coverage is not exactly [0, " + distinctStateIds + "): covered " + coveredStateIds.cardinality()
                    + ", first gap at " + coveredStateIds.nextClearBit(0)
            );
        }

        try (Stream<Path> files = Files.list(root)) {
            System.out.println("wrote " + root + " (" + files.count() + " files)");
        }
        return failedBlocks;
    }

    private static JsonObject definitionOf(
        Block block, Identifier id, List<BlockState> states, List<Map<String, JsonElement>> perState, int baseStateId
    ) {
        StateDefinition<Block, BlockState> stateDefinition = block.getStateDefinition();
        List<Property<?>> properties = List.copyOf(stateDefinition.getProperties());

        JsonObject description = new JsonObject();
        description.addProperty("identifier", id.toString());
        if (!properties.isEmpty()) {
            JsonObject propertyValues = new JsonObject();
            for (Property<?> property : properties) {
                JsonArray values = new JsonArray();
                for (Comparable<?> value : property.getPossibleValues()) {
                    values.add(valueJson(property, value));
                }
                propertyValues.add(property.getName(), values);
            }
            description.add("properties", propertyValues);
        }
        description.addProperty("protocol_id", BuiltInRegistries.BLOCK.getId(block));
        description.addProperty("base_state_id", baseStateId);
        description.addProperty("default_state_id", Block.getId(block.defaultBlockState()));

        Set<String> keys = new LinkedHashSet<>();
        perState.forEach(components -> keys.addAll(components.keySet()));

        JsonObject constant = new JsonObject();
        List<String> varying = new ArrayList<>();
        for (String key : keys) {
            JsonElement first = perState.get(0).get(key);
            boolean same = perState.stream().allMatch(components -> Objects.equals(components.get(key), first));
            if (same && first != null) {
                constant.add(key, first);
            } else if (!same) {
                varying.add(key);
            }
        }

        Map<BlockState, Integer> stateIndex = new IdentityHashMap<>(states.size());
        for (int i = 0; i < states.size(); i++) {
            stateIndex.put(states.get(i), i);
        }

        Map<List<Property<?>>, List<String>> byDependencies = new LinkedHashMap<>();
        for (String key : varying) {
            byDependencies.computeIfAbsent(dependenciesOf(key, states, perState, properties, stateIndex), unused -> new ArrayList<>()).add(key);
        }

        JsonObject definition = new JsonObject();
        definition.add("description", description);
        definition.add("components", constant);

        JsonArray permutations = new JsonArray();
        byDependencies.forEach((deps, componentKeys) -> appendPermutations(deps, componentKeys, states, perState, permutations));
        definition.add("permutations", permutations);

        JsonObject root = new JsonObject();
        root.addProperty("format_version", FORMAT_VERSION);
        root.add("minecraft:block", definition);
        return root;
    }

    private static void appendPermutations(
        List<Property<?>> deps, List<String> componentKeys, List<BlockState> states, List<Map<String, JsonElement>> perState, JsonArray out
    ) {
        Set<String> emitted = new LinkedHashSet<>();
        for (int i = 0; i < states.size(); i++) {
            BlockState state = states.get(i);
            String condition = conditionOf(deps, state);
            if (!emitted.add(condition)) {
                continue;
            }
            JsonObject components = new JsonObject();
            for (String key : componentKeys) {
                JsonElement value = perState.get(i).get(key);
                if (value != null) {
                    components.add(key, value);
                }
            }
            if (components.isEmpty()) {
                continue;
            }
            JsonObject permutation = new JsonObject();
            permutation.addProperty("condition", condition);
            permutation.add("components", components);
            out.add(permutation);
        }
    }

    private static String conditionOf(List<Property<?>> deps, BlockState state) {
        StringBuilder condition = new StringBuilder();
        for (Property<?> property : deps) {
            if (!condition.isEmpty()) {
                condition.append(" && ");
            }
            condition
                .append("q.block_state('")
                .append(property.getName())
                .append("') == ")
                .append(literalOf(property, state.getValue(property)));
        }
        return condition.toString();
    }

    private static List<Property<?>> dependenciesOf(
        String key,
        List<BlockState> states,
        List<Map<String, JsonElement>> perState,
        List<Property<?>> properties,
        Map<BlockState, Integer> stateIndex
    ) {
        List<Property<?>> deps = new ArrayList<>();
        for (Property<?> property : properties) {
            if (dependsOn(key, property, states, perState, stateIndex)) {
                deps.add(property);
            }
        }
        return deps;
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private static boolean dependsOn(
        String key,
        Property<?> property,
        List<BlockState> states,
        List<Map<String, JsonElement>> perState,
        Map<BlockState, Integer> stateIndex
    ) {
        Comparable pivot = property.getPossibleValues().iterator().next();
        for (int i = 0; i < states.size(); i++) {
            int siblingIndex = stateIndex.get(states.get(i).setValue((Property) property, pivot));
            if (!Objects.equals(perState.get(i).get(key), perState.get(siblingIndex).get(key))) {
                return true;
            }
        }
        return false;
    }

    private static Map<String, JsonElement> componentsOf(
        BlockState state, BlockGetter level, BlockPos pos, TagKey<Block> placementFilter
    ) {
        Map<String, JsonElement> components = new LinkedHashMap<>();
        Block block = state.getBlock();

        components.put("minecraft:light_emission", new JsonPrimitive(state.getLightEmission()));
        components.put("minecraft:light_dampening", new JsonPrimitive(state.getLightDampening()));
        components.put("minecraft:friction", number(block.getFriction()));
        components.put("minecraft:map_color", new JsonPrimitive(String.format(Locale.ROOT, "#%06x", state.getMapColor(level, pos).col & 0xFFFFFF)));
        components.put("minecraft:collision_box", boxes(state.getCollisionShape(level, pos, CollisionContext.empty())));
        components.put("minecraft:selection_box", boxes(state.getShape(level, pos, CollisionContext.empty())));

        JsonObject explosion = new JsonObject();
        explosion.add("explosion_resistance", number(block.getExplosionResistance()));
        components.put("minecraft:destructible_by_explosion", explosion);

        JsonObject redstone = new JsonObject();
        redstone.addProperty("redstone_conductor", state.isRedstoneConductor(level, pos));
        components.put("minecraft:redstone_conductivity", redstone);

        components.put("minecraft:movable", movable(state));
        components.put("minecraft:instrument_sound", instrumentSound(state.instrument()));
        if (state.canBeReplaced()) {
            components.put("minecraft:replaceable", new JsonObject());
        }
        if (state.ignitedByLava()) {
            JsonObject flammable = new JsonObject();
            flammable.addProperty("lava_flammable", "always");
            components.put("minecraft:flammable", flammable);
        }
        if (state.hasBlockEntity()) {
            components.put("minecraft:block_entity", blockEntity(state));
        }
        if (state.isSignalSource()) {
            components.put("minecraft:redstone_producer", redstoneProducer(state, level, pos));
        }

        components.put("minecraft:destructible_by_mining", destructibleByMining(state, level, pos));
        block.getLootTable().ifPresent(loot -> components.put("minecraft:loot", new JsonPrimitive(loot.identifier().toString())));
        IntProvider experience = experienceRange(block);
        if (experience != null) {
            components.put("mcrs:experience_drop", IntProviders.CODEC
                .encodeStart(JsonOps.INSTANCE, experience)
                .getOrThrow(error -> new IllegalStateException(
                    BuiltInRegistries.BLOCK.getKey(block) + " experience_drop: " + error)));
        }

        if (placementFilter != null) {
            components.put("minecraft:placement_filter", placementFilterOf(placementFilter));
        }

        components.put("mcrs:use_shape_for_light_occlusion", new JsonPrimitive(state.useShapeForLightOcclusion()));
        components.put("mcrs:occlusion_shape", boxes(state.getOcclusionShape()));
        components.put("mcrs:emissive_rendering", new JsonPrimitive(state.emissiveRendering()));

        assertDerivable(state, level, pos);

        FluidState fluid = state.getFluidState();
        if (!fluid.isEmpty()) {
            JsonObject fluidState = new JsonObject();
            fluidState.addProperty("fluid", BuiltInRegistries.FLUID.getKey(fluid.getType()).toString());
            fluidState.addProperty("level", fluid.getAmount());
            fluidState.addProperty("source", fluid.isSource());
            components.put("mcrs:fluid_state", fluidState);
        }
        return components;
    }

    private static IntProvider experienceRange(Block block) {
        Class<?> owner = block instanceof DropExperienceBlock ? DropExperienceBlock.class
            : block instanceof DropExperienceEntityBlock ? DropExperienceEntityBlock.class : null;
        if (owner == null) {
            return null;
        }
        try {
            Field field = owner.getDeclaredField("xpRange");
            field.setAccessible(true);
            return (IntProvider) field.get(block);
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException(BuiltInRegistries.BLOCK.getKey(block) + " has no readable xpRange", failure);
        }
    }

    private static JsonObject placementFilterOf(TagKey<Block> tag) {
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

    private record Group(List<TagKey<Block>> tags, List<Block> blocks) {}

    private static final class LevelRead extends RuntimeException {
        LevelRead() {
            super(null, null, false, false);
        }
    }

    private static final class SupportProbe implements InvocationHandler {
        private final BlockPos support = PROBE_POS.below();
        private final LevelReader level = (LevelReader) Proxy.newProxyInstance(
            LevelReader.class.getClassLoader(), new Class<?>[] {LevelReader.class}, this
        );
        private BlockState supporting;

        boolean survives(BlockState subject, BlockState supporting) {
            this.supporting = supporting;
            return subject.canSurvive(level, PROBE_POS);
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
            if (method.getName().equals("getBlockState") && support.equals(args[0])) {
                return supporting;
            }
            if (method.isDefault()) {
                return InvocationHandler.invokeDefault(proxy, method, args);
            }
            throw new LevelRead();
        }
    }

    private static Map<Block, TagKey<Block>> derivePlacementFilters() {
        List<Block> blocks = BuiltInRegistries.BLOCK.stream().toList();
        Map<TagKey<Block>, List<Holder<Block>>> resolved = new TreeMap<>(Comparator.comparing(tag -> tag.location().toString()));
        BuiltInRegistries.BLOCK.getTags().forEach(named -> resolved.put(named.key(), named.stream().toList()));
        Map<TagKey<Block>, Set<Block>> originalMembers = new HashMap<>();
        resolved.keySet().forEach(tag -> originalMembers.put(tag, liveMembers(tag)));

        Map<Set<Block>, List<TagKey<Block>>> tagsByMembers = new HashMap<>();
        resolved.keySet().forEach(tag -> tagsByMembers.computeIfAbsent(originalMembers.get(tag), members -> new ArrayList<>()).add(tag));

        SupportProbe probe = new SupportProbe();
        Map<String, Group> groups = new TreeMap<>();
        int overriding = 0;
        int readsLevel = 0;
        int matchesNoTag = 0;
        for (Block block : blocks) {
            if (!overridesSurvivalTest(block)) {
                continue;
            }
            overriding++;
            Set<Block> accepted;
            try {
                accepted = acceptedSupports(probe, block, blocks);
            } catch (LevelRead read) {
                readsLevel++;
                continue;
            }
            List<TagKey<Block>> candidates = accepted.isEmpty() ? null : tagsByMembers.get(accepted);
            if (candidates == null) {
                matchesNoTag++;
                continue;
            }
            groups.computeIfAbsent(candidates.toString(), key -> new Group(candidates, new ArrayList<>())).blocks().add(block);
        }

        Map<Block, TagKey<Block>> named = new HashMap<>();
        try {
            for (Group group : groups.values()) {
                Set<Block> members = originalMembers.get(group.tags().get(0));
                List<Block> markers = blocks.stream().filter(block -> !members.contains(block)).limit(group.tags().size()).toList();
                if (markers.size() != group.tags().size()) {
                    throw new IllegalStateException("not enough marker blocks for " + group.tags());
                }
                Map<TagKey<Block>, List<Holder<Block>>> overlay = new HashMap<>(resolved);
                for (int i = 0; i < markers.size(); i++) {
                    List<Holder<Block>> widened = new ArrayList<>(resolved.get(group.tags().get(i)));
                    widened.add(BuiltInRegistries.BLOCK.wrapAsHolder(markers.get(i)));
                    overlay.put(group.tags().get(i), widened);
                }
                rebindBlockTags(overlay);

                List<Block> candidates = blocks.stream().filter(block -> members.contains(block) || markers.contains(block)).toList();
                for (Block block : group.blocks()) {
                    Set<Block> accepted;
                    try {
                        accepted = acceptedSupports(probe, block, candidates);
                    } catch (LevelRead read) {
                        matchesNoTag++;
                        continue;
                    }
                    List<TagKey<Block>> matching = group.tags().stream().filter(tag -> liveMembers(tag).equals(accepted)).toList();
                    if (matching.size() > 1) {
                        throw new IllegalStateException(BuiltInRegistries.BLOCK.getKey(block) + " matches " + matching + " under the marker overlay");
                    }
                    if (matching.isEmpty()) {
                        matchesNoTag++;
                    } else {
                        named.put(block, matching.get(0));
                    }
                }
                System.out.println("placement filter group " + group.tags().stream().map(tag -> tag.location().toString()).toList()
                    + " markers " + markers.stream().map(marker -> BuiltInRegistries.BLOCK.getKey(marker).toString()).toList());
                group.blocks().forEach(block -> System.out.println(
                    "  " + BuiltInRegistries.BLOCK.getKey(block) + " -> " + (named.containsKey(block) ? named.get(block).location() : "none")));
            }
        } finally {
            rebindBlockTags(resolved);
        }
        resolved.keySet().forEach(tag -> {
            if (!liveMembers(tag).equals(originalMembers.get(tag))) {
                throw new IllegalStateException("block tag " + tag.location() + " was not restored after the marker probe");
            }
        });

        System.out.println(
            "placement filters: " + named.size() + " blocks carry one, " + overriding + " override the survival test, "
                + readsLevel + " read the level, " + matchesNoTag + " accept no tag's members"
        );
        return named;
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

    private static Set<Block> acceptedSupports(SupportProbe probe, Block subject, List<Block> candidates) {
        BlockState state = subject.defaultBlockState();
        Set<Block> accepted = new HashSet<>();
        for (Block candidate : candidates) {
            boolean everyState = true;
            for (BlockState supporting : candidate.getStateDefinition().getPossibleStates()) {
                if (!probe.survives(state, supporting)) {
                    everyState = false;
                    break;
                }
            }
            if (everyState) {
                accepted.add(candidate);
            }
        }
        return accepted;
    }

    private static Set<Block> liveMembers(TagKey<Block> tag) {
        Set<Block> members = new HashSet<>();
        for (Holder<Block> holder : BuiltInRegistries.BLOCK.getTagOrEmpty(tag)) {
            members.add(holder.value());
        }
        return members;
    }

    private static void rebindBlockTags(Map<TagKey<Block>, List<Holder<Block>>> tags) {
        BuiltInRegistries.BLOCK.prepareTagReload(new TagLoader.LoadResult<>(Registries.BLOCK, tags)).apply();
    }

    private static JsonObject movable(BlockState state) {
        JsonObject movable = new JsonObject();
        movable.addProperty("movement_type", movementType(state.getPistonPushReaction()));
        if (state.is(Blocks.SLIME_BLOCK) || state.is(Blocks.HONEY_BLOCK)) {
            movable.addProperty("sticky", "same");
        }
        return movable;
    }

    private static String movementType(PushReaction reaction) {
        return switch (reaction) {
            case PUSH_PULL -> "push_pull";
            case PUSH -> "push";
            case POPPED -> "popped";
            case IMMOVEABLE -> "immovable";
            case IGNORE_ENTITY -> "ignore_entity";
        };
    }

    private static JsonObject instrumentSound(NoteBlockInstrument instrument) {
        JsonObject sound = new JsonObject();
        sound.addProperty(instrument.worksAboveNoteBlock() ? "up" : "down", instrument.getSerializedName());
        return sound;
    }

    private static JsonObject redstoneProducer(BlockState state, BlockGetter level, BlockPos pos) {
        JsonObject producer = new JsonObject();
        producer.addProperty("power", state.getOwnSignal(level, pos));
        return producer;
    }

    private static JsonElement destructibleByMining(BlockState state, BlockGetter level, BlockPos pos) {
        float destroyTime = state.getDestroySpeed(level, pos);
        if (destroyTime < 0.0F) {
            return new JsonPrimitive(false);
        }
        JsonArray speeds = itemSpecificSpeeds(state, destroyTime);
        if (destroyTime == 0.0F && speeds.isEmpty()) {
            return new JsonPrimitive(true);
        }
        JsonObject destructible = new JsonObject();
        if (!speeds.isEmpty()) {
            destructible.add("item_specific_speeds", speeds);
        }
        destructible.add("seconds_to_destroy", number(destroyTime));
        return destructible;
    }

    /// The items that harvest the state, asked of the game rather than assumed:
    /// a tool family goes in as its tag when every item in the tag harvests it,
    /// and anything left over goes in by identifier.
    private static JsonArray itemSpecificSpeeds(BlockState state, float destroyTime) {
        JsonArray speeds = new JsonArray();
        if (!state.requiresCorrectToolForDrops()) {
            return speeds;
        }
        Set<Item> covered = new HashSet<>();
        for (TagKey<Item> family : TOOL_FAMILIES) {
            List<Item> items = new ArrayList<>();
            for (Holder<Item> holder : BuiltInRegistries.ITEM.getTagOrEmpty(family)) {
                items.add(holder.value());
            }
            if (items.isEmpty() || !items.stream().allMatch(item -> harvests(item, state))) {
                continue;
            }
            JsonObject tags = new JsonObject();
            tags.addProperty("tags", "q.any_tag('" + family.location() + "')");
            speeds.add(speedEntry(tags, destroyTime));
            covered.addAll(items);
        }
        for (Item item : BuiltInRegistries.ITEM) {
            if (!covered.contains(item) && harvests(item, state)) {
                speeds.add(speedEntry(new JsonPrimitive(BuiltInRegistries.ITEM.getKey(item).toString()), destroyTime));
            }
        }
        return speeds;
    }

    private static JsonObject speedEntry(JsonElement item, float destroyTime) {
        JsonObject entry = new JsonObject();
        entry.add("item", item);
        entry.add("destroy_speed", number(destroyTime));
        return entry;
    }

    private static boolean harvests(Item item, BlockState state) {
        return TOOL_STACKS.computeIfAbsent(item, ItemStack::new).isCorrectToolForDrops(state);
    }

    private static JsonObject blockEntity(BlockState state) {
        JsonObject blockEntity = new JsonObject();
        BlockEntity entity = state.getBlock() instanceof EntityBlock owner ? owner.newBlockEntity(PROBE_POS, state) : null;
        if (entity instanceof Container container) {
            JsonObject inventory = new JsonObject();
            inventory.addProperty("slot_count", container.getContainerSize());
            blockEntity.add("container", inventory);
        }
        return blockEntity;
    }

    /// Three values the corpus used to carry are recomputed by the loader. A
    /// version that breaks one of these identities has to be noticed here, not
    /// in a silently different light level.
    private static void assertDerivable(BlockState state, BlockGetter level, BlockPos pos) {
        if (state.isSolidRender() != Block.isShapeFullBlock(state.getOcclusionShape())) {
            throw new IllegalStateException(state + ": solid render no longer follows the occlusion shape");
        }
        if (state.isCollisionShapeFullBlock(level, pos)
            != Block.isShapeFullBlock(state.getCollisionShape(level, pos, CollisionContext.empty()))) {
            throw new IllegalStateException(state + ": collision fullness no longer follows the collision shape");
        }
        if (state.propagatesSkylightDown() != (state.getLightDampening() == 0)) {
            throw new IllegalStateException(state + ": skylight propagation no longer follows light dampening");
        }
    }

    private static JsonArray boxes(VoxelShape shape) {
        JsonArray out = new JsonArray();
        for (AABB box : shape.toAabbs()) {
            JsonObject entry = new JsonObject();
            entry.add("origin", vec(box.minX * 16.0 - 8.0, box.minY * 16.0, box.minZ * 16.0 - 8.0));
            entry.add("size", vec((box.maxX - box.minX) * 16.0, (box.maxY - box.minY) * 16.0, (box.maxZ - box.minZ) * 16.0));
            out.add(entry);
        }
        return out;
    }

    private static JsonArray vec(double x, double y, double z) {
        JsonArray out = new JsonArray();
        out.add(number(x));
        out.add(number(y));
        out.add(number(z));
        return out;
    }

    private static JsonPrimitive number(double value) {
        double rounded = Math.round(value * 10000.0) / 10000.0;
        return rounded == Math.rint(rounded) ? new JsonPrimitive((int) rounded) : new JsonPrimitive(rounded);
    }

    private static String toJson(JsonObject value) {
        String pretty = COORDINATE_TRIPLE.matcher(GSON.toJson(value)).replaceAll("[$1, $2, $3]");
        return BOX.matcher(pretty).replaceAll("{ $1, $2 }");
    }

    private static JsonElement valueJson(Property<?> property, Comparable<?> value) {
        String name = propertyName(property, value);
        if (property instanceof IntegerProperty) {
            return new JsonPrimitive(Integer.parseInt(name));
        }
        if (property instanceof BooleanProperty) {
            return new JsonPrimitive(Boolean.parseBoolean(name));
        }
        return new JsonPrimitive(name);
    }

    private static String literalOf(Property<?> property, Comparable<?> value) {
        JsonPrimitive primitive = valueJson(property, value).getAsJsonPrimitive();
        return primitive.isString() ? "'" + primitive.getAsString() + "'" : primitive.getAsString();
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private static String propertyName(Property<?> property, Comparable<?> value) {
        return Util.getPropertyName((Property) property, value);
    }

    private static String readme(Map<Block, TagKey<Block>> placementFilters) {
        StringBuilder carriers = new StringBuilder();
        placementFilters.entrySet().stream()
            .sorted(Comparator.comparing(entry -> BuiltInRegistries.BLOCK.getKey(entry.getKey()).toString()))
            .forEach(entry -> carriers
                .append("| `").append(BuiltInRegistries.BLOCK.getKey(entry.getKey()))
                .append("` | `#").append(entry.getValue().location()).append("` |\n"));
        return README.replace("{placement_filter_carriers}", carriers.toString());
    }

    private static final String README = """
            # mcrs block definition corpus

            Generated from the game's own registries by the `dumpDefinitions` task of
            `tools/vanilla-oracle`, at the game version that `assets/minecraft/version.json`
            states. One file per registry block, named after the registry path.

            The shape is Bedrock's block definition with three additions under
            `description`: `properties` (Java `StateDefinition` declaration order, values in
            `getPossibleValues()` order), `base_state_id` and `default_state_id`. State ids
            are contiguous: state `n` of a block is `base_state_id + n`. `protocol_id` is the
            block's index in `BuiltInRegistries.BLOCK`, which is what the network protocol
            names a block by.

            ## Deviations from the Bedrock schema

            - `minecraft:selection_box` is an array of boxes, not a single object. A Java
              `VoxelShape` is a union of boxes and splitting the encoding from
              `minecraft:collision_box` would be worse.
            - `description.properties` values are typed: integer properties are JSON numbers
              and boolean properties are JSON booleans. Molang conditions compare against the
              matching literal type.
            - `minecraft:destructible_by_mining.seconds_to_destroy` is a decimal. It is
              documented as an integer, but it states hardness rather than seconds, and
              hardness is fractional for most of the game.
            - `minecraft:movable.movement_type` gains `ignore_entity`, the fifth Java
              `PushReaction`. No vanilla block uses it, but the enum would otherwise be lossy.
            - `minecraft:instrument_sound` carries the Java `NoteBlockInstrument` name
              (`harp`) rather than a Bedrock sound alias (`note.harp`). The name selects the
              sound and also decides whether the instrument is tunable by the note property.
            - No `minecraft:geometry`. Block models come from the Java resource pack under
              `assets/minecraft/blockstates/`.

            ## Bedrock components and their Java source

            | Component | Java source |
            | --- | --- |
            | `minecraft:light_emission` | `BlockState.getLightEmission` |
            | `minecraft:light_dampening` | `BlockState.getLightDampening`, what older versions exposed as `getLightBlock` |
            | `minecraft:friction` | `Block.getFriction` |
            | `minecraft:map_color` | `BlockState.getMapColor` |
            | `minecraft:collision_box` | `BlockState.getCollisionShape` |
            | `minecraft:selection_box` | `BlockState.getShape` |
            | `minecraft:destructible_by_mining` | `BlockState.getDestroySpeed`: `false` for an unbreakable state, `true` for an instant one, `seconds_to_destroy` otherwise |
            | `minecraft:destructible_by_explosion` | `Block.getExplosionResistance` |
            | `minecraft:redstone_conductivity` | `BlockState.isRedstoneConductor` |
            | `minecraft:redstone_producer` | `BlockState.isSignalSource`, `power` from `getOwnSignal`, absent when the state is not a source |
            | `minecraft:movable` | `BlockState.getPistonPushReaction`; `sticky` is slime and honey, as `PistonStructureResolver.isSticky` has them |
            | `minecraft:instrument_sound` | `BlockState.instrument`, under `up` when `worksAboveNoteBlock` and `down` otherwise |
            | `minecraft:replaceable` | `BlockState.canBeReplaced`, absent when the state is not replaceable |
            | `minecraft:flammable` | `BlockState.ignitedByLava` as `lava_flammable`, absent when lava cannot ignite it |
            | `minecraft:block_entity` | `BlockState.hasBlockEntity`, with `container.slot_count` when the block entity is a `Container` |
            | `minecraft:loot` | `Block.getLootTable`, absent when the block has none |
            | `minecraft:placement_filter` | `BlockState.canSurvive`, see below |

            A producer whose power lives in its block entity (comparator, sculk sensor)
            reports `power: 0`, which is what it emits with no block entity present.

            `item_specific_speeds` states which items harvest a state that needs the right
            tool, asked of the game rather than assumed: a tool family goes in as its tag
            when every item in that tag harvests the state, and anything left over goes in
            by identifier. Stone gets `q.any_tag('minecraft:pickaxes')`; obsidian, which no
            wooden pickaxe harvests, gets the diamond and netherite pickaxes by name. The
            list is absent exactly when `requiresCorrectToolForDrops` is false, and
            `destroy_speed` repeats the hardness because Java applies its harvest bonus as a
            divisor rather than a second hardness.

            ## Placement filter

            `minecraft:placement_filter` states which supporting blocks let a block stay
            where it is: one condition whose `allowed_faces` is `up` and whose
            `block_filter` is one block tag. It is derived by asking the game, never
            written by hand.

            For each block that overrides `BlockState.canSurvive` the dump calls it on the
            default state with a stand-in level that answers one question, the state of the
            block directly below, and tries every state of every block there. The blocks it
            accepts are the accepted set. A block whose survival test asks the level
            anything else (a fluid, the light, a neighbour, the block above) gets no
            component, and neither does a block whose accepted set is no tag's members;
            nothing is answered with a made-up value. The protected `mayPlaceOn` is not
            called: a class can inherit it and never use it, so only the survival test says
            what the game accepts.

            Several block tags can hold the same members, and equal members do not say
            which tag the game tests. The dump groups the block tags by members on every
            run and probes each group: it rebinds the block tags with one distinct marker
            block added to each tag of the group, asks the game again, and names the one tag
            whose members under that overlay equal the accepted set under it. The tags are
            rebound unchanged before anything else reads them. When no tag matches, the
            block gets no component.

            The blocks that carry the component:

            | Block | Tag |
            | --- | --- |
            {placement_filter_carriers}
            ## What the loader recomputes

            Values Java itself derives, or that another asset already states, are not written
            here. The dump asserts the first three identities for every state, so a version
            that breaks one fails the dump instead of the game.

            | Value | Where it comes from instead |
            | --- | --- |
            | `BlockState.isSolidRender` | `Block.isShapeFullBlock(occlusion shape)` |
            | `BlockState.isCollisionShapeFullBlock` | `Block.isShapeFullBlock(collision shape)` |
            | `BlockState.propagatesSkylightDown` | `light_dampening == 0` |
            | `BlockState.isAir` | the identifier, as Bedrock has it: `air`, `cave_air`, `void_air` |
            | `BlockState.getRenderShape` | the resource pack. Every state Java calls `INVISIBLE` has a model with no elements, and so do the block-entity-rendered blocks it calls `MODEL`, so the model is the authority on what the block draws |
            | `BlockState.hasAnalogOutputSignal` | block behaviour. The flag alone says nothing without `getAnalogOutputSignal`, which is code — container fullness, cake bites, composter level. The container half of it is `minecraft:block_entity.container` |

            ## `mcrs:` components

            What is left is Java data no Bedrock component can carry. Each row states why.

            | Component | Java source | Why it has no Bedrock component |
            | --- | --- | --- |
            | `mcrs:occlusion_shape` | `BlockState.getOcclusionShape` | `minecraft:geometry.culling_shape` names a registered voxel shape definition, and vanilla registers only `minecraft:unit_cube` and `minecraft:empty`. A per-state box union has no name to give |
            | `mcrs:use_shape_for_light_occlusion` | `BlockState.useShapeForLightOcclusion` | `LightEngine.isEmptyShape` reads it to decide whether the occlusion shape counts at all, so it is behaviour and not a hint. Bedrock's light model is one integer per block and has no occlusion shape to switch on |
            | `mcrs:emissive_rendering` | `BlockState.emissiveRendering` | The nearest Bedrock field, `material_instances.face_dimming`, turns off directional shading rather than lighting, and lives in a component that must accompany `minecraft:geometry` |
            | `mcrs:fluid_state` | `BlockState.getFluidState` | `liquid_detection.can_contain_liquid` states that a block *can* be waterlogged. It carries no level, no source flag, and `liquid_type` accepts only water, so lava and flowing water have nowhere to go |
            | `mcrs:experience_drop` | `xpRange` of `DropExperienceBlock` or of `DropExperienceEntityBlock`, encoded by `IntProviders.CODEC` | Bedrock has no experience component at all; its ores drop experience from the loot table's `minecraft:furnace_smelt`-era behaviour rather than a declared range. The value is Java's `IntProvider`, so a constant is a bare integer and anything else is the dispatched object. Absent for a block that is neither: `SpawnerBlock` and `CreakingHeartBlock` compute their amount in code rather than holding a range |

            ## Java block properties this corpus does not write

            Expressible in Bedrock, not yet dumped:

            | Java | Bedrock component | Why it is not here yet |
            | --- | --- | --- |
            | `Properties.soundType` | `minecraft:sound` | Java's `SoundType` is an object of five sound events with volume and pitch, and has no name to give the component's one string. Naming the constants takes reflection over `SoundType`'s fields, and the Bedrock sound group names are not Java's |
            | `Properties.offsetFunction` | `minecraft:random_offset` | Java's offset is continuous and derived from the position hash. Bedrock states `steps` and a range, which has no Java counterpart, and the max offsets are only recoverable by sampling `getOffset` |
            | `Properties.spawnTerrainParticles` | `minecraft:destruction_particles` | A boolean against a particle count; the mapping is `false` to `particle_count: 0` and nothing else, and no reader wants it yet |
            | `Properties.descriptionId` | `minecraft:display_name` | It is `block.<namespace>.<path>` by convention for every vanilla block, so the identifier already states it |

            No Bedrock component exists for these, and nothing in mcrs reads them yet:
            `speedFactor`, `jumpFactor`, `bounceRestitution`, `fallDistanceReduction`,
            `isSuffocating`, `isViewBlocking`, `isValidSpawn`, `isRandomlyTicking`,
            `postProcess` and `requiredFeatures`.

            ## Permutations

            A component that is the same for every state sits in `components`. A component
            that varies sits in `permutations`, keyed by the properties it actually depends
            on. Conditions use only:
            `q.block_state('name')`, `==`, `!=`, `&&`, `||`, `!`, parentheses, string and
            number literals.

            A component absent from a state is absent from that state's permutation, and a
            permutation that would state nothing is not written. A loader applies
            `components` first, then every matching permutation in order.
            """;
}
