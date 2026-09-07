package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Predicate;
import net.minecraft.SharedConstants;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.IdMap;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.StructureManager;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.PalettedContainer;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.Strategy;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.structure.Structure;
import net.minecraft.world.level.levelgen.structure.StructureStart;
import net.minecraft.world.level.levelgen.synth.NormalNoise;
import net.minecraft.world.level.block.Block;

/**
 * Dumps the block states vanilla writes into a chunk by the fill and the
 * material rules, before any carver runs, so the Rust surface stage can be
 * compared against them position by position.
 */
public final class SurfaceOracle {
    private static final byte[] MAGIC = "MCSURFC0".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;
    private static final int MIN_Y = -64;
    private static final int HEIGHT = 384;

    private record Column(long seed, int chunkX, int chunkZ) {}

    public static void main(final String[] args) throws IOException {
        if (SharedConstants.DEBUG_DISABLE_CARVERS != true) {
            throw new IllegalStateException(
                "run with -DMC_DEBUG_ENABLED -DMC_DEBUG_DISABLE_CARVERS so the dump is the pre-carve state"
            );
        }
        Path outDir = Path.of(args[0]);
        Files.createDirectories(outDir);

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        HolderLookup.Provider registries = VanillaRegistries.createWorldLookup();
        HolderGetter<NormalNoise> noises = registries.lookupOrThrow(Registries.NOISE);
        HolderLookup.RegistryLookup<Biome> biomes = registries.lookupOrThrow(Registries.BIOME);

        ResourceKey<NoiseGeneratorSettings> settingsKey = NoiseGeneratorSettings.OVERWORLD;
        Holder<NoiseGeneratorSettings> settings = registries.lookupOrThrow(Registries.NOISE_SETTINGS)
            .getOrThrow(settingsKey);
        Holder<MultiNoiseBiomeSourceParameterList> preset = registries
            .lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
            .getOrThrow(MultiNoiseBiomeSourceParameterLists.OVERWORLD);
        MultiNoiseBiomeSource biomeSource = MultiNoiseBiomeSource.createFromPreset(preset);
        NoiseBasedChunkGenerator generator = new NoiseBasedChunkGenerator(biomeSource, settings);
        PalettedContainerFactory containers = containerFactory(biomes);

        if (args.length > 1 && args[1].equals("search")) {
            for (long seed : new long[] {42L, 2L}) {
                RandomState randomState = RandomState.create(noises, seed, settings.value());
                net.minecraft.world.level.biome.BiomeResolver resolver = biomeSource.createUncachedResolver(
                    randomState
                );
                for (String target : new String[] {
                    "minecraft:eroded_badlands", "minecraft:frozen_ocean", "minecraft:deep_frozen_ocean"
                }) {
                    outer:
                    for (int radius = 0; radius < 400; radius++) {
                        for (int cx = -radius; cx <= radius; cx++) {
                            for (int cz = -radius; cz <= radius; cz++) {
                                if (Math.max(Math.abs(cx), Math.abs(cz)) != radius) {
                                    continue;
                                }
                                int hits = 0;
                                for (int qx = 0; qx < 4; qx++) {
                                    for (int qz = 0; qz < 4; qz++) {
                                        Holder<Biome> biome = resolver.getNoiseBiome(
                                            cx * 4 + qx, 16, cz * 4 + qz
                                        );
                                        if (biome.unwrapKey().orElseThrow().identifier().toString().equals(target)) {
                                            hits++;
                                        }
                                    }
                                }
                                if (hits == 16) {
                                    System.out.println("seed " + seed + " " + target + " chunk " + cx + "," + cz);
                                    break outer;
                                }
                            }
                        }
                    }
                }
            }
            return;
        }

        List<Column> columns = List.of(
            new Column(42L, 0, 0),
            new Column(42L, 100, 100),
            new Column(2L, 3, -7),
            new Column(42L, -118, -119),
            new Column(2L, 19, -1),
            new Column(2L, 52, 36),
            new Column(2L, 20, -3)
        );

        if (args.length > 1 && args[1].equals("ice")) {
            int[][] centres = {{2, 19, -1}, {2, 52, 36}, {42, 348, -26}, {42, 381, 74}};
            for (int[] centre : centres) {
                long seed = centre[0];
                RandomState randomState = RandomState.create(noises, seed, settings.value());
                for (int cx = centre[1] - 8; cx <= centre[1] + 8; cx++) {
                    for (int cz = centre[2] - 8; cz <= centre[2] + 8; cz++) {
                        ProtoChunk chunk = generate(
                            generator, biomeSource, containers, randomState, new Column(seed, cx, cz)
                        );
                        for (int y = 60; y < 110; y++) {
                            for (int x = 0; x < 16; x++) {
                                for (int z = 0; z < 16; z++) {
                                    BlockState state = chunk.getBlockState(
                                        new BlockPos(cx * 16 + x, y, cz * 16 + z)
                                    );
                                    if (state.is(Blocks.PACKED_ICE)) {
                                        System.out.println("seed " + seed + " iceberg chunk " + cx + "," + cz);
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return;
        }

        for (Column column : columns) {
            RandomState randomState = RandomState.create(noises, column.seed(), settings.value());
            ProtoChunk chunk = generate(generator, biomeSource, containers, randomState, column);

            Path file = outDir.resolve(
                "surface_s" + column.seed() + "_c" + column.chunkX() + "_" + column.chunkZ() + ".bin"
            );
            write(file, settingsKey.identifier().toString(), column, chunk);
            System.out.println("wrote " + file + " (" + Files.size(file) + " bytes)");
        }
    }

    private static ProtoChunk generate(
        final NoiseBasedChunkGenerator generator,
        final MultiNoiseBiomeSource biomeSource,
        final PalettedContainerFactory containers,
        final RandomState randomState,
        final Column column
    ) {
        ProtoChunk chunk = new ProtoChunk(
            new ChunkPos(column.chunkX(), column.chunkZ()),
            UpgradeData.EMPTY,
            LevelHeightAccessor.create(MIN_Y, HEIGHT),
            containers,
            null
        );
        generator.buildTerrain(
                chunk,
                Blender.empty(),
                randomState,
                noStructures(),
                new BiomeManager(
                    biomeSource.createUncachedResolver(randomState),
                    BiomeManager.obfuscateSeed(column.seed())
                ),
                null,
                biomeSource.possibleBiomes()
            )
            .join();
        return chunk;
    }

    private static StructureManager noStructures() {
        return new StructureManager(null, null, null) {
            @Override
            public List<StructureStart> startsForStructure(
                final int sectionX, final int sectionZ, final Predicate<Structure> matcher
            ) {
                return List.of();
            }
        };
    }

    /**
     * `PalettedContainerFactory.create` wants a whole `RegistryAccess`, which
     * only a level owns; the pieces it takes from it are an id map over the
     * biomes and the plains holder.
     */
    private static PalettedContainerFactory containerFactory(final HolderLookup.RegistryLookup<Biome> biomes) {
        List<Holder<Biome>> all = new ArrayList<>(biomes.listElements().map(h -> (Holder<Biome>)h).toList());
        Map<Holder<Biome>, Integer> ids = new IdentityHashMap<>();
        for (int i = 0; i < all.size(); i++) {
            ids.put(all.get(i), i);
        }
        IdMap<Holder<Biome>> idMap = new IdMap<>() {
            @Override
            public int getId(final Holder<Biome> thing) {
                Integer id = ids.get(thing);
                return id == null ? -1 : id;
            }

            @Override
            public Holder<Biome> byId(final int id) {
                return id >= 0 && id < all.size() ? all.get(id) : null;
            }

            @Override
            public int size() {
                return all.size();
            }

            @Override
            public java.util.Iterator<Holder<Biome>> iterator() {
                return all.iterator();
            }
        };
        BlockState air = Blocks.AIR.defaultBlockState();
        Strategy<BlockState> blockStates = Strategy.createForBlockStates(Block.BLOCK_STATE_REGISTRY);
        Holder<Biome> plains = biomes.getOrThrow(Biomes.PLAINS);
        return new PalettedContainerFactory(
            blockStates,
            air,
            PalettedContainer.codecRW(BlockState.CODEC, blockStates, air),
            Strategy.createForBiomes(idMap),
            plains,
            null
        );
    }

    private static void write(
        final Path file, final String settingsId, final Column column, final ChunkAccess chunk
    ) throws IOException {
        Map<String, Integer> paletteIds = new HashMap<>();
        List<String> palette = new ArrayList<>();
        List<int[]> runs = new ArrayList<>();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        int minBlockX = column.chunkX() * 16;
        int minBlockZ = column.chunkZ() * 16;

        for (int z = 0; z < 16; z++) {
            for (int x = 0; x < 16; x++) {
                for (int y = MIN_Y; y < MIN_Y + HEIGHT; y++) {
                    BlockState state = chunk.getBlockState(pos.set(minBlockX + x, y, minBlockZ + z));
                    String name = BlockStateParser.serialize(state);
                    int id = paletteIds.computeIfAbsent(name, key -> {
                        palette.add(key);
                        return palette.size() - 1;
                    });
                    if (!runs.isEmpty() && runs.getLast()[0] == id) {
                        runs.getLast()[1]++;
                    } else {
                        runs.add(new int[] {id, 1});
                    }
                }
            }
        }

        try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
            out.write(MAGIC);
            i32(out, FORMAT_VERSION);
            i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
            str(out, settingsId);
            i64(out, column.seed());
            i32(out, column.chunkX());
            i32(out, column.chunkZ());
            i32(out, MIN_Y);
            i32(out, HEIGHT);
            i32(out, palette.size());
            for (String name : palette) {
                str(out, name);
            }
            i32(out, runs.size());
            for (int[] run : runs) {
                i32(out, run[0]);
                i32(out, run[1]);
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
        i32(out, (int)value);
        i32(out, (int)(value >>> 32));
    }

    private static void str(final OutputStream out, final String value) throws IOException {
        byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
        i32(out, bytes.length);
        out.write(bytes);
    }
}
