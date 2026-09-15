package mcrs.oracle;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Function;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.flag.FeatureFlagSet;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.level.CustomSpawner;
import net.minecraft.world.level.Level;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.sounds.SoundSource;
import net.minecraft.resources.ResourceKey;
import net.minecraft.util.RandomSource;
import net.minecraft.world.Difficulty;
import net.minecraft.world.DifficultyInstance;
import net.minecraft.world.attribute.EnvironmentAttributeReader;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.StructureManager;
import net.minecraft.world.level.WorldGenLevel;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.border.WorldBorder;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.ChunkSource;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.dimension.LevelStem;
import net.minecraft.world.level.entity.EntityTypeTest;
import net.minecraft.world.level.gameevent.GameEvent;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.levelgen.structure.Structure;
import net.minecraft.world.level.levelgen.structure.StructureStart;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.storage.LevelData;
import net.minecraft.world.level.storage.LevelStorageSource;
import net.minecraft.world.level.storage.ServerLevelData;
import net.minecraft.world.level.storage.TagValueOutput;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.util.ProblemReporter;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.ticks.LevelTickAccess;

/**
 * A `WorldGenLevel` over a flat floor. Every method throws until something a
 * feature actually calls forces it to be implemented, and each implemented one
 * is a delegation to the block map, the floor height or the bootstrapped
 * registries.
 */
public final class StubLevel implements WorldGenLevel {
    public static final long WORLD_SEED = 0x5EEDL;

    private final RegistryAccess registries;
    private final DimensionType dimensionType;
    private final Function<BlockPos, BlockState> base;
    private final BlockState air = Blocks.AIR.defaultBlockState();
    private final int seaLevel;
    private RandomSource levelRandom;
    private final long seed;
    private final Holder<Biome> biome;
    private final boolean heightsFromBase;

    private long subTick;
    private int nextEntityId;
    private final ServerLevel serverLevel;
    private final StructureManager structureManager;
    private StructureStart placing = StructureStart.INVALID_START;
    private EnvironmentAttributeReader environmentAttributes;
    private final Map<BlockPos, BlockEntity> blockEntities = new java.util.LinkedHashMap<>();
    private final Map<net.minecraft.world.level.ChunkPos, ChunkAccess> chunks = new HashMap<>();
    private final List<RecordedEntity> entities = new ArrayList<>();

    private final Map<BlockPos, BlockState> overrides = new HashMap<>();
    private final List<BlockPos> writeOrder = new ArrayList<>();
    private final List<Write> writes = new ArrayList<>();

    public record RecordedEntity(String type, Entity entity, CompoundTag nbt) {}

    public record Write(BlockPos pos, BlockState state) {}

    public StubLevel(
        final RegistryAccess registries,
        final DimensionType dimensionType,
        final Function<BlockPos, BlockState> base,
        final int seaLevel,
        final RandomSource levelRandom
    ) {
        this(registries, dimensionType, base, seaLevel, levelRandom, WORLD_SEED, null, false, null);
    }

    /**
     * The level a whole structure start is placed into: the biome is fixed,
     * the heightmaps answer the base alone (the reference primes its `_WG`
     * maps at the terrain step and never updates them), `getRandom()` is the
     * placement stream, difficulty is Normal at clock time zero, and with a
     * `server` entities are built for real and recorded instead of failing.
     */
    public StubLevel(
        final RegistryAccess registries,
        final DimensionType dimensionType,
        final Function<BlockPos, BlockState> base,
        final int seaLevel,
        final RandomSource levelRandom,
        final long seed,
        final Holder<Biome> biome,
        final boolean heightsFromBase,
        final MinecraftServer server
    ) {
        this.registries = registries;
        this.dimensionType = dimensionType;
        this.base = base;
        this.seaLevel = seaLevel;
        this.levelRandom = levelRandom;
        this.seed = seed;
        this.biome = biome;
        this.heightsFromBase = heightsFromBase;
        this.serverLevel = server != null ? StubServerLevel.allocate(this, server) : SeedOnlyServerLevel.allocate();
        this.structureManager = new PlacingStartManager(this, new WorldOptions(seed, true, false));
    }

