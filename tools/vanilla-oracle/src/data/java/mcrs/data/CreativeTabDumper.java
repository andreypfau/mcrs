package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.serialization.JsonOps;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Collection;
import java.util.List;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import net.minecraft.SharedConstants;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.RegistryOps;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.world.flag.FeatureFlagSet;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.item.CreativeModeTab;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.ItemStackTemplate;

public final class CreativeTabDumper {
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create();
    private static final Set<String> VANILLA_ONLY = Set.of("minecraft:vanilla");

    private CreativeTabDumper() {}

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

    private static void run(final Path out) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        try (MultiPackResourceManager resources = new MultiPackResourceManager(
                PackType.SERVER_DATA, List.of(ServerPacksSource.createVanillaPackSource().fullResources())
            )) {
            LayeredRegistryAccess<RegistryLayer> layers = RegistryLayer.createRegistryAccess();
            RegistryAccess.Frozen loaded = BlockDefinitionDumper.loadWorldRegistries(resources, layers);
            RegistryAccess.Frozen access = layers.replaceFrom(RegistryLayer.WORLD, loaded).compositeAccess();
            BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(access).forEach(pending -> pending.apply());
            RegistryOps<JsonElement> ops = access.createSerializationContext(JsonOps.INSTANCE);
            FeatureFlagSet features = FeatureFlags.REGISTRY.allFlags();

            JsonObject tabs = new JsonObject();
            for (CreativeModeTab tab : BuiltInRegistries.CREATIVE_MODE_TAB) {
                if (tab.getType() != CreativeModeTab.Type.CATEGORY) {
                    continue;
                }
                tab.buildContents(new CreativeModeTab.ItemDisplayParameters(features, false, access));
                boolean needsOperator = tab.getDisplayItems().isEmpty() && tab.getSearchTabDisplayItems().isEmpty();
                tab.buildContents(new CreativeModeTab.ItemDisplayParameters(features, true, access));
                JsonObject entry = new JsonObject();
                entry.addProperty("needs_operator", needsOperator);
                entry.add("tab_list", stacks(tab.getDisplayItems(), ops));
                entry.add("search_list", stacks(tab.getSearchTabDisplayItems(), ops));
                tabs.add(BuiltInRegistries.CREATIVE_MODE_TAB.getKey(tab).toString(), entry);
            }

            JsonObject report = new JsonObject();
            report.add("item_features", itemFeatures());
            report.add("tabs", tabs);
            Files.writeString(out, GSON.toJson(report) + "\n", StandardCharsets.UTF_8);
            System.out.println("wrote " + out);
        }
    }

    private static JsonObject itemFeatures() {
        TreeMap<String, JsonArray> byItem = new TreeMap<>();
        for (Item item : BuiltInRegistries.ITEM) {
            TreeSet<String> names = new TreeSet<>();
            FeatureFlags.REGISTRY.toNames(item.requiredFeatures()).forEach(name -> names.add(name.toString()));
            if (names.equals(VANILLA_ONLY)) {
                continue;
            }
            JsonArray flags = new JsonArray();
            names.forEach(flags::add);
            byItem.put(BuiltInRegistries.ITEM.getKey(item).toString(), flags);
        }
        JsonObject object = new JsonObject();
        byItem.forEach(object::add);
        return object;
    }

    private static JsonArray stacks(final Collection<ItemStack> stacks, final RegistryOps<JsonElement> ops) {
        JsonArray list = new JsonArray();
        for (ItemStack stack : stacks) {
            if (stack.getCount() != 1) {
                throw new IllegalStateException(stack + " has a count other than 1");
            }
            JsonElement saved = ItemStackTemplate.CODEC.encodeStart(ops, ItemStackTemplate.fromNonEmptyStack(stack)).getOrThrow();
            list.add(sortedKeys(saved));
        }
        return list;
    }

    private static JsonElement sortedKeys(final JsonElement element) {
        if (element.isJsonObject()) {
            JsonObject sorted = new JsonObject();
            new TreeMap<>(element.getAsJsonObject().asMap()).forEach((key, value) -> sorted.add(key, sortedKeys(value)));
            return sorted;
        }
        if (element.isJsonArray()) {
            JsonArray sorted = new JsonArray();
            element.getAsJsonArray().forEach(value -> sorted.add(sortedKeys(value)));
            return sorted;
        }
        return element;
    }
}
