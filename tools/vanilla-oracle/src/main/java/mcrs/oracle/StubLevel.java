package mcrs.oracle;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Predicate;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.sounds.SoundSource;
import net.minecraft.util.RandomSource;
import net.minecraft.world.DifficultyInstance;
import net.minecraft.world.attribute.EnvironmentAttributeReader;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.flag.FeatureFlagSet;
import net.minecraft.world.level.WorldGenLevel;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.border.WorldBorder;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.ChunkSource;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.entity.EntityTypeTest;
import net.minecraft.world.level.gameevent.GameEvent;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.storage.LevelData;
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
    private final RegistryAccess registries;
    private final DimensionType dimensionType;
    private final BlockState floor;
    private final BlockState air;
    private final int floorTop;
    private final RandomSource levelRandom;

    private long subTick;
    private final Map<BlockPos, BlockEntity> blockEntities = new HashMap<>();

    private final Map<BlockPos, BlockState> overrides = new HashMap<>();
    private final List<BlockPos> writeOrder = new ArrayList<>();

    public StubLevel(
        final RegistryAccess registries,
        final DimensionType dimensionType,
        final BlockState floor,
        final BlockState air,
        final int floorTop,
        final RandomSource levelRandom
    ) {
        this.registries = registries;
        this.dimensionType = dimensionType;
        this.floor = floor;
        this.air = air;
        this.floorTop = floorTop;
        this.levelRandom = levelRandom;
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
        return pos.getY() <= this.floorTop ? this.floor : this.air;
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
            if (isOpaque.test(this.getBlockState(pos.set(x, y, z)))) {
                return y + 1;
            }
        }
        return this.getMinY();
    }

    @Override
    public long getSeed() {
        throw new UnsupportedOperationException("getSeed");
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
        return this.floorTop + 1;
    }

    @Override
    public boolean isClientSide() {
        return false;
    }

    @Override
    public DifficultyInstance getCurrentDifficultyAt(final BlockPos pos) {
        throw new UnsupportedOperationException("getCurrentDifficultyAt");
    }

    @Override
    public ServerLevel getLevel() {
        throw new UnsupportedOperationException("getLevel");
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

    @Override
    public void playSound(
        final Entity except,
        final BlockPos pos,
        final SoundEvent sound,
        final SoundSource source,
        final float volume,
        final float pitch
    ) {
        throw new UnsupportedOperationException("playSound");
    }

    @Override
    public void levelEvent(final Entity source, final int type, final BlockPos pos, final int data) {
        throw new UnsupportedOperationException("levelEvent");
    }

    @Override
    public void gameEvent(final Holder<GameEvent> gameEvent, final Vec3 position, final GameEvent.Context context) {
        throw new UnsupportedOperationException("gameEvent");
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
        throw new UnsupportedOperationException("addParticle");
    }

    @Override
    public BiomeManager getBiomeManager() {
        throw new UnsupportedOperationException("getBiomeManager");
    }

    @Override
    public ChunkAccess getChunk(final int x, final int z, final ChunkStatus status, final boolean nonnull) {
        throw new UnsupportedOperationException("getChunk");
    }

    @Override
    public int getSkyDarken() {
        throw new UnsupportedOperationException("getSkyDarken");
    }

    @Override
    public FeatureFlagSet enabledFeatures() {
        throw new UnsupportedOperationException("enabledFeatures");
    }

    @Override
    public Holder<Biome> getUncachedNoiseBiome(final int x, final int y, final int z) {
        throw new UnsupportedOperationException("getUncachedNoiseBiome");
    }

    @Override
    public EnvironmentAttributeReader environmentAttributes() {
        throw new UnsupportedOperationException("environmentAttributes");
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
}
