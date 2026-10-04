package mcrs.data;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import com.google.gson.internal.LazilyParsedNumber;
import com.mojang.serialization.DataResult;
import io.netty.buffer.Unpooled;
import java.math.BigDecimal;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.RecordComponent;
import java.lang.reflect.Type;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;
import mcrs.data.CodecGoldens.Session;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderSet;
import net.minecraft.core.Registry;
import net.minecraft.core.component.DataComponentExactPredicate;
import net.minecraft.core.component.DataComponentMap;
import net.minecraft.core.component.DataComponentPatch;
import net.minecraft.core.component.DataComponentType;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.component.TypedDataComponent;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.nbt.Tag;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.tags.TagKey;
import net.minecraft.util.Unit;
import net.minecraft.world.entity.decoration.painting.PaintingVariant;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.ItemStackTemplate;
import net.minecraft.world.item.JukeboxPlayable;
import net.minecraft.world.item.JukeboxSong;

final class ItemGoldens {
    private static final Set<Class<?>> EMBEDDED_STACKS = Set.of(
        ItemStackTemplate.class, ItemStack.class, DataComponentPatch.class, DataComponentMap.class, DataComponentExactPredicate.class
    );
    private static final Set<Class<?>> REFERENCES = Set.of(Holder.class, HolderSet.class, ResourceKey.class, TagKey.class, Registry.class);

    private static final Map<String, String> RECORD_KINDS = Map.ofEntries(
        Map.entry("use_effects", "use_effects"),
        Map.entry("cmd", "custom_model_data"),
        Map.entry("tooltip", "tooltip_display"),
        Map.entry("tooltip_style", "tooltip_style"),
        Map.entry("food", "food"),
        Map.entry("cooldown", "use_cooldown"),
        Map.entry("weapon", "weapon"),
        Map.entry("attack_range", "attack_range"),
        Map.entry("block_state", "block_state"),
        Map.entry("writable", "writable_book_content"),
        Map.entry("written", "written_book_content"),
        Map.entry("profile", "profile"),
        Map.entry("lodestone", "lodestone_tracker"),
        Map.entry("explosion", "firework_explosion"),
        Map.entry("fireworks", "fireworks"),
        Map.entry("bucket", "bucket_entity_data"),
        Map.entry("map_dec", "map_decorations"),
        Map.entry("debug", "debug_stick_state"),
        Map.entry("recipes", "recipes"),
        Map.entry("loot", "container_loot"),
        Map.entry("compostable", "compostable"),
        Map.entry("cooking", "cooking_fuel"),
        Map.entry("brewing", "brewing_fuel"),
        Map.entry("sign", "sign_text_front"),
        Map.entry("sign_full", "sign_text_back"),
        Map.entry("item_model", "item_model"),
        Map.entry("note_block_sound", "note_block_sound")
    );
    private static final Map<String, String> RECORD_INPUTS = Map.ofEntries(
        Map.entry("use_effects_range", "{\"speed_multiplier\":1.5}"),
        Map.entry("food_neg", "{\"nutrition\":-1,\"saturation\":0.6}"),
        Map.entry("cooldown_zero", "{\"seconds\":0}"),
        Map.entry("attack_range_bad", "{\"mob_factor\":2.5}"),
        Map.entry("written_gen", "{\"title\":\"T\",\"author\":\"me\",\"generation\":4}"),
        Map.entry("profile_bad_name", "{\"name\":\"has space\"}"),
        Map.entry("profile_long_name", "\"abcdefghijklmnopq\""),
        Map.entry("lodestone_bad_pos", "{\"target\":{\"dimension\":\"minecraft:overworld\",\"pos\":[1,2]}}"),
        Map.entry("map_dec_bad", "{\"m1\":{\"type\":\"minecraft:nope\",\"x\":1.5,\"z\":-2.5,\"rotation\":90.0}}"),
        Map.entry("debug_bad", "{\"minecraft:oak_log\":\"nope\"}"),
        Map.entry("sign_three", "{\"messages\":[\"a\",\"b\",\"c\"]}"),
        Map.entry("bucket_snbt", "\"{Health:3.0f}\"")
    );

    private ItemGoldens() {}

    @FunctionalInterface
    private interface Row {
        JsonObject write(Session session, JsonObject in) throws Exception;
    }