    public StubLevel(
        final RegistryAccess registries,
        final DimensionType dimensionType,
        final BlockState floor,
        final BlockState air,
        final int floorTop,
        final RandomSource levelRandom
    ) {
        this(registries, dimensionType, pos -> pos.getY() <= floorTop ? floor : air, floorTop + 1, levelRandom);
    }

    public Map<BlockPos, BlockEntity> blockEntities() {
        return this.blockEntities;
    }

    /** Every entity `addFreshEntity` received, in arrival order, saved as it arrived. */
    public List<RecordedEntity> entities() {
        return this.entities;
    }

    /** Every `setBlock` in call order, rewrites included. */
    public List<Write> writes() {
        return this.writes;
    }

    public StructureManager structureManager() {
        return this.structureManager;
    }

    /** The start whose pieces are being placed: what the structure manager answers with. */
    public void placing(final StructureStart start) {
        this.placing = start;
    }

    /** The placement stream of the chunk about to be placed, reseeded per chunk as the reference does. */
    public void random(final RandomSource random) {
        this.levelRandom = random;
        if (this.serverLevel instanceof StubServerLevel stub) {
            stub.random(random);
        }
    }

    /**
     * Every position ever written, in the order it was first written, with the
     * state it ended up holding. First-write order is the trunk, foliage and
     * decorator order; the leaf relaxation only rewrites positions already in
     * the list, so it cannot reorder it.
     */
    public Map<BlockPos, BlockState> written() {
        Map<BlockPos, BlockState> out = new java.util.LinkedHashMap<>();
        for (BlockPos pos : this.writeOrder) {
            out.put(pos, this.overrides.get(pos));
        }
        return out;
    }

    @Override
    public BlockState getBlockState(final BlockPos pos) {
        if (this.isOutsideBuildHeight(pos)) {
            return this.air;
        }
        BlockState state = this.overrides.get(pos.immutable());
        if (state != null) {
            return state;
        }
        return this.base.apply(pos);
    }

    @Override
    public boolean setBlock(final BlockPos pos, final BlockState state, final int flags, final int limit) {
        if (this.isOutsideBuildHeight(pos)) {
            throw new UnsupportedOperationException("isClientSide");
        }
        BlockPos at = pos.immutable();
        if (this.overrides.put(at, state) == null) {
            this.writeOrder.add(at);
        }
        this.writes.add(new Write(at, state));
        return true;
    }

    @Override
    public boolean isStateAtPosition(final BlockPos pos, final Predicate<BlockState> predicate) {
        return predicate.test(this.getBlockState(pos));
    }

    @Override
    public boolean isFluidAtPosition(final BlockPos pos, final Predicate<FluidState> predicate) {
        return predicate.test(this.getFluidState(pos));
    }

    @Override
    public FluidState getFluidState(final BlockPos pos) {
        return this.getBlockState(pos).getFluidState();
    }

    @Override
    public DimensionType dimensionType() {
        return this.dimensionType;
    }

    @Override
    public RegistryAccess registryAccess() {
        return this.registries;
    }

    @Override
    public RandomSource getRandom() {
        return this.levelRandom;
    }

    @Override
    public int getHeight(final Heightmap.Types type, final int x, final int z) {
        Predicate<BlockState> isOpaque = type.isOpaque();
        BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        for (int y = this.getMaxY(); y >= this.getMinY(); y--) {
            pos.set(x, y, z);
            BlockState state = this.heightsFromBase ? this.base.apply(pos) : this.getBlockState(pos);
            if (isOpaque.test(state)) {
                return y + 1;
            }
        }
        return this.getMinY();
    }

