package mcrs.data;

import com.google.gson.JsonParser;
import com.mojang.datafixers.DataFixer;
import com.mojang.serialization.Codec;
import com.mojang.serialization.JsonOps;
import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.Executor;
import net.minecraft.SharedConstants;
import net.minecraft.util.Util;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.component.DataComponentInitializers;
import net.minecraft.core.component.DataComponentPatch;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.NbtOps;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.ReloadableServerRegistries;
import net.minecraft.server.Services;
import net.minecraft.server.WorldStem;
import net.minecraft.server.dedicated.DedicatedServer;
import net.minecraft.server.dedicated.DedicatedServerSettings;
import net.minecraft.server.jsonrpc.ManagementServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.notifications.NotificationManager;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagKey;
import net.minecraft.tags.TagLoader;
import net.minecraft.util.RandomSource;
import net.minecraft.util.context.ContextKey;
import net.minecraft.util.context.ContextKeySet;
import net.minecraft.world.SimpleContainer;
import net.minecraft.world.flag.FeatureFlagSet;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.CustomSpawner;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.LevelAccessor;
import net.minecraft.world.level.StructureManager;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.levelgen.structure.StructureCheck;
import net.minecraft.world.level.levelgen.structure.StructureStart;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.dimension.LevelStem;
import net.minecraft.world.level.gamerules.GameRules;
import net.minecraft.world.level.levelgen.structure.Structure;
import net.minecraft.world.level.storage.LevelStorageSource;
import net.minecraft.world.level.storage.ServerLevelData;
import net.minecraft.world.level.storage.loot.LootContext;
import net.minecraft.world.level.storage.loot.LootParams;
import net.minecraft.world.level.storage.loot.LootTable;
import net.minecraft.world.level.storage.loot.parameters.LootContextParamSets;
import net.minecraft.world.level.storage.loot.parameters.LootContextParams;
import net.minecraft.world.level.storage.loot.predicates.LootItemCondition;
import net.minecraft.world.level.storage.loot.providers.number.floats.ContextFloatProvider;
import net.minecraft.world.level.storage.loot.providers.number.floats.ContextFloatProviders;
import net.minecraft.world.level.storage.loot.providers.number.ints.ContextIntProvider;
import net.minecraft.world.level.storage.loot.providers.number.ints.ContextIntProviders;
import net.minecraft.world.phys.Vec3;

public final class LootOracle {
    private static final byte[] MAGIC = "MCLOOTR0".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;

    private static final int KIND_ROLL = 0;
    private static final int KIND_FILL = 1;
    private static final int KIND_CONDITION = 2;
    private static final int KIND_INT = 3;
    private static final int KIND_FLOAT = 4;

    private static final int SOURCE_ID = 0;
    private static final int SOURCE_INLINE = 1;

    private static final int VALUE_VEC3 = 0;
    private static final int VALUE_STACK = 1;
    private static final int VALUE_BLOCK_STATE = 2;
    private static final int VALUE_F32 = 3;

    private static final int GENERATOR_LEGACY = 0;

    private static final long[] FILL_SEEDS = {1L, 42L, 0x7FFF_FFFF_0000_0001L};
    private static final long CHEST_SEED = 42L;
    private static final List<String> CHEST_SHAPES = List.of(
        "minecraft:chests/ancient_city",
        "minecraft:chests/buried_treasure",
        "minecraft:chests/shipwreck_map",
        "minecraft:chests/stronghold_library",
        "minecraft:chests/village/village_weaponsmith"
    );
    private static final long[] SINGLE_SEEDS = {1L, 42L, -7L};
    private static final int CHEST_SLOTS = 27;
    private static final int SAMPLES = 16;
    private static final Vec3 ORIGIN = new Vec3(0.5, 64.5, 0.5);
    private static final String DEFAULT_BIOME = "minecraft:plains";

    private static RandomSource armed;

    private LootOracle() {}

    private record Value(ContextKey<?> key, Object value) {}

    private record Case(
        int kind,
        String name,
        int source,
        String sourceText,
        ContextKeySet paramSet,
        List<Value> values,
        long seed,
        int size,
        String biome
    ) {
        Case(final int kind, final String name, final int source, final String sourceText, final ContextKeySet paramSet,
            final List<Value> values, final long seed, final int size) {
            this(kind, name, source, sourceText, paramSet, values, seed, size, DEFAULT_BIOME);
        }
    }

