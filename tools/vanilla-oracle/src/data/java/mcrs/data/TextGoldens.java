package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.stream.JsonWriter;
import com.mojang.serialization.DataResult;
import com.mojang.serialization.JsonOps;
import io.netty.buffer.Unpooled;
import java.io.ByteArrayInputStream;
import java.io.DataInputStream;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.function.Supplier;
import net.minecraft.ChatFormatting;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.resources.RegistryOps;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;

final class TextGoldens {
    private static final HexFormat HEX = HexFormat.of();
    private static final Gson GSON = new GsonBuilder().disableHtmlEscaping().serializeNulls().create();

    private static final Map<String, Supplier<Component>> TYPED_WIRE_CASES = Map.of(
        "kill_multiple", () -> Component.translatable("commands.kill.success.multiple", 5),
        "mixed_args", () -> Component.translatable(
            "k", 5, true, 1.5f, 2.5d, 7L, "s", Component.literal("c").withStyle(ChatFormatting.RED)
        )
    );

    private TextGoldens() {}

    private record Session(RegistryAccess.Frozen access, RegistryOps<JsonElement> json, RegistryOps<Tag> nbt) {}

    @FunctionalInterface
    private interface Body {
        void write(Session session) throws Exception;
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
                access.createSerializationContext(NbtOps.INSTANCE)
            ));
        }
    }

    static void vanilla(final Path current, final Path output) throws Exception {
        forwardCases(current, output, "  ");
    }

    static void probe(final Path current, final Path output) throws Exception {
        forwardCases(current, output, " ");
    }

    static void nbt(final Path current, final Path output) throws Exception {
        withSession(session -> {
            JsonArray out = new JsonArray();
            for (JsonElement element : cases(current)) {
                JsonObject in = element.getAsJsonObject();
                String name = in.get("name").getAsString();
                try {
                    String hex = in.get("input").getAsString();
                    Tag tag = NbtIo.readAnyTag(
                        new DataInputStream(new ByteArrayInputStream(HEX.parseHex(hex))), NbtAccounter.unlimitedHeap()
                    );
                    JsonObject member = new JsonObject();
                    member.addProperty("name", name);
                    member.addProperty("input", hex);
                    member.addProperty("input_snbt", tag.toString());
                    result(session, name, in.has("error"), ComponentSerialization.CODEC.parse(session.nbt(), tag), member, false);
                    out.add(member);
                } catch (RuntimeException failure) {
                    throw new IllegalStateException("case " + name, failure);
                }
            }
            write(output, " ", out);
        });
    }

    static void wire(final Path current, final Path output) throws Exception {
        withSession(session -> {
            JsonArray out = new JsonArray();
            for (JsonElement element : cases(current)) {
                JsonObject in = element.getAsJsonObject();
                String name = in.get("name").getAsString();
                try {
                    Component component = TYPED_WIRE_CASES.containsKey(name)
                        ? TYPED_WIRE_CASES.get(name).get()
                        : ComponentSerialization.CODEC.parse(session.json(), in.get("json")).getOrThrow(IllegalStateException::new);
                    RegistryFriendlyByteBuf buffer = new RegistryFriendlyByteBuf(Unpooled.buffer(), session.access());
                    ComponentSerialization.STREAM_CODEC.encode(buffer, component);
                    byte[] bytes = new byte[buffer.readableBytes()];
                    buffer.readBytes(bytes);

                    JsonObject member = new JsonObject();
                    member.addProperty("name", name);
                    member.add("json", ComponentSerialization.CODEC.encodeStart(session.json(), component).getOrThrow(IllegalStateException::new));
                    member.addProperty("wire", HEX.formatHex(bytes));
                    out.add(member);
                } catch (RuntimeException failure) {
                    throw new IllegalStateException("case " + name, failure);
                }
            }
            write(output, " ", out);
        });
    }

    private static void forwardCases(final Path current, final Path output, final String indent) throws Exception {
        withSession(session -> {
            JsonArray out = new JsonArray();
            for (JsonElement element : cases(current)) {
                JsonObject in = element.getAsJsonObject();
                String name = in.get("name").getAsString();
                try {
                    JsonElement input = in.get("input");
                    JsonObject member = new JsonObject();
                    member.addProperty("name", name);
                    member.add("input", input);
                    result(session, name, in.has("error"), ComponentSerialization.CODEC.parse(session.json(), input), member, true);
                    out.add(member);
                } catch (RuntimeException failure) {
                    throw new IllegalStateException("case " + name, failure);
                }
            }
            write(output, indent, out);
        });
    }

    private static void result(
        final Session session,
        final String name,
        final boolean recordedRejection,
        final DataResult<Component> parsed,
        final JsonObject member,
        final boolean withBinary
    ) throws Exception {
        if (parsed.result().isEmpty()) {
            if (!recordedRejection) {
                throw new IllegalStateException("case " + name + " is now rejected: " + parsed.error().orElseThrow().message());
            }
            member.addProperty("error", parsed.error().orElseThrow().message());
            return;
        }
        if (recordedRejection) {
            throw new IllegalStateException("case " + name + " is now accepted");
        }
        Component component = parsed.result().orElseThrow();
        member.add("json", ComponentSerialization.CODEC.encodeStart(session.json(), component).getOrThrow(IllegalStateException::new));
        Tag tag = ComponentSerialization.CODEC.encodeStart(session.nbt(), component).getOrThrow(IllegalStateException::new);
        if (withBinary) {
            member.addProperty("nbt", NbtGoldens.binary(tag));
        }
        member.addProperty("snbt", tag.toString());
    }

    private static JsonArray cases(final Path current) throws Exception {
        return JsonParser.parseString(Files.readString(current, StandardCharsets.UTF_8)).getAsJsonArray();
    }

    private static void write(final Path output, final String indent, final JsonArray cases) throws Exception {
        StringWriter text = new StringWriter();
        try (JsonWriter json = new JsonWriter(text)) {
            json.setIndent(indent);
            GSON.toJson(cases, json);
        }
        Files.writeString(output, text.toString(), StandardCharsets.UTF_8);
    }
}
