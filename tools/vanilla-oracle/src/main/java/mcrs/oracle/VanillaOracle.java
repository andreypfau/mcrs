package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.NoiseRouter;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.densityfunction.DensityBuffer;
import net.minecraft.world.level.levelgen.densityfunction.DensityFunction;
import net.minecraft.world.level.levelgen.densityfunction.DensitySamplerSet;
import net.minecraft.world.level.levelgen.densityfunction.DensityVolume;
import net.minecraft.world.level.levelgen.densityfunction.SamplerContext;
import net.minecraft.world.level.levelgen.synth.NormalNoise;

public final class VanillaOracle {
    private static final byte[] MAGIC = "MCDFORCL".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;

    private record Root(String name, DensityFunction function) {}

    private record Chunk(int x, int z) {}

    public static void main(final String[] args) throws IOException {
        Path outDir = Path.of(args[0]);
        Files.createDirectories(outDir);

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        HolderLookup.Provider registries = VanillaRegistries.createWorldLookup();
        HolderGetter<NormalNoise> noises = registries.lookupOrThrow(Registries.NOISE);

        ResourceKey<NoiseGeneratorSettings> settingsKey = NoiseGeneratorSettings.OVERWORLD;
        String settingsId = settingsKey.identifier().toString();
        NoiseGeneratorSettings settings = registries.lookupOrThrow(Registries.NOISE_SETTINGS)
            .getOrThrow(settingsKey)
            .value();
        NoiseRouter router = settings.noiseRouter();
        List<Root> roots = List.of(
            new Root("temperature", router.temperature()),
            new Root("vegetation", router.vegetation()),
            new Root("continents", router.continents()),
            new Root("erosion", router.erosion()),
            new Root("depth", router.depth()),
            new Root("ridges", router.ridges()),
            new Root("chunk_surface_level", router.chunkSurfaceLevel()),
            new Root("final_density", router.finalDensity())
        );

        List<Chunk> chunks = List.of(
            new Chunk(0, 0), new Chunk(10, -7), new Chunk(100, 100), new Chunk(-33, 55), new Chunk(7, 7)
        );

        for (long seed : new long[] {1L, 2L, 42L}) {
            RandomState randomState = RandomState.create(noises, seed, settings);
            for (Chunk chunk : chunks) {
                DensityVolume lattice = new DensityVolume(
                    5, 49, 5, chunk.x() * 16, -64, chunk.z() * 16, 4, 8, 4
                );
                Path file = outDir.resolve(
                    "overworld_s" + seed + "_c" + chunk.x() + "_" + chunk.z() + ".bin"
                );
                writeDump(file, settingsId, seed, chunk, randomState, roots, lattice);
                System.out.println("wrote " + file + " (" + Files.size(file) + " bytes)");
            }
        }

        long denseSeed = 42L;
        Chunk denseChunk = new Chunk(0, 0);
        RandomState denseState = RandomState.create(noises, denseSeed, settings);
        DensityVolume dense = new DensityVolume(16, 384, 16, denseChunk.x() * 16, -64, denseChunk.z() * 16, 1, 1, 1);
        Path denseFile = outDir.resolve(
            "overworld_s" + denseSeed + "_c" + denseChunk.x() + "_" + denseChunk.z() + "_dense.bin"
        );
        writeDump(
            denseFile,
            settingsId,
            denseSeed,
            denseChunk,
            denseState,
            List.of(new Root("final_density", router.finalDensity())),
            dense
        );
        System.out.println("wrote " + denseFile + " (" + Files.size(denseFile) + " bytes)");
    }

    private static void writeDump(
        final Path file,
        final String settingsId,
        final long seed,
        final Chunk chunk,
        final RandomState randomState,
        final List<Root> roots,
        final DensityVolume volume
    ) throws IOException {
        SamplerContext context = SamplerContext.builder().enableCaches().build();
        DensitySamplerSet samplers = randomState.samplersWithContext(context);
        DensityBuffer buffer = DensityBuffer.createUnpooled(volume.size());

        try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
            out.write(MAGIC);
            i32(out, FORMAT_VERSION);
            i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
            str(out, settingsId);
            i64(out, seed);
            i32(out, chunk.x());
            i32(out, chunk.z());
            i32(out, roots.size());

            for (Root root : roots) {
                samplers.get(root.function()).sampleVolume(buffer, volume);
                str(out, root.name());
                i32(out, volume.sizeX());
                i32(out, volume.sizeY());
                i32(out, volume.sizeZ());
                i32(out, volume.minBlockX());
                i32(out, volume.minBlockY());
                i32(out, volume.minBlockZ());
                i32(out, volume.stepBlockX());
                i32(out, volume.stepBlockY());
                i32(out, volume.stepBlockZ());
                i32(out, volume.size());
                for (int i = 0; i < volume.size(); i++) {
                    i32(out, Float.floatToRawIntBits(buffer.get(i)));
                }
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
