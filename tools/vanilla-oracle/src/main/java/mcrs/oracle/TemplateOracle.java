package mcrs.oracle;

import java.io.BufferedOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import net.minecraft.SharedConstants;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.NbtUtils;
import net.minecraft.resources.FileToIdConverter;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.JigsawBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.levelgen.structure.templatesystem.StructureTemplate;
import net.minecraft.world.level.levelgen.structure.templatesystem.loader.ResourceManagerTemplateSource;

public final class TemplateOracle {
    private static final byte[] MAGIC = "MCTMPLT0".getBytes(StandardCharsets.US_ASCII);
    private static final int FORMAT_VERSION = 1;
    private static final int EXPECTED_TEMPLATE_COUNT = 1511;
    private static final FileToIdConverter STRUCTURE_FILES = new FileToIdConverter("structure", ".nbt");

    private static final List<String> LISTED = List.of(
        "minecraft:abandoned_camp/camp/snowy_taiga/campsite_snowy_taiga_4",
        "minecraft:ancient_city/city_center/city_center_1",
        "minecraft:bastion/units/center_pieces/center_0",
        "minecraft:empty",
        "minecraft:pillager_outpost/watchtower",
        "minecraft:shipwreck/rightsideup_backhalf",
        "minecraft:shipwreck/rightsideup_backhalf_degraded",
        "minecraft:shipwreck/rightsideup_fronthalf",
        "minecraft:shipwreck/rightsideup_fronthalf_degraded",
        "minecraft:shipwreck/rightsideup_full",
        "minecraft:shipwreck/rightsideup_full_degraded",
        "minecraft:shipwreck/sideways_backhalf",
        "minecraft:shipwreck/sideways_backhalf_degraded",
        "minecraft:shipwreck/sideways_fronthalf",
        "minecraft:shipwreck/sideways_fronthalf_degraded",
        "minecraft:shipwreck/sideways_full",
        "minecraft:shipwreck/sideways_full_degraded",
        "minecraft:shipwreck/upsidedown_backhalf",
        "minecraft:shipwreck/upsidedown_backhalf_degraded",
        "minecraft:shipwreck/upsidedown_fronthalf",
        "minecraft:shipwreck/upsidedown_fronthalf_degraded",
        "minecraft:shipwreck/upsidedown_full",
        "minecraft:shipwreck/upsidedown_full_degraded",
        "minecraft:shipwreck/with_mast",
        "minecraft:shipwreck/with_mast_degraded",
        "minecraft:spring/sulfur_spring_medium_1",
        "minecraft:trail_ruins/tower/tower_top_1",
        "minecraft:trial_chambers/chamber/chamber_2",
        "minecraft:trial_chambers/corridor/entrance_1",
        "minecraft:trial_chambers/hallway/encounter_4",
        "minecraft:village/desert/houses/desert_small_house_1",
        "minecraft:village/plains/houses/plains_small_house_1",
        "minecraft:village/savanna/savanna_lamp_post_01"
    );

    private record Loaded(Identifier id, StructureTemplate template, List<StructureTemplate.Palette> palettes, List<ListTag> rawPalettes) {}