    @Override
    public long getSeed() {
        return this.seed;
    }

    @Override
    public boolean addFreshEntity(final Entity entity) {
        TagValueOutput output = TagValueOutput.createWithContext(ProblemReporter.DISCARDING, this.registries);
        entity.saveWithoutId(output);
        String type = net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString();
        this.entities.add(new RecordedEntity(type, entity, output.buildResult()));
        return true;
    }

    @Override
    public BlockEntity getBlockEntity(final BlockPos pos) {
        BlockPos at = pos.immutable();
        BlockEntity existing = this.blockEntities.get(at);
        if (existing != null) {
            return existing;
        }
        BlockState state = this.getBlockState(at);
        if (!(state.getBlock() instanceof EntityBlock entityBlock)) {
            return null;
        }
        BlockEntity created = entityBlock.newBlockEntity(at, state);
        if (created != null) {
            this.blockEntities.put(at, created);
        }
        return created;
    }

    @Override
    public boolean removeBlock(final BlockPos pos, final boolean moving) {
        throw new UnsupportedOperationException("removeBlock");
    }

    @Override
    public int getSeaLevel() {
        return this.seaLevel;
    }

    @Override
    public boolean isClientSide() {
        return false;
    }

    /**
     * What `ServerLevel.getCurrentDifficultyAt` answers while the chunk is not
     * yet full: the server difficulty at overworld clock time zero, no
     * inhabited time and no moon.
     */
    @Override
    public DifficultyInstance getCurrentDifficultyAt(final BlockPos pos) {
        return new DifficultyInstance(Difficulty.NORMAL, 0L, 0L, 0.0F);
    }

    @Override
    public Difficulty getDifficulty() {
        return Difficulty.NORMAL;
    }

    @Override
    public ServerLevel getLevel() {
        return this.serverLevel;
    }

    @Override
    public long nextSubTickCount() {
        return this.subTick++;
    }

    @Override
    public long getGameTime() {
        return 0L;
    }

    @Override
    public LevelData getLevelData() {
        throw new UnsupportedOperationException("getLevelData");
    }

    @Override
    public ChunkSource getChunkSource() {
        throw new UnsupportedOperationException("getChunkSource");
    }

    /** Particles, sounds and events are swallowed exactly as `WorldGenRegion` swallows them. */
    @Override
    public void playSound(
        final Entity except,
        final BlockPos pos,
        final SoundEvent sound,
        final SoundSource source,
        final float volume,
        final float pitch
    ) {
    }

    @Override
    public void levelEvent(final Entity source, final int type, final BlockPos pos, final int data) {
    }

    @Override
    public void gameEvent(final Holder<GameEvent> gameEvent, final Vec3 position, final GameEvent.Context context) {
    }

    @Override
    public MinecraftServer getServer() {
        throw new UnsupportedOperationException("getServer");
    }

    @Override
    public void addParticle(
        final ParticleOptions particle,
        final double x,
        final double y,
        final double z,
        final double xd,
        final double yd,
        final double zd
    ) {
    }

    @Override
    public BiomeManager getBiomeManager() {
        if (this.biome == null) {
            throw new UnsupportedOperationException("getBiomeManager");
        }
        return new BiomeManager(this, BiomeManager.obfuscateSeed(this.seed));
    }

    @Override
    public Holder<Biome> getNoiseBiome(final int quartX, final int quartY, final int quartZ) {
        return this.getUncachedNoiseBiome(quartX, quartY, quartZ);
    }

    /**
     * A piece asks for the chunk only to mark a fence, bar or wall for the
     * post-processing pass, which the reference resolves once the chunk is
     * full and no dump reproduces; an empty proto chunk takes the mark.
     */
    @Override
    public ChunkAccess getChunk(final int x, final int z, final ChunkStatus status, final boolean nonnull) {
        if (this.biome == null) {
            throw new UnsupportedOperationException("getChunk");
        }
        return this.chunks.computeIfAbsent(
            new net.minecraft.world.level.ChunkPos(x, z),
            pos -> new net.minecraft.world.level.chunk.ProtoChunk(
                pos,
                net.minecraft.world.level.chunk.UpgradeData.EMPTY,
                this,
                net.minecraft.world.level.chunk.PalettedContainerFactory.create(this.registries),
                null
            )
        );
    }

