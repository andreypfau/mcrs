package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.BitSet;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.SharedConstants;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.Mth;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;
import net.minecraft.world.level.levelgen.feature.BlockReplacement;
import net.minecraft.world.level.levelgen.feature.OreFeature;
import net.minecraft.world.level.levelgen.structure.templatesystem.AlwaysTrueTest;
import net.minecraft.world.level.levelgen.structure.templatesystem.BlockMatchTest;
import net.minecraft.world.level.levelgen.structure.templatesystem.RandomBlockMatchTest;
import net.minecraft.world.level.levelgen.structure.templatesystem.RuleTest;

/**
 * Runs `OreFeature.place` / `doPlace` over a world that is stone everywhere and
 * records every block the vein writes, in write order, together with the random
 * state the call leaves behind.
 */
public final class OreOracle {
    private static final byte[] MAGIC = "MCOREVN0".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;
    private static final int MIN_Y = -64;
    private static final int MAX_Y = 320;

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

        List<OreCase> cases = cases();
        Path file = outDir.resolve("ore_vein.bin");
        try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
            out.write(MAGIC);
            Bin.i32(out, FORMAT_VERSION);
            Bin.i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
            Bin.i32(out, cases.size());
            for (OreCase oreCase : cases) {
                run(out, oreCase);
            }
        }
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

    private static void run(final OutputStream out, final OreCase oreCase) throws IOException {
        OreFeature feature = new OreFeature(
            oreCase.targets().stream().map(Target::replacement).toList(),
            oreCase.size(),
            oreCase.discardChance()
        );
        Map<BlockPos, BlockState> world = new HashMap<>();
        List<Placement> placements = new ArrayList<>();
        RandomSource random = new XoroshiroRandomSource(oreCase.seed());
        boolean result = place(feature, world, placements, random, oreCase.origin());
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

    private static BlockState getBlockState(final Map<BlockPos, BlockState> world, final BlockPos pos) {
        if (isOutsideBuildHeight(pos.getY())) {
            return air;
        }
        BlockState state = world.get(pos);
        return state == null ? stone : state;
    }

    private static boolean isOutsideBuildHeight(final int y) {
        return y < MIN_Y || y >= MAX_Y;
    }

    private static boolean place(
        final OreFeature feature,
        final Map<BlockPos, BlockState> world,
        final List<Placement> placements,
        final RandomSource random,
        final BlockPos origin
    ) {
        float dir = random.nextFloat() * (float)Math.PI;
        float spreadXY = feature.size() / 8.0F;
        int maxRadius = Mth.ceil((feature.size() / 16.0F * 2.0F + 1.0F) / 2.0F);
        double x0 = origin.getX() + Math.sin(dir) * spreadXY;
        double x1 = origin.getX() - Math.sin(dir) * spreadXY;
        double z0 = origin.getZ() + Math.cos(dir) * spreadXY;
        double z1 = origin.getZ() - Math.cos(dir) * spreadXY;
        double y0 = origin.getY() + random.nextInt(3) - 2;
        double y1 = origin.getY() + random.nextInt(3) - 2;
        int xStart = origin.getX() - Mth.ceil(spreadXY) - maxRadius;
        int yStart = origin.getY() - 2 - maxRadius;
        int zStart = origin.getZ() - Mth.ceil(spreadXY) - maxRadius;
        int sizeXZ = 2 * (Mth.ceil(spreadXY) + maxRadius);
        int sizeY = 2 * (2 + maxRadius);

        for (int xprobe = xStart; xprobe <= xStart + sizeXZ; xprobe++) {
            for (int zprobe = zStart; zprobe <= zStart + sizeXZ; zprobe++) {
                if (yStart <= MAX_Y) {
                    return doPlace(
                        feature, world, placements, random,
                        x0, x1, z0, z1, y0, y1, xStart, yStart, zStart, sizeXZ, sizeY
                    );
                }
            }
        }

        return false;
    }

    private static boolean doPlace(
        final OreFeature feature,
        final Map<BlockPos, BlockState> world,
        final List<Placement> placements,
        final RandomSource random,
        final double x0,
        final double x1,
        final double z0,
        final double z1,
        final double y0,
        final double y1,
        final int xStart,
        final int yStart,
        final int zStart,
        final int sizeXZ,
        final int sizeY
    ) {
        int size = feature.size();
        int placed = 0;
        BitSet tested = new BitSet(sizeXZ * sizeY * sizeXZ);
        BlockPos.MutableBlockPos orePos = new BlockPos.MutableBlockPos();
        double[] data = new double[size * 4];

        for (int i = 0; i < size; i++) {
            float step = (float)i / size;
            double xx = Mth.lerp((double)step, x0, x1);
            double yy = Mth.lerp((double)step, y0, y1);
            double zz = Mth.lerp((double)step, z0, z1);
            double ss = random.nextDouble() * size / 16.0;
            double r = ((Mth.sin((float)Math.PI * step) + 1.0F) * ss + 1.0) / 2.0;
            data[i * 4 + 0] = xx;
            data[i * 4 + 1] = yy;
            data[i * 4 + 2] = zz;
            data[i * 4 + 3] = r;
        }

        for (int i1 = 0; i1 < size - 1; i1++) {
            if (!(data[i1 * 4 + 3] <= 0.0)) {
                for (int i2 = i1 + 1; i2 < size; i2++) {
                    if (!(data[i2 * 4 + 3] <= 0.0)) {
                        double dx = data[i1 * 4 + 0] - data[i2 * 4 + 0];
                        double dy = data[i1 * 4 + 1] - data[i2 * 4 + 1];
                        double dz = data[i1 * 4 + 2] - data[i2 * 4 + 2];
                        double dr = data[i1 * 4 + 3] - data[i2 * 4 + 3];
                        if (dr * dr > dx * dx + dy * dy + dz * dz) {
                            if (dr > 0.0) {
                                data[i2 * 4 + 3] = -1.0;
                            } else {
                                data[i1 * 4 + 3] = -1.0;
                            }
                        }
                    }
                }
            }
        }

        for (int i = 0; i < size; i++) {
            double r = data[i * 4 + 3];
            if (!(r < 0.0)) {
                double xx = data[i * 4 + 0];
                double yy = data[i * 4 + 1];
                double zz = data[i * 4 + 2];
                int xMin = Math.max(Mth.floor(xx - r), xStart);
                int yMin = Math.max(Mth.floor(yy - r), yStart);
                int zMin = Math.max(Mth.floor(zz - r), zStart);
                int xMax = Math.max(Mth.floor(xx + r), xMin);
                int yMax = Math.max(Mth.floor(yy + r), yMin);
                int zMax = Math.max(Mth.floor(zz + r), zMin);

                for (int x = xMin; x <= xMax; x++) {
                    double xd = (x + 0.5 - xx) / r;
                    if (xd * xd < 1.0) {
                        for (int y = yMin; y <= yMax; y++) {
                            double yd = (y + 0.5 - yy) / r;
                            if (xd * xd + yd * yd < 1.0) {
                                for (int z = zMin; z <= zMax; z++) {
                                    double zd = (z + 0.5 - zz) / r;
                                    if (xd * xd + yd * yd + zd * zd < 1.0 && !isOutsideBuildHeight(y)) {
                                        int bitSetIndex = x
                                            - xStart
                                            + (y - yStart) * sizeXZ
                                            + (z - zStart) * sizeXZ * sizeY;
                                        if (!tested.get(bitSetIndex)) {
                                            tested.set(bitSetIndex);
                                            orePos.set(x, y, z);
                                            BlockState blockState = getBlockState(world, orePos);

                                            for (BlockReplacement targetState : feature.targetStates()) {
                                                if (feature.canPlaceOre(
                                                    blockState,
                                                    pos -> getBlockState(world, pos),
                                                    random,
                                                    targetState,
                                                    orePos
                                                )) {
                                                    BlockPos at = orePos.immutable();
                                                    world.put(at, targetState.state());
                                                    placements.add(new Placement(at, targetState.state()));
                                                    placed++;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        return placed > 0;
    }
}
