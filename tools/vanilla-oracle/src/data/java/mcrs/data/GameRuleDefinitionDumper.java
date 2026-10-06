package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.serialization.JsonOps;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.level.gamerules.GameRule;

public final class GameRuleDefinitionDumper {
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    private GameRuleDefinitionDumper() {}

    static List<String> dump(final Path root) throws IOException {
        Files.createDirectories(root);
        List<String> failed = new ArrayList<>();
        JsonObject rules = new JsonObject();
        for (GameRule<?> rule : BuiltInRegistries.GAME_RULE) {
            Identifier id = BuiltInRegistries.GAME_RULE.getKey(rule);
            try {
                rules.add(id.toString(), definitionOf(rule));
            } catch (Throwable failure) {
                failed.add(id.toString());
                failure.printStackTrace();
            }
        }
        Path file = root.resolve("game_rule.json");
        Files.writeString(file, GSON.toJson(rules) + "\n", StandardCharsets.UTF_8);
        System.out.println("wrote " + file + " (" + rules.size() + " rules)");
        return failed;
    }

    private static <T> JsonObject definitionOf(final GameRule<T> rule) {
        JsonObject definition = new JsonObject();
        definition.addProperty("category", rule.category().id().toString());
        definition.addProperty("type", rule.gameRuleType().getSerializedName());
        definition.add("default", rule.valueCodec().encodeStart(JsonOps.INSTANCE, rule.defaultValue()).getOrThrow());
        if (rule.argument() instanceof IntegerArgumentType range) {
            definition.addProperty("min", range.getMinimum());
            definition.addProperty("max", range.getMaximum());
        }
        if (!rule.requiredFeatures().isEmpty()) {
            JsonArray features = new JsonArray();
            FeatureFlags.REGISTRY.toNames(rule.requiredFeatures()).stream()
                .map(Identifier::toString)
                .sorted()
                .forEach(features::add);
            definition.add("required_features", features);
        }
        return definition;
    }
}