    @Override
    public int getSkyDarken() {
        throw new UnsupportedOperationException("getSkyDarken");
    }

    @Override
    public FeatureFlagSet enabledFeatures() {
        return FeatureFlags.DEFAULT_FLAGS;
    }

    @Override
    public Holder<Biome> getUncachedNoiseBiome(final int x, final int y, final int z) {
        if (this.biome == null) {
            throw new UnsupportedOperationException("getUncachedNoiseBiome");
        }
        return this.biome;
    }

    /** What `WorldGenRegion` builds: the dimension's and the biome's static layers, no timeline. */
    @Override
    public EnvironmentAttributeReader environmentAttributes() {
        if (this.biome == null) {
            throw new UnsupportedOperationException("environmentAttributes");
        }
        if (this.environmentAttributes == null) {
            this.environmentAttributes = net.minecraft.world.attribute.EnvironmentAttributeSystem.builder()
                .addStaticLayers(this)
                .build();
        }
        return this.environmentAttributes;
    }

    @Override
    public LevelLightEngine getLightEngine() {
        throw new UnsupportedOperationException("getLightEngine");
    }

    @Override
    public WorldBorder getWorldBorder() {
        throw new UnsupportedOperationException("getWorldBorder");
    }

    @Override
    public boolean destroyBlock(final BlockPos pos, final boolean drop, final Entity source, final int limit) {
        throw new UnsupportedOperationException("destroyBlock");
    }

    @Override
    public <T extends Entity> List<T> getEntities(
        final EntityTypeTest<Entity, T> test, final AABB bounds, final Predicate<? super T> predicate
    ) {
        throw new UnsupportedOperationException("getEntities");
    }

    @Override
    public List<Entity> getEntities(final Entity except, final AABB bounds, final Predicate<? super Entity> predicate) {
        throw new UnsupportedOperationException("getEntities");
    }

    @Override
    public List<? extends net.minecraft.world.entity.player.Player> players() {
        throw new UnsupportedOperationException("players");
    }

    @Override
    public LevelTickAccess<net.minecraft.world.level.block.Block> getBlockTicks() {
        return new NoTicks<>();
    }

    @Override
    public LevelTickAccess<net.minecraft.world.level.material.Fluid> getFluidTicks() {
        return new NoTicks<>();
    }

    /** Nothing on a dry flat floor schedules a tick; one that did would be a silent divergence. */
    private final class NoTicks<T> implements LevelTickAccess<T> {
        @Override
        public boolean willTickThisTick(final BlockPos pos, final T type) {
            return false;
        }

        @Override
        public void schedule(final net.minecraft.world.ticks.ScheduledTick<T> tick) {
        }

        @Override
        public boolean hasScheduledTick(final BlockPos pos, final T type) {
            return false;
        }

        @Override
        public int count() {
            return 0;
        }
    }

    static <T> T allocate(final Class<T> type) {
        try {
            java.lang.reflect.Field theUnsafe = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
            theUnsafe.setAccessible(true);
            sun.misc.Unsafe unsafe = (sun.misc.Unsafe) theUnsafe.get(null);
            return type.cast(unsafe.allocateInstance(type));
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException("cannot allocate " + type.getName(), e);
        }
    }

    private static void seat(final Object target, final Class<?> declaring, final String field, final Object value) {
        try {
            java.lang.reflect.Field f = declaring.getDeclaredField(field);
            f.setAccessible(true);
            f.set(target, value);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException("cannot seat " + declaring.getName() + "." + field, e);
        }
    }