    public static void main(final String[] args) throws Exception {
        Path out = Path.of(args[0]);
        Files.createDirectories(out);
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        try (MultiPackResourceManager resources = new MultiPackResourceManager(
            PackType.SERVER_DATA, List.of(ServerPacksSource.createVanillaPackSource().fullResources())
        )) {
            LayeredRegistryAccess<RegistryLayer> initial = RegistryLayer.createRegistryAccess();
            List<Registry.PendingTags<?>> staticTags = TagLoader.loadTagsForExistingRegistries(
                resources, initial.getLayer(RegistryLayer.STATIC)
            );
            RegistryAccess.Frozen worldContext = initial.getAccessForLoading(RegistryLayer.WORLD);
            List<HolderLookup.RegistryLookup<?>> worldLookups = TagLoader.buildUpdatedLookups(worldContext, staticTags);
            RegistryAccess.Frozen world = RegistryDataLoader.load(
                resources, worldLookups, RegistryDataLoader.WORLD_REGISTRIES, Util.backgroundExecutor()
            ).join();
            LayeredRegistryAccess<RegistryLayer> layers = initial.replaceFrom(RegistryLayer.WORLD, world);
            ReloadableServerRegistries.LoadResult reloaded = ReloadableServerRegistries.reload(
                layers, staticTags, resources, Util.backgroundExecutor()
            ).join();
            staticTags.forEach(Registry.PendingTags::apply);
            RegistryAccess access = reloaded.layers().compositeAccess();
            BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(access).forEach(DataComponentInitializers.PendingComponents::apply);

            HolderLookup.Provider lookup = reloaded.lookupWithUpdatedTags();
            ReloadableServerRegistries.Holder holder = new ReloadableServerRegistries.Holder(lookup);
            LootServer server = LootServer.allocate(reloaded.layers(), holder);
            LootLevel level = LootLevel.allocate(server, access);

            new LootOracle.Run(lookup, holder, level).write(out.resolve("loot.bin"));
        }
        System.exit(0);
    }

    private static final class Run {
        private final HolderLookup.Provider lookup;
        private final ReloadableServerRegistries.Holder holder;
        private final LootLevel level;

        Run(final HolderLookup.Provider lookup, final ReloadableServerRegistries.Holder holder, final LootLevel level) {
            this.lookup = lookup;
            this.holder = holder;
            this.level = level;
        }

        void write(final Path file) throws IOException {
            List<Case> cases = cases();
            try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
                out.write(MAGIC);
                i32(out, FORMAT_VERSION);
                i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
                i32(out, cases.size());
                for (Case c : cases) {
                    writeCase(out, c);
                }
            }
            System.out.println("wrote " + file + " (" + cases.size() + " cases, " + Files.size(file) + " bytes)");
        }

        private List<Case> cases() {
            List<Case> cases = new ArrayList<>();
            List<String> chests = this.lookup.lookupOrThrow(Registries.LOOT_TABLE).listElementIds()
                .map(key -> key.identifier().toString())
                .filter(id -> id.startsWith("minecraft:chests/"))
                .sorted()
                .toList();
            List<Value> chest = List.of(new Value(LootContextParams.ORIGIN, ORIGIN));
            for (String id : chests) {
                long[] seeds = CHEST_SHAPES.contains(id) ? FILL_SEEDS : new long[] {CHEST_SEED};
                for (long seed : seeds) {
                    cases.add(new Case(KIND_FILL, id, SOURCE_ID, id, LootContextParamSets.CHEST, chest, seed, CHEST_SLOTS));
                }
            }
            String[][] inBiome = {
                {"minecraft:chests/abandoned_mineshaft", "minecraft:sulfur_caves"},
                {"minecraft:chests/abandoned_camp_common_chest", "minecraft:bamboo_jungle"},
                {"minecraft:chests/abandoned_camp_common_chest", "minecraft:cherry_grove"},
            };
            for (String[] b : inBiome) {
                for (long seed : FILL_SEEDS) {
                    cases.add(new Case(KIND_FILL, b[0] + " in " + b[1], SOURCE_ID, b[0], LootContextParamSets.CHEST, chest, seed, CHEST_SLOTS, b[1]));
                }
            }
            for (long seed : FILL_SEEDS) {
                String id = "minecraft:chests/end_city_treasure";
                cases.add(new Case(KIND_FILL, id + " in 5 slots", SOURCE_ID, id, LootContextParamSets.CHEST, chest, seed, 5));
            }
            singleTypeCases(cases);
            return cases;
        }

