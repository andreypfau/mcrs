package mcrs.data;

import com.google.common.hash.HashCode;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.stream.JsonWriter;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import com.mojang.serialization.DynamicOps;
import com.mojang.serialization.JsonOps;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.function.Supplier;
import java.util.stream.Stream;
import net.minecraft.nbt.ByteArrayTag;
import net.minecraft.nbt.ByteTag;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.DoubleTag;
import net.minecraft.nbt.EndTag;
import net.minecraft.nbt.FloatTag;
import net.minecraft.nbt.IntArrayTag;
import net.minecraft.nbt.IntTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.LongArrayTag;
import net.minecraft.nbt.LongTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.ShortTag;
import net.minecraft.nbt.StringTag;
import net.minecraft.nbt.Tag;
import net.minecraft.nbt.TagParser;
import net.minecraft.util.HashOps;

final class NbtGoldens {
    private static final HexFormat HEX = HexFormat.of();
    private static final TagParser<Tag> PARSER = TagParser.create(NbtOps.INSTANCE);

    private NbtGoldens() {}

    static void snbt(final Path current, final Path output) throws Exception {
        JsonObject in = JsonParser.parseString(Files.readString(current, StandardCharsets.UTF_8)).getAsJsonObject();

        emit(output, out -> {
            out.beginObject();

            out.name("parses").beginArray();
            for (JsonElement element : in.getAsJsonArray("parses")) {
                String input = element.getAsJsonObject().get("input").getAsString();
                Tag tag = parse(input);
                out.beginObject();
                out.name("input").value(input);
                out.name("hex").value(binary(tag));
                out.name("snbt").value(tag.toString());
                out.endObject();
            }
            out.endArray();

            out.name("rejects").beginArray();
            for (JsonElement element : in.getAsJsonArray("rejects")) {
                String input = element.getAsString();
                try {
                    PARSER.parseFully(input);
                } catch (CommandSyntaxException refused) {
                    out.value(input);
                    continue;
                }
                throw new IllegalStateException("input is now accepted by the parser: " + input);
            }
            out.endArray();

            out.name("doubles").beginArray();
            for (JsonElement element : in.getAsJsonArray("doubles")) {
                long bits = element.getAsJsonObject().get("bits").getAsBigInteger().longValue();
                out.beginObject();
                out.name("bits").jsonValue(Long.toUnsignedString(bits));
                out.name("text").value(Double.toString(Double.longBitsToDouble(bits)));
                out.endObject();
            }
            out.endArray();

            out.name("floats").beginArray();
            for (JsonElement element : in.getAsJsonArray("floats")) {
                int bits = element.getAsJsonObject().get("bits").getAsBigInteger().intValue();
                out.beginObject();
                out.name("bits").jsonValue(Integer.toUnsignedString(bits));
                out.name("text").value(Float.toString(Float.intBitsToFloat(bits)));
                out.endObject();
            }
            out.endArray();

            out.name("json_typing").beginArray();
            for (JsonElement element : in.getAsJsonArray("json_typing")) {
                String json = element.getAsJsonObject().get("json").getAsString();
                Tag tag = JsonOps.INSTANCE.convertTo(NbtOps.INSTANCE, JsonParser.parseString(json));
                out.beginObject();
                out.name("json").value(json);
                out.name("tag").value(tag.toString());
                out.endObject();
            }
            out.endArray();

            Tag pretty = NbtIo.readAnyTag(
                new DataInputStream(new ByteArrayInputStream(HEX.parseHex(in.get("pretty_hex").getAsString()))),
                NbtAccounter.unlimitedHeap()
            );
            out.name("pretty").value(pretty.toString());
            out.name("pretty_hex").value(binary(pretty));

            out.endObject();
        });
    }

    static void hashOps(final Path current, final Path output) throws Exception {
        JsonObject in = JsonParser.parseString(Files.readString(current, StandardCharsets.UTF_8)).getAsJsonObject();
        Map<String, Supplier<HashCode>> inputs = hashInputs();
        if (!in.keySet().equals(inputs.keySet())) {
            throw new IllegalStateException("the hash inputs " + inputs.keySet() + " do not match the golden's members " + in.keySet());
        }

        emit(output, out -> {
            out.beginObject();
            for (Map.Entry<String, Supplier<HashCode>> input : inputs.entrySet()) {
                out.name(input.getKey()).value(input.getValue().get().asInt());
            }
            out.endObject();
        });
    }

