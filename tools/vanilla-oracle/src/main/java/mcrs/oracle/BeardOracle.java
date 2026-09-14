package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.levelgen.Beardifier;
import net.minecraft.world.level.levelgen.densityfunction.DensityBuffer;
import net.minecraft.world.level.levelgen.densityfunction.DensityVolume;
import net.minecraft.world.level.levelgen.densityfunction.SamplerContext;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.levelgen.structure.TerrainAdjustment;
import net.minecraft.world.level.levelgen.structure.pools.JigsawJunction;
import net.minecraft.world.level.levelgen.structure.pools.StructureTemplatePool;
import org.jspecify.annotations.Nullable;

public final class BeardOracle {
    private static final byte[] MAGIC = "MCBEARD0".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;
    private static final int KERNEL_LEN = 24 * 24 * 24;

    private record BeardCase(
        String name,
        List<Beardifier.Rigid> rigids,
        List<JigsawJunction> junctions,
        @Nullable BoundingBox affectedBox,
        DensityVolume volume
    ) {}

    public static void main(final String[] args) throws Exception {
        Path outDir = Path.of(args[0]);
        Files.createDirectories(outDir);

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        float[] kernel = kernel();
        List<BeardCase> cases = cases();
        Path file = outDir.resolve("beard.bin");
        try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
            out.write(MAGIC);
            Bin.i32(out, FORMAT_VERSION);
            Bin.i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
            Bin.i32(out, kernel.length);
            for (float value : kernel) {
                Bin.f32(out, value);
            }
            Bin.i32(out, cases.size());
            for (BeardCase beardCase : cases) {
                run(out, beardCase);
            }
        }
        System.out.println("wrote " + file + " (" + Files.size(file) + " bytes)");
    }

    private static float[] kernel() throws ReflectiveOperationException {
        Field field = Beardifier.class.getDeclaredField("BEARD_KERNEL");
        field.setAccessible(true);
        float[] kernel = (float[])field.get(null);
        if (kernel.length != KERNEL_LEN) {
            throw new IllegalStateException("BEARD_KERNEL has " + kernel.length + " entries, expected " + KERNEL_LEN);
        }
        return kernel;
    }

    private static DensityVolume chunk(final int chunkX, final int chunkZ) {
        return new DensityVolume(16, 384, 16, chunkX * 16, -64, chunkZ * 16, 1, 1, 1);
    }

    private static DensityVolume lattice(final int chunkX, final int chunkZ) {
        return new DensityVolume(5, 49, 5, chunkX * 16, -64, chunkZ * 16, 4, 8, 4);
    }

    private static Beardifier.Rigid rigid(
        final int minX,
        final int minY,
        final int minZ,
        final int maxX,
        final int maxY,
        final int maxZ,
        final TerrainAdjustment adjustment,
        final int groundLevelDelta
    ) {
        return new Beardifier.Rigid(new BoundingBox(minX, minY, minZ, maxX, maxY, maxZ), adjustment, groundLevelDelta);
    }

    private static JigsawJunction junction(
        final int x, final int groundY, final int z, final int deltaY, final StructureTemplatePool.Projection projection
    ) {
        return new JigsawJunction(x, groundY, z, deltaY, projection);
    }

    private static BeardCase synthetic(
        final String name,
        final List<Beardifier.Rigid> rigids,
        final List<JigsawJunction> junctions,
        final DensityVolume volume
    ) {
        BoundingBox union = null;
        for (Beardifier.Rigid rigid : rigids) {
            union = union == null ? rigid.box() : BoundingBox.encapsulating(union, rigid.box());
        }
        for (JigsawJunction junction : junctions) {
            BoundingBox point = new BoundingBox(
                new BlockPos(junction.getSourceX(), junction.getSourceGroundY(), junction.getSourceZ())
            );
            union = union == null ? point : BoundingBox.encapsulating(union, point);
        }
        if (union == null) {
            throw new IllegalStateException(name + ": a synthetic case needs at least one rigid or junction");
        }
        return new BeardCase(name, rigids, junctions, union.inflatedBy(24), volume);
    }

    private static List<BeardCase> cases() {
        StructureTemplatePool.Projection rigidProjection = StructureTemplatePool.Projection.RIGID;
        StructureTemplatePool.Projection terrainMatching = StructureTemplatePool.Projection.TERRAIN_MATCHING;
        List<Beardifier.Rigid> negativeRigids = List.of(
            rigid(-38, 70, -22, -27, 80, -12, TerrainAdjustment.BEARD_BOX, 2),
            rigid(-55, 66, -36, -44, 71, -28, TerrainAdjustment.BURY, 1)
        );
        List<JigsawJunction> negativeJunctions = List.of(
            junction(-35, 72, -18, 1, terrainMatching),
            junction(-47, 67, -30, 0, rigidProjection)
        );
        return List.of(
            synthetic(
                "adjustment_none",
                List.of(rigid(9, 60, 4, 21, 72, 11, TerrainAdjustment.NONE, 1)),
                List.of(),
                chunk(0, 0)
            ),
            synthetic(
                "adjustment_bury",
                List.of(rigid(10, 300, -3, 20, 312, 6, TerrainAdjustment.BURY, 2)),
                List.of(),
                chunk(0, 0)
            ),
            synthetic(
                "adjustment_beard_thin",
                List.of(rigid(12, 64, 9, 24, 75, 18, TerrainAdjustment.BEARD_THIN, 3)),
                List.of(),
                chunk(0, 0)
            ),
            synthetic(
                "adjustment_beard_box",
                List.of(rigid(-6, 58, 11, 5, 70, 22, TerrainAdjustment.BEARD_BOX, 4)),
                List.of(),
                chunk(0, 0)
            ),
            synthetic(
                "adjustment_encapsulate",
                List.of(rigid(4, -60, -8, 13, -44, 3, TerrainAdjustment.ENCAPSULATE, -2)),
                List.of(),
                chunk(0, 0)
            ),
            synthetic(
                "junctions_only",
                List.of(),
                List.of(
                    junction(3, 65, 14, 1, rigidProjection),
                    junction(15, 70, -2, -1, terrainMatching),
                    junction(-4, 62, 8, 0, terrainMatching),
                    junction(20, 68, 20, 2, rigidProjection)
                ),
                chunk(0, 0)
            ),
            synthetic(
                "village",
                List.of(
                    rigid(36, 63, 20, 44, 70, 28, TerrainAdjustment.BEARD_THIN, 1),
                    rigid(26, 64, 14, 33, 72, 21, TerrainAdjustment.BEARD_THIN, 1),
                    rigid(45, 66, 27, 52, 75, 34, TerrainAdjustment.BEARD_THIN, 2),
                    rigid(30, 58, 29, 38, 64, 40, TerrainAdjustment.BURY, 3),
                    rigid(40, 61, 10, 49, 68, 17, TerrainAdjustment.BEARD_BOX, 1),
                    rigid(22, 50, 25, 29, 62, 33, TerrainAdjustment.ENCAPSULATE, 1),
                    rigid(48, 67, 15, 55, 73, 22, TerrainAdjustment.NONE, 1)
                ),
                List.of(
                    junction(35, 64, 24, 0, terrainMatching),
                    junction(40, 71, 29, 1, rigidProjection),
                    junction(33, 63, 18, -1, terrainMatching),
                    junction(46, 67, 26, 2, rigidProjection),
                    junction(29, 69, 31, 0, terrainMatching),
                    junction(50, 72, 19, -2, terrainMatching),
                    junction(42, 60, 13, 3, rigidProjection)
                ),
                chunk(2, 1)
            ),
            synthetic(
                "affected_box_misses",
                List.of(rigid(200, 64, 200, 210, 72, 210, TerrainAdjustment.BEARD_THIN, 1)),
                List.of(junction(190, 66, 195, 0, terrainMatching)),
                chunk(0, 0)
            ),
            new BeardCase("empty", List.of(), List.of(), null, chunk(0, 0)),
            synthetic(
                "stepped_lattice",
                List.of(rigid(30, 17, 10, 40, 29, 14, TerrainAdjustment.BEARD_THIN, 2)),
                List.of(junction(34, 25, 12, 0, rigidProjection)),
                lattice(1, 0)
            ),
            synthetic("negative_chunk", negativeRigids, negativeJunctions, chunk(-3, -2)),
            synthetic("negative_stepped_lattice", negativeRigids, negativeJunctions, lattice(-3, -2))
        );
    }

    private static int adjustmentCode(final TerrainAdjustment adjustment) {
        return switch (adjustment) {
            case NONE -> 0;
            case BURY -> 1;
            case BEARD_THIN -> 2;
            case BEARD_BOX -> 3;
            case ENCAPSULATE -> 4;
        };
    }

    private static int projectionCode(final StructureTemplatePool.Projection projection) {
        return switch (projection) {
            case RIGID -> 0;
            case TERRAIN_MATCHING -> 1;
        };
    }

    private static void box(final OutputStream out, final BoundingBox box) throws IOException {
        Bin.i32(out, box.minX());
        Bin.i32(out, box.minY());
        Bin.i32(out, box.minZ());
        Bin.i32(out, box.maxX());
        Bin.i32(out, box.maxY());
        Bin.i32(out, box.maxZ());
    }

    private static void run(final OutputStream out, final BeardCase beardCase) throws IOException {
        Beardifier beardifier = beardCase.affectedBox() == null
            ? Beardifier.EMPTY
            : new Beardifier(beardCase.rigids(), beardCase.junctions(), beardCase.affectedBox());
        DensityVolume volume = beardCase.volume();
        SamplerContext context = SamplerContext.builder().enableCaches().build();
        DensityBuffer buffer = DensityBuffer.createUnpooled(volume.size());
        beardifier.sampleVolume(context, buffer, volume);

        Bin.str(out, beardCase.name());
        Bin.i32(out, beardCase.rigids().size());
        for (Beardifier.Rigid rigid : beardCase.rigids()) {
            box(out, rigid.box());
            out.write(adjustmentCode(rigid.terrainAdjustment()));
            Bin.i32(out, rigid.groundLevelDelta());
        }
        Bin.i32(out, beardCase.junctions().size());
        for (JigsawJunction junction : beardCase.junctions()) {
            Bin.i32(out, junction.getSourceX());
            Bin.i32(out, junction.getSourceGroundY());
            Bin.i32(out, junction.getSourceZ());
            Bin.i32(out, junction.getDeltaY());
            out.write(projectionCode(junction.getDestProjection()));
        }
        if (beardCase.affectedBox() == null) {
            out.write(0);
        } else {
            out.write(1);
            box(out, beardCase.affectedBox());
        }
        Bin.i32(out, volume.minBlockX());
        Bin.i32(out, volume.minBlockY());
        Bin.i32(out, volume.minBlockZ());
        Bin.i32(out, volume.sizeX());
        Bin.i32(out, volume.sizeY());
        Bin.i32(out, volume.sizeZ());
        Bin.i32(out, volume.stepBlockX());
        Bin.i32(out, volume.stepBlockY());
        Bin.i32(out, volume.stepBlockZ());

        int nonZero = 0;
        float min = Float.POSITIVE_INFINITY;
        float max = Float.NEGATIVE_INFINITY;
        for (int z = 0; z < volume.sizeZ(); z++) {
            for (int x = 0; x < volume.sizeX(); x++) {
                for (int y = 0; y < volume.sizeY(); y++) {
                    float value = buffer.get(volume.indexUnchecked(x, y, z));
                    int blockX = volume.blockX(x);
                    int blockY = volume.blockY(y);
                    int blockZ = volume.blockZ(z);
                    float single = beardifier.sampleValue(context, blockX, blockY, blockZ);
                    if (Float.floatToRawIntBits(single) != Float.floatToRawIntBits(value)) {
                        throw new IllegalStateException(
                            beardCase.name() + ": sampleValue(" + blockX + ", " + blockY + ", " + blockZ + ") = "
                                + single + " but sampleVolume wrote " + value
                        );
                    }
                    Bin.f32(out, value);
                    if (value != 0.0F) {
                        nonZero++;
                    }
                    min = Math.min(min, value);
                    max = Math.max(max, value);
                }
            }
        }
        System.out.println(
            beardCase.name() + ": rigids=" + beardCase.rigids().size() + " junctions=" + beardCase.junctions().size()
                + " volume=" + volume.sizeX() + "x" + volume.sizeY() + "x" + volume.sizeZ()
                + " step=" + volume.stepBlockX() + "x" + volume.stepBlockY() + "x" + volume.stepBlockZ()
                + " nonzero=" + nonZero + " min=" + min + " max=" + max
        );
    }
}
