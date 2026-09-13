package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.tags.TagLoader;
import net.minecraft.util.Util;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.biome.BiomeSource;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.biome.MultiNoiseBiomeSource;
import net.minecraft.world.level.biome.MultiNoiseBiomeSourceParameterLists;
import net.minecraft.world.level.biome.TheEndBiomeSource;
import net.minecraft.world.level.chunk.ChunkGeneratorStructureState;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.NoiseBasedChunkGenerator;
import net.minecraft.world.level.levelgen.NoiseGeneratorSettings;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.densityfunction.SamplerContext;
import net.minecraft.world.level.levelgen.structure.Structure;
import net.minecraft.world.level.levelgen.structure.StructureSet;
import net.minecraft.world.level.levelgen.structure.placement.ConcentricRingsStructurePlacement;
import net.minecraft.world.level.levelgen.structure.placement.RandomSpreadStructurePlacement;
import net.minecraft.world.level.levelgen.structure.placement.StructurePlacement;
import net.minecraft.world.level.levelgen.structure.structures.JigsawStructure;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager;
import net.minecraft.world.level.storage.LevelStorageSource;

public final class PlacementOracle {
    private static final byte[] CELLS_MAGIC = "MCPLACE0".getBytes(StandardCharsets.US_ASCII);
    private static final byte[] SITES_MAGIC = "MCSITES0".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;
    private static final long[] SEEDS = {1L, 42L, 12345L, -7L, 0x7FFF_FFFF_0000_0001L};
    private static final int[][] GRIDS = {{-24, -24, 49}, {2000, 2000, 25}};
    private static final int SITE_CASES = 16;
    private static final int SITE_MAX_RADIUS = 600;
    private static final int HEIGHT_PROBES = 64;

    private record Dim(
        String id,
        NoiseBasedChunkGenerator generator,
        BiomeSource biomeSource,
        NoiseGeneratorSettings settings,
        LevelHeightAccessor heights,
        int expectedSets
    ) {}

    private record SeedState(long seed, RandomState randomState, ChunkGeneratorStructureState state) {}

    public static void main(final String[] args) throws Exception {
        Path outDir = Path.of(args[0]);
        Files.createDirectories(outDir);

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        Path saveDir = Files.createTempDirectory("mcrs-oracle-saves");
        try (MultiPackResourceManager resources = new MultiPackResourceManager(
                PackType.SERVER_DATA, List.of(ServerPacksSource.createVanillaPackSource().fullResources())
            );
            LevelStorageSource.LevelStorageAccess storage = LevelStorageSource.createDefault(saveDir).createAccess("oracle")
        ) {
            RegistryAccess.Frozen loaded = loadWorldRegistries(resources);
            StructureTemplateManager templates = new StructureTemplateManager(
                resources, storage, DataFixers.getDataFixer(), BuiltInRegistries.BLOCK
            );

            HolderLookup.RegistryLookup<NoiseGeneratorSettings> noiseSettings = loaded.lookupOrThrow(Registries.NOISE_SETTINGS);
            var presets = loaded.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
            List<Dim> dims = List.of(
                dim("minecraft:overworld", noiseSettings.getOrThrow(NoiseGeneratorSettings.OVERWORLD),
                    MultiNoiseBiomeSource.createFromPreset(presets.getOrThrow(MultiNoiseBiomeSourceParameterLists.OVERWORLD)),
                    LevelHeightAccessor.create(-64, 384), 18),
                dim("minecraft:the_nether", noiseSettings.getOrThrow(NoiseGeneratorSettings.NETHER),
                    MultiNoiseBiomeSource.createFromPreset(presets.getOrThrow(MultiNoiseBiomeSourceParameterLists.NETHER)),
                    LevelHeightAccessor.create(0, 256), 3),
                dim("minecraft:the_end", noiseSettings.getOrThrow(NoiseGeneratorSettings.END),
                    TheEndBiomeSource.create(loaded.lookupOrThrow(Registries.BIOME)),
                    LevelHeightAccessor.create(0, 256), 1)
            );

            List<List<SeedState>> states = new ArrayList<>();
            for (Dim dim : dims) {
                List<SeedState> perSeed = new ArrayList<>();
                for (long seed : SEEDS) {
                    RandomState randomState = RandomState.create(loaded.lookupOrThrow(Registries.NOISE), seed, dim.settings());
                    ChunkGeneratorStructureState state = ChunkGeneratorStructureState.createForNormal(
                        randomState, seed, ChunkPos.ZERO, dim.biomeSource(), loaded.lookupOrThrow(Registries.STRUCTURE_SET)
                    );
                    perSeed.add(new SeedState(seed, randomState, state));
                }
                states.add(perSeed);
                List<String> sets = perSeed.get(0).state().possibleStructureSets().stream().map(PlacementOracle::id).toList();
                System.out.println(dim.id() + " live structure sets (" + sets.size() + "): " + sets);
                if (sets.size() != dim.expectedSets()) {
                    throw new IllegalStateException(dim.id() + ": expected " + dim.expectedSets() + " live sets, got " + sets.size());
                }
            }

            Path cells = outDir.resolve("structure_cells.bin");
            try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(cells))) {
                header(out, CELLS_MAGIC);
                Bin.i32(out, SEEDS.length);
                for (int s = 0; s < SEEDS.length; s++) {
                    Bin.i64(out, SEEDS[s]);
                    Bin.i32(out, dims.size());
                    for (int d = 0; d < dims.size(); d++) {
                        writeCells(out, dims.get(d), states.get(d).get(s));
                    }
                }
            }
            System.out.println("wrote " + cells + " (" + Files.size(cells) + " bytes)");