    static void plain(final Path current, final Path output) throws Exception {
        CodecGoldens.withSession(session -> {
            JsonObject in = read(current).getAsJsonObject();
            JsonObject out = new JsonObject();
            out.add("values", rows(session, in, "values", ItemGoldens::value));
            out.add("errors", rows(session, in, "errors", ItemGoldens::error));
            out.add("decodes", rows(session, in, "decodes", ItemGoldens::decoded));
            out.add("decode_errors", rows(session, in, "decode_errors", ItemGoldens::decodeError));
            CodecGoldens.writeJson(output, "  ", true, out);
        });
    }

    static void nested(final Path current, final Path output) throws Exception {
        CodecGoldens.withSession(session -> {
            JsonObject in = read(current).getAsJsonObject();
            JsonObject out = new JsonObject();
            out.add("lookup", lookup(session, in.getAsJsonObject("lookup")));
            out.add("cases", rows(session, in, "cases", ItemGoldens::nestedCase));
            CodecGoldens.writeJson(output, " ", false, out);
        });
    }

    static void predicate(final Path current, final Path output) throws Exception {
        CodecGoldens.withSession(session -> {
            JsonArray out = new JsonArray();
            JsonArray in = read(current).getAsJsonArray();
            for (int index = 0; index < in.size(); index++) {
                out.add(attempt(session, "row " + index, in.get(index).getAsJsonObject(), ItemGoldens::predicateRow));
            }
            CodecGoldens.writeJson(output, " ", false, out);
        });
    }

    static void kinds(final Path current, final Path output) throws Exception {
        Map<DataComponentType<?>, Type> valueTypes = valueTypes();
        Set<Class<?>> registered = registeredClasses();
        JsonArray rows = new JsonArray();
        for (DataComponentType<?> type : BuiltInRegistries.DATA_COMPONENT_TYPE) {
            Identifier id = BuiltInRegistries.DATA_COMPONENT_TYPE.getKey(type);
            Type valueType = valueTypes.get(type);
            if (valueType == null) {
                throw new IllegalStateException("component " + id + " is not a field of DataComponents");
            }
            JsonObject row = new JsonObject();
            row.addProperty("id", id.getPath());
            row.addProperty("wire_id", BuiltInRegistries.DATA_COMPONENT_TYPE.getId(type));
            row.addProperty("transient", type.isTransient());
            row.addProperty("unit", valueType == Unit.class);
            row.addProperty("ignore_swap_animation", type.ignoreSwapAnimation());
            row.addProperty("nested_stacks", embedsStacks(valueType, new HashSet<>(), registered));
            rows.add(row);
        }
        JsonObject out = new JsonObject();
        out.add("kinds", rows);
        CodecGoldens.writeJson(output, "  ", true, out);
    }

    static void holders(final Path current, final Path output) throws Exception {
        CodecGoldens.withSession(session -> {
            List<String> in = Files.readAllLines(current, StandardCharsets.UTF_8);
            List<String> out = new ArrayList<>();
            int index = 0;
            while (index < in.size()) {
                String line = in.get(index);
                if (line.startsWith("id ")) {
                    out.add(CodecGoldens.idLine(session, line, false));
                    index++;
                    continue;
                }
                String label = line.substring(0, line.indexOf(' '));
                List<String> group = new ArrayList<>();
                while (index < in.size() && in.get(index).startsWith(label + " ")) {
                    group.add(in.get(index++));
                }
                out.addAll(attempt(label, group, () -> holderCase(session, label, component(holderKind(label)), group)));
            }
            CodecGoldens.writeLines(output, out);
        });
    }

    static void registryRefs(final Path current, final Path output) throws Exception {
        CodecGoldens.withSession(session -> {
            List<String> in = Files.readAllLines(current, StandardCharsets.UTF_8);
            List<String> out = new ArrayList<>();
            int index = 0;
            while (index < in.size()) {
                String line = in.get(index++);
                if (line.startsWith("id ")) {
                    out.add(CodecGoldens.idLine(session, line, true));
                    continue;
                }
                if (!line.startsWith("sample ")) {
                    throw new IllegalStateException("unexpected line: " + line);
                }
                List<String> group = new ArrayList<>();
                while (index < in.size() && in.get(index).startsWith("  ")) {
                    group.add(in.get(index++));
                }
                String header = line;
                out.add(header);
                out.addAll(attempt(header, group, () -> {
                    String rest = header.substring("sample ".length());
                    int space = rest.indexOf(' ');
                    return referenceSample(session, component(rest.substring(0, space)), rest.substring(space + 1), group);
                }));
            }
            CodecGoldens.writeLines(output, out);
        });
    }

