package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.lang.reflect.Method;
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
import net.minecraft.world.level.Level;
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
import net.minecraft.world.level.levelgen.structure.StructureStart;
import net.minecraft.world.level.levelgen.structure.structures.JigsawStructure;
import net.minecraft.world.level.levelgen.structure.structures.MineshaftStructure;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager;
import net.minecraft.world.level.storage.LevelStorageSource;

public final class PlacementOracle {
    private static final byte[] CELLS_MAGIC = "MCPLACE0".getBytes(StandardCharsets.US_ASCII);
    private static final byte[] SITES_MAGIC = "MCSITES1".getBytes(StandardCharsets.US_ASCII);
    static final int FORMAT_VERSION = 1;
    static final long[] SEEDS = {1L, 42L, 12345L, -7L, 0x7FFF_FFFF_0000_0001L};
    private static final int[][] GRIDS = {{-24, -24, 49}, {2000, 2000, 25}};
    private static final int SITE_CASES = 16;
    private static final int SITE_MAX_RADIUS = 600;
    private static final int HEIGHT_PROBES = 64;

    record Dim(
        String id,
        ResourceKey<Level> level,
        NoiseBasedChunkGenerator generator,
        BiomeSource biomeSource,
        NoiseGeneratorSettings settings,
        LevelHeightAccessor heights,
        int expectedSets
    ) {}

    record SeedState(long seed, RandomState randomState, ChunkGeneratorStructureState state) {}

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
            RegistryAccess.Frozen access = RegistryLayer.createRegistryAccess()
                .replaceFrom(RegistryLayer.WORLD, loaded)
                .compositeAccess();
            StructureTemplateManager templates = new StructureTemplateManager(
                resources, storage, DataFixers.getDataFixer(), BuiltInRegistries.BLOCK
            );

            List<Dim> dims = dims(loaded);

