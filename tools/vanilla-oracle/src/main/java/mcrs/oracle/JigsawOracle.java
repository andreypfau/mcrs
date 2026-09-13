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
import net.minecraft.core.Holder;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.levelgen.placement.PlacedFeature;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.levelgen.structure.PoolElementStructurePiece;
import net.minecraft.world.level.levelgen.structure.Structure;
import net.minecraft.world.level.levelgen.structure.StructurePiece;
import net.minecraft.world.level.levelgen.structure.StructureSet;
import net.minecraft.world.level.levelgen.structure.StructureStart;
import net.minecraft.world.level.levelgen.structure.pools.EmptyPoolElement;
import net.minecraft.world.level.levelgen.structure.pools.FeaturePoolElement;
import net.minecraft.world.level.levelgen.structure.pools.JigsawJunction;
import net.minecraft.world.level.levelgen.structure.pools.LegacySinglePoolElement;
import net.minecraft.world.level.levelgen.structure.pools.ListPoolElement;
import net.minecraft.world.level.levelgen.structure.pools.SinglePoolElement;
import net.minecraft.world.level.levelgen.structure.pools.StructurePoolElement;
import net.minecraft.world.level.levelgen.structure.pools.StructureTemplatePool;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager;
import net.minecraft.world.level.storage.LevelStorageSource;

public final class JigsawOracle {
    private static final byte[] MAGIC = "MCJIGSW0".getBytes(StandardCharsets.US_ASCII);
    private static final int DIMENSIONS = 2;
    private static final Field FEATURE;

    static {
        try {
            FEATURE = FeaturePoolElement.class.getDeclaredField("feature");
            FEATURE.setAccessible(true);
        } catch (NoSuchFieldException e) {
            throw new ExceptionInInitializerError(e);
        }
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
            StructureTemplateManager templates = new StructureTemplateManager(
                resources, storage, DataFixers.getDataFixer(), BuiltInRegistries.BLOCK
            );
            List<PlacementOracle.Dim> dims = PlacementOracle.dims(loaded).subList(0, DIMENSIONS);

            int[] totals = new int[3];
            Path layouts = outDir.resolve("structure_layouts.bin");
            try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(layouts))) {
                PlacementOracle.header(out, MAGIC);
                Bin.i32(out, PlacementOracle.SEEDS.length);
                for (long seed : PlacementOracle.SEEDS) {
                    Bin.i64(out, seed);
                    Bin.i32(out, dims.size());
                    for (PlacementOracle.Dim dim : dims) {
                        writeLayouts(out, loaded, templates, dim, PlacementOracle.seedState(loaded, dim, seed), totals);
                    }
                }
            }
            System.out.println(
                "total: " + totals[0] + " cases, " + totals[1] + " present, " + totals[2] + " pieces"
            );
            System.out.println("wrote " + layouts + " (" + Files.size(layouts) + " bytes)");
        }
        System.exit(0);
    }

    private static void writeLayouts(
        final OutputStream out,
        final RegistryAccess.Frozen registries,
        final StructureTemplateManager templates,
        final PlacementOracle.Dim dim,
        final PlacementOracle.SeedState seed,
        final int[] totals
    ) throws Exception {
        Bin.str(out, dim.id());
        List<Holder.Reference<Structure>> jigsaws = PlacementOracle.jigsaws(registries, seed.state());
        Bin.i32(out, jigsaws.size());
        Climate.Sampler climate = PlacementOracle.climate(seed);
        for (Holder.Reference<Structure> holder : jigsaws) {
            Structure structure = holder.value();
            Holder<StructureSet> set = PlacementOracle.singleSet(seed.state(), holder);
            Bin.str(out, PlacementOracle.id(holder));
            Bin.str(out, PlacementOracle.id(set));
            List<ChunkPos> cases = PlacementOracle.caseChunks(seed.state(), set.value().placement());
            Bin.i32(out, cases.size());
            int present = 0;
            int pieces = 0;
            for (ChunkPos chunk : cases) {
                Bin.i32(out, chunk.x());
                Bin.i32(out, chunk.z());
                StructureStart start = structure.generate(
                    holder, dim.level(), registries, dim.generator(), dim.biomeSource(), climate, seed.randomState(),
                    templates, seed.seed(), chunk, 0, dim.heights(), structure.biomes()::contains
                );
                out.write(start.isValid() ? 1 : 0);
                if (!start.isValid()) {
                    continue;
                }
                present++;
                pieces += start.getPieces().size();
                box(out, start.getBoundingBox());
                Bin.i32(out, start.getPieces().size());
                for (StructurePiece piece : start.getPieces()) {
                    writePiece(out, (PoolElementStructurePiece) piece);
                }
            }
            totals[0] += cases.size();
            totals[1] += present;
            totals[2] += pieces;
            System.out.println(
                "  layouts " + dim.id() + " seed " + seed.seed() + " " + PlacementOracle.id(holder) + ": " + cases.size()
                    + " cases, " + present + " present, " + pieces + " pieces"
            );
        }
    }

    private static void writePiece(final OutputStream out, final PoolElementStructurePiece piece) throws Exception {
        Bin.str(out, render(piece.getElement()));
        out.write(projection(piece.getElement().getProjection()));
        BlockPos position = piece.getPosition();
        Bin.i32(out, position.getX());
        Bin.i32(out, position.getY());
        Bin.i32(out, position.getZ());
        out.write(switch (piece.getRotation()) {
            case NONE -> 0;
            case CLOCKWISE_90 -> 1;
            case CLOCKWISE_180 -> 2;
            case COUNTERCLOCKWISE_90 -> 3;
        });
        box(out, piece.getBoundingBox());
        Bin.i32(out, piece.getGroundLevelDelta());
        Bin.i32(out, piece.getJunctions().size());
        for (JigsawJunction junction : piece.getJunctions()) {
            Bin.i32(out, junction.getSourceX());
            Bin.i32(out, junction.getSourceGroundY());
            Bin.i32(out, junction.getSourceZ());
            Bin.i32(out, junction.getDeltaY());
            out.write(projection(junction.getDestProjection()));
        }
    }

    private static int projection(final StructureTemplatePool.Projection projection) {
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

    @SuppressWarnings("unchecked")
    private static String render(final StructurePoolElement element) throws Exception {
        return switch (element) {
            case LegacySinglePoolElement legacy -> "legacy:" + legacy.getTemplateLocation();
            case SinglePoolElement single -> "single:" + single.getTemplateLocation();
            case FeaturePoolElement feature -> "feature:" + PlacementOracle.id((Holder<PlacedFeature>) FEATURE.get(feature));
            case ListPoolElement list -> {
                StringBuilder joined = new StringBuilder("list[");
                for (int i = 0; i < list.getElements().size(); i++) {
                    if (i > 0) {
                        joined.append(',');
                    }
                    joined.append(render(list.getElements().get(i)));
                }
                yield joined.append(']').toString();
            }
            case EmptyPoolElement empty -> "empty";
            default -> throw new IllegalStateException("unknown pool element " + element);
        };
    }
}
