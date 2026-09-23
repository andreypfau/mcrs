package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.function.Function;
import java.util.stream.Stream;
import net.minecraft.SharedConstants;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.Vec3i;
import net.minecraft.core.component.DataComponentInitializers;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtIo;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.util.RandomSource;
import net.minecraft.util.Util;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.world.RandomizableContainer;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.Mirror;
import net.minecraft.world.level.block.Rotation;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.dimension.BuiltinDimensionTypes;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.feature.Feature;
import net.minecraft.world.level.levelgen.feature.FossilFeature;
import net.minecraft.world.level.levelgen.feature.TemplateFeature;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.levelgen.structure.pools.LegacySinglePoolElement;
import net.minecraft.world.level.levelgen.structure.pools.ListPoolElement;
import net.minecraft.world.level.levelgen.structure.pools.SinglePoolElement;
import net.minecraft.world.level.levelgen.structure.pools.StructurePoolElement;
import net.minecraft.world.level.levelgen.structure.pools.StructureTemplatePool;
import net.minecraft.world.level.levelgen.structure.structures.RuinedPortalPiece;
import net.minecraft.world.level.levelgen.structure.templatesystem.LiquidSettings;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructurePlaceSettings;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureProcessor;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureProcessorList;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplate;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager;
import net.minecraft.world.level.storage.LevelStorageSource;

public final class TemplatePlacementOracle {
    private static final byte[] MAGIC = "MCTMPLP2".getBytes(StandardCharsets.US_ASCII);
    private static final BlockPos PIECE_POSITION = new BlockPos(8, 62, 8);
    private static final BlockPos FEATURE_ORIGIN = new BlockPos(8, 64, 8);
    private static final int SEA_LEVEL = 64;
    private static final List<String> FEATURES = List.of(
        "minecraft:desert_well", "minecraft:sulfur_spring", "minecraft:fossil_coal", "minecraft:fossil_diamonds"
    );
    private static final List<String> PORTALS = List.of(
        "minecraft:ruined_portal/portal_1",
        "minecraft:ruined_portal/portal_2",
        "minecraft:ruined_portal/portal_3",
        "minecraft:ruined_portal/portal_4",
        "minecraft:ruined_portal/portal_5",
        "minecraft:ruined_portal/portal_6",
        "minecraft:ruined_portal/portal_7",
        "minecraft:ruined_portal/portal_8",
        "minecraft:ruined_portal/portal_9",
        "minecraft:ruined_portal/portal_10",
        "minecraft:ruined_portal/giant_portal_1",
        "minecraft:ruined_portal/giant_portal_2",
        "minecraft:ruined_portal/giant_portal_3"
    );
    private static final int PORTAL_SETUPS_PER_TEMPLATE = 6;
    private static final float[] MOSSINESS = {0.0F, 0.2F, 0.5F, 0.8F, 1.0F};
    private static final long FNV_OFFSET = 0xcbf29ce484222325L;
    private static final long FNV_PRIME = 0x100000001b3L;
    private static final Field PROCESSORS;
    private static final Method MAKE_SETTINGS;

