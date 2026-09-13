package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.HolderSet;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeSource;
import net.minecraft.world.level.biome.FeatureSorter;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterList;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;
import net.minecraft.world.level.biome.TheEndBiomeSource;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;

/**
 * Dumps the per-step placed-feature order that `ChunkGenerator` derives from a
 * biome source, plus which of those features each biome contributes.
 */
public final class FeatureStepOracle {
    private static final byte[] MAGIC = "MCFSTEP0".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;

    private record Source(String id, BiomeSource biomeSource) {}

    public static void main(final String[] args) throws IOException {
        Path outDir = Path.of(args[0]);
        Files.createDirectories(outDir);

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        HolderLookup.Provider registries = VanillaRegistries.createWorldLookup();
        HolderGetter<Biome> biomes = registries.lookupOrThrow(Registries.BIOME);

        Map<PlacedFeature, String> featureIds = new IdentityHashMap<>();
        registries.lookupOrThrow(Registries.PLACED_FEATURE)
            .listElements()
            .forEach(entry -> featureIds.put(entry.value(), entry.key().identifier().toString()));

        List<Source> sources = List.of(
            new Source("minecraft:overworld", MultiNoiseBiomeSource.createFromPreset(
                preset(registries, MultiNoiseBiomeSourceParameterLists.OVERWORLD)
            )),
            new Source("minecraft:the_nether", MultiNoiseBiomeSource.createFromPreset(
                preset(registries, MultiNoiseBiomeSourceParameterLists.NETHER)
            )),
            new Source("minecraft:the_end", TheEndBiomeSource.create(biomes))
        );

        Path file = outDir.resolve("feature_steps.bin");
        try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
            out.write(MAGIC);
            Bin.i32(out, FORMAT_VERSION);
            Bin.i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
            Bin.i32(out, sources.size());
            for (Source source : sources) {
                writeSource(out, source, featureIds);
            }
        }
        System.out.println("wrote " + file + " (" + Files.size(file) + " bytes)");
    }

    private static Holder<MultiNoiseBiomeSourceParameterList> preset(
        final HolderLookup.Provider registries,
        final net.minecraft.resources.ResourceKey<MultiNoiseBiomeSourceParameterList> key
    ) {
        return registries.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST).getOrThrow(key);
    }

    private static void writeSource(
        final OutputStream out, final Source source, final Map<PlacedFeature, String> featureIds
    ) throws IOException {
        List<Holder<Biome>> possibleBiomes = List.copyOf(source.biomeSource().possibleBiomes());
        List<FeatureSorter.StepFeatureData> steps = FeatureSorter.buildFeaturesPerStep(
            possibleBiomes, biome -> biome.value().getGenerationSettings().features(), true
        );

        Bin.str(out, source.id());
        Bin.i32(out, possibleBiomes.size());
        for (Holder<Biome> biome : possibleBiomes) {
            Bin.str(out, biome.unwrapKey().orElseThrow().identifier().toString());
        }
        Bin.i32(out, steps.size());
        for (FeatureSorter.StepFeatureData step : steps) {
            Bin.i32(out, step.features().size());
            for (PlacedFeature feature : step.features()) {
                Bin.str(out, id(featureIds, feature));
            }
        }

        for (Holder<Biome> biome : possibleBiomes) {
            List<HolderSet<PlacedFeature>> featuresInBiome = biome.value().getGenerationSettings().features();
            for (int stepIndex = 0; stepIndex < steps.size(); stepIndex++) {
                FeatureSorter.StepFeatureData step = steps.get(stepIndex);
                byte[] bits = new byte[(step.features().size() + 7) / 8];
                if (stepIndex < featuresInBiome.size()) {
                    for (Holder<PlacedFeature> holder : featuresInBiome.get(stepIndex)) {
                        int index = step.indexMapping().applyAsInt(holder.value());
                        if (index < 0) {
                            throw new IllegalStateException(
                                "feature " + id(featureIds, holder.value()) + " missing from step " + stepIndex
                            );
                        }
                        bits[index >> 3] |= (byte)(1 << (index & 7));
                    }
                }
                Bin.i32(out, bits.length);
                out.write(bits);
            }
        }
    }

    private static String id(final Map<PlacedFeature, String> featureIds, final PlacedFeature feature) {
        String id = featureIds.get(feature);
        if (id == null) {
            throw new IllegalStateException("placed feature is not a registry entry: " + feature);
        }
        return id;
    }
}