    static void records(final Path current, final Path output) throws Exception {
        CodecGoldens.withSession(session -> {
            List<String> in = Files.readAllLines(current, StandardCharsets.UTF_8);
            List<String> out = new ArrayList<>();
            int index = 0;
            while (index < in.size()) {
                String label = in.get(index).substring(0, in.get(index).indexOf(' '));
                List<String> group = new ArrayList<>();
                while (index < in.size() && in.get(index).startsWith(label + " ")) {
                    group.add(in.get(index++));
                }
                out.addAll(attempt(label, group, () -> recordRows(session, label, component(recordKind(label)), group)));
            }
            CodecGoldens.writeLines(output, out);
        });
    }

    @FunctionalInterface
    private interface Lines {
        List<String> write() throws Exception;
    }

    private static List<String> attempt(final String label, final List<String> group, final Lines lines) throws Exception {
        try {
            return lines.write();
        } catch (RuntimeException failure) {
            throw new IllegalStateException(label + " " + group, failure);
        }
    }

    private static boolean isComponent(final String id) {
        return BuiltInRegistries.DATA_COMPONENT_TYPE.containsKey(Identifier.withDefaultNamespace(id));
    }

    private static String holderKind(final String label) {
        for (String candidate = label; ; candidate = candidate.substring(0, candidate.lastIndexOf('_'))) {
            if (isComponent(candidate)) {
                return candidate;
            }
            if (isComponent(candidate.replace('_', '/'))) {
                return candidate.replace('_', '/');
            }
            if (candidate.indexOf('_') < 0) {
                throw new IllegalStateException("no component kind for label " + label);
            }
        }
    }

    private static String recordKind(final String label) {
        String best = null;
        for (String prefix : RECORD_KINDS.keySet()) {
            if ((label.equals(prefix) || label.startsWith(prefix + "_")) && (best == null || prefix.length() > best.length())) {
                best = prefix;
            }
        }
        if (best == null) {
            throw new IllegalStateException("no component kind for label " + label);
        }
        return RECORD_KINDS.get(best);
    }

    private static String keyOf(final String line, final int skip) {
        String[] parts = line.trim().split(" ", skip + 2);
        return parts[skip];
    }

    private static <T> List<String> holderCase(
        final Session session, final String label, final DataComponentType<T> type, final List<String> group
    ) throws Exception {
        T value = null;
        for (String line : group) {
            if (keyOf(line, 1).equals("in")) {
                String input = line.substring((label + " in ").length());
                DataResult<T> parsed = type.codecOrThrow().parse(session.json(), JsonParser.parseString(input));
                value = parsed.result().orElseThrow(() -> new IllegalStateException("now rejected: " + parsed.error().orElseThrow().message()));
            }
        }
        if (value == null) {
            value = wireOnlyValue(session, label, type);
        }
        List<String> out = new ArrayList<>();
        for (String line : group) {
            String key = line.split(" ", 3)[1];
            out.add(switch (key) {
                case "in" -> line;
                case "json" -> label + " json " + json(session, type, value);
                case "nbt" -> label + " nbt " + NbtGoldens.binary(nbt(session, type, value));
                case "hash" -> label + " hash " + hash(session, type, value);
                case "wire" -> label + " wire " + wire(session, type, value);
                default -> throw new IllegalStateException("unknown field '" + key + "'");
            });
        }
        return out;
    }