    /**
     * A `MinecraftServer` for the entities a structure spawns: a dedicated
     * server allocated without a constructor, holding only what a mob reaches
     * through `level().getServer()` while it is built (the debug subscriber
     * table its path finder consults) and the registries and templates a
     * later query would ask for.
     */
    public static MinecraftServer server(
        final net.minecraft.core.LayeredRegistryAccess<net.minecraft.server.RegistryLayer> registries,
        final net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager templates
    ) {
        MinecraftServer server = allocate(net.minecraft.server.dedicated.DedicatedServer.class);
        seat(server, MinecraftServer.class, "debugSubscribers", new net.minecraft.util.debug.ServerDebugSubscribers(server));
        seat(server, MinecraftServer.class, "registries", registries);
        seat(server, MinecraftServer.class, "structureTemplateManager", templates);
        return server;
    }

    /**
     * The only thing template placement asks of `getLevel()` is the world seed
     * (`CappedProcessor.finalizeProcessing`); template entities reach it through
     * `createEntityIgnoreException`, whose `catch (Exception)` swallows the throws
     * below, so no entity is ever built. Allocated without running a constructor:
     * `ServerLevel` has no constructible shape without a server.
     */
    public static final class SeedOnlyServerLevel extends ServerLevel {
        private SeedOnlyServerLevel(
            final MinecraftServer server,
            final java.util.concurrent.Executor executor,
            final LevelStorageSource.LevelStorageAccess storage,
            final ServerLevelData levelData,
            final ResourceKey<Level> dimension,
            final LevelStem stem,
            final List<CustomSpawner> spawners
        ) {
            super(server, executor, storage, levelData, dimension, stem, false, 0L, spawners, false);
        }

        static ServerLevel allocate() {
            return StubLevel.allocate(SeedOnlyServerLevel.class);
        }

        @Override
        public long getSeed() {
            return WORLD_SEED;
        }

        @Override
        public int getNextEntityId() {
            throw new UnsupportedOperationException("getNextEntityId");
        }

        @Override
        public FeatureFlagSet enabledFeatures() {
            throw new UnsupportedOperationException("enabledFeatures");
        }
    }

    /**
     * A `ServerLevel` for the entities a structure spawns: allocated without a
     * constructor like the seed-only one, then every method an entity reaches
     * through `level()` while it is built, finalized and saved is answered from
     * the stub that owns it. `Level.random` is a final field the constructor
     * would have set, so it is written by reflection to the placement stream.
     */
    public static final class StubServerLevel extends ServerLevel {
        private StubLevel owner;
        private MinecraftServer server;

        private StubServerLevel(
            final MinecraftServer server,
            final java.util.concurrent.Executor executor,
            final LevelStorageSource.LevelStorageAccess storage,
            final ServerLevelData levelData,
            final ResourceKey<Level> dimension,
            final LevelStem stem,
            final List<CustomSpawner> spawners
        ) {
            super(server, executor, storage, levelData, dimension, stem, false, 0L, spawners, false);
        }

        static ServerLevel allocate(final StubLevel owner, final MinecraftServer server) {
            StubServerLevel level = StubLevel.allocate(StubServerLevel.class);
            level.owner = owner;
            level.server = server;
            level.random(owner.levelRandom);
            seat(level, Level.class, "soundSeedGenerator", RandomSource.createThreadSafe());
            return level;
        }

        @Override
        public MinecraftServer getServer() {
            return this.server;
        }

        void random(final RandomSource random) {
            seat(this, Level.class, "random", random);
        }

        @Override
        public long getSeed() {
            return this.owner.seed;
        }

        @Override
        public int getNextEntityId() {
            return ++this.owner.nextEntityId;
        }

        @Override
        public FeatureFlagSet enabledFeatures() {
            return FeatureFlags.DEFAULT_FLAGS;
        }