            Path sites = outDir.resolve("structure_sites.bin");
            try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(sites))) {
                header(out, SITES_MAGIC);
                Bin.i32(out, SEEDS.length);
                for (int s = 0; s < SEEDS.length; s++) {
                    Bin.i64(out, SEEDS[s]);
                    writeRings(out, states.get(0).get(s).state());
                    Bin.i32(out, 2);
                    for (int d = 0; d < 2; d++) {
                        writeSites(out, loaded, templates, dims.get(d), states.get(d).get(s));
                    }
                    Bin.i32(out, 2);
                    for (int d = 0; d < 2; d++) {
                        writeHeights(out, dims.get(d), states.get(d).get(s));
                    }
                }
            }
            System.out.println("wrote " + sites + " (" + Files.size(sites) + " bytes)");
        }
        System.exit(0);
    }

    private static RegistryAccess.Frozen loadWorldRegistries(final MultiPackResourceManager resources) {
        LayeredRegistryAccess<RegistryLayer> initialLayers = RegistryLayer.createRegistryAccess();
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

    private static Dim dim(
        final String id,
        final Holder<NoiseGeneratorSettings> settings,
        final BiomeSource biomeSource,
        final LevelHeightAccessor heights,
        final int expectedSets
    ) {
        return new Dim(id, new NoiseBasedChunkGenerator(biomeSource, settings), biomeSource, settings.value(), heights, expectedSets);
    }

    private static void header(final OutputStream out, final byte[] magic) throws IOException {
        out.write(magic);
        Bin.i32(out, FORMAT_VERSION);
        Bin.i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
    }

    private static String id(final Holder<?> holder) {
        return holder.unwrapKey().map(ResourceKey::identifier).orElseThrow().toString();
    }

    private static void writeCells(final OutputStream out, final Dim dim, final SeedState seed) throws IOException {
        Bin.str(out, dim.id());
        List<Holder<StructureSet>> spread = seed.state().possibleStructureSets().stream()
            .filter(set -> set.value().placement() instanceof RandomSpreadStructurePlacement)
            .toList();
        Bin.i32(out, spread.size());
        for (Holder<StructureSet> set : spread) {
            RandomSpreadStructurePlacement placement = (RandomSpreadStructurePlacement) set.value().placement();
            Bin.str(out, id(set));
            Bin.i32(out, placement.spacing());
            Bin.i32(out, placement.separation());
            Bin.i32(out, GRIDS.length);
            int hits = 0;
            for (int[] grid : GRIDS) {
                Bin.i32(out, grid[0]);
                Bin.i32(out, grid[1]);
                Bin.i32(out, grid[2]);
                for (int z = grid[1]; z < grid[1] + grid[2]; z++) {
                    for (int x = grid[0]; x < grid[0] + grid[2]; x++) {
                        ChunkPos potential = placement.getPotentialStructureChunk(seed.seed(), x, z);
                        boolean structureChunk = placement.isStructureChunk(seed.state(), x, z);
                        Bin.i32(out, potential.x());
                        Bin.i32(out, potential.z());
                        out.write(structureChunk ? 1 : 0);
                        if (structureChunk) {
                            hits++;
                        }
                    }
                }
            }
            System.out.println("  cells " + dim.id() + " seed " + seed.seed() + " " + id(set) + ": " + hits + " structure chunks");
        }
    }

    private static void writeRings(final OutputStream out, final ChunkGeneratorStructureState state) throws IOException {
        List<Holder<StructureSet>> rings = state.possibleStructureSets().stream()
            .filter(set -> set.value().placement() instanceof ConcentricRingsStructurePlacement)
            .toList();
        Bin.i32(out, rings.size());
        for (Holder<StructureSet> set : rings) {
            List<ChunkPos> positions = state.getRingPositionsFor((ConcentricRingsStructurePlacement) set.value().placement());
            Bin.str(out, id(set));
            Bin.i32(out, positions.size());
            for (ChunkPos pos : positions) {
                Bin.i32(out, pos.x());
                Bin.i32(out, pos.z());
            }
            System.out.println("  rings " + id(set) + ": " + positions.size() + " positions, first " + positions.get(0));
        }
    }

    private static void writeSites(
        final OutputStream out,
        final RegistryAccess.Frozen registries,
        final StructureTemplateManager templates,
        final Dim dim,
        final SeedState seed
    ) throws IOException {
        Bin.str(out, dim.id());
        ChunkGeneratorStructureState state = seed.state();
        List<Holder.Reference<Structure>> jigsaws = registries.lookupOrThrow(Registries.STRUCTURE).listElements()
            .filter(holder -> holder.value() instanceof JigsawStructure && !state.getPlacementsForStructure(holder).isEmpty())
            .toList();
        Bin.i32(out, jigsaws.size());
        Climate.Sampler climate = seed.randomState().createClimateSampler(SamplerContext.builder().enableCaches().build());
        for (Holder.Reference<Structure> holder : jigsaws) {
            JigsawStructure structure = (JigsawStructure) holder.value();
            List<StructurePlacement> placements = state.getPlacementsForStructure(holder);
            if (placements.size() != 1) {
                throw new IllegalStateException(id(holder) + " has " + placements.size() + " placements");
            }
            StructurePlacement placement = placements.get(0);
            Holder<StructureSet> set = state.possibleStructureSets().stream()
                .filter(candidate -> candidate.value().placement() == placement)
                .findFirst()
                .orElseThrow();
            Bin.str(out, id(holder));
            Bin.str(out, id(set));
            List<ChunkPos> cases = new ArrayList<>();
            search:
            for (int radius = 0; radius <= SITE_MAX_RADIUS; radius++) {
                for (int x = -radius; x <= radius; x++) {
                    for (int z = -radius; z <= radius; z++) {
                        if (Math.max(Math.abs(x), Math.abs(z)) != radius) {
                            continue;
                        }
                        if (placement.isStructureChunk(state, x, z)) {
                            cases.add(new ChunkPos(x, z));
                            if (cases.size() == SITE_CASES) {
                                break search;
                            }
                        }
                    }
                }
            }
            Bin.i32(out, cases.size());
            int present = 0;
            int valid = 0;
            for (ChunkPos chunk : cases) {
                Bin.i32(out, chunk.x());
                Bin.i32(out, chunk.z());
                Optional<Structure.GenerationStub> site = structure.findGenerationPoint(
                    context(registries, templates, dim, seed, climate, structure, chunk)
                );
                out.write(site.isPresent() ? 1 : 0);
                if (site.isPresent()) {
                    present++;
                    Bin.i32(out, site.get().position().getX());
                    Bin.i32(out, site.get().position().getY());
                    Bin.i32(out, site.get().position().getZ());
                    boolean biomeOk = structure.findValidGenerationPoint(
                        context(registries, templates, dim, seed, climate, structure, chunk)
                    ).isPresent();
                    out.write(biomeOk ? 1 : 0);
                    if (biomeOk) {
                        valid++;
                    }
                }
            }
            System.out.println(
                "  sites " + dim.id() + " seed " + seed.seed() + " " + id(holder) + ": " + cases.size() + " cases, "
                    + present + " present, " + valid + " biome ok"
            );
        }
    }

    private static Structure.GenerationContext context(
        final RegistryAccess.Frozen registries,
        final StructureTemplateManager templates,
        final Dim dim,
        final SeedState seed,
        final Climate.Sampler climate,
        final Structure structure,
        final ChunkPos chunk
    ) {
        return new Structure.GenerationContext(
            registries, dim.generator(), dim.biomeSource(), climate, seed.randomState(), templates,
            seed.seed(), chunk, dim.heights(), structure.biomes()::contains
        );
    }

    private static void writeHeights(final OutputStream out, final Dim dim, final SeedState seed) throws IOException {
        Bin.str(out, dim.id());
        Bin.i32(out, HEIGHT_PROBES);
        for (int i = 0; i < HEIGHT_PROBES; i++) {
            int x = i * 37 - 1000;
            int z = i * 53 - 700;
            Bin.i32(out, x);
            Bin.i32(out, z);
            Bin.i32(out, dim.generator().getBaseHeight(x, z, Heightmap.Types.WORLD_SURFACE_WG, dim.heights(), seed.randomState()));
            Bin.i32(out, dim.generator().getBaseHeight(x, z, Heightmap.Types.OCEAN_FLOOR_WG, dim.heights(), seed.randomState()));
        }
    }
}
