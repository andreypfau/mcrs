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
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.BitSet;
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
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.DropExperienceBlock;
import net.minecraft.world.level.block.DropExperienceEntityBlock;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
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
            SupportDeriver.Result placementFilters = SupportDeriver.derive();
            RegistryAccess.Frozen access = layers.replaceFrom(RegistryLayer.WORLD, loaded).compositeAccess();
            BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(access).forEach(pending -> pending.apply());

            List<String> failed = new ArrayList<>(dump(outDir.resolve("block_definition"), placementFilters));
            failed.addAll(ItemDefinitionDumper.dump(access, outDir.resolve("item_definition")));
            failed.addAll(GameRuleDefinitionDumper.dump(outDir.resolve("registry_definition")));
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

    private static List<String> dump(final Path root, final SupportDeriver.Result placementFilters) throws IOException {
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
                    perState.add(componentsOf(state, level, PROBE_POS, placementFilters.filters().get(state)));
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
        BlockState state, BlockGetter level, BlockPos pos, JsonObject placementFilter
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
            components.put("minecraft:placement_filter", placementFilter);
        }
        JsonObject support = SupportDeriver.supportShape(state, level, pos, BlockDefinitionDumper::boxes);
        if (support != null) {
            components.put("minecraft:support", support);
        }
        JsonPrimitive solid = SupportDeriver.solidValue(state, level, pos);
        if (solid != null) {
            components.put("mcrs:solid", solid);
        }

        components.put("mcrs:use_shape_for_light_occlusion", new JsonPrimitive(state.useShapeForLightOcclusion()));
        components.put("mcrs:occlusion_shape", boxes(state.getOcclusionShape()));
        components.put("mcrs:emissive_rendering", new JsonPrimitive(state.emissiveRendering()));
        float occlusionShade = state.getShadeBrightness(level, pos);
        float derivedOcclusionShade = state.isCollisionShapeFullBlock(level, pos) ? 0.2F : 1.0F;
        if (occlusionShade != derivedOcclusionShade) {
            components.put("mcrs:occlusion_shade", new JsonPrimitive(occlusionShade));
        }

        assertDerivable(state, level, pos);

        FluidState fluid = state.getFluidState();
        if (!fluid.isEmpty()) {
            JsonObject fluidState = new JsonObject();
            fluidState.addProperty("fluid", BuiltInRegistries.FLUID.getKey(fluid.getType()).toString());
            fluidState.addProperty("level", fluid.getAmount());
            fluidState.addProperty("source", fluid.isSource());
            components.put("mcrs:fluid_state", fluidState);
        }
        state.getSounds().ifPresent(sounds -> components.put("mcrs:block_sound_set", new JsonPrimitive(sounds.identifier().toString())));
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

    private static String readme(SupportDeriver.Result placementFilters) {
        StringBuilder carriers = new StringBuilder();
        placementFilters.carriers().forEach((block, rows) -> rows.forEach((predicate, variants) -> {
            List<String> faces = variants.stream().map(variant -> variant.substring(0, variant.indexOf('|'))).distinct().toList();
            List<String> tested = variants.stream().map(variant -> variant.substring(variant.indexOf('|') + 1)).distinct().toList();
            carriers.append("| `").append(block).append("` | ").append(predicate)
                .append(" | ").append(String.join("; ", faces))
                .append(" | ").append(String.join("; ", tested)).append(" |\n");
        }));
        return README.replace("{placement_filter_carriers}", carriers.toString());
    }

    private static final String README = """
            # mcrs block definition corpus

            Generated from the game's own registries by the `dumpDefinitions` task of
            `tools/vanilla-oracle`, at the game version that `assets/minecraft/version.json`
            states. One file per registry block, named after the registry path.

            The shape is Bedrock's block definition with three additions under
            `description`: `properties` (in the order of the game's block-state report, values
            in the order of that report), `base_state_id` and `default_state_id`. State ids
            are contiguous: state `n` of a block is `base_state_id + n`. `protocol_id` is the
            block's index in the `minecraft:block` registry, which is what the network
            protocol names a block by.

            ## Deviations from the Bedrock schema

            - `minecraft:selection_box` is an array of boxes, not a single object. A Java
              block shape is a union of boxes and splitting the encoding from
              `minecraft:collision_box` would be worse.
            - `description.properties` values are typed: integer properties are JSON numbers
              and boolean properties are JSON booleans. Molang conditions compare against the
              matching literal type.
            - `minecraft:destructible_by_mining.seconds_to_destroy` is a decimal. It is
              documented as an integer, but it states hardness rather than seconds, and
              hardness is fractional for most of the game.
            - `minecraft:movable.movement_type` gains `ignore_entity`, the fifth Java piston
              reaction. No vanilla block uses it, but the enum would otherwise be lossy.
            - `minecraft:instrument_sound` carries the Java instrument name (`harp`) rather
              than a Bedrock sound alias (`note.harp`). The name selects the sound and also
              decides whether the instrument is tunable by the note property.
            - No `minecraft:geometry`. Block models come from the Java resource pack under
              `assets/minecraft/blockstates/`.

            ## Bedrock components and what they state

            | Component | What it states |
            | --- | --- |
            | `minecraft:light_emission` | the light level that the state emits |
            | `minecraft:light_dampening` | the light levels that the state removes from light that passes through it |
            | `minecraft:friction` | the friction of the block |
            | `minecraft:map_color` | the map colour of the state |
            | `minecraft:collision_box` | the collision shape of the state |
            | `minecraft:selection_box` | the outline shape of the state |
            | `minecraft:destructible_by_mining` | the hardness: `false` for an unbreakable state, `true` for an instant one, `seconds_to_destroy` otherwise |
            | `minecraft:destructible_by_explosion` | the explosion resistance of the block |
            | `minecraft:redstone_conductivity` | whether the state conducts redstone power |
            | `minecraft:redstone_producer` | whether the state is a power source, with `power` its own signal; absent when the state is not a source |
            | `minecraft:movable` | how a piston moves the state; `sticky` is slime and honey, the blocks that pull their neighbours |
            | `minecraft:instrument_sound` | the instrument of a note block next to the state, under `up` when the instrument sounds from a note block above it and `down` otherwise |
            | `minecraft:replaceable` | present when placing a block can replace the state |
            | `minecraft:flammable` | `lava_flammable` when lava can set the state on fire; absent otherwise |
            | `minecraft:block_entity` | present when the state has a block entity, with `container.slot_count` when the block entity is a container |
            | `minecraft:loot` | the loot table of the block, absent when the block has none |
            | `minecraft:placement_filter` | where the neighbours must be for the block to stay, see below |
            | `minecraft:support` | the support shape, when it is not the collision shape, see below |

            A producer whose power lives in its block entity (comparator, sculk sensor)
            reports `power: 0`, which is what it emits with no block entity present.

            `item_specific_speeds` states which items harvest a state that needs the right
            tool, asked of the game rather than assumed: a tool family goes in as its tag
            when every item in that tag harvests the state, and anything left over goes in
            by identifier. Stone gets `q.any_tag('minecraft:pickaxes')`; obsidian, which no
            wooden pickaxe harvests, gets the diamond and netherite pickaxes by name. The
            list is absent exactly when the state drops its loot without the right tool, and
            `destroy_speed` repeats the hardness because Java applies its harvest bonus as a
            divisor rather than a second hardness.

            ## Placement filter

            `minecraft:placement_filter` holds the support predicate of a block: where its
            neighbours must be for the block to stay (`docs/spec/block-support.md`). It is
            derived by asking the game, never written by hand.

            For each state of each block with its own survival test, the dump runs the test
            with a stand-in level. The level answers the state of the block itself and of
            its six neighbours, air at five of them, and says that no neighbour has a block
            entity. The dump puts each state of each block at the sixth neighbour, for each
            of the six. The states that let the block stay are the accepted sets of that
            state. A state whose test asks the level anything else (the light, a fluid
            further away, a block two steps away) gets no component. Nothing is answered
            with a made-up value.

            The dump then tries the candidate predicates of section 5 of the specification,
            in its order, and writes the first one whose result at each neighbour is the
            accepted set: a block tag below; the `center` face test, then the `full` face
            test, on each side; the snow layer predicate; not a block tag below; the `ring`
            or `center` face test on each side; `solid` on each side. A state that can never
            stay, a state that always stays, and a state that no candidate matches get no
            component. The face tests and `solid` are asked of the game for each state.

            A candidate that reads a tag can match several tags, because several tags can
            hold the same members and a tag can differ only in blocks the rest of the
            predicate decides. The dump probes each tag it could write: it adds one marker
            block to that tag alone, picked so that the candidate would answer differently
            for it, rebinds the block tags, asks the game again, and keeps the tag only when
            the game follows it. The tags are rebound unchanged before anything else reads
            them. A carpet follows no tag at all: it tests that the block below is not air.
            The dump then writes `#minecraft:air`, the tag that the specification names for
            air, when its members are exactly the blocks the game reports as air. Any other
            choice that stays open gives no component.

            `tested_directions` is derived as well: for each matched state, with every
            neighbour air, the dump sends a shape update from each of the six directions and
            records which ones remove the block. The field is written only when that set
            differs from the directions of the offsets that the predicate reads. A block
            that no shape update removes gets no component.

            Plants keep the short form of Bedrock, one condition whose `allowed_faces` is
            `up` and whose `block_filter` is one block tag. Every other block has the
            predicate form, a block predicate of the data pack under `predicate`. A
            component in `permutations` gives the predicate of the states it selects, so a
            block on a wall names its supporting side for each `facing`, and the upper half
            of a tall plant or a door carries none.

            The blocks that carry the component. The supporting position is the side of the
            block that the predicate reads, one entry for each state that differs:

            | Block | Predicate | Supporting position | Tested directions |
            | --- | --- | --- | --- |
            {placement_filter_carriers}

            ## Support shape and solid

            `minecraft:support` holds the support shape of a state under `boxes`, in the
            form of `minecraft:selection_box`: the shape whose faces the face tests of a
            placement filter read. It is written only for a state whose support shape is
            not its collision shape, for example leaves (empty), snow layers, fence gates
            and hanging signs. The shape is asked of the game with no block entity, so a
            shulker box has its closed shape.

            `mcrs:solid` is written only for a state whose solid value is not the one that
            its collision shape gives: not solid when the shape is empty; solid when the
            mean of the three sizes of its bounds is `0.7291666666666666` or more, or its
            height is a full block; not solid otherwise. A loader computes the value from
            `minecraft:collision_box` with exactly that rule.

            ## What the loader recomputes

            Values the game itself derives, or that another asset already states, are not
            written here. The dump asserts the first three identities for every state, so a
            version that breaks one fails the dump instead of the game.

            | Value | Where it comes from instead |
            | --- | --- |
            | the state is an opaque full cube | the occlusion shape covers the whole block |
            | the collision shape is a full block | the collision shape covers the whole block |
            | sky light passes down through the state | `light_dampening == 0` |
            | the state is air | the identifier, as Bedrock has it: `air`, `cave_air`, `void_air` |
            | how the state is drawn | the resource pack. Every state that the game does not draw has a model with no elements, and so do the blocks that a block-entity renderer draws, so the model is the authority on what the block draws |
            | the state gives a comparator signal | block behaviour. The flag alone says nothing without the signal value, which is code: container fullness, cake bites, composter level. The container half of it is `minecraft:block_entity.container` |

            ## `mcrs:` components

            What is left is Java data no Bedrock component can carry. Each row states why.

            | Component | What it states | Why it has no Bedrock component |
            | --- | --- | --- |
            | `mcrs:occlusion_shape` | the shape that hides the faces of neighbours and blocks light | `minecraft:geometry.culling_shape` names a registered voxel shape definition, and vanilla registers only `minecraft:unit_cube` and `minecraft:empty`. A per-state box union has no name to give |
            | `mcrs:use_shape_for_light_occlusion` | whether the occlusion shape blocks light at all | The light engine reads it to decide whether the occlusion shape counts at all, so it is behaviour and not a hint. Bedrock's light model is one integer per block and has no occlusion shape to switch on |
            | `mcrs:emissive_rendering` | the state is drawn at full brightness | The nearest Bedrock field, `material_instances.face_dimming`, turns off directional shading rather than lighting, and lives in a component that must accompany `minecraft:geometry` |
            | `mcrs:occlusion_shade` | the factor by which the state darkens the corners of the faces next to it | Bedrock has no per-state value for the darkening of corners. Written only when the value differs from 0.2 for a full collision box and 1.0 otherwise |
            | `mcrs:fluid_state` | the fluid in the state | `liquid_detection.can_contain_liquid` states that a block *can* be waterlogged. It carries no level, no source flag, and `liquid_type` accepts only water, so lava and flowing water have nowhere to go |
            | `mcrs:experience_drop` | the experience range that breaking the block drops | Bedrock has no experience component at all; its ores drop experience from the loot table's `minecraft:furnace_smelt`-era behaviour rather than a declared range. The value is an int provider, so a constant is a bare integer and anything else is the dispatched object. Absent for a block without a range: the spawner and the creaking heart compute their amount in code |
            | `mcrs:solid` | the solid value, when the collision shape does not give it, see below | Bedrock has no solid flag; the value is the one that the data pack predicate `minecraft:solid` and the placement filters read |
            | `mcrs:block_sound_set` | The key of the state's entry in the `minecraft:block_sound_set` registry | `minecraft:sound` names a Bedrock sound group, and Java's sound set names are not Bedrock's. Absent for a block with no sound set: water, lava and bubble column |

            ## Java block properties this corpus does not write

            Expressible in Bedrock, not yet dumped:

            | Java property | Bedrock component | Why it is not here yet |
            | --- | --- | --- |
            | the random offset of the model | `minecraft:random_offset` | Java's offset is continuous and derived from the position hash. Bedrock states `steps` and a range, which has no Java counterpart, and the max offsets are only recoverable by sampling the offset |
            | the terrain particles on break | `minecraft:destruction_particles` | A boolean against a particle count; the mapping is `false` to `particle_count: 0` and nothing else, and no reader wants it yet |
            | the translation key | `minecraft:display_name` | It is `block.<namespace>.<path>` by convention for every vanilla block, so the identifier already states it |

            No Bedrock component exists for these, and nothing in mcrs reads them yet: the
            speed factor, the jump factor, the bounce, the fall distance reduction, whether
            the state suffocates, whether it blocks the view, whether mobs can spawn on it,
            whether it ticks randomly, whether it needs post-processing, and the features
            that it requires.

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
