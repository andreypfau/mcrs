package mcrs.oracle;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Proxy;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.SharedConstants;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LevelHeightAccessor;
import net.minecraft.world.level.WorldGenLevel;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.PalettedContainerFactory;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.UpgradeData;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.feature.BlockReplacement;
import net.minecraft.world.level.levelgen.feature.OreFeature;
import net.minecraft.world.level.levelgen.structure.templatesystem.AlwaysTrueTest;
import net.minecraft.world.level.levelgen.structure.templatesystem.BlockMatchTest;
import net.minecraft.world.level.levelgen.structure.templatesystem.RandomBlockMatchTest;
import net.minecraft.world.level.levelgen.structure.templatesystem.RuleTest;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.ticks.ProtoChunkTicks;

/**
 * Runs the game's `OreFeature.place` on a stub level whose chunk sections are
 * stone from y -64 to 319 and records every block a section is asked to write,
 * in write order, together with the random state the call leaves behind.
 */
public final class OreOracle {
    private static final byte[] MAGIC = "MCOREVN0".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;
    private static final int MIN_Y = -64;
    private static final int HEIGHT = 384;
    private static final int COLUMN_HEIGHT = 320;

    private static BlockState stone;
    private static BlockState air;

    private record Target(String type, String block, float probability, BlockReplacement replacement) {}

    private record OreCase(String name, long seed, BlockPos origin, int size, float discardChance, List<Target> targets) {}

    private record Placement(BlockPos pos, BlockState state) {}

    public static void main(final String[] args) throws IOException {
        Path outDir = Path.of(args[0]);
        Files.createDirectories(outDir);

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        stone = Blocks.STONE.defaultBlockState();
        air = Blocks.AIR.defaultBlockState();

        PalettedContainerFactory containers = SurfaceOracle.containerFactory(
            VanillaRegistries.createWorldLookup().lookupOrThrow(Registries.BIOME)
        );
        List<OreCase> cases = cases();
        Path file = outDir.resolve("ore_vein.bin");
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write(MAGIC);
        Bin.i32(out, FORMAT_VERSION);
        Bin.i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
        Bin.i32(out, cases.size());
        for (OreCase oreCase : cases) {
            run(out, containers, oreCase);
        }
        Files.write(file, out.toByteArray());
        System.out.println("wrote " + file + " (" + Files.size(file) + " bytes)");
    }

