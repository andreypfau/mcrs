package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.serialization.JsonOps;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.TreeMap;
import java.util.stream.Stream;
import net.minecraft.core.Holder;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.component.DataComponentMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryOps;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStackTemplate;

public final class ItemDefinitionDumper {
    private static final String FORMAT_VERSION = "1.21.130";
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    private ItemDefinitionDumper() {}

    static List<String> dump(final RegistryAccess access, final Path root) throws IOException {
        Files.createDirectories(root);
        RegistryOps<JsonElement> ops = access.createSerializationContext(JsonOps.INSTANCE);

        List<String> failed = new ArrayList<>();
        for (Item item : BuiltInRegistries.ITEM) {
            Identifier id = BuiltInRegistries.ITEM.getKey(item);
            try {
                Files.writeString(root.resolve(id.getPath() + ".json"), GSON.toJson(definitionOf(item, id, ops)) + "\n", StandardCharsets.UTF_8);
            } catch (Throwable failure) {
                failed.add(id.toString());
                failure.printStackTrace();
            }
        }

        try (Stream<Path> files = Files.list(root)) {
            System.out.println("wrote " + root + " (" + files.count() + " files)");
        }
        return failed;
    }

    private static JsonObject sortedByKey(final JsonObject object) {
        JsonObject sorted = new JsonObject();
        new TreeMap<>(object.asMap()).forEach(sorted::add);
        return sorted;
    }

    private static JsonObject definitionOf(final Item item, final Identifier id, final RegistryOps<JsonElement> ops) {
        Holder.Reference<Item> holder = item.builtInRegistryHolder();
        if (!holder.areComponentsBound()) {
            throw new IllegalStateException(id + " components not bound");
        }

        JsonObject description = new JsonObject();
        description.addProperty("identifier", id.toString());
        description.addProperty("protocol_id", BuiltInRegistries.ITEM.getId(item));

        JsonObject definition = new JsonObject();
        definition.add("description", description);
        definition.add("components", sortedByKey(DataComponentMap.CODEC.encodeStart(ops, holder.components()).getOrThrow().getAsJsonObject()));
        if (item instanceof BlockItem blockItem) {
            definition.addProperty("block_placer", BuiltInRegistries.BLOCK.getKey(blockItem.getBlock()).toString());
        }
        ItemStackTemplate remainder = item.getCraftingRemainder();
        if (remainder != null) {
            definition.add("crafting_remainder", ItemStackTemplate.CODEC.encodeStart(ops, remainder).getOrThrow());
        }

        JsonObject root = new JsonObject();
        root.addProperty("format_version", FORMAT_VERSION);
        root.add("minecraft:item", definition);
        return root;
    }
}