    public static void main(final String[] args) throws Exception {
        Path outDir = Path.of(args[0]);
        Files.createDirectories(outDir);

        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        Field palettesField = StructureTemplate.class.getDeclaredField("palettes");
        palettesField.setAccessible(true);

        List<Loaded> loaded = new ArrayList<>();
        try (MultiPackResourceManager manager = new MultiPackResourceManager(
            PackType.SERVER_DATA, List.of(ServerPacksSource.createVanillaPackSource().fullResources())
        )) {
            ResourceManagerTemplateSource source = new ResourceManagerTemplateSource(
                DataFixers.getDataFixer(), BuiltInRegistries.BLOCK, manager, STRUCTURE_FILES
            );
            List<Identifier> ids = source.list().sorted(Comparator.comparing(Identifier::toString)).toList();
            if (ids.size() != EXPECTED_TEMPLATE_COUNT) {
                throw new IllegalStateException("expected " + EXPECTED_TEMPLATE_COUNT + " templates, listed " + ids.size());
            }
            for (Identifier id : ids) {
                StructureTemplate template = source.load(id)
                    .orElseThrow(() -> new IllegalStateException("template " + id + " did not load"));
                @SuppressWarnings("unchecked")
                List<StructureTemplate.Palette> palettes = (List<StructureTemplate.Palette>) palettesField.get(template);
                List<ListTag> rawPalettes = rawPalettes(manager.open(STRUCTURE_FILES.idToFile(id)));
                if (rawPalettes.size() != palettes.size()) {
                    throw new IllegalStateException(id + ": file has " + rawPalettes.size() + " palettes, template loaded " + palettes.size());
                }
                loaded.add(new Loaded(id, template, palettes, rawPalettes));
            }
        }

        Map<String, Loaded> byId = new HashMap<>();
        for (Loaded entry : loaded) {
            byId.put(entry.id().toString(), entry);
        }
        for (String id : LISTED) {
            if (!byId.containsKey(id)) {
                throw new IllegalStateException("listed template " + id + " is not in the corpus");
            }
        }

        List<String> dynamic = new ArrayList<>();
        for (Block block : BuiltInRegistries.BLOCK) {
            if (block.hasDynamicShape()) {
                dynamic.add(BuiltInRegistries.BLOCK.getKey(block).toString());
            }
        }

        Path file = outDir.resolve("templates.bin");
        try (OutputStream out = new BufferedOutputStream(Files.newOutputStream(file))) {
            out.write(MAGIC);
            Bin.i32(out, FORMAT_VERSION);
            Bin.i32(out, SharedConstants.getCurrentVersion().dataVersion().version());
            Bin.i32(out, dynamic.size());
            for (String id : dynamic) {
                Bin.str(out, id);
            }

            Bin.i32(out, loaded.size());
            for (Loaded entry : loaded) {
                writeManifest(out, entry);
            }

            Bin.i32(out, LISTED.size());
            for (String id : LISTED) {
                Loaded entry = byId.get(id);
                Bin.str(out, id);
                Bin.i32(out, entry.palettes().size());
                for (StructureTemplate.Palette palette : entry.palettes()) {
                    writeBlocks(out, palette);
                }
            }
        }
        System.out.println(
            "wrote " + file + " (" + Files.size(file) + " bytes, " + loaded.size() + " templates, "
                + LISTED.size() + " listed, " + dynamic.size() + " dynamic-shape blocks)"
        );
    }

    private static List<ListTag> rawPalettes(final InputStream in) throws IOException {
        CompoundTag tag;
        try (in) {
            tag = NbtIo.readCompressed(in, NbtAccounter.unlimitedHeap());
        }
        List<ListTag> palettes = new ArrayList<>();
        ListTag many = tag.getList("palettes").orElse(null);
        if (many != null) {
            for (int p = 0; p < many.size(); p++) {
                palettes.add(many.getListOrEmpty(p));
            }
        } else {
            palettes.add(tag.getListOrEmpty("palette"));
        }
        return palettes;
    }

    private static boolean isFullBlock(final BlockState state) {
        return !state.getBlock().hasDynamicShape()
            && state.isCollisionShapeFullBlock(EmptyBlockGetter.INSTANCE, BlockPos.ZERO);
    }

    private static void writeManifest(final OutputStream out, final Loaded entry) throws IOException {
        Bin.str(out, entry.id().toString());
        Bin.i32(out, entry.template().getSize().getX());
        Bin.i32(out, entry.template().getSize().getY());
        Bin.i32(out, entry.template().getSize().getZ());
        Bin.i32(out, entry.palettes().size());
        for (int p = 0; p < entry.palettes().size(); p++) {
            writePalette(out, entry.palettes().get(p), entry.rawPalettes().get(p));
        }
    }