    private static List<OreCase> cases() {
        Target stoneToCoal = new Target(
            "block_match", "minecraft:stone", 0.0F,
            new BlockReplacement(new BlockMatchTest(Blocks.STONE), Blocks.COAL_ORE.defaultBlockState())
        );
        Target anythingToDiamond = new Target(
            "always_true", "", 0.0F,
            new BlockReplacement(AlwaysTrueTest.INSTANCE, Blocks.DIAMOND_ORE.defaultBlockState())
        );
        Target halfStoneToIron = new Target(
            "random_block_match", "minecraft:stone", 0.5F,
            new BlockReplacement(new RandomBlockMatchTest(Blocks.STONE, 0.5F), Blocks.IRON_ORE.defaultBlockState())
        );
        Target dirtToGold = new Target(
            "block_match", "minecraft:dirt", 0.0F,
            new BlockReplacement(new BlockMatchTest(Blocks.DIRT), Blocks.GOLD_ORE.defaultBlockState())
        );
        return List.of(
            new OreCase("size0", 42L, new BlockPos(0, 40, 0), 0, 0.0F, List.of(stoneToCoal)),
            new OreCase("size1", 1L, new BlockPos(3, 3, 3), 1, 0.0F, List.of(stoneToCoal)),
            new OreCase("size9", 42L, new BlockPos(0, 40, 0), 9, 0.0F, List.of(stoneToCoal)),
            new OreCase("size17", 42L, new BlockPos(7, -13, -21), 17, 0.0F, List.of(stoneToCoal)),
            new OreCase("size33", 7L, new BlockPos(-100, 200, 55), 33, 0.0F, List.of(stoneToCoal)),
            new OreCase("size64", 12345L, new BlockPos(0, 0, 0), 64, 0.0F, List.of(stoneToCoal)),
            new OreCase("size12_always_true", 2L, new BlockPos(-5, 100, 9), 12, 0.0F, List.of(anythingToDiamond)),
            new OreCase("size20_random_match", 99L, new BlockPos(16, 60, -16), 20, 0.0F, List.of(halfStoneToIron)),
            new OreCase("size8_discard_half", 42L, new BlockPos(0, 40, 0), 8, 0.5F, List.of(stoneToCoal)),
            new OreCase("size16_two_targets", 5L, new BlockPos(-40, 12, 40), 16, 0.0F, List.of(dirtToGold, stoneToCoal)),
            new OreCase("size24_clipped_at_top", 3L, new BlockPos(0, 320, 0), 24, 0.0F, List.of(stoneToCoal)),
            new OreCase("size24_clipped_at_bottom", 3L, new BlockPos(0, -64, 0), 24, 0.0F, List.of(stoneToCoal)),
            new OreCase("size32_sin_index", 58L, new BlockPos(0, 64, 0), 32, 0.0F, List.of(anythingToDiamond)),
            new OreCase("size64_sin_index_a", 19L, new BlockPos(0, 64, 0), 64, 0.0F, List.of(anythingToDiamond)),
            new OreCase("size64_sin_index_b", 26L, new BlockPos(0, 64, 0), 64, 0.0F, List.of(anythingToDiamond)),
            new OreCase("ceiling_always_true", 11L, new BlockPos(0, 318, 0), 24, 0.0F, List.of(anythingToDiamond)),
            new OreCase("floor_always_true", 11L, new BlockPos(0, -62, 0), 24, 0.0F, List.of(anythingToDiamond)),
            new OreCase("ceiling_discard_half", 8L, new BlockPos(0, 318, 0), 24, 0.5F, List.of(stoneToCoal)),
            new OreCase("two_matching_targets", 17L, new BlockPos(0, 64, 0), 20, 0.0F, List.of(anythingToDiamond, stoneToCoal)),
            new OreCase("size18_sin_table", 21L, new BlockPos(0, 64, 0), 18, 0.0F, List.of(anythingToDiamond)),
            new OreCase("size30_sin_table", 17L, new BlockPos(0, 64, 0), 30, 0.0F, List.of(anythingToDiamond))
        );
    }

    private static void run(
        final OutputStream out, final PalettedContainerFactory containers, final OreCase oreCase
    ) throws IOException {
        OreFeature feature = new OreFeature(
            oreCase.targets().stream().map(Target::replacement).toList(),
            oreCase.size(),
            oreCase.discardChance()
        );
        List<Placement> placements = new ArrayList<>();
        WorldGenLevel level = level(containers, placements);
        RandomSource random = new WorldgenRandom(new XoroshiroRandomSource(oreCase.seed()));
        boolean result = feature.place(level, null, random, oreCase.origin());
        long stateLo = random.nextLong();
        long stateHi = random.nextLong();

        Map<String, Integer> paletteIds = new HashMap<>();
        List<String> palette = new ArrayList<>();
        List<int[]> encoded = new ArrayList<>();
        for (Placement placement : placements) {
            String name = BlockStateParser.serialize(placement.state());
            int id = paletteIds.computeIfAbsent(name, key -> {
                palette.add(key);
                return palette.size() - 1;
            });
            encoded.add(new int[] {placement.pos().getX(), placement.pos().getY(), placement.pos().getZ(), id});
        }

        Bin.str(out, oreCase.name());
        Bin.i64(out, oreCase.seed());
        Bin.i32(out, oreCase.origin().getX());
        Bin.i32(out, oreCase.origin().getY());
        Bin.i32(out, oreCase.origin().getZ());
        Bin.i32(out, oreCase.size());
        Bin.f32(out, oreCase.discardChance());
        Bin.i32(out, oreCase.targets().size());
        for (Target target : oreCase.targets()) {
            Bin.str(out, target.type());
            Bin.str(out, target.block());
            Bin.f32(out, target.probability());
            Bin.str(out, BlockStateParser.serialize(target.replacement().state()));
        }
        Bin.i32(out, result ? 1 : 0);
        Bin.i64(out, stateLo);
        Bin.i64(out, stateHi);
        Bin.i32(out, palette.size());
        for (String name : palette) {
            Bin.str(out, name);
        }
        Bin.i32(out, encoded.size());
        for (int[] entry : encoded) {
            Bin.i32(out, entry[0]);
            Bin.i32(out, entry[1]);
            Bin.i32(out, entry[2]);
            Bin.i32(out, entry[3]);
        }
        System.out.println(
            oreCase.name() + ": placed=" + result + " blocks=" + encoded.size()
                + " state=" + stateLo + "," + stateHi
        );
    }

