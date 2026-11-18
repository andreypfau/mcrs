package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.item.alchemy.Potion;
import net.minecraft.world.item.alchemy.PotionContents;

public final class PotionColorOracle {
    private static final int DEFAULT_COLOR = -13083194;
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().serializeNulls().create();

    private record Effect(String id, int amplifier, boolean showParticles) {}

    private record Case(String potion, Integer customColor, List<Effect> customEffects) {}

    private PotionColorOracle() {}

    public static void main(final String[] args) {
        int status;
        try {
            SharedConstants.tryDetectVersion();
            Bootstrap.bootStrap();
            Path out = Path.of(args[0]);
            Files.createDirectories(out);
            JsonArray cases = new JsonArray();
            for (Case input : cases()) {
                cases.add(observe(input));
            }
            JsonObject root = new JsonObject();
            root.addProperty("default", DEFAULT_COLOR);
            root.add("cases", cases);
            Files.writeString(out.resolve("potion_colors.json"), GSON.toJson(root) + "\n", StandardCharsets.UTF_8);
            status = 0;
        } catch (Throwable failure) {
            failure.printStackTrace();
            status = 1;
        }
        System.exit(status);
    }

    private static List<Case> cases() {
        return List.of(
            new Case(null, null, List.of()),
            new Case("minecraft:water", null, List.of()),
            new Case("minecraft:strong_healing", null, List.of()),
            new Case("minecraft:turtle_master", null, List.of()),
            new Case("minecraft:strong_turtle_master", null, List.of()),
            new Case(null, null, List.of(new Effect("minecraft:speed", 0, true))),
            new Case(null, null, List.of(new Effect("minecraft:speed", 0, false))),
            new Case(
                "minecraft:night_vision",
                null,
                List.of(new Effect("minecraft:speed", 1, true), new Effect("minecraft:poison", 0, false))
            ),
            new Case(null, null, List.of(new Effect("minecraft:speed", 255, true), new Effect("minecraft:poison", 0, true))),
            new Case(null, null, List.of(new Effect("minecraft:instant_health", 2, true), new Effect("minecraft:wither", 1, true))),
            new Case("minecraft:strong_healing", 0x00FF00, List.of(new Effect("minecraft:speed", 0, true))),
            new Case(null, 0, List.of())
        );
    }

    private static JsonObject observe(final Case input) {
        List<MobEffectInstance> effects = new ArrayList<>();
        JsonArray customEffects = new JsonArray();
        for (Effect effect : input.customEffects()) {
            effects.add(new MobEffectInstance(
                BuiltInRegistries.MOB_EFFECT.get(Identifier.parse(effect.id())).orElseThrow(),
                1,
                effect.amplifier(),
                false,
                effect.showParticles()
            ));
            JsonObject entry = new JsonObject();
            entry.addProperty("id", effect.id());
            entry.addProperty("amplifier", effect.amplifier());
            entry.addProperty("show_particles", effect.showParticles());
            customEffects.add(entry);
        }
        PotionContents contents = new PotionContents(
            Optional.ofNullable(input.potion()).<Holder<Potion>>map(id -> BuiltInRegistries.POTION.get(Identifier.parse(id)).orElseThrow()),
            Optional.ofNullable(input.customColor()),
            effects,
            Optional.empty()
        );
        JsonObject observed = new JsonObject();
        observed.addProperty("potion", input.potion());
        observed.addProperty("custom_color", input.customColor());
        observed.add("custom_effects", customEffects);
        observed.addProperty("color", contents.getColorOr(DEFAULT_COLOR));
        return observed;
    }
}
