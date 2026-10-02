package mcrs.data;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;

public final class CodecGoldens {
    @FunctionalInterface
    interface Golden {
        void write(Path current, Path output) throws Exception;
    }

    private static final Map<String, Golden> GOLDENS = new LinkedHashMap<>();

    static {
        GOLDENS.put("snbt", NbtGoldens::snbt);
        GOLDENS.put("hash_ops", NbtGoldens::hashOps);
        GOLDENS.put("text_vanilla", TextGoldens::vanilla);
        GOLDENS.put("text_probe", TextGoldens::probe);
        GOLDENS.put("text_nbt", TextGoldens::nbt);
        GOLDENS.put("text_wire", TextGoldens::wire);
        GOLDENS.put("item_plain", ItemGoldens::plain);
        GOLDENS.put("item_nested", ItemGoldens::nested);
        GOLDENS.put("item_predicate", ItemGoldens::predicate);
        GOLDENS.put("item_kinds", ItemGoldens::kinds);
        GOLDENS.put("item_holders", ItemGoldens::holders);
        GOLDENS.put("item_registry_refs", ItemGoldens::registryRefs);
        GOLDENS.put("item_records", ItemGoldens::records);
        GOLDENS.put("recipe_packets", PacketGoldens::recipePackets);
        GOLDENS.put("particles", PacketGoldens::particles);
        GOLDENS.put("inventory_packets", PacketGoldens::inventoryPackets);
        GOLDENS.put("join_packets", PacketGoldens::joinPackets);
        GOLDENS.put("frames", FrameGoldens::frames);
        GOLDENS.put("vanilla_player", PlayerGoldens::vanillaPlayer);
    }

    private CodecGoldens() {}

    public static void main(final String[] args) {
        int status;
        try {
            run(Path.of(args[0]), args[1], Path.of(args[2]));
            status = 0;
        } catch (Throwable failure) {
            failure.printStackTrace();
            status = 1;
        }
        System.exit(status);
    }

    private static void run(final Path outDir, final String name, final Path current) throws Exception {
        Golden golden = GOLDENS.get(name);
        if (golden == null) {
            throw new IllegalArgumentException("unknown golden '" + name + "'; known goldens: " + String.join(", ", GOLDENS.keySet()));
        }
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        Files.createDirectories(outDir);
        Path output = outDir.resolve(current.getFileName());
        golden.write(current, output);
        System.out.println("wrote " + output);
    }
}