    private static Map<String, Supplier<HashCode>> hashInputs() {
        HashOps ops = HashOps.CRC32C_INSTANCE;
        Map<String, Supplier<HashCode>> inputs = new LinkedHashMap<>();
        inputs.put("empty", () -> hash(EndTag.INSTANCE));
        inputs.put("empty_map", () -> hash(new CompoundTag()));
        inputs.put("empty_list", () -> hash(new ListTag()));
        inputs.put("a_byte_1", () -> hash(compound("a", ByteTag.valueOf((byte) 1))));
        inputs.put("a_int_1", () -> hash(compound("a", IntTag.valueOf(1))));
        inputs.put("a_short_neg2", () -> hash(compound("a", ShortTag.valueOf((short) -2))));
        inputs.put("str_empty", () -> hash(StringTag.valueOf("")));
        inputs.put("str_hello", () -> hash(StringTag.valueOf("hello")));
        inputs.put("str_astral", () -> hash(StringTag.valueOf("a😀b")));
        inputs.put("list_ints", () -> hash(list(IntTag.valueOf(1), IntTag.valueOf(-2), IntTag.valueOf(300))));
        inputs.put("nested", () -> {
            CompoundTag inner = new CompoundTag();
            inner.put("x", StringTag.valueOf("y"));
            inner.put("n", LongTag.valueOf(-5L));
            CompoundTag nested = new CompoundTag();
            nested.put("inner", inner);
            nested.put("flag", ByteTag.valueOf((byte) 1));
            nested.put("pos", new IntArrayTag(new int[] {1, 2, 3}));
            return hash(nested);
        });
        inputs.put("int_array", () -> hash(new IntArrayTag(new int[] {7, -8, 9})));
        inputs.put("long_array", () -> hash(new LongArrayTag(new long[] {1L << 40, -1L})));
        inputs.put("byte_array", () -> hash(new ByteArrayTag(new byte[] {1, 2, -3})));
        inputs.put("float", () -> hash(FloatTag.valueOf(1.5f)));
        inputs.put("double", () -> hash(DoubleTag.valueOf(-2.25)));
        inputs.put("long", () -> hash(LongTag.valueOf(1234567890123L)));
        inputs.put("bool_true", () -> ops.createBoolean(true));
        inputs.put("bool_false", () -> ops.createBoolean(false));
        inputs.put("byte_true", () -> hash(ByteTag.valueOf((byte) 1)));
        inputs.put("list_of_bools", () -> ops.createList(Stream.of(ops.createBoolean(true), ops.createBoolean(false))));
        inputs.put("sixteen_keys", () -> {
            CompoundTag sixteen = new CompoundTag();
            for (char key = 'a'; key <= 'p'; key++) {
                sixteen.put(String.valueOf(key), IntTag.valueOf(key));
            }
            return hash(sixteen);
        });
        inputs.put("mixed_sign_keys", () -> {
            CompoundTag mixed = new CompoundTag();
            mixed.put("k", StringTag.valueOf("v"));
            mixed.put("k2", StringTag.valueOf("v2"));
            mixed.put("count", StringTag.valueOf("1"));
            mixed.put("id", StringTag.valueOf("x"));
            return hash(mixed);
        });
        inputs.put("key_a", () -> hash(StringTag.valueOf("a")));
        inputs.put("key_h", () -> hash(StringTag.valueOf("h")));
        inputs.put("key_p", () -> hash(StringTag.valueOf("p")));
        return inputs;
    }

    private static HashCode hash(final Tag tag) {
        DynamicOps<HashCode> ops = HashOps.CRC32C_INSTANCE;
        return NbtOps.INSTANCE.convertTo(ops, tag);
    }

    private static CompoundTag compound(final String key, final Tag value) {
        CompoundTag tag = new CompoundTag();
        tag.put(key, value);
        return tag;
    }

    private static ListTag list(final Tag... elements) {
        ListTag list = new ListTag();
        for (Tag element : elements) {
            list.add(element);
        }
        return list;
    }

    private static Tag parse(final String input) {
        try {
            return PARSER.parseFully(input);
        } catch (CommandSyntaxException refused) {
            throw new IllegalStateException("input is no longer accepted by the parser: " + input, refused);
        }
    }

    static String binary(final Tag tag) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        NbtIo.writeAnyTag(tag, new DataOutputStream(bytes));
        return HEX.formatHex(bytes.toByteArray());
    }

    @FunctionalInterface
    private interface Body {
        void write(JsonWriter out) throws Exception;
    }

    private static void emit(final Path output, final Body body) throws Exception {
        StringWriter text = new StringWriter();
        try (JsonWriter json = new JsonWriter(text)) {
            json.setIndent("  ");
            json.setHtmlSafe(false);
            body.write(json);
        }
        Files.writeString(output, text + "\n", StandardCharsets.UTF_8);
    }
}