        private void singleTypeCases(final List<Case> cases) {
            List<Value> chest = List.of(new Value(LootContextParams.ORIGIN, ORIGIN));
            List<Value> blockPlain = blockValues("{\"id\":\"minecraft:iron_pickaxe\"}", null);
            List<Value> blockFortune = blockValues(
                "{\"id\":\"minecraft:iron_pickaxe\",\"components\":{\"minecraft:enchantments\":{\"minecraft:fortune\":3}}}", null
            );
            List<Value> blockExploded = blockValues("{\"id\":\"minecraft:iron_pickaxe\"}", 4.0F);

            String[][] tables = {
                {"set_count uniform", "chest", item("minecraft:arrow", "{\"type\":\"minecraft:set_count\",\"count\":{\"type\":\"minecraft:uniform\",\"min\":1,\"max\":64}}")},
                {"set_count binomial", "chest", item("minecraft:arrow", "{\"type\":\"minecraft:set_count\",\"count\":{\"type\":\"minecraft:binomial\",\"n\":20,\"p\":0.35}}")},
                {"set_damage uniform", "chest", item("minecraft:iron_sword", "{\"type\":\"minecraft:set_damage\",\"damage\":{\"type\":\"minecraft:uniform\",\"min\":0.1,\"max\":0.9}}")},
                {"enchant_randomly", "chest", item("minecraft:book", "{\"type\":\"minecraft:enchant_randomly\"}")},
                {"enchant_randomly sword", "chest", item("minecraft:iron_sword", "{\"type\":\"minecraft:enchant_randomly\"}")},
                {"enchant_with_levels", "chest", item("minecraft:diamond_chestplate", "{\"type\":\"minecraft:enchant_with_levels\",\"levels\":{\"type\":\"minecraft:uniform\",\"min\":5,\"max\":30}}")},
                {"enchant_with_levels book", "chest", item("minecraft:book", "{\"type\":\"minecraft:enchant_with_levels\",\"levels\":30}")},
                {"stack split", "chest", item("minecraft:ender_pearl", "{\"type\":\"minecraft:set_count\",\"count\":40}")},
                {"weighted pool", "chest", "{\"pools\":[{\"rolls\":{\"type\":\"minecraft:uniform\",\"min\":2,\"max\":6},\"bonus_rolls\":0,\"entries\":["
                    + "{\"type\":\"minecraft:item\",\"name\":\"minecraft:stick\",\"weight\":10},"
                    + "{\"type\":\"minecraft:item\",\"name\":\"minecraft:coal\",\"weight\":5,\"quality\":2},"
                    + "{\"type\":\"minecraft:empty\",\"weight\":3},"
                    + "{\"type\":\"minecraft:item\",\"name\":\"minecraft:diamond\",\"weight\":1}]}]}"},
                {"apply_bonus ore_drops", "fortune", item("minecraft:diamond", "{\"type\":\"minecraft:apply_bonus\",\"enchantment\":\"minecraft:fortune\",\"formula\":\"minecraft:ore_drops\"}")},
                {"apply_bonus uniform_bonus_count", "fortune", item("minecraft:glowstone_dust", "{\"type\":\"minecraft:apply_bonus\",\"enchantment\":\"minecraft:fortune\",\"formula\":\"minecraft:uniform_bonus_count\",\"parameters\":{\"bonusMultiplier\":1}}")},
                {"apply_bonus binomial_with_bonus_count", "fortune", item("minecraft:wheat_seeds", "{\"type\":\"minecraft:apply_bonus\",\"enchantment\":\"minecraft:fortune\",\"formula\":\"minecraft:binomial_with_bonus_count\",\"parameters\":{\"extra\":3,\"probability\":0.5714286}}")},
                {"explosion_decay", "exploded", item("minecraft:cobblestone", "{\"type\":\"minecraft:set_count\",\"count\":8}", "{\"type\":\"minecraft:explosion_decay\"}")},
            };
            for (String[] t : tables) {
                ContextKeySet set = t[1].equals("chest") ? LootContextParamSets.CHEST : LootContextParamSets.BLOCK;
                List<Value> values = switch (t[1]) {
                    case "chest" -> chest;
                    case "fortune" -> blockFortune;
                    default -> blockExploded;
                };
                for (long seed : SINGLE_SEEDS) {
                    cases.add(new Case(KIND_ROLL, t[0], SOURCE_INLINE, t[2], set, values, seed, 0));
                }
            }

            String[][] conditions = {
                {"random_chance", "chest", "{\"type\":\"minecraft:random_chance\",\"chance\":0.3}"},
                {"random_chance_with_enchanted_bonus without attacker", "chest", "{\"type\":\"minecraft:random_chance_with_enchanted_bonus\",\"enchantment\":\"minecraft:looting\",\"unenchanted_chance\":0.25,\"enchanted_chance\":{\"type\":\"minecraft:linear\",\"base\":0.3,\"per_level_above_first\":0.1}}"},
                {"table_bonus plain tool", "plain", "{\"type\":\"minecraft:table_bonus\",\"enchantment\":\"minecraft:fortune\",\"chances\":[0.05,0.0625,0.083333336,0.1]}"},
                {"table_bonus fortune 3", "fortune", "{\"type\":\"minecraft:table_bonus\",\"enchantment\":\"minecraft:fortune\",\"chances\":[0.05,0.0625,0.083333336,0.1]}"},
                {"survives_explosion", "exploded", "{\"type\":\"minecraft:survives_explosion\"}"},
                {"inverted random_chance", "chest", "{\"type\":\"minecraft:inverted\",\"term\":{\"type\":\"minecraft:random_chance\",\"chance\":0.7}}"},
                {"all_of random_chance", "chest", "{\"type\":\"minecraft:all_of\",\"terms\":[{\"type\":\"minecraft:random_chance\",\"chance\":0.5},{\"type\":\"minecraft:random_chance\",\"chance\":0.5}]}"},
                {"any_of random_chance", "chest", "{\"type\":\"minecraft:any_of\",\"terms\":[{\"type\":\"minecraft:random_chance\",\"chance\":0.2},{\"type\":\"minecraft:random_chance\",\"chance\":0.2}]}"},
            };
            for (String[] c : conditions) {
                ContextKeySet set = c[1].equals("chest") ? LootContextParamSets.CHEST : LootContextParamSets.BLOCK;
                List<Value> values = switch (c[1]) {
                    case "chest" -> chest;
                    case "plain" -> blockPlain;
                    case "fortune" -> blockFortune;
                    default -> blockExploded;
                };
                for (long seed : SINGLE_SEEDS) {
                    cases.add(new Case(KIND_CONDITION, c[0], SOURCE_INLINE, c[2], set, values, seed, SAMPLES));
                }
            }

            String[][] ints = {
                {"int constant", "7"},
                {"int uniform", "{\"type\":\"minecraft:uniform\",\"min\":-3,\"max\":12}"},
                {"int binomial", "{\"type\":\"minecraft:binomial\",\"n\":10,\"p\":0.3}"},
                {"int weighted_list", "{\"type\":\"minecraft:weighted_list\",\"distribution\":[{\"data\":1,\"weight\":65},{\"data\":0,\"weight\":35}]}"},
                {"int sum of uniforms", "{\"type\":\"minecraft:add\",\"inputs\":[{\"type\":\"minecraft:uniform\",\"min\":0,\"max\":3},{\"type\":\"minecraft:uniform\",\"min\":10,\"max\":20}]}"},
            };
            for (String[] n : ints) {
                for (long seed : SINGLE_SEEDS) {
                    cases.add(new Case(KIND_INT, n[0], SOURCE_INLINE, n[1], LootContextParamSets.CHEST, chest, seed, SAMPLES));
                }
            }
            String[][] floats = {
                {"float constant", "0.75"},
                {"float uniform", "{\"type\":\"minecraft:uniform\",\"min\":0.25,\"max\":4.5}"},
                {"float weighted_list", "{\"type\":\"minecraft:weighted_list\",\"distribution\":[{\"data\":0.5,\"weight\":3},{\"data\":2.0,\"weight\":1}]}"},
            };
            for (String[] n : floats) {
                for (long seed : SINGLE_SEEDS) {
                    cases.add(new Case(KIND_FLOAT, n[0], SOURCE_INLINE, n[1], LootContextParamSets.CHEST, chest, seed, SAMPLES));
                }
            }
        }

