package mcrs.oracle;

import com.mojang.serialization.MapCodec;
import io.netty.buffer.Unpooled;
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
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.function.Function;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.Vec3i;
import net.minecraft.core.component.DataComponentInitializers;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtIo;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.level.WorldGenRegion;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.util.RandomSource;
import net.minecraft.util.Util;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.NoiseColumn;
import net.minecraft.world.level.StructureManager;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.FixedBiomeSource;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.Rotation;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.ChunkGenerator;
import net.minecraft.world.level.dimension.BuiltinDimensionTypes;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.blending.Blender;
import net.minecraft.world.level.levelgen.densityfunction.SamplerContext;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.levelgen.structure.Structure;
import net.minecraft.world.level.levelgen.structure.StructurePiece;
import net.minecraft.world.level.levelgen.structure.StructureStart;
import net.minecraft.world.level.levelgen.structure.TemplateStructurePiece;
import net.minecraft.world.level.levelgen.structure.structures.JigsawStructure;
import net.minecraft.world.level.levelgen.structure.structures.ShipwreckPieces;
import net.minecraft.world.level.levelgen.structure.structures.ShipwreckStructure;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructurePlaceSettings;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager;
import net.minecraft.world.level.storage.LevelStorageSource;

/**
 * Whole starts generated over a flat world with one biome and placed chunk by
 * chunk into a stub level, the way `ChunkGenerator.applyBiomeDecoration` places
 * them: what every column of the start writes, which block entities it loads
 * and which entities it spawns.
 */
public final class StructureGeometryOracle {
    private static final byte[] MAGIC = "MCSTRGE0".getBytes(StandardCharsets.US_ASCII);
    private static final long WORLD_SEED = StubLevel.WORLD_SEED;
    private static final List<ChunkPos> CHUNKS = List.of(new ChunkPos(0, 0), new ChunkPos(7, -3), new ChunkPos(-12, 25));
    /// The jigsaw structures placed here: the codec path exists for them, so
    /// they prove the placement harness; their template entities are none.
    private static final List<String> JIGSAW_CASES = List.of("minecraft:trail_ruins");
    private static final int FULL_EVERY = 50;
    private static final int ENTITY_DATA_EOF = 255;
    private static final long FNV_OFFSET = 0xcbf29ce484222325L;
    private static final long FNV_PRIME = 0x100000001b3L;

    enum Base {
        DRY,
        WATER,
        CAVE
    }

    record Dimension(String id, DimensionType type, LevelHeightAccessor heights, int seaLevel, PlacementOracle.SeedState seed) {}

    record Case(Holder.Reference<Structure> structure, Dimension dimension, Base base, ChunkPos chunk, int index) {}

    private final RegistryAccess access;
    private final StructureTemplateManager templates;
    private final MinecraftServer server;
    private final TemplatePlacementOracle.Palette palette = new TemplatePlacementOracle.Palette();
    private int runningPlacement;

