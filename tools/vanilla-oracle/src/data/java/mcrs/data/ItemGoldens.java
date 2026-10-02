package mcrs.data;

import com.google.common.hash.HashCode;
import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import com.google.gson.internal.LazilyParsedNumber;
import com.google.gson.stream.JsonWriter;
import com.mojang.serialization.DataResult;
import com.mojang.serialization.JsonOps;
import io.netty.buffer.Unpooled;
import java.io.StringWriter;
import java.math.BigDecimal;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.RecordComponent;
import java.lang.reflect.Type;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderSet;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.component.DataComponentExactPredicate;
import net.minecraft.core.component.DataComponentMap;
import net.minecraft.core.component.DataComponentPatch;
import net.minecraft.core.component.DataComponentType;
import net.minecraft.core.component.DataComponents;
import net.minecraft.core.component.TypedDataComponent;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryOps;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagKey;
import net.minecraft.util.HashOps;
import net.minecraft.util.Unit;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.ItemStackTemplate;

final class ItemGoldens {
    private static final HexFormat HEX = HexFormat.of();
    private static final Gson GSON = new GsonBuilder().disableHtmlEscaping().serializeNulls().create();
    private static final Set<Class<?>> EMBEDDED_STACKS = Set.of(
        ItemStackTemplate.class, ItemStack.class, DataComponentPatch.class, DataComponentMap.class, DataComponentExactPredicate.class
    );
    private static final Set<Class<?>> REFERENCES = Set.of(Holder.class, HolderSet.class, ResourceKey.class, TagKey.class, Registry.class);

    private ItemGoldens() {}

    private record Session(
        RegistryAccess.Frozen access, RegistryOps<JsonElement> json, RegistryOps<Tag> nbt, RegistryOps<HashCode> hash
    ) {}

    @FunctionalInterface
    private interface Body {
        void write(Session session) throws Exception;
    }

    @FunctionalInterface
    private interface Row {
        JsonObject write(Session session, JsonObject in) throws Exception;
    }

    private static void withSession(final Body body) throws Exception {
        try (MultiPackResourceManager resources = new MultiPackResourceManager(
                PackType.SERVER_DATA, List.of(ServerPacksSource.createVanillaPackSource().fullResources())
            )) {
            LayeredRegistryAccess<RegistryLayer> layers = RegistryLayer.createRegistryAccess();
            RegistryAccess.Frozen loaded = BlockDefinitionDumper.loadWorldRegistries(resources, layers);
            RegistryAccess.Frozen access = layers.replaceFrom(RegistryLayer.WORLD, loaded).compositeAccess();
            BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(access).forEach(pending -> pending.apply());
            body.write(new Session(
                access,
                access.createSerializationContext(JsonOps.INSTANCE),
                access.createSerializationContext(NbtOps.INSTANCE),
                access.createSerializationContext(HashOps.CRC32C_INSTANCE)
            ));
        }
    }

    static void plain(final Path current, final Path output) throws Exception {
        withSession(session -> {
            JsonObject in = read(current).getAsJsonObject();
            JsonObject out = new JsonObject();
            out.add("values", rows(session, in, "values", ItemGoldens::value));
            out.add("errors", rows(session, in, "errors", ItemGoldens::error));
            out.add("decodes", rows(session, in, "decodes", ItemGoldens::decoded));
            out.add("decode_errors", rows(session, in, "decode_errors", ItemGoldens::decodeError));
            write(output, "  ", true, out);
        });
    }

    static void nested(final Path current, final Path output) throws Exception {
        withSession(session -> {
            JsonObject in = read(current).getAsJsonObject();
            JsonObject out = new JsonObject();
            out.add("lookup", lookup(session, in.getAsJsonObject("lookup")));
            out.add("cases", rows(session, in, "cases", ItemGoldens::nestedCase));
            write(output, " ", false, out);
        });
    }

    static void predicate(final Path current, final Path output) throws Exception {
        withSession(session -> {
            JsonArray out = new JsonArray();
            JsonArray in = read(current).getAsJsonArray();
            for (int index = 0; index < in.size(); index++) {
                out.add(attempt(session, "row " + index, in.get(index).getAsJsonObject(), ItemGoldens::predicateRow));
            }
            write(output, " ", false, out);
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
        write(output, "  ", true, out);
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
        member.add("json", plainNumbers(json(session, type, value)));
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
                ids.addProperty(name, rawId(registry, entry));
            }
            out.add(registryEntry.getKey(), ids);
        }
        return out;
    }

    @SuppressWarnings("unchecked")
    private static int rawId(final Registry<?> registry, final Object entry) {
        return ((Registry<Object>) registry).getId(entry);
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
        RegistryFriendlyByteBuf buffer = new RegistryFriendlyByteBuf(Unpooled.buffer(), session.access());
        type.streamCodec().encode(buffer, value);
        byte[] bytes = new byte[buffer.readableBytes()];
        buffer.readBytes(bytes);
        return HEX.formatHex(bytes);
    }

    private static <T> T unwire(final Session session, final DataComponentType<T> type, final String wire) {
        RegistryFriendlyByteBuf buffer = new RegistryFriendlyByteBuf(Unpooled.wrappedBuffer(HEX.parseHex(wire)), session.access());
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

    private static JsonElement read(final Path path) throws Exception {
        return JsonParser.parseString(Files.readString(path, StandardCharsets.UTF_8));
    }

    private static void write(final Path output, final String indent, final boolean endsWithNewline, final JsonElement root) throws Exception {
        StringWriter text = new StringWriter();
        try (JsonWriter json = new JsonWriter(text)) {
            json.setIndent(indent);
            GSON.toJson(root, json);
        }
        Files.writeString(output, endsWithNewline ? text + "\n" : text.toString(), StandardCharsets.UTF_8);
    }
}
