package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.Registry;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
import net.minecraft.world.entity.ai.attributes.AttributeModifier;
import net.minecraft.world.entity.ai.attributes.AttributeSupplier;
import net.minecraft.world.entity.ai.attributes.DefaultAttributes;

/**
 * Asks the game for the base attributes of each living entity type and for the value of an
 * attribute after a sequence of modifier changes. Doubles are written as their raw bits in hex.
 */
public final class AttributeOracle {
    private static final Gson COMPACT = new GsonBuilder().disableHtmlEscaping().create();
    private static final double TWO_53 = 9007199254740992.0;

    private AttributeOracle() {}

    private record Step(String id, double amount, int operation) {
        static Step add(final String id, final double amount, final int operation) {
            return new Step(id, amount, operation);
        }

        static Step remove(final String id) {
            return new Step(id, 0.0, -1);
        }
    }

    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Path out = Path.of(args[0]);
        Files.createDirectories(out);
        StringBuilder text = new StringBuilder();
        text.append("{\n");
        text.append("  \"game_version\": ").append(COMPACT.toJson(SharedConstants.getCurrentVersion().id())).append(",\n");
        text.append("  \"operations\": [\"add_value\", \"add_multiplied_base\", \"add_multiplied_total\"],\n");
        rows(text, "default_attributes", defaultAttributes());
        text.append(",\n");
        rows(text, "value_cases", valueCases());
        text.append(",\n");
        rows(text, "order_cases", orderCases());
        text.append("\n}\n");
        Files.writeString(out.resolve("attributes.json"), text.toString(), StandardCharsets.UTF_8);
    }

    private static void rows(final StringBuilder text, final String name, final List<JsonArray> rows) {
        text.append("  ").append(COMPACT.toJson(name)).append(": [\n");
        for (int i = 0; i < rows.size(); i++) {
            text.append("    ").append(COMPACT.toJson(rows.get(i))).append(i + 1 < rows.size() ? ",\n" : "\n");
        }
        text.append("  ]");
    }

    private static String d(final double v) {
        return String.format("%016x", Double.doubleToRawLongBits(v));
    }

    /** One row for each type that has attributes: the type, then one pair for each attribute in id order. */
    @SuppressWarnings("unchecked")
    private static List<JsonArray> defaultAttributes() {
        List<JsonArray> rows = new ArrayList<>();
        Registry<EntityType<?>> types = BuiltInRegistries.ENTITY_TYPE;
        Registry<Attribute> attributes = BuiltInRegistries.ATTRIBUTE;
        for (int typeId = 0; typeId < types.size(); typeId++) {
            EntityType<?> type = types.byId(typeId);
            if (!DefaultAttributes.hasSupplier(type)) {
                continue;
            }
            AttributeSupplier supplier = DefaultAttributes.getSupplier((EntityType<? extends LivingEntity>)type);
            JsonArray row = new JsonArray();
            row.add(types.getKey(type).toString());
            for (int id = 0; id < attributes.size(); id++) {
                Holder<Attribute> attribute = attributes.wrapAsHolder(attributes.byId(id));
                if (supplier.hasAttribute(attribute)) {
                    JsonArray pair = new JsonArray();
                    pair.add(attributes.getKey(attribute.value()).toString());
                    pair.add(d(supplier.getBaseValue(attribute)));
                    row.add(pair);
                }
            }
            rows.add(row);
        }
        return rows;
    }

    private static AttributeInstance instance(final String attribute, final double base, final List<Step> steps) {
        Registry<Attribute> attributes = BuiltInRegistries.ATTRIBUTE;
        Attribute value = attributes.getValue(Identifier.parse(attribute));
        AttributeInstance instance = new AttributeInstance(attributes.wrapAsHolder(value), changed -> {});
        instance.setBaseValue(base);
        for (Step step : steps) {
            Identifier id = Identifier.parse(step.id());
            if (step.operation() < 0) {
                instance.removeModifier(id);
            } else {
                instance.addTransientModifier(
                    new AttributeModifier(id, step.amount(), AttributeModifier.Operation.values()[step.operation()])
                );
            }
        }
        return instance;
    }

    private static JsonArray stepRows(final List<Step> steps) {
        JsonArray rows = new JsonArray();
        for (Step step : steps) {
            JsonArray row = new JsonArray();
            if (step.operation() < 0) {
                row.add("remove");
                row.add(step.id());
            } else {
                row.add("add");
                row.add(step.id());
                row.add(d(step.amount()));
                row.add(step.operation());
            }
            rows.add(row);
        }
        return rows;
    }

    private static void value(
        final List<JsonArray> rows, final String name, final String attribute, final double base, final Step... steps
    ) {
        JsonArray row = new JsonArray();
        row.add(name);
        row.add(attribute);
        row.add(d(base));
        row.add(stepRows(List.of(steps)));
        row.add(d(instance(attribute, base, List.of(steps)).getValue()));
        rows.add(row);
    }

    /** Each row: name, attribute, base, the steps in order, the value that the game gives. */
    private static List<JsonArray> valueCases() {
        List<JsonArray> rows = new ArrayList<>();
        String speed = "minecraft:movement_speed";
        double player = 0.1F;
        double zombie = 0.23F;
        Step sprint = Step.add("minecraft:sprinting", 0.3F, 2);
        value(rows, "player_base", speed, player);
        value(rows, "player_sprint", speed, player, sprint);
        value(rows, "player_sprint_stop", speed, player, sprint, Step.remove("minecraft:sprinting"));
        for (int a = 0; a < 4; a++) {
            Step fast = Step.add("minecraft:effect.speed", 0.2F * (double)(a + 1), 2);
            Step slow = Step.add("minecraft:effect.slowness", -0.15F * (double)(a + 1), 2);
            value(rows, "player_speed_" + a, speed, player, fast);
            value(rows, "player_sprint_speed_" + a, speed, player, sprint, fast);
            value(rows, "player_speed_sprint_" + a, speed, player, fast, sprint);
            value(rows, "player_slowness_" + a, speed, player, slow);
            value(rows, "player_sprint_slowness_" + a, speed, player, sprint, slow);
            value(rows, "player_sprint_speed_slowness_" + a, speed, player, sprint, fast, slow);
            value(rows, "player_slowness_speed_sprint_" + a, speed, player, slow, fast, sprint);
        }
        for (int[] pair : new int[][] {{0, 2}, {2, 0}, {1, 1}, {3, 4}}) {
            Step fast = Step.add("minecraft:effect.speed", 0.2F * (double)(pair[0] + 1), 2);
            Step slow = Step.add("minecraft:effect.slowness", -0.15F * (double)(pair[1] + 1), 2);
            String name = "order_speed_" + pair[0] + "_slowness_" + pair[1];
            value(rows, name + "_sprint_first", speed, player, sprint, fast, slow);
            value(rows, name + "_sprint_last", speed, player, slow, fast, sprint);
            value(rows, name + "_sprint_again", speed, player, sprint, slow, fast, Step.remove("minecraft:sprinting"), sprint);
        }
        value(rows, "player_slowness_6", speed, player, Step.add("minecraft:effect.slowness", -0.15F * 7.0, 2));
        for (int frozen : new int[] {1, 70, 139, 140}) {
            float amount = -0.05F * ((float)frozen / 140.0F);
            Step snow = Step.add("minecraft:powder_snow", amount, 0);
            value(rows, "player_powder_snow_" + frozen, speed, player, snow);
            value(rows, "player_powder_snow_sprint_" + frozen, speed, player, snow, sprint);
        }
        value(rows, "zombie_baby", speed, zombie, Step.add("minecraft:baby", 0.5, 1));
        value(
            rows,
            "three_operations",
            speed,
            0.1,
            Step.add("test:a", 0.2, 0),
            Step.add("test:c", 0.1, 1),
            Step.add("test:e", 0.2, 1),
            Step.add("test:g", 0.3, 2),
            Step.add("test:r", 0.7, 2)
        );
        value(rows, "creative_block_range", "minecraft:block_interaction_range", 4.5, Step.add("minecraft:creative_mode_block_range", 0.5, 0));
        value(rows, "creative_entity_range", "minecraft:entity_interaction_range", 3.0, Step.add("minecraft:creative_mode_entity_range", 2.0, 0));
        value(rows, "above_max", "minecraft:max_health", 20.0, Step.add("test:a", 2000.0, 0));
        value(rows, "below_min", "minecraft:max_health", 20.0, Step.add("test:a", -40.0, 0));
        value(rows, "base_above_max", "minecraft:max_health", 5000.0);
        value(rows, "base_below_min", "minecraft:max_health", -5.0);
        value(rows, "base_above_max_modifier", "minecraft:max_health", 5000.0, Step.add("test:a", -4990.0, 0));
        value(rows, "nan_amount", "minecraft:max_health", 20.0, Step.add("test:a", Double.NaN, 0));
        value(rows, "nan_base", "minecraft:gravity", Double.NaN);
        value(rows, "positive_infinity", "minecraft:gravity", 0.08, Step.add("test:a", Double.POSITIVE_INFINITY, 0));
        value(rows, "negative_infinity", "minecraft:gravity", 0.08, Step.add("test:a", Double.NEGATIVE_INFINITY, 0));
        value(rows, "negative_zero_min_zero", speed, -0.0);
        value(rows, "negative_zero_min_negative", "minecraft:gravity", -0.0);
        value(rows, "total_minus_one", speed, player, Step.add("test:a", -1.0, 2));
        return rows;
    }

    private static void order(final List<JsonArray> rows, final int operation, final String... sequence) {
        List<String> ids = new ArrayList<>();
        for (String step : sequence) {
            if (step.startsWith("-")) {
                ids.remove(step.substring(1));
            } else if (step.indexOf('|') < 0) {
                ids.add(step);
            }
        }
        // The id with the most ids after it is the first one of the computation.
        int[] after = new int[ids.size()];
        for (int i = 0; i < ids.size(); i++) {
            for (int j = 0; j < ids.size(); j++) {
                if (i != j && isBefore(operation, sequence, ids.get(i), ids.get(j))) {
                    after[i]++;
                }
            }
        }
        JsonArray order = new JsonArray();
        for (int rank = ids.size() - 1; rank >= 0; rank--) {
            for (int i = 0; i < ids.size(); i++) {
                if (after[i] == rank) {
                    order.add(ids.get(i));
                }
            }
        }
        JsonArray steps = new JsonArray();
        for (String step : sequence) {
            steps.add(step);
        }
        JsonArray row = new JsonArray();
        row.add(operation);
        row.add(steps);
        row.add(order);
        rows.add(row);
    }

    /**
     * Whether the computation takes `first` before `second`. The sum of 1, 2^53 and -2^53 is 0 when
     * 2^53 comes first and 1 when -2^53 comes first; each other modifier has the amount 0.
     */
    private static boolean isBefore(final int operation, final String[] sequence, final String first, final String second) {
        List<Step> steps = new ArrayList<>();
        for (String step : sequence) {
            int bar = step.indexOf('|');
            if (step.startsWith("-")) {
                steps.add(Step.remove(step.substring(1)));
            } else if (bar >= 0) {
                steps.add(Step.add(step.substring(0, bar), 0.0, Integer.parseInt(step.substring(bar + 1))));
            } else {
                double amount = step.equals(first) ? TWO_53 : step.equals(second) ? -TWO_53 : 0.0;
                steps.add(Step.add(step, amount, operation));
            }
        }
        double value = instance("minecraft:luck", 1.0, steps).getValue();
        if (value != 0.0 && value != 1.0) {
            throw new IllegalStateException("order probe gave " + value);
        }
        return value == 0.0;
    }

    /**
     * Each row: the operation, the steps (an id adds a modifier, "-id" removes it, "id|n" adds a
     * modifier of the amount 0 with the operation n), and the ids of the operation in the order in
     * which the computation takes them.
     */
    private static List<JsonArray> orderCases() {
        List<JsonArray> rows = new ArrayList<>();
        String sprint = "minecraft:sprinting";
        String fast = "minecraft:effect.speed";
        String slow = "minecraft:effect.slowness";
        for (int operation = 0; operation < 2; operation++) {
            order(rows, operation, sprint, fast, slow);
            order(rows, operation, slow, fast, sprint);
            order(rows, operation, fast, slow, sprint, "-" + sprint, sprint);
            order(rows, operation, "minecraft:powder_snow", "minecraft:enchantment.soul_speed/feet");
            order(rows, operation, "minecraft:armor.helmet", "minecraft:armor.chestplate", "minecraft:armor.leggings", "minecraft:armor.boots");
            order(rows, operation, "minecraft:armor.boots", "minecraft:armor.leggings", "minecraft:armor.chestplate", "minecraft:armor.helmet");
            order(rows, operation, "test:a", "test:b", "test:c", "test:d", "test:e", "test:f", "test:g", "test:h");
            order(rows, operation, "test:h", "test:g", "test:f", "test:e", "test:d", "test:c", "test:b", "test:a");
            order(rows, operation, "test:v", "test:w", "test:x");
            order(rows, operation, "test:x", "test:w", "test:v");
            order(rows, operation, "test:v", "test:w", "test:x", "-test:v");
            order(rows, operation, "test:v", "test:w", "test:x", "-test:v", "test:v");
            order(rows, operation, "test:b", "test:aw", "test:aq");
            order(rows, operation, "test:aq", "test:aw", "test:b");
            order(rows, operation, "test:b", "test:aw", "test:aq", "-test:b");
            order(rows, operation, "test:t", "test:y", "test:u", "-test:t", "test:t");
            order(rows, operation, "test:v|2", "test:x", "test:w");
            order(rows, operation, "test:v|" + (1 - operation), "test:x", "test:w");
        }
        String[] many = new String[24];
        for (int i = 0; i < many.length; i++) {
            many[i] = "test:m" + i;
        }
        order(rows, 0, many);
        return rows;
    }
}