    static {
        try {
            PROCESSORS = SinglePoolElement.class.getDeclaredField("processors");
            PROCESSORS.setAccessible(true);
            MAKE_SETTINGS = RuinedPortalPiece.class.getDeclaredMethod(
                "makeSettings",
                HolderLookup.Provider.class,
                Mirror.class,
                Rotation.class,
                RuinedPortalPiece.VerticalPlacement.class,
                BlockPos.class,
                RuinedPortalPiece.Properties.class
            );
            MAKE_SETTINGS.setAccessible(true);
        } catch (NoSuchFieldException | NoSuchMethodException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    static final class Palette {
        private final Map<String, Integer> indices = new LinkedHashMap<>();

        int of(final BlockState state) {
            return this.indices.computeIfAbsent(BlockStateParser.serialize(state), k -> this.indices.size());
        }

        int size() {
            return this.indices.size();
        }

        void write(final OutputStream out) throws IOException {
            Bin.i32(out, this.indices.size());
            for (String state : this.indices.keySet()) {
                Bin.str(out, state);
            }
        }
    }

    private record PoolCase(SinglePoolElement element, String template, String processors) {}

    private record FeatureCase(Feature feature, String templates, String processors) {}

    private record PortalCase(
        String template,
        Rotation rotation,
        Mirror mirror,
        RuinedPortalPiece.VerticalPlacement vertical,
        RuinedPortalPiece.Properties properties
    ) {
        String key() {
            return "portal:" + this.vertical.getSerializedName()
                + ",cold=" + this.properties.cold()
                + ",air_pocket=" + this.properties.airPocket()
                + ",mossiness=" + this.properties.mossiness()
                + ",blackstone=" + this.properties.replaceWithBlackstone();
        }
    }

    private final RegistryAccess access;
    private final DimensionType overworld;
    private final StructureTemplateManager templates;
    private final MinecraftServer server;
    private final Palette palette = new Palette();
    private int runningPlacement;

    private TemplatePlacementOracle(
        final RegistryAccess access,
        final DimensionType overworld,
        final StructureTemplateManager templates,
        final MinecraftServer server
    ) {
        this.access = access;
        this.overworld = overworld;
        this.templates = templates;
        this.server = server;
    }

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
            RegistryAccess.Frozen loaded = PlacementOracle.loadWorldRegistries(resources);
            LayeredRegistryAccess<RegistryLayer> layered = RegistryLayer.createRegistryAccess()
                .replaceFrom(RegistryLayer.WORLD, loaded);
            RegistryAccess access = layered.compositeAccess();
            BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(access).forEach(DataComponentInitializers.PendingComponents::apply);
            StructureTemplateManager templates = new StructureTemplateManager(
                resources, storage, DataFixers.getDataFixer(), BuiltInRegistries.BLOCK
            );
            DimensionType overworld = loaded.lookupOrThrow(Registries.DIMENSION_TYPE)
                .getOrThrow(BuiltinDimensionTypes.OVERWORLD)
                .value();
            TemplatePlacementOracle oracle = new TemplatePlacementOracle(
                access, overworld, templates, StubLevel.server(layered, templates)
            );

            Path file = outDir.resolve("template_placement.bin");
            try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
                PlacementOracle.header(out, MAGIC);
                writeBlockEntityTypes(out);
                writeBlockEntityBlocks(out);
                writeRotationCensus(out);
                oracle.writePlacements(out);
            }
            System.out.println("wrote " + file + " (" + Files.size(file) + " bytes)");
        }
        System.exit(0);
    }

    private static void writeBlockEntityTypes(final OutputStream out) throws IOException {
        List<String> ids = new ArrayList<>();
        for (BlockEntityType<?> type : BuiltInRegistries.BLOCK_ENTITY_TYPE) {
            ids.add(BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(type).toString());
        }
        Bin.i32(out, ids.size());
        for (String id : ids) {
            Bin.str(out, id);
        }
        System.out.println("block entity types (" + ids.size() + "): " + ids);
    }

    private static void writeBlockEntityBlocks(final OutputStream out) throws IOException {
        List<String> lootSeeded = new ArrayList<>();
        ByteArrayOutputStream rows = new ByteArrayOutputStream();
        int count = 0;
        for (Block block : BuiltInRegistries.BLOCK) {
            if (!(block instanceof EntityBlock entityBlock)) {
                continue;
            }
            BlockEntity created = entityBlock.newBlockEntity(BlockPos.ZERO, block.defaultBlockState());
            if (created == null) {
                continue;
            }
            String blockId = BuiltInRegistries.BLOCK.getKey(block).toString();
            String typeId = BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(created.getType()).toString();
            boolean seeded = created instanceof RandomizableContainer;
            Bin.str(rows, blockId);
            Bin.str(rows, typeId);
            rows.write(seeded ? 1 : 0);
            count++;
            if (seeded) {
                lootSeeded.add(blockId + "->" + typeId);
            }
        }
        Bin.i32(out, count);
        rows.writeTo(out);
        System.out.println("entity blocks: " + count + ", loot seeded: " + lootSeeded);
    }

    private static void writeRotationCensus(final OutputStream out) throws IOException {
        Palette states = new Palette();
        ByteArrayOutputStream rows = new ByteArrayOutputStream();
        int count = 0;
        int blocks = 0;
        for (Block block : BuiltInRegistries.BLOCK) {
            boolean any = false;
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                BlockState cw = state.rotate(Rotation.CLOCKWISE_90);
                BlockState half = state.rotate(Rotation.CLOCKWISE_180);
                BlockState ccw = state.rotate(Rotation.COUNTERCLOCKWISE_90);
                BlockState leftRight = state.mirror(Mirror.LEFT_RIGHT);
                BlockState frontBack = state.mirror(Mirror.FRONT_BACK);
                if (cw == state && half == state && ccw == state && leftRight == state && frontBack == state) {
                    continue;
                }
                Bin.i32(rows, states.of(state));
                Bin.i32(rows, states.of(cw));
                Bin.i32(rows, states.of(half));
                Bin.i32(rows, states.of(ccw));
                Bin.i32(rows, states.of(leftRight));
                Bin.i32(rows, states.of(frontBack));
                count++;
                any = true;
            }
            if (any) {
                blocks++;
            }
        }
        states.write(out);
        Bin.i32(out, count);
        rows.writeTo(out);
        System.out.println("rotation census: " + count + " rotating or mirroring states over " + blocks + " blocks, "
            + states.indices.size() + " distinct states");
    }

    private void writePlacements(final OutputStream out) throws Exception {
        List<PoolCase> poolCases = this.poolCases();
        List<FeatureCase> featureCases = this.featureCases();
        List<PortalCase> portalCases = portalCases();
        ByteArrayOutputStream cases = new ByteArrayOutputStream();
        int index = 0;
        for (PoolCase c : poolCases) {
            this.writePoolCase(cases, c, index++);
        }
        for (FeatureCase c : featureCases) {
            this.writeFeatureCase(cases, c);
            index++;
        }
        for (PortalCase c : portalCases) {
            this.writePortalCase(cases, c, index++);
        }
        this.palette.write(out);
        Bin.i32(out, poolCases.size() + featureCases.size() + portalCases.size());
        cases.writeTo(out);
        System.out.println("cases: " + poolCases.size() + " pool, " + featureCases.size() + " feature, "
            + portalCases.size() + " portal, " + this.runningPlacement + " placements, "
            + this.palette.indices.size() + " palette states");
    }

    private List<PoolCase> poolCases() throws Exception {
        Registry<StructureTemplatePool> pools = this.access.lookupOrThrow(Registries.TEMPLATE_POOL);
        Map<String, PoolCase> cases = new LinkedHashMap<>();
        pools.listElements()
            .sorted(Comparator.comparing(h -> h.key().identifier().toString()))
            .forEach(pool -> {
                for (var pair : pool.value().getTemplates()) {
                    collect(pair.getFirst(), cases);
                }
            });
        return new ArrayList<>(cases.values());
    }

    @SuppressWarnings("unchecked")
    private static void collect(final StructurePoolElement element, final Map<String, PoolCase> cases) {
        switch (element) {
            case ListPoolElement list -> list.getElements().forEach(e -> collect(e, cases));
            case SinglePoolElement single -> {
                String template = single.getTemplateLocation().toString();
                String processors;
                try {
                    processors = processorsKey(Optional.of((Holder<StructureProcessorList>) PROCESSORS.get(single)), false);
                } catch (IllegalAccessException e) {
                    throw new IllegalStateException(e);
                }
                String key = template + "|" + processors + "|" + single.getProjection() + "|"
                    + (single instanceof LegacySinglePoolElement);
                cases.putIfAbsent(key, new PoolCase(single, template, processors));
            }
            default -> {}
        }
    }

    private List<FeatureCase> featureCases() {
        Registry<Feature> features = this.access.lookupOrThrow(Registries.FEATURE);
        Map<String, FeatureCase> cases = new LinkedHashMap<>();
        for (String id : FEATURES) {
            Holder.Reference<Feature> holder = features.getOrThrow(ResourceKey.create(Registries.FEATURE, Identifier.parse(id)));
            Stream.concat(Stream.of(holder), holder.value().getSubFeatures())
                .map(Holder::value)
                .forEach(feature -> {
                    String templates;
                    String processors;
                    if (feature instanceof TemplateFeature template) {
                        templates = String.join(
                            ",", template.templates().unwrap().stream().map(w -> w.value().template().toString()).toList()
                        );
                        processors = processorsKey(template.processors(), true);
                    } else if (feature instanceof FossilFeature fossil) {
                        templates = ids(fossil.fossilStructures()) + ";" + ids(fossil.overlayStructures());
                        processors = processorsKey(Optional.of(fossil.fossilProcessors()), false)
                            + ";" + processorsKey(Optional.of(fossil.overlayProcessors()), false);
                    } else {
                        return;
                    }
                    cases.putIfAbsent(templates + "|" + processors, new FeatureCase(feature, templates, processors));
                });
        }
        return new ArrayList<>(cases.values());
    }

    private static String ids(final List<Identifier> ids) {
        return String.join(",", ids.stream().map(Identifier::toString).toList());
    }

    private static List<PortalCase> portalCases() {
        List<PortalCase> cases = new ArrayList<>();
        int k = 0;
        for (String template : PORTALS) {
            for (int setup = 0; setup < PORTAL_SETUPS_PER_TEMPLATE; setup++, k++) {
                RuinedPortalPiece.Properties properties = new RuinedPortalPiece.Properties(
                    k % 9 < 4, MOSSINESS[k % 5], k % 7 < 3, false, false, k % 11 < 4
                );
                cases.add(new PortalCase(
                    template,
                    Rotation.values()[k % 4],
                    Mirror.values()[k % 3],
                    RuinedPortalPiece.VerticalPlacement.values()[k % 6],
                    properties
                ));
            }
        }
        return cases;
    }

    private static String processorsKey(
        final Optional<Holder<StructureProcessorList>> processors, final boolean allowInlineRules
    ) {
        if (processors.isEmpty()) {
            return "none";
        }
        Holder<StructureProcessorList> holder = processors.get();
        Optional<String> reference = holder.unwrapKey().map(k -> "ref:" + k.identifier());
        if (reference.isPresent()) {
            return reference.get();
        }
        if (!allowInlineRules && !holder.value().list().isEmpty()) {
            throw new IllegalStateException("non-empty inline processor list: " + holder.value().list());
        }
        return "inline";
    }

    private static Function<BlockPos, BlockState> floor(final int floor) {
        BlockState air = Blocks.AIR.defaultBlockState();
        BlockState dirt = Blocks.DIRT.defaultBlockState();
        BlockState stone = Blocks.STONE.defaultBlockState();
        BlockState water = Blocks.WATER.defaultBlockState();
        BlockState lava = Blocks.LAVA.defaultBlockState();
        return switch (floor) {
            case 0 -> pos -> pos.getY() <= 63 ? dirt : air;
            case 1 -> pos -> pos.getY() <= 60 ? stone : (pos.getY() <= 63 ? water : air);
            case 2 -> pos -> pos.getY() <= 60 ? stone : (pos.getY() <= 63 ? lava : air);
            default -> throw new IllegalArgumentException("floor " + floor);
        };
    }

    private StubLevel level(final int floor) {
        return new StubLevel(this.access, this.overworld, floor(floor), SEA_LEVEL, new XoroshiroRandomSource(0L));
    }

    private StubLevel serverLevel(final int floor) {
        return new StubLevel(
            this.access, this.overworld, floor(floor), SEA_LEVEL, new XoroshiroRandomSource(0L),
            StubLevel.WORLD_SEED, null, false, this.server
        );
    }

    private void writePoolCase(final OutputStream out, final PoolCase c, final int index) throws Exception {
        Rotation rotation = Rotation.values()[index % 4];
        LiquidSettings liquid = index % 8 == 7 ? LiquidSettings.IGNORE_WATERLOGGING : LiquidSettings.APPLY_WATERLOGGING;
        BoundingBox clip = new BoundingBox(
            0, this.overworld.minY() + 1, 0, 15, this.overworld.minY() + this.overworld.height() - 1, 15
        );
        BoundingBox bb = this.templates.getOrCreate(c.element().getTemplateLocation())
            .getBoundingBox(new StructurePlaceSettings().setRotation(rotation), PIECE_POSITION);
        BlockPos reference = new BlockPos(bb.getCenter().getX(), bb.minY(), bb.getCenter().getZ());

        out.write(0);
        Bin.str(out, c.template());
        Bin.str(out, c.processors());
        out.write(c.element().getProjection() == StructureTemplatePool.Projection.RIGID ? 0 : 1);
        out.write(c.element() instanceof LegacySinglePoolElement ? 1 : 0);
        out.write(rotation.ordinal());
        out.write(liquid == LiquidSettings.IGNORE_WATERLOGGING ? 1 : 0);
        out.write(0);
        pos(out, BlockPos.ZERO);
        pos(out, PIECE_POSITION);
        pos(out, reference);
        out.write(1);
        Bin.i32(out, clip.minX());
        Bin.i32(out, clip.minY());
        Bin.i32(out, clip.minZ());
        Bin.i32(out, clip.maxX());
        Bin.i32(out, clip.maxY());
        Bin.i32(out, clip.maxZ());
        Bin.i32(out, 2);
        for (int floor = 0; floor < 2; floor++) {
            RandomSource random = new WorldgenRandom(new XoroshiroRandomSource(index));
            StubLevel level = this.level(floor);
            boolean placed = c.element().place(
                this.templates, level, null, null, PIECE_POSITION, reference, rotation, clip, random, liquid, false
            );
            this.writePlacement(out, level, floor, index, c.template(), rotation, PIECE_POSITION, placed, random, false);
        }
    }

    private void writeFeatureCase(final OutputStream out, final FeatureCase c) throws Exception {
        out.write(1);
        Bin.str(out, c.templates());
        Bin.str(out, c.processors());
        out.write(0);
        out.write(0);
        out.write(0);
        out.write(0);
        out.write(0);
        pos(out, BlockPos.ZERO);
        pos(out, FEATURE_ORIGIN);
        pos(out, FEATURE_ORIGIN);
        out.write(0);
        int seeds = c.feature() instanceof FossilFeature ? 8 : 3;
        Bin.i32(out, 2 * seeds);
        for (int seed = 0; seed < seeds; seed++) {
            for (int floor = 0; floor < 2; floor++) {
                RandomSource random = new WorldgenRandom(new XoroshiroRandomSource(seed));
                if (c.feature() instanceof FossilFeature fossil) {
                    StubLevel level = this.serverLevel(floor);
                    boolean placed = fossil.place(level, null, random, FEATURE_ORIGIN);
                    RandomSource replay = new WorldgenRandom(new XoroshiroRandomSource(seed));
                    Rotation rotation = Rotation.getRandom(replay);
                    Identifier drawn = fossil.fossilStructures().get(replay.nextInt(fossil.fossilStructures().size()));
                    this.writePlacement(
                        out, level, floor, seed, drawn.toString(), rotation, FEATURE_ORIGIN, placed, random, seed == 0
                    );
                    continue;
                }
                StubLevel level = this.level(floor);
                TemplateFeature feature = (TemplateFeature) c.feature();
                TemplateFeature.TemplateEntry entry = feature.templates().getRandomOrThrow(random);
                Rotation rotation = Util.getRandom(entry.rotations(), random);
                StructureTemplate template = this.templates.getOrCreate(entry.template());
                Vec3i size = template.getSize();
                BlockPos position = FEATURE_ORIGIN
                    .offset(rotation.rotate(Direction.WEST).getUnitVec3i().multiply(size.getX() / 2))
                    .offset(rotation.rotate(Direction.NORTH).getUnitVec3i().multiply(size.getZ() / 2));
                StructurePlaceSettings settings = new StructurePlaceSettings()
                    .setRotation(rotation)
                    .setRandom(random)
                    .setKnownShape(true);
                if (feature.processors().isPresent()) {
                    for (StructureProcessor processor : feature.processors().get().value().list()) {
                        settings.addProcessor(processor);
                    }
                }
                boolean placed = template.placeInWorld(level, position, position, settings, random, 3);
                this.writePlacement(
                    out, level, floor, seed, entry.template().toString(), rotation, position, placed, random, seed == 0
                );
            }
        }
    }

    private void writePortalCase(final OutputStream out, final PortalCase c, final int index) throws Exception {
        StructureTemplate template = this.templates.getOrCreate(Identifier.parse(c.template()));
        BlockPos pivot = new BlockPos(template.getSize().getX() / 2, 0, template.getSize().getZ() / 2);
        StructurePlaceSettings settings = (StructurePlaceSettings) MAKE_SETTINGS.invoke(
            null, this.access, c.mirror(), c.rotation(), c.vertical(), pivot, c.properties()
        );
        settings.setKnownShape(true);
        BoundingBox bb = template.getBoundingBox(settings, PIECE_POSITION);
        BlockPos reference = new BlockPos(bb.getCenter().getX(), bb.minY(), bb.getCenter().getZ());

        out.write(2);
        Bin.str(out, c.template());
        Bin.str(out, c.key());
        out.write(0);
        out.write(0);
        out.write(c.rotation().ordinal());
        out.write(0);
        out.write(c.mirror().ordinal());
        pos(out, pivot);
        pos(out, PIECE_POSITION);
        pos(out, reference);
        out.write(0);
        Bin.i32(out, 3);
        for (int floor = 0; floor < 3; floor++) {
            RandomSource random = new WorldgenRandom(new XoroshiroRandomSource(index));
            StubLevel level = this.level(floor);
            boolean placed = template.placeInWorld(level, PIECE_POSITION, reference, settings, random, 2);
            this.writePlacement(
                out, level, floor, index, c.template(), c.rotation(), PIECE_POSITION, placed, random, index % 6 == 0
            );
        }
    }

    private void writePlacement(
        final OutputStream out,
        final StubLevel level,
        final int floor,
        final long seed,
        final String templateDrawn,
        final Rotation rotationDrawn,
        final BlockPos position,
        final boolean placed,
        final RandomSource random,
        final boolean forceFull
    ) throws IOException {
        Map<BlockPos, BlockState> written = level.written();
        boolean full = forceFull || this.runningPlacement % 100 == 0;
        this.runningPlacement++;

        out.write(floor);
        Bin.i64(out, seed);
        Bin.str(out, templateDrawn);
        out.write(rotationDrawn.ordinal());
        pos(out, position);
        out.write(placed ? 1 : 0);
        Bin.i32(out, written.size());

        long hash = FNV_OFFSET;
        ByteArrayOutputStream entries = new ByteArrayOutputStream();
        for (Map.Entry<BlockPos, BlockState> entry : written.entrySet()) {
            BlockPos at = entry.getKey();
            int index = this.palette.of(entry.getValue());
            pos(entries, at);
            Bin.i32(entries, index);
        }
        for (byte b : entries.toByteArray()) {
            hash ^= b & 0xFF;
            hash *= FNV_PRIME;
        }
        Bin.i64(out, hash);
        out.write(full ? 1 : 0);
        if (full) {
            entries.writeTo(out);
        }

        List<Map.Entry<BlockPos, BlockEntity>> blockEntities = new ArrayList<>(level.blockEntities().entrySet());
        Comparator<BlockPos> byXyz = Comparator.<BlockPos>comparingInt(BlockPos::getX)
            .thenComparingInt(BlockPos::getY)
            .thenComparingInt(BlockPos::getZ);
        blockEntities.sort(Map.Entry.comparingByKey(byXyz));
        Bin.i32(out, blockEntities.size());
        for (Map.Entry<BlockPos, BlockEntity> entry : blockEntities) {
            BlockEntity be = entry.getValue();
            pos(out, entry.getKey());
            Bin.str(out, BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(be.getType()).toString());
            CompoundTag tag = be.saveWithFullMetadata(this.access);
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            NbtIo.write(tag, new DataOutputStream(bytes));
            Bin.i32(out, bytes.size());
            bytes.writeTo(out);
        }

        Bin.i64(out, random.nextLong());
        Bin.i64(out, random.nextLong());
    }

    private static void pos(final OutputStream out, final BlockPos pos) throws IOException {
        Bin.i32(out, pos.getX());
        Bin.i32(out, pos.getY());
        Bin.i32(out, pos.getZ());
    }
}