    @SuppressWarnings("unchecked")
    private static <T> T wireOnlyValue(final Session session, final String label, final DataComponentType<T> type) {
        Registry<SoundEvent> sounds = session.access().lookupOrThrow(Registries.SOUND_EVENT);
        Holder<SoundEvent> itemBreak = sounds.getOrThrow(ResourceKey.create(Registries.SOUND_EVENT, Identifier.parse("entity.item.break")));
        Object value = switch (label) {
            case "jukebox_playable_direct" -> new JukeboxPlayable(Holder.direct(new JukeboxSong(itemBreak, Component.literal("Song"), 12.5F, 7)));
            case "jukebox_playable_direct_sound" -> new JukeboxPlayable(Holder.direct(new JukeboxSong(
                Holder.direct(SoundEvent.createVariableRangeEvent(Identifier.parse("mcrs:song"))), Component.translatable("song.mcrs"), 1.0F, 0
            )));
            case "painting_variant_direct" -> Holder.direct(new PaintingVariant(
                2, 1, Identifier.parse("mcrs:art"), Optional.of(Component.literal("T")), Optional.empty()
            ));
            case "painting_variant_direct_both" -> Holder.direct(new PaintingVariant(
                16, 16, Identifier.parse("mcrs:big"), Optional.empty(), Optional.of(Component.translatable("author.mcrs"))
            ));
            default -> throw new IllegalStateException("label " + label + " has no input and no value built for it");
        };
        DataComponentType<?> expected = label.startsWith("jukebox_playable") ? DataComponents.JUKEBOX_PLAYABLE : DataComponents.PAINTING_VARIANT;
        if (type != expected) {
            throw new IllegalStateException("label " + label + " is not a value of " + BuiltInRegistries.DATA_COMPONENT_TYPE.getKey(type));
        }
        return (T) value;
    }

    private static <T> List<String> referenceSample(
        final Session session, final DataComponentType<T> type, final String input, final List<String> group
    ) throws Exception {
        boolean wasError = group.stream().anyMatch(line -> keyOf(line, 0).equals("error"));
        DataResult<T> parsed = type.codecOrThrow().parse(session.json(), JsonParser.parseString(input));
        if (wasError != parsed.result().isEmpty()) {
            throw new IllegalStateException(wasError ? "now accepted" : "now rejected: " + parsed.error().orElseThrow().message());
        }
        if (wasError) {
            return List.of("  error " + parsed.error().orElseThrow().message());
        }
        T value = parsed.result().orElseThrow();
        List<String> out = new ArrayList<>();
        for (String line : group) {
            String key = keyOf(line, 0);
            out.add("  " + switch (key) {
                case "json" -> "json " + json(session, type, value);
                case "snbt" -> "snbt " + nbt(session, type, value);
                case "nbt" -> "nbt " + NbtGoldens.binary(nbt(session, type, value));
                case "wire" -> "wire " + wire(session, type, value);
                case "wire_roundtrip" -> roundtrip(session, type, value);
                case "hash" -> "hash " + hash(session, type, value);
                default -> throw new IllegalStateException("unknown field '" + key + "'");
            });
        }
        return out;
    }

    private static <T> String roundtrip(final Session session, final DataComponentType<T> type, final T value) {
        RegistryFriendlyByteBuf buffer = new RegistryFriendlyByteBuf(Unpooled.wrappedBuffer(CodecGoldens.HEX.parseHex(wire(session, type, value))), session.access());
        T decoded = type.streamCodec().decode(buffer);
        return "wire_roundtrip " + json(session, type, decoded).equals(json(session, type, value)) + " remaining " + buffer.readableBytes();
    }

    private static <T> List<String> recordRows(
        final Session session, final String label, final DataComponentType<T> type, final List<String> group
    ) throws Exception {
        boolean wasError = group.stream().anyMatch(line -> line.split(" ", 4)[1].equals("error"));
        String input = RECORD_INPUTS.get(label);
        if (input == null) {
            if (wasError) {
                throw new IllegalStateException("label " + label + " rejects its input and no input is known for it");
            }
            input = group.stream().filter(line -> line.split(" ", 4)[1].equals("json")).findFirst().orElseThrow().split(" ", 4)[3];
        }
        DataResult<T> parsed = type.codecOrThrow().parse(session.json(), JsonParser.parseString(input));
        if (wasError != parsed.result().isEmpty()) {
            throw new IllegalStateException(wasError ? "now accepted" : "now rejected: " + parsed.error().orElseThrow().message());
        }
        if (wasError) {
            return List.of(label + " error = " + parsed.error().orElseThrow().message());
        }
        T value = parsed.result().orElseThrow();
        List<String> out = new ArrayList<>();
        for (String line : group) {
            String key = line.split(" ", 4)[1];
            out.add(label + " " + key + " = " + switch (key) {
                case "json" -> json(session, type, value);
                case "nbt" -> NbtGoldens.binary(nbt(session, type, value));
                case "wire" -> wire(session, type, value);
                case "hash" -> hash(session, type, value);
                default -> throw new IllegalStateException("unknown field '" + key + "'");
            });
        }
        return out;
    }