        private List<Value> blockValues(final String tool, final Float explosion) {
            List<Value> values = new ArrayList<>();
            values.add(new Value(LootContextParams.ORIGIN, ORIGIN));
            values.add(new Value(LootContextParams.BLOCK_STATE, BuiltInRegistries.BLOCK.getValue(Identifier.parse("minecraft:stone")).defaultBlockState()));
            values.add(new Value(LootContextParams.TOOL, stack(tool)));
            if (explosion != null) {
                values.add(new Value(LootContextParams.EXPLOSION_RADIUS, explosion));
            }
            return values;
        }

        private static String item(final String id, final String... modifiers) {
            StringBuilder entry = new StringBuilder("{\"type\":\"minecraft:item\",\"name\":\"").append(id).append('"');
            if (modifiers.length == 1) {
                entry.append(",\"modifier\":").append(modifiers[0]);
            } else if (modifiers.length > 1) {
                entry.append(",\"modifier\":[").append(String.join(",", modifiers)).append(']');
            }
            entry.append('}');
            return "{\"pools\":[{\"rolls\":1,\"entries\":[" + entry + "]}]}";
        }

        private ItemStack stack(final String json) {
            return decode(ItemStack.CODEC, json);
        }

        private <T> T decode(final Codec<T> codec, final String json) {
            return codec.parse(this.lookup.createSerializationContext(JsonOps.INSTANCE), JsonParser.parseString(json)).getOrThrow();
        }