        @Override
        public RegistryAccess registryAccess() {
            return this.owner.registries;
        }

        @Override
        public DimensionType dimensionType() {
            return this.owner.dimensionType;
        }

        @Override
        public RandomSource getRandom() {
            return this.owner.levelRandom;
        }

        @Override
        public long getGameTime() {
            return 0L;
        }

        @Override
        public DifficultyInstance getCurrentDifficultyAt(final BlockPos pos) {
            return this.owner.getCurrentDifficultyAt(pos);
        }

        @Override
        public Difficulty getDifficulty() {
            return this.owner.getDifficulty();
        }

        @Override
        public BlockState getBlockState(final BlockPos pos) {
            return this.owner.getBlockState(pos);
        }

        @Override
        public FluidState getFluidState(final BlockPos pos) {
            return this.owner.getFluidState(pos);
        }

        @Override
        public BlockEntity getBlockEntity(final BlockPos pos) {
            return this.owner.getBlockEntity(pos);
        }

        @Override
        public int getHeight(final Heightmap.Types type, final int x, final int z) {
            return this.owner.getHeight(type, x, z);
        }

        @Override
        public int getSeaLevel() {
            return this.owner.seaLevel;
        }

        @Override
        public BiomeManager getBiomeManager() {
            return this.owner.getBiomeManager();
        }

        @Override
        public Holder<Biome> getNoiseBiome(final int quartX, final int quartY, final int quartZ) {
            return this.owner.getUncachedNoiseBiome(quartX, quartY, quartZ);
        }

        @Override
        public Holder<Biome> getUncachedNoiseBiome(final int quartX, final int quartY, final int quartZ) {
            return this.owner.getUncachedNoiseBiome(quartX, quartY, quartZ);
        }

        @Override
        public StructureManager structureManager() {
            return this.owner.structureManager;
        }

        @Override
        public net.minecraft.world.attribute.EnvironmentAttributeSystem environmentAttributes() {
            return (net.minecraft.world.attribute.EnvironmentAttributeSystem) this.owner.environmentAttributes();
        }

        @Override
        public boolean addFreshEntity(final Entity entity) {
            return this.owner.addFreshEntity(entity);
        }

        /** Sounds and events go to players; there are none, and the sound seed is a random of its own. */
        @Override
        public void playSeededSound(
            final Entity except,
            final double x,
            final double y,
            final double z,
            final Holder<SoundEvent> sound,
            final SoundSource source,
            final float volume,
            final float pitch,
            final long seed
        ) {
        }

        @Override
        public void playSeededSound(
            final Entity except,
            final Entity sourceEntity,
            final Holder<SoundEvent> sound,
            final SoundSource source,
            final float volume,
            final float pitch,
            final long seed
        ) {
        }

        @Override
        public void levelEvent(final Entity source, final int type, final BlockPos pos, final int data) {
        }

        @Override
        public void gameEvent(final Holder<GameEvent> gameEvent, final Vec3 position, final GameEvent.Context context) {
        }
    }

    /**
     * The structure manager placement sees: every start query is answered with
     * the start being placed, so a spawn condition asking which structure
     * stands here (the cat's) reads the placing start and nothing else.
     */
    private static final class PlacingStartManager extends StructureManager {
        private final StubLevel owner;

        PlacingStartManager(final StubLevel owner, final WorldOptions options) {
            super(owner, options, null);
            this.owner = owner;
        }

        @Override
        public List<StructureStart> startsForStructure(
            final int sectionX, final int sectionZ, final Predicate<Structure> matcher
        ) {
            StructureStart start = this.owner.placing;
            return start.isValid() && matcher.test(start.getStructure()) ? List.of(start) : List.of();
        }

        @Override
        public List<StructureStart> startsForStructure(final int sectionX, final int sectionZ, final Structure structure) {
            return this.startsForStructure(sectionX, sectionZ, structure::equals);
        }
    }
}