            List<List<SeedState>> states = new ArrayList<>();
            for (Dim dim : dims) {
                List<SeedState> perSeed = new ArrayList<>();
                for (long seed : SEEDS) {
                    perSeed.add(seedState(loaded, dim, seed));
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
                    Bin.i32(out, 2);
                    for (int d = 0; d < 2; d++) {
                        writeSites(out, loaded, templates, dims.get(d), states.get(d).get(s), hardcoded(loaded, states.get(d).get(s).state()));
                    }
                    Bin.i32(out, 2);
                    for (int d = 0; d < 2; d++) {
                        writeSelection(out, access, templates, dims.get(d), states.get(d).get(s));
                    }
                }
            }
            System.out.println("wrote " + sites + " (" + Files.size(sites) + " bytes)");
        }
        System.exit(0);
    }

    static RegistryAccess.Frozen loadWorldRegistries(final MultiPackResourceManager resources) {
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

    static List<Dim> dims(final RegistryAccess.Frozen loaded) {
        HolderLookup.RegistryLookup<NoiseGeneratorSettings> noiseSettings = loaded.lookupOrThrow(Registries.NOISE_SETTINGS);
        var presets = loaded.lookupOrThrow(Registries.MULTI_NOISE_BIOME_SOURCE_PARAMETER_LIST);
        return List.of(
            dim("minecraft:overworld", Level.OVERWORLD, noiseSettings.getOrThrow(NoiseGeneratorSettings.OVERWORLD),
                MultiNoiseBiomeSource.createFromPreset(presets.getOrThrow(MultiNoiseBiomeSourceParameterLists.OVERWORLD)),
                LevelHeightAccessor.create(-64, 384), 18),
            dim("minecraft:the_nether", Level.NETHER, noiseSettings.getOrThrow(NoiseGeneratorSettings.NETHER),
                MultiNoiseBiomeSource.createFromPreset(presets.getOrThrow(MultiNoiseBiomeSourceParameterLists.NETHER)),
                LevelHeightAccessor.create(0, 256), 3),
            dim("minecraft:the_end", Level.END, noiseSettings.getOrThrow(NoiseGeneratorSettings.END),
                TheEndBiomeSource.create(loaded.lookupOrThrow(Registries.BIOME)),
                LevelHeightAccessor.create(0, 256), 1)
        );
    }

    private static Dim dim(
        final String id,
        final ResourceKey<Level> level,
        final Holder<NoiseGeneratorSettings> settings,
        final BiomeSource biomeSource,
        final LevelHeightAccessor heights,
        final int expectedSets
    ) {
        return new Dim(id, level, new NoiseBasedChunkGenerator(biomeSource, settings), biomeSource, settings.value(), heights, expectedSets);
    }

    static SeedState seedState(final RegistryAccess.Frozen loaded, final Dim dim, final long seed) {
        RandomState randomState = RandomState.create(loaded.lookupOrThrow(Registries.NOISE), seed, dim.settings());
        ChunkGeneratorStructureState state = ChunkGeneratorStructureState.createForNormal(
            randomState, seed, ChunkPos.ZERO, dim.biomeSource(), loaded.lookupOrThrow(Registries.STRUCTURE_SET)
        );
        return new SeedState(seed, randomState, state);
    }

    static Climate.Sampler climate(final SeedState seed) {
        return seed.randomState().createClimateSampler(SamplerContext.builder().enableCaches().build());
    }

    static List<Holder.Reference<Structure>> jigsaws(final RegistryAccess.Frozen registries, final ChunkGeneratorStructureState state) {
        return registries.lookupOrThrow(Registries.STRUCTURE).listElements()
            .filter(holder -> holder.value() instanceof JigsawStructure && !state.getPlacementsForStructure(holder).isEmpty())
            .toList();
    }

    /// Every type with its own `findGenerationPoint` except the mineshaft, whose
    /// site builds its whole piece tree first.
    static List<Holder.Reference<Structure>> hardcoded(final RegistryAccess.Frozen registries, final ChunkGeneratorStructureState state) {
        return registries.lookupOrThrow(Registries.STRUCTURE).listElements()
            .filter(holder -> !(holder.value() instanceof JigsawStructure) && !(holder.value() instanceof MineshaftStructure))
            .filter(holder -> !state.getPlacementsForStructure(holder).isEmpty())
            .toList();
    }

    static Holder<StructureSet> singleSet(final ChunkGeneratorStructureState state, final Holder<Structure> holder) {
        List<StructurePlacement> placements = state.getPlacementsForStructure(holder);
        if (placements.size() != 1) {
            throw new IllegalStateException(id(holder) + " has " + placements.size() + " placements");
        }
        return state.possibleStructureSets().stream()
            .filter(candidate -> candidate.value().placement() == placements.get(0))
            .findFirst()
            .orElseThrow();
    }

    static List<ChunkPos> caseChunks(final ChunkGeneratorStructureState state, final StructurePlacement placement) {
        List<ChunkPos> cases = new ArrayList<>();
        for (int radius = 0; radius <= SITE_MAX_RADIUS; radius++) {
            for (int x = -radius; x <= radius; x++) {
                for (int z = -radius; z <= radius; z++) {
                    if (Math.max(Math.abs(x), Math.abs(z)) != radius) {
                        continue;
                    }
                    if (placement.isStructureChunk(state, x, z)) {
                        cases.add(new ChunkPos(x, z));
                        if (cases.size() == SITE_CASES) {
                            return cases;
                        }
                    }
                }
            }
        }
        return cases;
    }

    static void header(final OutputStream out, final byte[] magic) throws IOException {
        out.write(magic);
        Bin.i32(out, FORMAT_VERSION);
        Bin.i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
    }

    static String id(final Holder<?> holder) {
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
        writeSites(out, registries, templates, dim, seed, jigsaws(registries, seed.state()));
    }

    private static void writeSites(
        final OutputStream out,
        final RegistryAccess.Frozen registries,
        final StructureTemplateManager templates,
        final Dim dim,
        final SeedState seed,
        final List<Holder.Reference<Structure>> structures
    ) throws IOException {
        Bin.str(out, dim.id());
        ChunkGeneratorStructureState state = seed.state();
        Bin.i32(out, structures.size());
        Climate.Sampler climate = climate(seed);
        for (Holder.Reference<Structure> holder : structures) {
            Structure structure = holder.value();
            Holder<StructureSet> set = singleSet(state, holder);
            Bin.str(out, id(holder));
            Bin.str(out, id(set));
            List<ChunkPos> cases = caseChunks(state, set.value().placement());
            Bin.i32(out, cases.size());
            int present = 0;
            int valid = 0;
            for (ChunkPos chunk : cases) {
                Bin.i32(out, chunk.x());
                Bin.i32(out, chunk.z());
                Optional<Structure.GenerationStub> site = findGenerationPoint(
                    structure, context(registries, templates, dim, seed, climate, structure, chunk)
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

    private static void writeSelection(
        final OutputStream out,
        final RegistryAccess.Frozen registries,
        final StructureTemplateManager templates,
        final Dim dim,
        final SeedState seed
    ) throws IOException {
        Bin.str(out, dim.id());
        ChunkGeneratorStructureState state = seed.state();
        List<Holder<StructureSet>> sets = state.possibleStructureSets();
        Bin.i32(out, sets.size());
        Climate.Sampler climate = climate(seed);
        for (Holder<StructureSet> set : sets) {
            Bin.str(out, id(set));
            List<ChunkPos> cases = caseChunks(state, set.value().placement());
            Bin.i32(out, cases.size());
            int selected = 0;
            for (ChunkPos chunk : cases) {
                Bin.i32(out, chunk.x());
                Bin.i32(out, chunk.z());
                Optional<String> choice = selected(registries, templates, dim, seed, climate, set.value(), chunk);
                Bin.str(out, choice.orElse(""));
                if (choice.isPresent()) {
                    selected++;
                }
            }
            System.out.println(
                "  selection " + dim.id() + " seed " + seed.seed() + " " + id(set) + ": " + cases.size() + " cases, "
                    + selected + " selected"
            );
        }
    }

    /// `ChunkGenerator.createStructures` over one set at one chunk: the weighted
    /// draw with removal, each entry tried through `Structure.generate`.
    private static Optional<String> selected(
        final RegistryAccess.Frozen registries,
        final StructureTemplateManager templates,
        final Dim dim,
        final SeedState seed,
        final Climate.Sampler climate,
        final StructureSet set,
        final ChunkPos chunk
    ) {
        List<StructureSet.StructureSelectionEntry> entries = set.structures();
        if (entries.size() == 1) {
            StructureSet.StructureSelectionEntry only = entries.get(0);
            return tryGenerate(registries, templates, dim, seed, climate, only, chunk) ? Optional.of(id(only.structure())) : Optional.empty();
        }
        List<StructureSet.StructureSelectionEntry> options = new ArrayList<>(entries);
        WorldgenRandom random = new WorldgenRandom(new LegacyRandomSource(0L));
        random.setLargeFeatureSeed(seed.seed(), chunk.x(), chunk.z());
        int total = 0;
        for (StructureSet.StructureSelectionEntry option : options) {
            total += option.weight();
        }
        while (!options.isEmpty()) {
            int choice = random.nextInt(total);
            int index = 0;
            for (StructureSet.StructureSelectionEntry option : options) {
                choice -= option.weight();
                if (choice < 0) {
                    break;
                }
                index++;
            }
            StructureSet.StructureSelectionEntry picked = options.get(index);
            if (tryGenerate(registries, templates, dim, seed, climate, picked, chunk)) {
                return Optional.of(id(picked.structure()));
            }
            options.remove(index);
            total -= picked.weight();
        }
        return Optional.empty();
    }

    private static boolean tryGenerate(
        final RegistryAccess.Frozen registries,
        final StructureTemplateManager templates,
        final Dim dim,
        final SeedState seed,
        final Climate.Sampler climate,
        final StructureSet.StructureSelectionEntry entry,
        final ChunkPos chunk
    ) {
        Structure structure = entry.structure().value();
        StructureStart start = structure.generate(
            entry.structure(), dim.level(), registries, dim.generator(), dim.biomeSource(), climate, seed.randomState(),
            templates, seed.seed(), chunk, 0, dim.heights(), structure.biomes()::contains
        );
        return start.isValid();
    }

    private static final Method FIND_GENERATION_POINT;

    static {
        try {
            FIND_GENERATION_POINT = Structure.class.getDeclaredMethod("findGenerationPoint", Structure.GenerationContext.class);
            FIND_GENERATION_POINT.setAccessible(true);
        } catch (NoSuchMethodException e) {
            throw new IllegalStateException(e);
        }
    }

    /// `findGenerationPoint` is protected on `Structure`; every shipped type
    /// widens it to public, but the call must go through the base type here.
    @SuppressWarnings("unchecked")
    private static Optional<Structure.GenerationStub> findGenerationPoint(
        final Structure structure, final Structure.GenerationContext context
    ) {
        try {
            return (Optional<Structure.GenerationStub>) FIND_GENERATION_POINT.invoke(structure, context);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
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