        private void writeCase(final OutputStream out, final Case c) throws IOException {
            this.level.biome = this.lookup.lookupOrThrow(Registries.BIOME)
                .getOrThrow(ResourceKey.create(Registries.BIOME, Identifier.parse(c.biome())));
            LootParams params = params(c);
            out.write(c.kind());
            str(out, c.name());
            str(out, c.biome());
            out.write(c.source());
            str(out, c.sourceText());
            str(out, LootContextParamSets.CODEC.encodeStart(JsonOps.INSTANCE, c.paramSet()).getOrThrow().getAsString());
            f32(out, params.getLuck());
            i32(out, c.values().size());
            for (Value v : c.values()) {
                writeValue(out, v);
            }
            switch (c.kind()) {
                case KIND_ROLL -> roll(out, c, params);
                case KIND_FILL -> fill(out, c, params);
                default -> sample(out, c, params);
            }
        }

        private void roll(final OutputStream out, final Case c, final LootParams params) throws IOException {
            LootTable table = table(c);
            List<ItemStack> expected = table.getRandomItems(params, c.seed());
            RandomSource random = generator(c.seed());
            List<ItemStack> stacks = table.getRandomItems(params, random);
            if (!sameStacks(expected, stacks)) {
                throw new IllegalStateException(c.name() + " seed " + c.seed() + ": the seeded roll differs from the roll with the dumper's generator");
            }
            out.write(GENERATOR_LEGACY);
            i64(out, c.seed());
            i32(out, stacks.size());
            for (ItemStack s : stacks) {
                writeStack(out, s);
            }
            after(out, random);
        }

        private void fill(final OutputStream out, final Case c, final LootParams params) throws IOException {
            LootTable table = table(c);
            SimpleContainer expected = new SimpleContainer(c.size());
            table.fill(expected, params, c.seed());
            SimpleContainer filled = new SimpleContainer(c.size());
            armed = generator(c.seed());
            table.fill(filled, params, LootTable.RANDOMIZE_SEED);
            RandomSource random = armed;
            armed = null;
            if (!sameSlots(expected, filled)) {
                throw new IllegalStateException(c.name() + " seed " + c.seed() + ": the seeded fill differs from the fill with the dumper's generator");
            }
            out.write(GENERATOR_LEGACY);
            i64(out, c.seed());
            i32(out, c.size());
            int count = 0;
            for (int i = 0; i < c.size(); i++) {
                if (!filled.getItem(i).isEmpty()) {
                    count++;
                }
            }
            i32(out, count);
            for (int i = 0; i < c.size(); i++) {
                ItemStack s = filled.getItem(i);
                if (!s.isEmpty()) {
                    i32(out, i);
                    writeStack(out, s);
                }
            }
            after(out, random);
        }

