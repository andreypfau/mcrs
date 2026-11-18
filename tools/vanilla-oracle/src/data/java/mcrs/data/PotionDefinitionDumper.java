package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.item.alchemy.Potion;

public final class PotionDefinitionDumper {
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    private PotionDefinitionDumper() {}

    static List<String> dump(final Path root) throws IOException {
        Files.createDirectories(root);
        List<String> failed = new ArrayList<>();

        JsonObject effects = new JsonObject();
        for (Identifier id : sortedKeys(BuiltInRegistries.MOB_EFFECT)) {
            try {
                MobEffect effect = BuiltInRegistries.MOB_EFFECT.getValue(id);
                JsonObject definition = new JsonObject();
                definition.addProperty("color", effect.getColor() & 0xFFFFFF);
                effects.add(id.toString(), definition);
            } catch (Throwable failure) {
                failed.add(id.toString());
                failure.printStackTrace();
            }
        }
        JsonObject potions = new JsonObject();
        for (Identifier id : sortedKeys(BuiltInRegistries.POTION)) {
            try {
                Potion potion = BuiltInRegistries.POTION.getValue(id);
                JsonArray instances = new JsonArray();
                for (MobEffectInstance instance : potion.getEffects()) {
                    JsonObject entry = new JsonObject();
                    entry.addProperty("ambient", instance.isAmbient());
                    entry.addProperty("amplifier", instance.getAmplifier());
                    entry.addProperty("duration", instance.getDuration());
                    entry.addProperty("id", instance.getEffect().unwrapKey().orElseThrow().identifier().toString());
                    entry.addProperty("show_icon", instance.showIcon());
                    entry.addProperty("show_particles", instance.isVisible());
                    instances.add(entry);
                }
                JsonObject definition = new JsonObject();
                definition.add("effects", instances);
                potions.add(id.toString(), definition);
            } catch (Throwable failure) {
                failed.add(id.toString());
                failure.printStackTrace();
            }
        }
        if (!failed.isEmpty()) {
            return failed;
        }
        write(root.resolve("mob_effect.json"), effects, "effects");
        write(root.resolve("potion.json"), potions, "potions");
        return failed;
    }

    private static List<Identifier> sortedKeys(final Registry<?> registry) {
        List<Identifier> keys = new ArrayList<>(registry.keySet());
        keys.sort(Comparator.comparing(Identifier::toString));
        return keys;
    }

    private static void write(final Path file, final JsonObject json, final String noun) throws IOException {
        Files.writeString(file, GSON.toJson(json) + "\n", StandardCharsets.UTF_8);
        System.out.println("wrote " + file + " (" + json.size() + " " + noun + ")");
    }
}