    private static JsonObject attempt(final Session session, final String label, final JsonObject in, final Row row) throws Exception {
        try {
            return row.write(session, in);
        } catch (RuntimeException failure) {
            throw new IllegalStateException(label + " " + in, failure);
        }
    }

    private static JsonArray rows(final Session session, final JsonObject in, final String section, final Row row) throws Exception {
        JsonArray out = new JsonArray();
        JsonArray rows = in.getAsJsonArray(section);
        for (int index = 0; index < rows.size(); index++) {
            out.add(attempt(session, section + " row " + index, rows.get(index).getAsJsonObject(), row));
        }
        return out;
    }

    private static DataComponentType<?> component(final String id) {
        DataComponentType<?> type = BuiltInRegistries.DATA_COMPONENT_TYPE.getValue(Identifier.parse(id));
        if (type == null) {
            throw new IllegalStateException("unknown component kind '" + id + "'");
        }
        return type;
    }

    private static JsonObject value(final Session session, final JsonObject in) throws Exception {
        String kind = in.get("kind").getAsString();
        return in.has("input")
            ? persistentValue(session, kind, component(kind), in.get("input").getAsString())
            : transientValue(session, kind, component(kind), in.get("value").getAsString());
    }

    private static <T> JsonObject persistentValue(
        final Session session, final String kind, final DataComponentType<T> type, final String input
    ) throws Exception {
        DataResult<T> parsed = type.codecOrThrow().parse(session.json(), JsonParser.parseString(input));
        T value = parsed.result().orElseThrow(() -> new IllegalStateException("now rejected: " + parsed.error().orElseThrow().message()));
        JsonObject member = new JsonObject();
        member.addProperty("kind", kind);
        member.addProperty("input", input);
        member.addProperty("json", json(session, type, value).toString());
        member.addProperty("nbt", NbtGoldens.binary(nbt(session, type, value)));
        member.addProperty("hash", hash(session, type, value));
        member.addProperty("wire", wire(session, type, value));
        return member;
    }

    private static <T> JsonObject transientValue(
        final Session session, final String kind, final DataComponentType<T> type, final String text
    ) {
        T value = valueOf(type, text);
        JsonObject member = new JsonObject();
        member.addProperty("kind", kind);
        member.addProperty("value", text);
        member.addProperty("wire", wire(session, type, value));
        return member;
    }

    @SuppressWarnings("unchecked")
    private static <T> T valueOf(final DataComponentType<T> type, final String text) {
        Type valueType = valueTypes().get(type);
        if (valueType == Integer.class) {
            return (T) Integer.valueOf(text);
        }
        if (valueType == Unit.class) {
            return (T) Unit.INSTANCE;
        }
        if (valueType instanceof Class<?> enumType && enumType.isEnum()) {
            return (T) Enum.valueOf((Class<? extends Enum>) enumType, text);
        }
        throw new IllegalStateException("no way to build a " + valueType + " from '" + text + "'");
    }

    private static JsonObject error(final Session session, final JsonObject in) {
        String kind = in.get("kind").getAsString();
        String input = in.get("input").getAsString();
        JsonObject member = new JsonObject();
        member.addProperty("kind", kind);
        member.addProperty("input", input);
        member.addProperty("error", rejection(session, component(kind), input));
        return member;
    }

    private static <T> String rejection(final Session session, final DataComponentType<T> type, final String input) {
        DataResult<T> parsed = type.codecOrThrow().parse(session.json(), JsonParser.parseString(input));
        if (parsed.result().isPresent()) {
            throw new IllegalStateException("now accepted");
        }
        return parsed.error().orElseThrow().message();
    }

    private static JsonObject decoded(final Session session, final JsonObject in) {
        String kind = in.get("kind").getAsString();
        String wire = in.get("wire").getAsString();
        JsonObject member = new JsonObject();
        member.addProperty("kind", kind);
        member.addProperty("wire", wire);
        member.addProperty("json", decodedJson(session, component(kind), wire));
        return member;
    }

    private static <T> String decodedJson(final Session session, final DataComponentType<T> type, final String wire) {
        T value = unwire(session, type, wire);
        if (type.isTransient()) {
            return value instanceof Enum<?> constant ? constant.name() : String.valueOf(value);
        }
        return json(session, type, value).toString();
    }