        private void sample(final OutputStream out, final Case c, final LootParams params) throws IOException {
            RandomSource random = generator(c.seed());
            LootContext context = new LootContext.Builder(params).withOptionalRandomSource(random).create(Optional.empty());
            out.write(GENERATOR_LEGACY);
            i64(out, c.seed());
            i32(out, c.size());
            switch (c.kind()) {
                case KIND_CONDITION -> {
                    LootItemCondition condition = decode(LootItemCondition.DIRECT_CODEC, c.sourceText());
                    for (int i = 0; i < c.size(); i++) {
                        out.write(condition.test(context) ? 1 : 0);
                    }
                }
                case KIND_INT -> {
                    ContextIntProvider provider = decode(ContextIntProviders.DIRECT_CODEC, c.sourceText());
                    for (int i = 0; i < c.size(); i++) {
                        i32(out, provider.getInt(context));
                    }
                }
                default -> {
                    ContextFloatProvider provider = decode(ContextFloatProviders.DIRECT_CODEC, c.sourceText());
                    for (int i = 0; i < c.size(); i++) {
                        f32(out, provider.getFloat(context));
                    }
                }
            }
            after(out, random);
        }

        private LootTable table(final Case c) {
            if (c.source() == SOURCE_ID) {
                return this.holder.getLootTable(ResourceKey.create(Registries.LOOT_TABLE, Identifier.parse(c.sourceText())));
            }
            return decode(LootTable.DIRECT_CODEC, c.sourceText());
        }

        @SuppressWarnings("unchecked")
        private LootParams params(final Case c) {
            LootParams.Builder builder = new LootParams.Builder(this.level);
            for (Value v : c.values()) {
                builder.withParameter((ContextKey<Object>) v.key(), v.value());
            }
            return builder.create(c.paramSet());
        }

        private void writeValue(final OutputStream out, final Value v) throws IOException {
            str(out, v.key().name().toString());
            switch (v.value()) {
                case Vec3 p -> {
                    out.write(VALUE_VEC3);
                    f64(out, p.x);
                    f64(out, p.y);
                    f64(out, p.z);
                }
                case ItemStack s -> {
                    out.write(VALUE_STACK);
                    writeStack(out, s);
                }
                case BlockState s -> {
                    out.write(VALUE_BLOCK_STATE);
                    str(out, BlockStateParser.serialize(s));
                }
                case Float f -> {
                    out.write(VALUE_F32);
                    f32(out, f);
                }
                default -> throw new IllegalStateException("no encoding for " + v.value().getClass());
            }
        }

        private void writeStack(final OutputStream out, final ItemStack s) throws IOException {
            str(out, BuiltInRegistries.ITEM.getKey(s.getItem()).toString());
            i32(out, s.getCount());
            DataComponentPatch patch = s.getComponentsPatch();
            str(out, DataComponentPatch.CODEC.encodeStart(this.lookup.createSerializationContext(NbtOps.INSTANCE), patch).getOrThrow().toString());
        }

        private static void after(final OutputStream out, final RandomSource random) throws IOException {
            i64(out, random.nextLong());
            i64(out, random.nextLong());
        }

        private static RandomSource generator(final long seed) {
            return RandomSource.create(seed);
        }

        private static boolean sameStacks(final List<ItemStack> a, final List<ItemStack> b) {
            if (a.size() != b.size()) {
                return false;
            }
            for (int i = 0; i < a.size(); i++) {
                if (!ItemStack.matches(a.get(i), b.get(i))) {
                    return false;
                }
            }
            return true;
        }

