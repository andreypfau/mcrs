package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import com.google.gson.JsonElement;
import com.mojang.serialization.DynamicOps;
import com.mojang.serialization.JsonOps;
import net.minecraft.SharedConstants;
import net.minecraft.resources.RegistryOps;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.dimension.BuiltinDimensionTypes;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.feature.Feature;
import net.minecraft.world.level.levelgen.feature.TreeFeature;

/**
 * Runs every shipped `tree` feature over a flat floor and records the blocks it
 * writes plus the random state it leaves behind.
 *
 * The level is `StubLevel`, whose unimplemented methods throw: a feature that
 * needs one it does not have is skipped and named, which is how the needed
 * subset is discovered.
 */
public final class TreeOracle {
    private static final byte[] MAGIC = "MCTREEG0".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;
    private static final int FLOOR_TOP = 63;
    private static final long[] SEEDS = {42L, 1L, 7L, 12345L};

    private record Case(String featureId, long seed, BlockPos origin) {}

    private record Result(boolean placed, long rngLo, long rngHi, List<String> palette, List<int[]> blocks) {}

    public static void main(final String[] args) throws IOException {
        Path outDir = Path.of(args[0]);
        Files.createDirectories(outDir);

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        bindTags();
        HolderLookup.Provider registries = VanillaRegistries.createWorldLookup();
        DimensionType overworld = registries.lookupOrThrow(Registries.DIMENSION_TYPE)
            .getOrThrow(BuiltinDimensionTypes.OVERWORLD)
            .value();
        RegistryAccess access = RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);

        DynamicOps<JsonElement> ops = RegistryOps.create(JsonOps.INSTANCE, registries);
        Map<String, Feature> trees = new java.util.TreeMap<>();
        registries.lookupOrThrow(Registries.FEATURE).listElements().forEach(entry -> {
            if (entry.value() instanceof TreeFeature) {
                trees.put(entry.key().identifier().toString(), throughDataPack(ops, entry.value()));
            }
        });
        System.out.println("tree features in registry: " + trees.size());

        List<Case> cases = new ArrayList<>();
        List<Result> results = new ArrayList<>();
        Map<String, String> skipped = new java.util.TreeMap<>();
        for (Map.Entry<String, Feature> entry : trees.entrySet()) {
            for (long seed : SEEDS) {
                BlockPos origin = new BlockPos(0, FLOOR_TOP + 1, 0);
                try {
                    Result result = run(access, overworld, entry.getKey(), entry.getValue(), seed, origin);
                    cases.add(new Case(entry.getKey(), seed, origin));
                    results.add(result);
                } catch (UnsupportedOperationException e) {
                    skipped.put(entry.getKey(), e.getMessage());
                    break;
                }
            }
        }

        Path file = outDir.resolve("tree_geometry.bin");
        try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
            out.write(MAGIC);
            Bin.i32(out, FORMAT_VERSION);
            Bin.i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
            Bin.i32(out, cases.size());
            for (int i = 0; i < cases.size(); i++) {
                write(out, cases.get(i), results.get(i));
            }
        }
        for (Map.Entry<String, String> skip : skipped.entrySet()) {
            System.out.println("skipped " + skip.getKey() + ": needs " + skip.getValue());
        }
        System.out.println("wrote " + file + " (" + Files.size(file) + " bytes, " + cases.size() + " cases)");
    }

    /**
     * The feature as a data pack carries it, not as `TreeFeatures` builds it.
     * The two differ: `CherryFoliagePlacer.CODEC` reads `corner_hole_chance`
     * into the field but writes `wide_bottom_layer_hole_chance` back out, so
     * the shipped `cherry.json` holds 0.25 where the bootstrap object holds
     * 0.5. A server — vanilla or ours — loads the JSON, so the JSON is what a
     * port must reproduce.
     */
    private static Feature throughDataPack(final DynamicOps<JsonElement> ops, final Feature feature) {
        JsonElement json = Feature.DIRECT_CODEC.encodeStart(ops, feature).getOrThrow();
        return Feature.DIRECT_CODEC.parse(ops, json).getOrThrow();
    }

    private static void bindTags() {
        try (MultiPackResourceManager manager = new MultiPackResourceManager(
            PackType.SERVER_DATA, List.of(ServerPacksSource.createVanillaPackSource().fullResources())
        )) {
            TagLoader.loadTagsForExistingRegistries(
                manager, RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY)
            ).forEach(pending -> pending.apply());
        }
    }

    private static Result run(
        final RegistryAccess access,
        final DimensionType dimensionType,
        final String featureId,
        final Feature feature,
        final long seed,
        final BlockPos origin
    ) {
        RandomSource random = new WorldgenRandom(new XoroshiroRandomSource(seed));
        StubLevel level = new StubLevel(
            access,
            dimensionType,
            Blocks.DIRT.defaultBlockState(),
            Blocks.AIR.defaultBlockState(),
            FLOOR_TOP,
            new XoroshiroRandomSource(0L)
        );
        boolean placed = feature.place(level, null, random, origin);
        long rngLo = random.nextLong();
        long rngHi = random.nextLong();

        Map<String, Integer> ids = new HashMap<>();
        List<String> palette = new ArrayList<>();
        List<int[]> blocks = new ArrayList<>();
        for (Map.Entry<BlockPos, BlockState> written : level.written().entrySet()) {
            String name = BlockStateParser.serialize(written.getValue());
            int id = ids.computeIfAbsent(name, key -> {
                palette.add(key);
                return palette.size() - 1;
            });
            BlockPos pos = written.getKey();
            blocks.add(new int[] {pos.getX(), pos.getY(), pos.getZ(), id});
        }
        return new Result(placed, rngLo, rngHi, palette, blocks);
    }

    private static void write(final OutputStream out, final Case treeCase, final Result result) throws IOException {
        Bin.str(out, treeCase.featureId());
        Bin.i64(out, treeCase.seed());
        Bin.i32(out, treeCase.origin().getX());
        Bin.i32(out, treeCase.origin().getY());
        Bin.i32(out, treeCase.origin().getZ());
        Bin.i32(out, result.placed() ? 1 : 0);
        Bin.i64(out, result.rngLo());
        Bin.i64(out, result.rngHi());
        Bin.i32(out, result.palette().size());
        for (String name : result.palette()) {
            Bin.str(out, name);
        }
        Bin.i32(out, result.blocks().size());
        for (int[] block : result.blocks()) {
            Bin.i32(out, block[0]);
            Bin.i32(out, block[1]);
            Bin.i32(out, block[2]);
            Bin.i32(out, block[3]);
        }
        System.out.println(
            treeCase.featureId() + "@" + treeCase.seed() + ": placed=" + result.placed()
                + " blocks=" + result.blocks().size() + " state=" + result.rngLo() + "," + result.rngHi()
        );
    }
}