    private static JsonObject decodeError(final Session session, final JsonObject in) {
        String kind = in.get("kind").getAsString();
        String wire = in.get("wire").getAsString();
        JsonObject member = new JsonObject();
        member.addProperty("kind", kind);
        member.addProperty("wire", wire);
        member.addProperty("error", refusal(session, component(kind), wire));
        return member;
    }

    private static <T> String refusal(final Session session, final DataComponentType<T> type, final String wire) {
        try {
            unwire(session, type, wire);
        } catch (RuntimeException failure) {
            return failure.getMessage();
        }
        throw new IllegalStateException("now decodes");
    }

    private static JsonObject nestedCase(final Session session, final JsonObject in) throws Exception {
        String kind = in.get("kind").getAsString();
        return nestedCase(session, in.get("name").getAsString(), kind, component(kind), in.get("json").getAsString());
    }

    private static <T> JsonObject nestedCase(
        final Session session, final String name, final String kind, final DataComponentType<T> type, final String input
    ) throws Exception {
        T value = type.codecOrThrow().parse(session.json(), JsonParser.parseString(input)).getOrThrow(IllegalStateException::new);
        JsonObject member = new JsonObject();
        member.addProperty("name", name);
        member.addProperty("kind", kind);
        member.addProperty("json", json(session, type, value).toString());
        member.addProperty("nbt", NbtGoldens.binary(nbt(session, type, value)));
        member.addProperty("wire", wire(session, type, value));
        member.addProperty("hash", hash(session, type, value));
        return member;
    }

    private static JsonObject predicateRow(final Session session, final JsonObject in) {
        String kind = in.get("kind").getAsString();
        return predicateRow(session, kind, component(kind), in.get("input").getAsString(), in.get("ordered").getAsBoolean());
    }

    private static <T> JsonObject predicateRow(
        final Session session, final String kind, final DataComponentType<T> type, final String input, final boolean ordered
    ) {
        T value = type.codecOrThrow().parse(session.json(), JsonParser.parseString(input)).getOrThrow(IllegalStateException::new);
        JsonObject member = new JsonObject();
        member.addProperty("kind", kind);
        member.addProperty("input", input);
        member.add("json", sortedComponents(plainNumbers(json(session, type, value))));
        member.addProperty("hash", hash(session, type, value));
        member.addProperty("wire", wire(session, type, value));
        member.addProperty("ordered", ordered);
        return member;
    }

    private static JsonObject lookup(final Session session, final JsonObject listed) {
        JsonObject out = new JsonObject();
        for (Map.Entry<String, JsonElement> registryEntry : listed.entrySet()) {
            Registry<?> registry = session.access().lookupOrThrow(
                ResourceKey.createRegistryKey(Identifier.withDefaultNamespace(registryEntry.getKey()))
            );
            JsonObject ids = new JsonObject();
            for (String name : registryEntry.getValue().getAsJsonObject().keySet()) {
                Object entry = registry.getValue(Identifier.withDefaultNamespace(name));
                if (entry == null) {
                    throw new IllegalStateException("registry " + registryEntry.getKey() + " has no entry " + name);
                }
                ids.addProperty(name, CodecGoldens.rawId(registry, entry));
            }
            out.add(registryEntry.getKey(), ids);
        }
        return out;
    }

    private static <T> JsonElement json(final Session session, final DataComponentType<T> type, final T value) {
        return type.codecOrThrow().encodeStart(session.json(), value).getOrThrow(IllegalStateException::new);
    }

    private static <T> Tag nbt(final Session session, final DataComponentType<T> type, final T value) {
        return type.codecOrThrow().encodeStart(session.nbt(), value).getOrThrow(IllegalStateException::new);
    }

    private static <T> int hash(final Session session, final DataComponentType<T> type, final T value) {
        return new TypedDataComponent<>(type, value).encodeValue(session.hash()).getOrThrow(IllegalStateException::new).asInt();
    }

    private static <T> String wire(final Session session, final DataComponentType<T> type, final T value) {
        return CodecGoldens.hex(session, type.streamCodec(), value);
    }

    private static <T> T unwire(final Session session, final DataComponentType<T> type, final String wire) {
        RegistryFriendlyByteBuf buffer = new RegistryFriendlyByteBuf(Unpooled.wrappedBuffer(CodecGoldens.HEX.parseHex(wire)), session.access());
        return type.streamCodec().decode(buffer);
    }

