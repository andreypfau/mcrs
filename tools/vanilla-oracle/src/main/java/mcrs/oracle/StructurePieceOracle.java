package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.NbtIo;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.biome.Climate;
import net.minecraft.world.level.levelgen.structure.BoundingBox;
import net.minecraft.world.level.levelgen.structure.Structure;
import net.minecraft.world.level.levelgen.structure.StructurePiece;
import net.minecraft.world.level.levelgen.structure.StructureSet;
import net.minecraft.world.level.levelgen.structure.StructureStart;
import net.minecraft.world.level.levelgen.structure.pieces.StructurePieceSerializationContext;
import net.minecraft.world.level.levelgen.structure.structures.JigsawStructure;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplateManager;
import net.minecraft.world.level.storage.LevelStorageSource;

/**
 * Every piece of every start the reference builds, in the form the save
 * carries: `StructurePiece.createTag`, type-specific fields included.
 */
public final class StructurePieceOracle {
    private static final byte[] MAGIC = "MCSTRPC0".getBytes(StandardCharsets.US_ASCII);

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
            StructureTemplateManager templates = new StructureTemplateManager(
                resources, storage, DataFixers.getDataFixer(), BuiltInRegistries.BLOCK
            );
            StructurePieceSerializationContext context = new StructurePieceSerializationContext(resources, access, templates);
            List<PlacementOracle.Dim> dims = PlacementOracle.dims(loaded);

            ByteArrayOutputStream body = new ByteArrayOutputStream();
            int[] totals = new int[3];
            Set<String> jigsawDone = new HashSet<>();
            for (long seed : PlacementOracle.SEEDS) {
                for (PlacementOracle.Dim dim : dims) {
                    PlacementOracle.SeedState state = PlacementOracle.seedState(loaded, dim, seed);
                    writeStarts(body, loaded, access, templates, context, dim, state, jigsawDone, totals);
                }
            }
            Path file = outDir.resolve("structure_pieces.bin");
            try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
                PlacementOracle.header(out, MAGIC);
                Bin.i32(out, totals[0]);
                body.writeTo(out);
            }
            System.out.println("total: " + totals[0] + " cases, " + totals[1] + " present, " + totals[2] + " pieces");
            System.out.println("wrote " + file + " (" + Files.size(file) + " bytes)");
        }
        System.exit(0);
    }

    /// Every structure with a placement in the dimension that is not a jigsaw
    /// structure, at the same sixteen case chunks the site dump uses; a jigsaw
    /// structure contributes its first present case over all seeds only, as
    /// the parity check of the one piece codec that exists before any other
    /// type is ported.
    private static void writeStarts(
        final OutputStream out,
        final RegistryAccess.Frozen registries,
        final RegistryAccess access,
        final StructureTemplateManager templates,
        final StructurePieceSerializationContext context,
        final PlacementOracle.Dim dim,
        final PlacementOracle.SeedState seed,
        final Set<String> jigsawDone,
        final int[] totals
    ) throws IOException {
        Climate.Sampler climate = PlacementOracle.climate(seed);
        List<Holder.Reference<Structure>> structures = registries.lookupOrThrow(Registries.STRUCTURE)
            .listElements()
            .filter(holder -> !seed.state().getPlacementsForStructure(holder).isEmpty())
            .toList();
        for (Holder.Reference<Structure> holder : structures) {
            Structure structure = holder.value();
            boolean jigsaw = structure instanceof JigsawStructure;
            if (jigsaw && jigsawDone.contains(PlacementOracle.id(holder))) {
                continue;
            }
            Holder<StructureSet> set = PlacementOracle.singleSet(seed.state(), holder);
            int present = 0;
            int pieces = 0;
            int cases = 0;
            for (ChunkPos chunk : PlacementOracle.caseChunks(seed.state(), set.value().placement())) {
                StructureStart start = structure.generate(
                    holder, dim.level(), access, dim.generator(), dim.biomeSource(), climate, seed.randomState(),
                    templates, seed.seed(), chunk, 0, dim.heights(), structure.biomes()::contains
                );
                if (jigsaw && !start.isValid()) {
                    continue;
                }
                cases++;
                Bin.i64(out, seed.seed());
                Bin.str(out, dim.id());
                Bin.str(out, PlacementOracle.id(holder));
                Bin.i32(out, chunk.x());
                Bin.i32(out, chunk.z());
                out.write(start.isValid() ? 1 : 0);
                if (start.isValid()) {
                    present++;
                    pieces += start.getPieces().size();
                    box(out, start.getBoundingBox());
                    Bin.i32(out, start.getPieces().size());
                    for (StructurePiece piece : start.getPieces()) {
                        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
                        NbtIo.write(piece.createTag(context), new DataOutputStream(bytes));
                        Bin.i32(out, bytes.size());
                        bytes.writeTo(out);
                    }
                }
                if (jigsaw) {
                    jigsawDone.add(PlacementOracle.id(holder));
                    break;
                }
            }
            totals[0] += cases;
            totals[1] += present;
            totals[2] += pieces;
            System.out.println(
                "  pieces " + dim.id() + " seed " + seed.seed() + " " + PlacementOracle.id(holder) + ": " + cases
                    + " cases, " + present + " present, " + pieces + " pieces"
            );
        }
    }

    private static void box(final OutputStream out, final BoundingBox box) throws IOException {
        Bin.i32(out, box.minX());
        Bin.i32(out, box.minY());
        Bin.i32(out, box.minZ());
        Bin.i32(out, box.maxX());
        Bin.i32(out, box.maxY());
        Bin.i32(out, box.maxZ());
    }
}