        private static boolean sameSlots(final SimpleContainer a, final SimpleContainer b) {
            for (int i = 0; i < a.getContainerSize(); i++) {
                if (!ItemStack.matches(a.getItem(i), b.getItem(i))) {
                    return false;
                }
            }
            return true;
        }
    }

    private static RandomSource armedOrThrow(final String what) {
        if (armed == null) {
            throw new UnsupportedOperationException(what + " outside an armed fill");
        }
        return armed;
    }

    static final class LootServer extends DedicatedServer {
        private LayeredRegistryAccess<RegistryLayer> layers;
        private ReloadableServerRegistries.Holder holder;

        private LootServer(
            final Thread thread,
            final LevelStorageSource.LevelStorageAccess storage,
            final PackRepository packs,
            final WorldStem stem,
            final Optional<GameRules> rules,
            final DedicatedServerSettings settings,
            final DataFixer fixer,
            final Services services,
            final ManagementServer management,
            final NotificationManager notifications
        ) {
            super(thread, storage, packs, stem, rules, settings, fixer, services, management, notifications);
        }

        static LootServer allocate(final LayeredRegistryAccess<RegistryLayer> layers, final ReloadableServerRegistries.Holder holder) {
            LootServer server = Unsafe.allocate(LootServer.class);
            server.layers = layers;
            server.holder = holder;
            return server;
        }

        @Override
        public ReloadableServerRegistries.Holder reloadableRegistries() {
            return this.holder;
        }

        @Override
        public LayeredRegistryAccess<RegistryLayer> registries() {
            return this.layers;
        }

        @Override
        public RegistryAccess.Frozen registryAccess() {
            return this.layers.compositeAccess();
        }

        @Override
        public RandomSource getRandomSequence(final Identifier id) {
            return armedOrThrow("random sequence " + id);
        }
    }

    static final class LootLevel extends ServerLevel {
        private MinecraftServer server;
        private RegistryAccess access;
        private Holder<Biome> biome;

        private LootLevel(
            final MinecraftServer server,
            final Executor executor,
            final LevelStorageSource.LevelStorageAccess storage,
            final ServerLevelData levelData,
            final ResourceKey<Level> dimension,
            final LevelStem stem,
            final List<CustomSpawner> spawners
        ) {
            super(server, executor, storage, levelData, dimension, stem, spawners, false);
        }

        static LootLevel allocate(final MinecraftServer server, final RegistryAccess access) {
            LootLevel level = Unsafe.allocate(LootLevel.class);
            level.server = server;
            level.access = access;
            return level;
        }

        @Override
        public MinecraftServer getServer() {
            return this.server;
        }

        @Override
        public RegistryAccess registryAccess() {
            return this.access;
        }

        @Override
        public FeatureFlagSet enabledFeatures() {
            return FeatureFlags.DEFAULT_FLAGS;
        }

        @Override
        public RandomSource getRandom() {
            return armedOrThrow("level random");
        }

        @Override
        public StructureManager structureManager() {
            return NoStructures.INSTANCE;
        }

        @Override
        public boolean isLoaded(final net.minecraft.core.BlockPos pos) {
            return true;
        }

        @Override
        public Holder<Biome> getBiome(final int x, final int y, final int z) {
            return this.biome;
        }

        @Override
        public net.minecraft.core.BlockPos findNearestMapStructure(
            final TagKey<Structure> structures, final net.minecraft.core.BlockPos origin, final int radius, final boolean unexplored
        ) {
            return null;
        }

        @Override
        public net.minecraft.core.BlockPos findNearestMapStructure(
            final net.minecraft.core.HolderSet<Structure> structures, final net.minecraft.core.BlockPos origin, final int radius, final boolean unexplored
        ) {
            return null;
        }
    }

    static final class NoStructures extends StructureManager {
        static final NoStructures INSTANCE = Unsafe.allocate(NoStructures.class);

        private NoStructures(final LevelAccessor level, final WorldOptions options, final StructureCheck check) {
            super(level, options, check);
        }

        @Override
        public StructureStart getStructureAt(final net.minecraft.core.BlockPos pos, final Structure structure) {
            return null;
        }

        @Override
        public StructureStart getStructureAt(final net.minecraft.core.BlockPos pos, final net.minecraft.core.HolderSet<Structure> structures) {
            return null;
        }
    }

    private static final class Unsafe {
        static <T> T allocate(final Class<T> type) {
            try {
                java.lang.reflect.Field theUnsafe = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
                theUnsafe.setAccessible(true);
                sun.misc.Unsafe unsafe = (sun.misc.Unsafe) theUnsafe.get(null);
                return type.cast(unsafe.allocateInstance(type));
            } catch (ReflectiveOperationException e) {
                throw new IllegalStateException("cannot allocate " + type.getName(), e);
            }
        }
    }

    private static void i32(final OutputStream out, final int value) throws IOException {
        out.write(value);
        out.write(value >>> 8);
        out.write(value >>> 16);
        out.write(value >>> 24);
    }

    private static void i64(final OutputStream out, final long value) throws IOException {
        i32(out, (int) value);
        i32(out, (int) (value >>> 32));
    }

    private static void f32(final OutputStream out, final float value) throws IOException {
        i32(out, Float.floatToRawIntBits(value));
    }

    private static void f64(final OutputStream out, final double value) throws IOException {
        i64(out, Double.doubleToRawLongBits(value));
    }

    private static void str(final OutputStream out, final String value) throws IOException {
        byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
        i32(out, bytes.length);
        out.write(bytes);
    }
}
