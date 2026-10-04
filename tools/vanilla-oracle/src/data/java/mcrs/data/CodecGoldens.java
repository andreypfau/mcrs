package mcrs.data;

import com.google.common.hash.HashCode;
import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.google.gson.stream.JsonWriter;
import com.mojang.serialization.JsonOps;
import io.netty.buffer.Unpooled;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.SharedConstants;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryOps;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.util.HashOps;

public final class CodecGoldens {
    @FunctionalInterface
    interface Golden {
        void write(Path current, Path output) throws Exception;
    }

    record Session(
        RegistryAccess.Frozen access, RegistryOps<JsonElement> json, RegistryOps<Tag> nbt, RegistryOps<HashCode> hash
    ) {}

    @FunctionalInterface
    interface Body {
        void write(Session session) throws Exception;
    }

    @FunctionalInterface
    interface JsonBody {
        void write(JsonWriter out) throws Exception;
    }

    static final HexFormat HEX = HexFormat.of();
    static final Gson GSON = new GsonBuilder().disableHtmlEscaping().serializeNulls().create();

    private static final Map<String, Golden> GOLDENS = new LinkedHashMap<>();

    static {
        GOLDENS.put("snbt", NbtGoldens::snbt);
        GOLDENS.put("hash_ops", NbtGoldens::hashOps);
        GOLDENS.put("text_vanilla", TextGoldens::vanilla);
        GOLDENS.put("text_probe", TextGoldens::probe);
        GOLDENS.put("text_nbt", TextGoldens::nbt);
        GOLDENS.put("text_wire", TextGoldens::wire);
        GOLDENS.put("item_plain", ItemGoldens::plain);
        GOLDENS.put("item_nested", ItemGoldens::nested);
        GOLDENS.put("item_predicate", ItemGoldens::predicate);
        GOLDENS.put("item_kinds", ItemGoldens::kinds);
        GOLDENS.put("item_holders", ItemGoldens::holders);
        GOLDENS.put("item_registry_refs", ItemGoldens::registryRefs);
        GOLDENS.put("item_records", ItemGoldens::records);
        GOLDENS.put("recipe_packets", PacketGoldens::recipePackets);
        GOLDENS.put("particles", PacketGoldens::particles);
        GOLDENS.put("inventory_packets", PacketGoldens::inventoryPackets);
        GOLDENS.put("join_packets", PacketGoldens::joinPackets);
        GOLDENS.put("serverbound_game_packets", PacketGoldens::serverboundGamePackets);
        GOLDENS.put("frames", FrameGoldens::frames);
        GOLDENS.put("vanilla_player", PlayerGoldens::vanillaPlayer);
        GOLDENS.put("registry_values", RegistryGoldens::registryValues);
    }

    private CodecGoldens() {}

    public static void main(final String[] args) {
        int status;
        try {
            run(Path.of(args[0]), args[1], Path.of(args[2]));
            status = 0;
        } catch (Throwable failure) {
            failure.printStackTrace();
            status = 1;
        }
        System.exit(status);
    }

    private static void run(final Path outDir, final String name, final Path current) throws Exception {
        Golden golden = GOLDENS.get(name);
        if (golden == null) {
            throw new IllegalArgumentException("unknown golden '" + name + "'; known goldens: " + String.join(", ", GOLDENS.keySet()));
        }
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();

        Files.createDirectories(outDir);
        Path output = outDir.resolve(current.getFileName());
        golden.write(current, output);
        System.out.println("wrote " + output);
    }

    static void withSession(final Body body) throws Exception {
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

    static String idLine(final Session session, final String line, final boolean namespacedRegistry) {
        String[] parts = line.split(" ");
        if (parts.length != 4) {
            throw new IllegalStateException("malformed id line: " + line);
        }
        Identifier registryId = namespacedRegistry ? Identifier.parse(parts[1]) : Identifier.withDefaultNamespace(parts[1]);
        Registry<?> registry = session.access().lookupOrThrow(ResourceKey.createRegistryKey(registryId));
        Object entry = registry.getValue(Identifier.parse(parts[2]));
        if (entry == null) {
            throw new IllegalStateException("registry " + parts[1] + " has no entry " + parts[2]);
        }
        return "id " + parts[1] + " " + parts[2] + " " + rawId(registry, entry);
    }

    @SuppressWarnings("unchecked")
    static int rawId(final Registry<?> registry, final Object entry) {
        return ((Registry<Object>) registry).getId(entry);
    }

    static <T> String hex(final Session session, final StreamCodec<? super RegistryFriendlyByteBuf, T> codec, final T value) {
        RegistryFriendlyByteBuf buffer = new RegistryFriendlyByteBuf(Unpooled.buffer(), session.access());
        codec.encode(buffer, value);
        byte[] bytes = new byte[buffer.readableBytes()];
        buffer.readBytes(bytes);
        return HEX.formatHex(bytes);
    }

    static void writeLines(final Path output, final List<String> lines) throws Exception {
        Files.writeString(output, String.join("\n", lines) + "\n", StandardCharsets.UTF_8);
    }

    static void writeJson(final Path output, final String indent, final boolean endsWithNewline, final JsonBody body) throws Exception {
        StringWriter text = new StringWriter();
        try (JsonWriter json = new JsonWriter(text)) {
            json.setIndent(indent);
            json.setHtmlSafe(false);
            body.write(json);
        }
        Files.writeString(output, endsWithNewline ? text + "\n" : text.toString(), StandardCharsets.UTF_8);
    }

    static void writeJson(final Path output, final String indent, final boolean endsWithNewline, final JsonElement root) throws Exception {
        writeJson(output, indent, endsWithNewline, json -> GSON.toJson(root, json));
    }
}