    private static WorldGenLevel level(
        final PalettedContainerFactory containers, final List<Placement> placements
    ) {
        Map<Long, ProtoChunk> chunks = new HashMap<>();
        InvocationHandler handler = (proxy, method, args) -> {
            Class<?>[] types = method.getParameterTypes();
            switch (method.getName()) {
                case "hashCode" -> {
                    if (types.length == 0) {
                        return System.identityHashCode(proxy);
                    }
                }
                case "equals" -> {
                    if (types.length == 1) {
                        return proxy == args[0];
                    }
                }
                case "toString" -> {
                    if (types.length == 0) {
                        return "OreOracle stub level";
                    }
                }
                case "getMinY" -> {
                    if (types.length == 0) {
                        return MIN_Y;
                    }
                }
                case "getHeight" -> {
                    if (types.length == 0) {
                        return HEIGHT;
                    }
                    if (types.length == 3 && types[0] == Heightmap.Types.class) {
                        return COLUMN_HEIGHT;
                    }
                }
                case "getChunk" -> {
                    if (types.length == 4 && types[2] == ChunkStatus.class) {
                        int chunkX = (Integer)args[0];
                        int chunkZ = (Integer)args[1];
                        return chunks.computeIfAbsent(
                            ChunkPos.pack(chunkX, chunkZ),
                            key -> chunk(containers, placements, chunkX, chunkZ)
                        );
                    }
                }
                default -> {
                }
            }
            if (method.isDefault()) {
                return InvocationHandler.invokeDefault(proxy, method, args);
            }
            throw new UnsupportedOperationException(
                "the stub level does not answer " + method.getDeclaringClass().getSimpleName() + "." + method.getName()
            );
        };
        return (WorldGenLevel)Proxy.newProxyInstance(
            OreOracle.class.getClassLoader(), new Class<?>[] {WorldGenLevel.class}, handler
        );
    }

    private static ProtoChunk chunk(
        final PalettedContainerFactory containers,
        final List<Placement> placements,
        final int chunkX,
        final int chunkZ
    ) {
        int minSectionY = MIN_Y >> 4;
        RecordingSection[] sections = new RecordingSection[HEIGHT >> 4];
        for (int index = 0; index < sections.length; index++) {
            sections[index] = new RecordingSection(containers, placements, chunkX, minSectionY + index, chunkZ);
        }
        ProtoChunk chunk = new ProtoChunk(
            new ChunkPos(chunkX, chunkZ),
            UpgradeData.EMPTY,
            sections,
            new ProtoChunkTicks<Block>(),
            new ProtoChunkTicks<Fluid>(),
            LevelHeightAccessor.create(MIN_Y, HEIGHT),
            containers,
            null
        );
        for (RecordingSection section : sections) {
            section.startRecording();
        }
        return chunk;
    }

    private static final class RecordingSection extends LevelChunkSection {
        private final List<Placement> placements;
        private final int minX;
        private final int minY;
        private final int minZ;
        private boolean recording;

        RecordingSection(
            final PalettedContainerFactory containers,
            final List<Placement> placements,
            final int chunkX,
            final int sectionY,
            final int chunkZ
        ) {
            super(containers);
            this.placements = placements;
            this.minX = chunkX << 4;
            this.minY = sectionY << 4;
            this.minZ = chunkZ << 4;
            for (int x = 0; x < 16; x++) {
                for (int y = 0; y < 16; y++) {
                    for (int z = 0; z < 16; z++) {
                        super.setBlockState(x, y, z, stone);
                    }
                }
            }
        }

        void startRecording() {
            this.recording = true;
        }

        @Override
        public BlockState setBlockState(
            final int sectionX, final int sectionY, final int sectionZ, final BlockState state, final boolean checkThreading
        ) {
            if (this.recording) {
                this.placements.add(
                    new Placement(new BlockPos(this.minX + sectionX, this.minY + sectionY, this.minZ + sectionZ), state)
                );
            }
            return super.setBlockState(sectionX, sectionY, sectionZ, state, checkThreading);
        }
    }
}
