package mcrs.data;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.serialization.DataResult;
import java.io.ByteArrayInputStream;
import java.io.DataInputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.function.Supplier;
import mcrs.data.CodecGoldens.Session;
import net.minecraft.ChatFormatting;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.Tag;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;

final class TextGoldens {
    private static final Map<String, Supplier<Component>> TYPED_WIRE_CASES = Map.of(
        "kill_multiple", () -> Component.translatable("commands.kill.success.multiple", 5),
        "mixed_args", () -> Component.translatable(
            "k", 5, true, 1.5f, 2.5d, 7L, "s", Component.literal("c").withStyle(ChatFormatting.RED)
        )
    );

    private TextGoldens() {}

    @FunctionalInterface
    private interface Case {
        JsonObject write(Session session, String name, JsonObject in) throws Exception;
    }

    private static void eachCase(final Path current, final Path output, final String indent, final Case body) throws Exception {
        CodecGoldens.withSession(session -> {
            JsonArray out = new JsonArray();
            for (JsonElement element : cases(current)) {
                JsonObject in = element.getAsJsonObject();
                String name = in.get("name").getAsString();
                try {
                    out.add(body.write(session, name, in));
                } catch (RuntimeException failure) {
                    throw new IllegalStateException("case " + name, failure);
                }
            }
            CodecGoldens.writeJson(output, indent, false, out);
        });
    }

    static void vanilla(final Path current, final Path output) throws Exception {
        forwardCases(current, output, "  ");
    }

    static void probe(final Path current, final Path output) throws Exception {
        forwardCases(current, output, " ");
    }

    static void nbt(final Path current, final Path output) throws Exception {
        eachCase(current, output, " ", (session, name, in) -> {
            String hex = in.get("input").getAsString();
            Tag tag = NbtIo.readAnyTag(
                new DataInputStream(new ByteArrayInputStream(CodecGoldens.HEX.parseHex(hex))), NbtAccounter.unlimitedHeap()
            );
            JsonObject member = new JsonObject();
            member.addProperty("name", name);
            member.addProperty("input", hex);
            member.addProperty("input_snbt", tag.toString());
            result(session, name, in.has("error"), ComponentSerialization.CODEC.parse(session.nbt(), tag), member, false);
            return member;
        });
    }

    static void wire(final Path current, final Path output) throws Exception {
        eachCase(current, output, " ", (session, name, in) -> {
            Component component = TYPED_WIRE_CASES.containsKey(name)
                ? TYPED_WIRE_CASES.get(name).get()
                : ComponentSerialization.CODEC.parse(session.json(), in.get("json")).getOrThrow(IllegalStateException::new);
            JsonObject member = new JsonObject();
            member.addProperty("name", name);
            member.add("json", ComponentSerialization.CODEC.encodeStart(session.json(), component).getOrThrow(IllegalStateException::new));
            member.addProperty("wire", CodecGoldens.hex(session, ComponentSerialization.STREAM_CODEC, component));
            return member;
        });
    }

    private static void forwardCases(final Path current, final Path output, final String indent) throws Exception {
        eachCase(current, output, indent, (session, name, in) -> {
            JsonElement input = in.get("input");
            JsonObject member = new JsonObject();
            member.addProperty("name", name);
            member.add("input", input);
            result(session, name, in.has("error"), ComponentSerialization.CODEC.parse(session.json(), input), member, true);
            return member;
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
}