    private static Map<DataComponentType<?>, Type> valueTypes() {
        Map<DataComponentType<?>, Type> types = new IdentityHashMap<>();
        for (Field field : DataComponents.class.getFields()) {
            if (!Modifier.isStatic(field.getModifiers()) || field.getType() != DataComponentType.class) {
                continue;
            }
            try {
                types.put((DataComponentType<?>) field.get(null), ((ParameterizedType) field.getGenericType()).getActualTypeArguments()[0]);
            } catch (IllegalAccessException failure) {
                throw new IllegalStateException(failure);
            }
        }
        return types;
    }

    private static Set<Class<?>> registeredClasses() {
        Set<Class<?>> classes = new HashSet<>();
        for (Registry<?> registry : BuiltInRegistries.REGISTRY) {
            for (Object entry : registry) {
                for (Class<?> clazz = entry.getClass(); clazz != Object.class; clazz = clazz.getSuperclass()) {
                    classes.add(clazz);
                }
            }
        }
        return classes;
    }

    private static boolean embedsStacks(final Type type, final Set<Type> seen, final Set<Class<?>> registered) {
        if (!seen.add(type)) {
            return false;
        }
        if (type instanceof ParameterizedType parameterized) {
            if (REFERENCES.contains(parameterized.getRawType())) {
                return false;
            }
            for (Type argument : parameterized.getActualTypeArguments()) {
                if (embedsStacks(argument, seen, registered)) {
                    return true;
                }
            }
            return embedsStacks(parameterized.getRawType(), seen, registered);
        }
        if (!(type instanceof Class<?> clazz) || !clazz.getName().startsWith("net.minecraft.")) {
            return false;
        }
        if (EMBEDDED_STACKS.contains(clazz)) {
            return true;
        }
        if (clazz.isEnum() || clazz.isInterface() || registered.contains(clazz)) {
            return false;
        }
        if (clazz.isRecord()) {
            for (RecordComponent component : clazz.getRecordComponents()) {
                if (embedsStacks(component.getGenericType(), seen, registered)) {
                    return true;
                }
            }
            return false;
        }
        for (Field field : clazz.getDeclaredFields()) {
            if (!Modifier.isStatic(field.getModifiers()) && embedsStacks(field.getGenericType(), seen, registered)) {
                return true;
            }
        }
        return false;
    }

    // Gson prints 12345678.0 as 1.2345678E7; the golden files spell every double without an exponent.
    private static JsonElement plainNumbers(final JsonElement element) {
        if (element instanceof JsonObject object) {
            JsonObject copy = new JsonObject();
            object.entrySet().forEach(entry -> copy.add(entry.getKey(), plainNumbers(entry.getValue())));
            return copy;
        }
        if (element instanceof JsonArray array) {
            JsonArray copy = new JsonArray();
            array.forEach(item -> copy.add(plainNumbers(item)));
            return copy;
        }
        if (element instanceof JsonPrimitive primitive && primitive.isNumber()
            && (primitive.getAsNumber() instanceof Double || primitive.getAsNumber() instanceof Float)) {
            String text = new BigDecimal(primitive.getAsNumber().toString()).stripTrailingZeros().toPlainString();
            return new JsonPrimitive(new LazilyParsedNumber(text.contains(".") ? text : text + ".0"));
        }
        return element;
    }

    // DataComponentExactPredicate encodes through a HashMap keyed by identity-hashed types, so its member order changes between runs.
    private static JsonElement sortedComponents(final JsonElement element) {
        if (element instanceof JsonObject object) {
            JsonObject copy = new JsonObject();
            object.entrySet().forEach(entry -> {
                JsonElement member = sortedComponents(entry.getValue());
                if (entry.getKey().equals("components") && member instanceof JsonObject components) {
                    JsonObject sorted = new JsonObject();
                    components.keySet().stream().sorted().forEach(key -> sorted.add(key, components.get(key)));
                    member = sorted;
                }
                copy.add(entry.getKey(), member);
            });
            return copy;
        }
        if (element instanceof JsonArray array) {
            JsonArray copy = new JsonArray();
            array.forEach(item -> copy.add(sortedComponents(item)));
            return copy;
        }
        return element;
    }

    private static JsonElement read(final Path path) throws Exception {
        return JsonParser.parseString(Files.readString(path, StandardCharsets.UTF_8));
    }
}
