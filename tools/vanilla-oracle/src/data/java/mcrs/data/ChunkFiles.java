package mcrs.data;

import com.mojang.serialization.Dynamic;
import it.unimi.dsi.fastutil.shorts.ShortList;
import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.EnumMap;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.Direction;
import net.minecraft.core.Holder;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.LongArrayTag;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.StringTag;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.biome.NoiseBiomeChunk;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.LiquidBlock;
import net.minecraft.world.level.block.RotatedPillarBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.PalettedContainer;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.level.chunk.storage.SerializableChunkData;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.RetroGen;

public final class ChunkFiles {
    private static final int MIN_SECTION_Y = -4;
    private static final int SECTIONS = 24;
    private static final LevelHeightAccessor HEIGHT = LevelHeightAccessor.create(MIN_SECTION_Y * 16, SECTIONS * 16);
    private static final ChunkPos POS = new ChunkPos(3, -2);

    private ChunkFiles() {}

    public static void main(final String[] args) {
        int status;
        try {
            run(Path.of(args[0]));
            status = 0;
        } catch (Throwable failure) {
            failure.printStackTrace();
            status = 1;
        }
        System.exit(status);
    }

    private static void run(final Path outDir) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Files.createDirectories(outDir);

        try (MultiPackResourceManager resources = new MultiPackResourceManager(
                PackType.SERVER_DATA, List.of(ServerPacksSource.createVanillaPackSource().fullResources())
            )) {
            LayeredRegistryAccess<RegistryLayer> layers = RegistryLayer.createRegistryAccess();
            RegistryAccess.Frozen loaded = BlockDefinitionDumper.loadWorldRegistries(resources, layers);
            RegistryAccess.Frozen access = layers.replaceFrom(RegistryLayer.WORLD, loaded).compositeAccess();
            Registry<Biome> biomes = access.lookupOrThrow(Registries.BIOME);
            PalettedContainerFactory factory = PalettedContainerFactory.create(access);

            write(outDir, "chunk_full.nbt", chunk(factory, biomes, ChunkStatus.FULL, null, null));
            write(outDir, "chunk_terrain.nbt", chunk(factory, biomes, ChunkStatus.TERRAIN, noiseBiomes(factory, biomes), null));
            write(outDir, "chunk_retrogen.nbt", chunk(factory, biomes, ChunkStatus.NOISE_BIOMES, null, retrogen(true)));
            write(outDir, "chunk_retrogen_minimal.nbt", chunk(factory, biomes, ChunkStatus.NOISE_BIOMES, null, retrogen(false)));
        }
    }

    private static void write(final Path outDir, final String name, final SerializableChunkData data) throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        NbtIo.write(data.write(), new DataOutputStream(bytes));
        Files.write(outDir.resolve(name), bytes.toByteArray());
        System.out.println("wrote " + outDir.resolve(name));
    }

    private static RetroGen retrogen(final boolean all) {
        CompoundTag record = new CompoundTag();
        record.putString("target_status", "minecraft:full");
        ListTag rerun = new ListTag();
        rerun.add(StringTag.valueOf("minecraft:biomes"));
        record.put("statuses_to_rerun", rerun);
        if (all) {
            record.putBoolean("has_below_zero_retrogen", true);
            record.put("missing_bedrock", new LongArrayTag(new long[] {5L, 0L, 9L}));
        }
        return RetroGen.CODEC.parse(new Dynamic<>(NbtOps.INSTANCE, record)).getOrThrow(IllegalStateException::new);
    }

    private static NoiseBiomeChunk noiseBiomes(final PalettedContainerFactory factory, final Registry<Biome> biomes) {
        Holder<Biome> forest = biomes.getOrThrow(Biomes.FOREST);
        NoiseBiomeChunk.Builder builder = new NoiseBiomeChunk.Builder(HEIGHT, POS);
        for (int y = MIN_SECTION_Y; y < MIN_SECTION_Y + SECTIONS; y++) {
            PalettedContainer<Holder<Biome>> container = factory.createForNoiseBiomes();
            if (y == 0) {
                container.set(1, 2, 3, forest);
            }
            builder.addSection(y, container);
        }
        return builder.build(factory);
    }

    private static SerializableChunkData chunk(
        final PalettedContainerFactory factory,
        final Registry<Biome> biomes,
        final ChunkStatus status,
        final NoiseBiomeChunk noiseBiomeChunk,
        final RetroGen retroGen
    ) {
        List<SerializableChunkData.SectionData> sections = new ArrayList<>();
        for (int y = MIN_SECTION_Y; y < MIN_SECTION_Y + SECTIONS; y++) {
            sections.add(new SerializableChunkData.SectionData(y, section(factory, biomes, y), null, null));
        }
        return new SerializableChunkData(
            factory,
            POS,
            MIN_SECTION_Y,
            100L,
            7L,
            status,
            null,
            retroGen,
            UpgradeData.EMPTY,
            new EnumMap<Heightmap.Types, long[]>(Heightmap.Types.class),
            new ChunkAccess.PackedTicks(List.of(), List.of()),
            new ShortList[SECTIONS],
            true,
            sections,
            noiseBiomeChunk,
            List.of(),
            List.of(),
            new CompoundTag()
        );
    }

    private static LevelChunkSection section(final PalettedContainerFactory factory, final Registry<Biome> biomes, final int y) {
        PalettedContainer<BlockState> states = factory.createForBlockStates();
        PalettedContainer<Holder<Biome>> cells = factory.createForBiomes();
        if (y == 0) {
            Holder<Biome> forest = biomes.getOrThrow(Biomes.FOREST);
            BlockState stone = Blocks.STONE.defaultBlockState();
            for (int x = 0; x < 16; x++) {
                for (int yy = 0; yy < 16; yy++) {
                    for (int z = 0; z < 16; z++) {
                        states.set(x, yy, z, stone);
                    }
                }
            }
            states.set(1, 2, 3, Blocks.OAK_LOG.defaultBlockState().setValue(RotatedPillarBlock.AXIS, Direction.Axis.X));
            states.set(4, 5, 6, Blocks.WATER.defaultBlockState().setValue(LiquidBlock.LEVEL, 3));
            cells.set(5, 9, 13, forest);
        } else if (y == 1) {
            Holder<Biome> forest = biomes.getOrThrow(Biomes.FOREST);
            Holder<Biome> desert = biomes.getOrThrow(Biomes.DESERT);
            for (int x = 0; x < 16; x++) {
                for (int yy = 0; yy < 16; yy++) {
                    for (int z = 0; z < 16; z++) {
                        if (yy >= 11) {
                            cells.set(x, yy, z, desert);
                        } else if (yy >= 5) {
                            cells.set(x, yy, z, forest);
                        }
                    }
                }
            }
        }
        return new LevelChunkSection(states, cells);
    }
}