    private static void writePalette(final OutputStream out, final StructureTemplate.Palette palette, final ListTag rawEntries) throws IOException {
        int full = 0;
        int other = 0;
        int entity = 0;
        for (StructureTemplate.StructureBlockInfo block : palette.blocks()) {
            if (block.nbt() != null) {
                entity++;
            } else if (isFullBlock(block.state())) {
                full++;
            } else {
                other++;
            }
        }
        Bin.i32(out, full);
        Bin.i32(out, other);
        Bin.i32(out, entity);

        Bin.i32(out, rawEntries.size());
        for (int i = 0; i < rawEntries.size(); i++) {
            Bin.str(out, BlockStateParser.serialize(NbtUtils.readBlockState(BuiltInRegistries.BLOCK, rawEntries.getCompoundOrEmpty(i))));
        }

        List<StructureTemplate.JigsawBlockInfo> jigsaws = palette.jigsaws();
        List<StructureTemplate.StructureBlockInfo> jigsawBlocks = palette.blocks(Blocks.JIGSAW);
        Bin.i32(out, jigsaws.size());
        for (int i = 0; i < jigsaws.size(); i++) {
            StructureTemplate.JigsawBlockInfo jigsaw = jigsaws.get(i);
            Bin.i32(out, jigsaw.pos().getX());
            Bin.i32(out, jigsaw.pos().getY());
            Bin.i32(out, jigsaw.pos().getZ());
            Bin.str(out, BlockStateParser.serialize(jigsaw.state()));
            Bin.str(out, JigsawBlock.getFrontFacing(jigsaw.state()).getSerializedName());
            Bin.str(out, JigsawBlock.getTopFacing(jigsaw.state()).getSerializedName());
            Bin.str(out, jigsaw.jointType().getSerializedName());
            Bin.str(out, jigsaw.name().toString());
            Bin.str(out, jigsaw.pool().identifier().toString());
            Bin.str(out, jigsaw.target().toString());
            Bin.i32(out, jigsaw.placementPriority());
            Bin.i32(out, jigsaw.selectionPriority());
            String rawState = jigsawBlocks.get(i).nbt().getStringOr("final_state", "minecraft:air");
            Bin.str(out, rawState);
            String parsed;
            try {
                parsed = BlockStateParser.serialize(BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, rawState, true).blockState());
            } catch (CommandSyntaxException e) {
                parsed = "";
            }
            Bin.str(out, parsed);
        }
    }

    private static void writeBlocks(final OutputStream out, final StructureTemplate.Palette palette) throws IOException {
        Map<String, Integer> ids = new HashMap<>();
        List<String> names = new ArrayList<>();
        List<StructureTemplate.StructureBlockInfo> blocks = palette.blocks();
        int[] indices = new int[blocks.size()];
        for (int i = 0; i < blocks.size(); i++) {
            String name = BlockStateParser.serialize(blocks.get(i).state());
            indices[i] = ids.computeIfAbsent(name, key -> {
                names.add(key);
                return names.size() - 1;
            });
        }
        Bin.i32(out, names.size());
        for (String name : names) {
            Bin.str(out, name);
        }
        Bin.i32(out, blocks.size());
        for (int i = 0; i < blocks.size(); i++) {
            BlockPos pos = blocks.get(i).pos();
            Bin.i32(out, pos.getX());
            Bin.i32(out, pos.getY());
            Bin.i32(out, pos.getZ());
            Bin.i32(out, indices[i]);
        }
        byte[] nbtBits = new byte[(blocks.size() + 7) / 8];
        for (int i = 0; i < blocks.size(); i++) {
            if (blocks.get(i).nbt() != null) {
                nbtBits[i >> 3] |= (byte) (1 << (i & 7));
            }
        }
        out.write(nbtBits);
    }
}