    private StructureGeometryOracle(
        final RegistryAccess access, final StructureTemplateManager templates, final MinecraftServer server
    ) {
        this.access = access;
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
            StructureGeometryOracle oracle = new StructureGeometryOracle(access, templates, StubLevel.server(layered, templates));
            List<Case> cases = cases(loaded);

            ByteArrayOutputStream body = new ByteArrayOutputStream();
            int present = 0;
            for (Case c : cases) {
                present += oracle.writeCase(body, c) ? 1 : 0;
            }
            Path file = outDir.resolve("structure_geometry.bin");
            try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
                PlacementOracle.header(out, MAGIC);
                oracle.palette.write(out);
                Bin.i32(out, cases.size());
                body.writeTo(out);
            }
            System.out.println("cases: " + cases.size() + ", present: " + present + ", placed chunks: "
                + oracle.runningPlacement + ", palette states: " + oracle.palette.size());
            System.out.println("wrote " + file + " (" + Files.size(file) + " bytes)");
        }
        System.exit(0);
    }

    /// Every non-jigsaw structure of the registry plus the jigsaw smoke cases,
    /// in registry order, each in the first dimension whose sets place it, at
    /// every case chunk. `index` is the structure's position among the
    /// structures of its step in registry order, as `applyBiomeDecoration`
    /// counts it.
    private static List<Case> cases(final RegistryAccess.Frozen loaded) throws Exception {
        List<PlacementOracle.Dim> dims = PlacementOracle.dims(loaded);
        Map<String, Dimension> dimensions = new LinkedHashMap<>();
        for (PlacementOracle.Dim dim : dims) {
            DimensionType type = loaded.lookupOrThrow(Registries.DIMENSION_TYPE)
                .getOrThrow(
                    dim.level() == Level.OVERWORLD
                        ? BuiltinDimensionTypes.OVERWORLD
                        : dim.level() == Level.NETHER ? BuiltinDimensionTypes.NETHER : BuiltinDimensionTypes.END
                )
                .value();
            dimensions.put(
                dim.id(),
                new Dimension(dim.id(), type, dim.heights(), dim.settings().seaLevel(), PlacementOracle.seedState(loaded, dim, WORLD_SEED))
            );
        }
        List<Holder.Reference<Structure>> all = loaded.lookupOrThrow(Registries.STRUCTURE).listElements().toList();
        List<Case> cases = new ArrayList<>();
        int[] stepCounts = new int[16];
        for (Holder.Reference<Structure> holder : all) {
            Structure structure = holder.value();
            int index = stepCounts[structure.step().ordinal()]++;
            if (structure instanceof JigsawStructure && !JIGSAW_CASES.contains(PlacementOracle.id(holder))) {
                continue;
            }
            Dimension dimension = dimensions.values().stream()
                .filter(d -> !d.seed().state().getPlacementsForStructure(holder).isEmpty())
                .findFirst()
                .orElseThrow(() -> new IllegalStateException(PlacementOracle.id(holder) + " has no placement"));
            Base base = base(structure);
            for (ChunkPos chunk : CHUNKS) {
                cases.add(new Case(holder, dimension, base, chunk, index));
            }
        }
        return cases;
    }

    private static final Field IS_BEACHED;
    private static final Field BEACHED_TEMPLATES;
    /// Template pieces place with `knownShape` false, which runs
    /// `updateFromNeighbourShapes` over every placed block against the live
    /// world's light and neighbours; the port does not reproduce that pass,
    /// so it is switched off here exactly as the template placement dump does.
    private static final Field PLACE_SETTINGS;

    static {
        try {
            IS_BEACHED = ShipwreckStructure.class.getDeclaredField("isBeached");
            IS_BEACHED.setAccessible(true);
            BEACHED_TEMPLATES = ShipwreckPieces.class.getDeclaredField("STRUCTURE_LOCATION_BEACHED");
            BEACHED_TEMPLATES.setAccessible(true);
            PLACE_SETTINGS = TemplateStructurePiece.class.getDeclaredField("placeSettings");
            PLACE_SETTINGS.setAccessible(true);
        } catch (NoSuchFieldException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    private static Base base(final Structure structure) throws IllegalAccessException {
        return switch (BuiltInRegistries.STRUCTURE_TYPE.getKey(structure.type()).getPath()) {
            case "ocean_ruin", "ocean_monument", "buried_treasure" -> Base.WATER;
            case "shipwreck" -> IS_BEACHED.getBoolean(structure) ? Base.DRY : Base.WATER;
            case "nether_fossil", "mineshaft" -> Base.CAVE;
            default -> Base.DRY;
        };
    }

    /// Layers from the dimension floor up; `null` is air. Every base ends at
    /// y = 63 so the overworld sea level sits one above it.
    private static BlockState[] layers(final Base base, final int minY) {
        BlockState stone = Blocks.STONE.defaultBlockState();
        BlockState[] layers = new BlockState[64 - minY];
        for (int y = minY; y <= 63; y++) {
            layers[y - minY] = switch (base) {
                case DRY -> y <= 60 ? stone : y <= 62 ? Blocks.DIRT.defaultBlockState() : Blocks.GRASS_BLOCK.defaultBlockState();
                case WATER -> y <= 30 ? stone : y <= 40 ? Blocks.GRAVEL.defaultBlockState() : y <= 62 ? Blocks.WATER.defaultBlockState() : null;
                case CAVE -> y >= 33 && y <= 40 ? null : stone;
            };
        }
        return layers;
    }

    private boolean writeCase(final OutputStream out, final Case c) throws Exception {
        Structure structure = c.structure().value();
        Dimension dim = c.dimension();
        Holder<Biome> biome = structure.biomes().stream().findFirst().orElseThrow();
        int minY = dim.heights().getMinY();
        BlockState[] layers = layers(c.base(), minY);
        FlatGenerator generator = new FlatGenerator(biome, layers, minY, dim.heights().getHeight(), dim.seaLevel());
        RandomState randomState = dim.seed().randomState();
        StructureStart start = structure.generate(
            c.structure(), levelKey(dim), this.access, generator, generator.getBiomeSource(),
            PlacementOracle.climate(dim.seed()), randomState, this.templates, WORLD_SEED, c.chunk(), 0, dim.heights(),
            structure.biomes()::contains
        );

        Bin.str(out, PlacementOracle.id(c.structure()));
        Bin.str(out, dim.id());
        Bin.str(out, PlacementOracle.id(biome));
        out.write(c.base().ordinal());
        Bin.i64(out, WORLD_SEED);
        Bin.i32(out, c.chunk().x());
        Bin.i32(out, c.chunk().z());
        out.write(start.isValid() ? 1 : 0);
        if (!start.isValid()) {
            System.out.println("  " + PlacementOracle.id(c.structure()) + " at " + c.chunk() + ": no start");
            return false;
        }
        for (StructurePiece piece : start.getPieces()) {
            if (piece instanceof TemplateStructurePiece) {
                ((StructurePlaceSettings) PLACE_SETTINGS.get(piece)).setKnownShape(true);
            }
        }
        BoundingBox box = start.getBoundingBox();
        box(out, box);
        BlockState air = Blocks.AIR.defaultBlockState();
        Function<BlockPos, BlockState> base = pos -> {
            int layer = pos.getY() - minY;
            BlockState state = layer >= 0 && layer < layers.length ? layers[layer] : null;
            return state == null ? air : state;
        };
        StubLevel level = new StubLevel(
            this.access, dim.type(), base, dim.seaLevel(), new XoroshiroRandomSource(0L), WORLD_SEED, biome, true, this.server
        );
        if (structure instanceof ShipwreckStructure ship && IS_BEACHED.getBoolean(ship)) {
            lowerBeachedShipwreck(start, level, c.chunk());
        }
        Bin.i32(out, start.getPieces().size());
        int step = structure.step().ordinal();
        out.write(step);
        Bin.i32(out, c.index());

        level.placing(start);
        int maxY = dim.heights().getMaxY();
        List<ChunkPos> chunks = new ArrayList<>();
        for (int x = box.minX() >> 4; x <= box.maxX() >> 4; x++) {
            for (int z = box.minZ() >> 4; z <= box.maxZ() >> 4; z++) {
                chunks.add(new ChunkPos(x, z));
            }
        }
        Bin.i32(out, chunks.size());
        int writes = 0;
        int entities = 0;
        for (ChunkPos chunk : chunks) {
            long decorationSeed = new WorldgenRandom(new XoroshiroRandomSource(0L))
                .setDecorationSeed(WORLD_SEED, chunk.getMinBlockX(), chunk.getMinBlockZ());
            long streamSeed = decorationSeed + c.index() + 10000L * step;
            RandomSource random = new XoroshiroRandomSource(streamSeed);
            level.random(random);
            int writeMark = level.writes().size();
            int blockEntityMark = level.blockEntities().size();
            int entityMark = level.entities().size();
            BoundingBox chunkBB = new BoundingBox(
                chunk.getMinBlockX(), minY + 1, chunk.getMinBlockZ(), chunk.getMaxBlockX(), maxY, chunk.getMaxBlockZ()
            );
            start.placeInChunk(level, level.structureManager(), generator, random, chunkBB, chunk);

            Bin.i32(out, chunk.x());
            Bin.i32(out, chunk.z());
            Bin.i64(out, streamSeed);
            writes += this.writeWrites(out, level.writes().subList(writeMark, level.writes().size()));
            this.writeBlockEntities(out, level.blockEntities(), blockEntityMark);
            List<StubLevel.RecordedEntity> spawned = level.entities().subList(entityMark, level.entities().size());
            entities += spawned.size();
            this.writeEntities(out, spawned);
            Bin.i64(out, random.nextLong());
            Bin.i64(out, random.nextLong());
        }
        System.out.println("  " + PlacementOracle.id(c.structure()) + " at " + c.chunk() + " (" + c.base() + ", "
            + PlacementOracle.id(biome) + "): " + start.getPieces().size() + " pieces, " + chunks.size()
            + " chunks, " + writes + " writes, " + level.blockEntities().size() + " block entities, " + entities
            + " entities");
        return true;
    }

    /// A beached shipwreck that fits its region is lowered by the first chunk
    /// that decorates it, from that chunk's live heightmaps and a `nextInt(3)`
    /// of that chunk's placement stream. The port fixes the height at layout
    /// and spends the draw at the end of the layout stream, so the piece is
    /// lowered here before any chunk is placed: the same footprint walk
    /// `ShipwreckPiece.postProcess` makes, over the stub's heights, with the
    /// draw taken from the layout's `WorldgenRandom` replayed past the
    /// rotation and template picks of `ShipwreckStructure.generatePieces`.
    private static void lowerBeachedShipwreck(final StructureStart start, final StubLevel level, final ChunkPos chunk)
        throws IllegalAccessException {
        for (StructurePiece piece : start.getPieces()) {
            if (!(piece instanceof ShipwreckPieces.ShipwreckPiece ship) || ship.isTooBigToFitInWorldGenRegion()) {
                continue;
            }
            Vec3i size = ship.template().getSize();
            BlockPos position = ship.templatePosition();
            BlockPos corner = position.offset(size.getX() - 1, 0, size.getZ() - 1);
            int lowest = level.getMaxY() + 1;
            for (BlockPos p : BlockPos.betweenClosed(position, corner)) {
                lowest = Math.min(lowest, level.getHeight(Heightmap.Types.WORLD_SURFACE_WG, p.getX(), p.getZ()));
            }
            WorldgenRandom layout = new WorldgenRandom(new LegacyRandomSource(0L));
            layout.setLargeFeatureSeed(WORLD_SEED, chunk.x(), chunk.z());
            Rotation.getRandom(layout);
            Util.getRandom((Object[]) BEACHED_TEMPLATES.get(null), layout);
            ship.adjustPositionHeight(ship.calculateBeachedPosition(lowest, layout));
        }
    }

    private static net.minecraft.resources.ResourceKey<Level> levelKey(final Dimension dim) {
        return switch (dim.id()) {
            case "minecraft:overworld" -> Level.OVERWORLD;
            case "minecraft:the_nether" -> Level.NETHER;
            case "minecraft:the_end" -> Level.END;
            default -> throw new IllegalArgumentException(dim.id());
        };
    }

    /// The chunk's writes as the template placement dump records a placement:
    /// first write fixes the order, last write fixes the state.
    private int writeWrites(final OutputStream out, final List<StubLevel.Write> log) throws IOException {
        Map<BlockPos, BlockState> written = new LinkedHashMap<>();
        for (StubLevel.Write write : log) {
            written.put(write.pos(), write.state());
        }
        boolean full = this.runningPlacement % FULL_EVERY == 0;
        this.runningPlacement++;
        Bin.i32(out, written.size());
        long hash = FNV_OFFSET;
        ByteArrayOutputStream entries = new ByteArrayOutputStream();
        for (Map.Entry<BlockPos, BlockState> entry : written.entrySet()) {
            pos(entries, entry.getKey());
            Bin.i32(entries, this.palette.of(entry.getValue()));
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
        return written.size();
    }

    private void writeBlockEntities(final OutputStream out, final Map<BlockPos, BlockEntity> all, final int mark) throws IOException {
        List<Map.Entry<BlockPos, BlockEntity>> created = new ArrayList<>(all.entrySet()).subList(mark, all.size());
        Comparator<BlockPos> byXyz = Comparator.<BlockPos>comparingInt(BlockPos::getX)
            .thenComparingInt(BlockPos::getY)
            .thenComparingInt(BlockPos::getZ);
        created.sort(Map.Entry.comparingByKey(byXyz));
        Bin.i32(out, created.size());
        for (Map.Entry<BlockPos, BlockEntity> entry : created) {
            BlockEntity be = entry.getValue();
            pos(out, entry.getKey());
            Bin.str(out, BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(be.getType()).toString());
            nbt(out, be.saveWithFullMetadata(this.access));
        }
    }

    /// Each spawned entity as it arrived at `addFreshEntity`: its save tag with
    /// every value the entity drew from its own random masked, and the packed
    /// non-default synched data a client would be sent at pairing.
    private void writeEntities(final OutputStream out, final List<StubLevel.RecordedEntity> spawned) throws IOException {
        Bin.i32(out, spawned.size());
        for (StubLevel.RecordedEntity recorded : spawned) {
            Bin.str(out, recorded.type());
            CompoundTag tag = recorded.nbt().copy();
            mask(tag, recorded.type());
            nbt(out, tag);
            RegistryFriendlyByteBuf buf = new RegistryFriendlyByteBuf(Unpooled.buffer(), this.access);
            List<SynchedEntityData.DataValue<?>> values = recorded.entity().getEntityData().getNonDefaultValues();
            if (values != null) {
                for (SynchedEntityData.DataValue<?> value : values) {
                    value.write(buf);
                }
            }
            buf.writeByte(ENTITY_DATA_EOF);
            byte[] bytes = new byte[buf.readableBytes()];
            buf.readBytes(bytes);
            Bin.i32(out, bytes.length);
            out.write(bytes);
        }
    }

    /// What an entity draws from `Entity.random`, which is seeded from the
    /// clock: its `UUID`; the yaw `LivingEntity`'s constructor rolls, kept only
    /// by the shulker since every other mob is snapped to a heading; and the
    /// two attribute rolls of `Zombie.handleAttributes`. Passengers carry the
    /// same fields under their own id.
    private static void mask(final CompoundTag tag, final String type) {
        tag.remove("UUID");
        if (type.equals("minecraft:shulker")) {
            tag.remove("Rotation");
        }
        tag.getList("attributes").ifPresent(attributes -> attributes.removeIf(entry -> {
            String id = entry.asCompound().flatMap(c -> c.getString("id")).orElse("");
            return id.equals("minecraft:knockback_resistance") || id.equals("minecraft:spawn_reinforcements");
        }));
        tag.getList("Passengers").ifPresent(passengers -> {
            for (int i = 0; i < passengers.size(); i++) {
                passengers.getCompound(i).ifPresent(passenger -> mask(passenger, passenger.getStringOr("id", "")));
            }
        });
    }

    private static void nbt(final OutputStream out, final CompoundTag tag) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        NbtIo.write(tag, new DataOutputStream(bytes));
        Bin.i32(out, bytes.size());
        bytes.writeTo(out);
    }

    private static void box(final OutputStream out, final BoundingBox box) throws IOException {
        Bin.i32(out, box.minX());
        Bin.i32(out, box.minY());
        Bin.i32(out, box.minZ());
        Bin.i32(out, box.maxX());
        Bin.i32(out, box.maxY());
        Bin.i32(out, box.maxZ());
    }

    private static void pos(final OutputStream out, final BlockPos pos) throws IOException {
        Bin.i32(out, pos.getX());
        Bin.i32(out, pos.getY());
        Bin.i32(out, pos.getZ());
    }

    /**
     * A chunk generator that is a flat floor with one biome: what a layout may
     * read of a world (`getBaseHeight`, `getBaseColumn`, the sea level and the
     * height range), answered from the layers, and nothing else.
     */
    static final class FlatGenerator extends ChunkGenerator {
        private final BlockState[] layers;
        private final int minY;
        private final int height;
        private final int seaLevel;

        FlatGenerator(final Holder<Biome> biome, final BlockState[] layers, final int minY, final int height, final int seaLevel) {
            super(new FixedBiomeSource(biome));
            this.layers = layers;
            this.minY = minY;
            this.height = height;
            this.seaLevel = seaLevel;
        }

        @Override
        protected MapCodec<? extends ChunkGenerator> codec() {
            throw new UnsupportedOperationException("codec");
        }

        @Override
        public void spawnOriginalMobs(final WorldGenRegion region) {
        }

        @Override
        public int getGenDepth() {
            return this.height;
        }

        @Override
        public CompletableFuture<ChunkAccess> buildTerrain(
            final ChunkAccess chunk,
            final Blender blender,
            final RandomState randomState,
            final StructureManager structureManager,
            final BiomeManager biomeManager,
            final WorldGenRegion carverBiomeRegion,
            final Set<Holder<Biome>> possibleBiomes
        ) {
            throw new UnsupportedOperationException("buildTerrain");
        }

        @Override
        public int getSeaLevel() {
            return this.seaLevel;
        }

        @Override
        public int getMinY() {
            return this.minY;
        }

        @Override
        public int getBaseHeight(
            final int x, final int z, final Heightmap.Types type, final LevelHeightAccessor heights, final RandomState randomState
        ) {
            for (int layer = Math.min(this.layers.length - 1, heights.getMaxY() - this.minY); layer >= 0; layer--) {
                BlockState state = this.layers[layer];
                if (state != null && type.isOpaque().test(state)) {
                    return this.minY + layer + 1;
                }
            }
            return this.minY;
        }

        @Override
        public NoiseColumn getBaseColumn(final int x, final int z, final LevelHeightAccessor heights, final RandomState randomState) {
            BlockState air = Blocks.AIR.defaultBlockState();
            BlockState[] column = new BlockState[this.layers.length];
            for (int layer = 0; layer < column.length; layer++) {
                column[layer] = this.layers[layer] == null ? air : this.layers[layer];
            }
            return new NoiseColumn(this.minY, column);
        }

        @Override
        public void addDebugScreenInfo(
            final List<String> result, final RandomState randomState, final BlockPos feetPos, final SamplerContext samplerContext
        ) {
        }
    }
}
