package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.lang.reflect.Field;
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
import net.minecraft.world.level.block.Rotation;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.dimension.BuiltinDimensionTypes;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.feature.Feature;
import net.minecraft.world.level.levelgen.feature.TemplateFeature;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.levelgen.structure.pools.LegacySinglePoolElement;
import net.minecraft.world.level.levelgen.structure.pools.ListPoolElement;
import net.minecraft.world.level.levelgen.structure.pools.SinglePoolElement;
import net.minecraft.world.level.levelgen.structure.pools.StructurePoolElement;
import net.minecraft.world.level.levelgen.structure.pools.StructureTemplatePool;
import net.minecraft.world.level.levelgen.structure.templatesystem.LiquidSettings;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructurePlaceSettings;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureProcessor;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureProcessorList;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplate;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager;
import net.minecraft.world.level.storage.LevelStorageSource;

public final class TemplatePlacementOracle {
    private static final byte[] MAGIC = "MCTMPLP0".getBytes(StandardCharsets.US_ASCII);
    private static final BlockPos PIECE_POSITION = new BlockPos(8, 62, 8);
    private static final BlockPos FEATURE_ORIGIN = new BlockPos(8, 64, 8);
    private static final int SEA_LEVEL = 64;
    private static final List<String> FEATURES = List.of("minecraft:desert_well", "minecraft:sulfur_spring");
    private static final long FNV_OFFSET = 0xcbf29ce484222325L;
    private static final long FNV_PRIME = 0x100000001b3L;
    private static final Field PROCESSORS;

    static {
        try {
            PROCESSORS = SinglePoolElement.class.getDeclaredField("processors");
            PROCESSORS.setAccessible(true);
        } catch (NoSuchFieldException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    private static final class Palette {
        private final Map<String, Integer> indices = new LinkedHashMap<>();

        int of(final BlockState state) {
            return this.indices.computeIfAbsent(BlockStateParser.serialize(state), k -> this.indices.size());
        }

        void write(final OutputStream out) throws IOException {
            Bin.i32(out, this.indices.size());
            for (String state : this.indices.keySet()) {
                Bin.str(out, state);
            }
        }
    }

    private record PoolCase(SinglePoolElement element, String template, String processors) {}

    private record FeatureCase(TemplateFeature feature, String templates, String processors) {}

    private final RegistryAccess access;
    private final DimensionType overworld;
    private final StructureTemplateManager templates;
    private final Palette palette = new Palette();
    private int runningPlacement;

    private TemplatePlacementOracle(
        final RegistryAccess access, final DimensionType overworld, final StructureTemplateManager templates
    ) {
        this.access = access;
        this.overworld = overworld;
        this.templates = templates;
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
            RegistryAccess access = RegistryLayer.createRegistryAccess()
                .replaceFrom(RegistryLayer.WORLD, loaded)
                .compositeAccess();
            BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(access).forEach(DataComponentInitializers.PendingComponents::apply);
            StructureTemplateManager templates = new StructureTemplateManager(
                resources, storage, DataFixers.getDataFixer(), BuiltInRegistries.BLOCK
            );
            DimensionType overworld = loaded.lookupOrThrow(Registries.DIMENSION_TYPE)
                .getOrThrow(BuiltinDimensionTypes.OVERWORLD)
                .value();
            TemplatePlacementOracle oracle = new TemplatePlacementOracle(access, overworld, templates);

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
                if (cw == state && half == state && ccw == state) {
                    continue;
                }
                Bin.i32(rows, states.of(state));
                Bin.i32(rows, states.of(cw));
                Bin.i32(rows, states.of(half));
                Bin.i32(rows, states.of(ccw));
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
        System.out.println("rotation census: " + count + " rotating states over " + blocks + " blocks, "
            + states.indices.size() + " distinct states");
    }

    private void writePlacements(final OutputStream out) throws Exception {
        List<PoolCase> poolCases = this.poolCases();
        List<FeatureCase> featureCases = this.featureCases();
        ByteArrayOutputStream cases = new ByteArrayOutputStream();
        int index = 0;
        for (PoolCase c : poolCases) {
            this.writePoolCase(cases, c, index++);
        }
        for (FeatureCase c : featureCases) {
            this.writeFeatureCase(cases, c);
        }
        this.palette.write(out);
        Bin.i32(out, poolCases.size() + featureCases.size());
        cases.writeTo(out);
        System.out.println("cases: " + poolCases.size() + " pool, " + featureCases.size() + " feature, "
            + this.runningPlacement + " placements, " + this.palette.indices.size() + " palette states");
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
                .filter(TemplateFeature.class::isInstance)
                .map(TemplateFeature.class::cast)
                .forEach(feature -> {
                    String templates = String.join(
                        ",", feature.templates().unwrap().stream().map(w -> w.value().template().toString()).toList()
                    );
                    String processors = processorsKey(feature.processors(), true);
                    cases.putIfAbsent(templates + "|" + processors, new FeatureCase(feature, templates, processors));
                });
        }
        return new ArrayList<>(cases.values());
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
        return switch (floor) {
            case 0 -> pos -> pos.getY() <= 63 ? dirt : air;
            case 1 -> pos -> pos.getY() <= 60 ? stone : (pos.getY() <= 63 ? water : air);
            default -> throw new IllegalArgumentException("floor " + floor);
        };
    }

    private StubLevel level(final int floor) {
        return new StubLevel(this.access, this.overworld, floor(floor), SEA_LEVEL, new XoroshiroRandomSource(0L));
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
            RandomSource random = new XoroshiroRandomSource(index);
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
        pos(out, FEATURE_ORIGIN);
        pos(out, FEATURE_ORIGIN);
        out.write(0);
        Bin.i32(out, 6);
        for (int seed = 0; seed < 3; seed++) {
            for (int floor = 0; floor < 2; floor++) {
                RandomSource random = new XoroshiroRandomSource(seed);
                StubLevel level = this.level(floor);
                TemplateFeature.TemplateEntry entry = c.feature().templates().getRandomOrThrow(random);
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
                if (c.feature().processors().isPresent()) {
                    for (StructureProcessor processor : c.feature().processors().get().value().list()) {
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
