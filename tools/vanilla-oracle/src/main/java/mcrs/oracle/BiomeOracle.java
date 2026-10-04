package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.QuartPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;
import net.minecraft.world.level.biome.NoiseBiomeResolver;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public final class BiomeOracle {
    private static final byte[] MAGIC = "MCBIOME0".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;
    private static final int MIN_Y = -64;
    private static final int HEIGHT = 384;
    private static final int BORDER_COLUMNS = 2;
    private static final int BORDER_SEED = 42;
    private static final int BORDER_QUART_Y = 16;
    private static final int BORDER_SEARCH_RADIUS = 400;

    private record Column(long seed, int chunkX, int chunkZ) {}

    public static void main(final String[] args) throws IOException {
        Path outDir = Path.of(args[0]);
        Files.createDirectories(outDir);

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        HolderLookup.Provider registries = VanillaRegistries.createWorldLookup();
        HolderGetter<NormalNoise> noises = registries.lookupOrThrow(Registries.NOISE);
        HolderLookup.RegistryLookup<Biome> biomes = registries.lookupOrThrow(Registries.BIOME);

        Holder<NoiseGeneratorSettings> settings = registries.lookupOrThrow(Registries.NOISE_SETTINGS)
            .getOrThrow(NoiseGeneratorSettings.OVERWORLD);
        Holder<MultiNoiseBiomeSourceParameterList> preset = registries
            .lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST)
            .getOrThrow(MultiNoiseBiomeSourceParameterLists.OVERWORLD);
        MultiNoiseBiomeSource biomeSource = MultiNoiseBiomeSource.createFromPreset(preset);
        NoiseBasedChunkGenerator generator = new NoiseBasedChunkGenerator(biomeSource, settings);
        PalettedContainerFactory containers = SurfaceOracle.containerFactory(biomes);

        List<Column> columns = new ArrayList<>(
            List.of(
                new Column(42L, 0, 0),
                new Column(42L, 100, 100),
                new Column(2L, 3, -7),
                new Column(42L, -118, -119),
                new Column(2L, 19, -1),
                new Column(2L, 52, 36),
                new Column(2L, 20, -3)
            )
        );
        RandomState borderState = RandomState.create(noises, BORDER_SEED, settings.value());
        for (Column border : findBorders(biomeSource, borderState, columns)) {
            System.out.println("border chunk " + border.chunkX() + "," + border.chunkZ() + " at seed " + border.seed());
            columns.add(border);
        }

        Path file = outDir.resolve("biome_containers.bin");
        try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
            out.write(MAGIC);
            Bin.i32(out, FORMAT_VERSION);
            Bin.i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
            Bin.i32(out, columns.size());
            for (Column column : columns) {
                RandomState randomState = RandomState.create(noises, column.seed(), settings.value());
                ProtoChunk chunk = fillBiomes(generator, containers, randomState, column.chunkX(), column.chunkZ());
                write(out, column, chunk);
            }
        }
        System.out.println("wrote " + file + " (" + Files.size(file) + " bytes)");
    }

    static ProtoChunk fillBiomes(
        final NoiseBasedChunkGenerator generator,
        final PalettedContainerFactory containers,
        final RandomState randomState,
        final int chunkX,
        final int chunkZ
    ) {
        ProtoChunk[][] neighbourhood = new ProtoChunk[3][3];
        for (int dx = 0; dx < 3; dx++) {
            for (int dz = 0; dz < 3; dz++) {
                ProtoChunk chunk = new ProtoChunk(
                    new ChunkPos(chunkX + dx - 1, chunkZ + dz - 1),
                    UpgradeData.EMPTY,
                    LevelHeightAccessor.create(MIN_Y, HEIGHT),
                    containers,
                    null
                );
                chunk.setPersistedStatus(ChunkStatus.BIOMES);
                generator.createNoiseBiomes(randomState, Blender.empty(), chunk, containers).join();
                neighbourhood[dx][dz] = chunk;
            }
        }

        NoiseBiomeResolver resolver = (quartX, quartY, quartZ) -> {
            int dx = QuartPos.toSection(quartX) - chunkX + 1;
            int dz = QuartPos.toSection(quartZ) - chunkZ + 1;
            return neighbourhood[dx][dz].getNoiseBiomeChunk().getNoiseBiome(quartX, quartY, quartZ);
        };
        ProtoChunk centre = neighbourhood[1][1];
        generator.createBiomes(resolver, randomState, centre).join();
        return centre;
    }

    private static List<Column> findBorders(
        final MultiNoiseBiomeSource biomeSource, final RandomState randomState, final List<Column> taken
    ) {
        NoiseBiomeResolver resolver = biomeSource.createUncachedResolver(randomState);
        List<Column> found = new ArrayList<>();
        for (int radius = 0; radius < BORDER_SEARCH_RADIUS && found.size() < BORDER_COLUMNS; radius++) {
            for (int chunkX = -radius; chunkX <= radius && found.size() < BORDER_COLUMNS; chunkX++) {
                for (int chunkZ = -radius; chunkZ <= radius && found.size() < BORDER_COLUMNS; chunkZ++) {
                    if (Math.max(Math.abs(chunkX), Math.abs(chunkZ)) != radius) {
                        continue;
                    }
                    Column column = new Column(BORDER_SEED, chunkX, chunkZ);
                    if (taken.contains(column)) {
                        continue;
                    }
                    Set<Holder<Biome>> distinct = new HashSet<>();
                    for (int quartX = 0; quartX < 4; quartX++) {
                        for (int quartZ = 0; quartZ < 4; quartZ++) {
                            distinct.add(
                                resolver.getNoiseBiome(chunkX * 4 + quartX, BORDER_QUART_Y, chunkZ * 4 + quartZ)
                            );
                        }
                    }
                    if (distinct.size() > 1) {
                        found.add(column);
                    }
                }
            }
        }
        if (found.size() < BORDER_COLUMNS) {
            throw new IllegalStateException("found " + found.size() + " border columns within the search radius");
        }
        return found;
    }

    private static void write(final OutputStream out, final Column column, final ProtoChunk chunk) throws IOException {
        Map<String, Integer> palette = new LinkedHashMap<>();
        List<List<int[]>> sections = new ArrayList<>();
        for (int index = 0; index < chunk.getSectionsCount(); index++) {
            List<int[]> runs = new ArrayList<>();
            for (int y = 0; y < 16; y++) {
                for (int z = 0; z < 16; z++) {
                    for (int x = 0; x < 16; x++) {
                        String name = chunk.getSection(index)
                            .getBiome(x, y, z)
                            .unwrapKey()
                            .orElseThrow()
                            .identifier()
                            .toString();
                        int id = palette.computeIfAbsent(name, key -> palette.size());
                        if (!runs.isEmpty() && runs.getLast()[0] == id) {
                            runs.getLast()[1]++;
                        } else {
                            runs.add(new int[] {id, 1});
                        }
                    }
                }
            }
            sections.add(runs);
        }

        Bin.i64(out, column.seed());
        Bin.i32(out, column.chunkX());
        Bin.i32(out, column.chunkZ());
        Bin.i32(out, chunk.getMinSectionY());
        Bin.i32(out, chunk.getSectionsCount());
        Bin.i32(out, palette.size());
        for (String name : palette.keySet()) {
            Bin.str(out, name);
        }
        for (List<int[]> runs : sections) {
            Bin.i32(out, runs.size());
            for (int[] run : runs) {
                Bin.i32(out, run[0]);
                Bin.i32(out, run[1]);
            }
        }
    }
}
