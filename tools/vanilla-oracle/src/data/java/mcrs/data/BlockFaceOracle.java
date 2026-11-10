package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.math.Quadrant;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.Reader;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Proxy;
import java.net.JarURLConnection;
import java.net.URI;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.jar.JarFile;
import java.util.stream.Stream;
import net.minecraft.SharedConstants;
import net.minecraft.client.renderer.block.dispatch.BlockModelRotation;
import net.minecraft.client.renderer.block.dispatch.ModelState;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.model.ModelBaker;
import net.minecraft.client.resources.model.cuboid.CuboidModel;
import net.minecraft.client.resources.model.cuboid.UnbakedCuboidGeometry;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.client.resources.model.geometry.QuadCollection;
import net.minecraft.client.resources.model.geometry.UnbakedGeometry;
import net.minecraft.client.resources.model.sprite.Material;
import net.minecraft.client.resources.model.sprite.MaterialBaker;
import net.minecraft.client.resources.model.sprite.TextureSlots;
import net.minecraft.client.resources.model.ModelDebugName;
import net.minecraft.client.model.geom.builders.UVPair;
import net.minecraft.core.Direction;
import org.joml.Vector3fc;

/**
 * Bakes the cuboid geometry of every distinct (model, x, y, z, uvlock) entry of the vanilla
 * blockstate files and writes the faces as JSON. Each texture slot maps to a sprite that covers
 * the whole atlas, so the written uv is local to the sprite. Nothing is drawn.
 *
 * <p>Arguments: the output directory.
 */
public final class BlockFaceOracle {
    private static final int FORMAT = 1;
    private static final Gson LINE = new GsonBuilder().disableHtmlEscaping().serializeNulls().create();
    private static final String BLOCKSTATES = "assets/minecraft/blockstates/";
    private static final String MODELS = "assets/minecraft/models/";

    private BlockFaceOracle() {}

    public static void main(final String[] args) {
        int status;
        try {
            SharedConstants.tryDetectVersion();
            Path out = Path.of(args[0]);
            Files.createDirectories(out);
            write(out.resolve("block_faces.json"), cases());
            status = 0;
        } catch (Throwable failure) {
            failure.printStackTrace();
            status = 1;
        }
        System.exit(status);
    }

    private record Entry(String model, int x, int y, int z, boolean uvlock) implements Comparable<Entry> {
        @Override
        public int compareTo(final Entry other) {
            int c = this.model.compareTo(other.model);
            if (c != 0) return c;
            c = Integer.compare(this.x, other.x);
            if (c != 0) return c;
            c = Integer.compare(this.y, other.y);
            if (c != 0) return c;
            c = Integer.compare(this.z, other.z);
            if (c != 0) return c;
            return Boolean.compare(this.uvlock, other.uvlock);
        }
    }

    private static List<JsonObject> cases() throws Exception {
        TreeSet<Entry> entries = new TreeSet<>();
        for (String name : list(BLOCKSTATES)) {
            JsonObject root = JsonParser.parseString(read(name)).getAsJsonObject();
            if (root.has("variants")) {
                for (Map.Entry<String, JsonElement> variant : root.getAsJsonObject("variants").entrySet()) {
                    collect(variant.getValue(), entries);
                }
            }
            if (root.has("multipart")) {
                for (JsonElement part : root.getAsJsonArray("multipart")) {
                    collect(part.getAsJsonObject().get("apply"), entries);
                }
            }
        }

        TreeSet<Entry> owners = new TreeSet<>();
        for (Entry entry : entries) {
            String owner = geometryOwner(entry.model);
            if (owner != null) {
                owners.add(new Entry(owner, entry.x, entry.y, entry.z, entry.uvlock));
            }
        }
        List<JsonObject> rows = new ArrayList<>();
        for (Entry entry : owners) {
            UnbakedGeometry top = topGeometry(entry.model);
            if (!(top instanceof UnbakedCuboidGeometry cuboid) || cuboid.elements().isEmpty()) {
                continue;
            }
            rows.add(bake(entry, cuboid));
        }
        return rows;
    }

    private static void collect(final JsonElement value, final TreeSet<Entry> entries) {
        if (value.isJsonArray()) {
            for (JsonElement one : value.getAsJsonArray()) {
                collect(one, entries);
            }
            return;
        }
        JsonObject object = value.getAsJsonObject();
        entries.add(new Entry(
            qualify(object.get("model").getAsString()),
            object.has("x") ? object.get("x").getAsInt() : 0,
            object.has("y") ? object.get("y").getAsInt() : 0,
            object.has("z") ? object.get("z").getAsInt() : 0,
            object.has("uvlock") && object.get("uvlock").getAsBoolean()
        ));
    }

    private static String qualify(final String id) {
        return id.contains(":") ? id : "minecraft:" + id;
    }

    private static final Map<String, String> OWNERS = new TreeMap<>();

    private static String geometryOwner(final String model) {
        if (OWNERS.containsKey(model)) {
            return OWNERS.get(model);
        }
        String current = model;
        String owner = null;
        while (current != null) {
            String text;
            try {
                text = read(MODELS + current.substring(current.indexOf(':') + 1) + ".json");
            } catch (IOException missing) {
                break;
            }
            JsonObject object = JsonParser.parseString(text).getAsJsonObject();
            if (object.has("elements")) {
                owner = current;
                break;
            }
            current = object.has("parent") ? qualify(object.get("parent").getAsString()) : null;
        }
        OWNERS.put(model, owner);
        return owner;
    }

    private static UnbakedGeometry topGeometry(final String model) {
        String current = model;
        while (current != null) {
            String path = MODELS + current.substring(current.indexOf(':') + 1) + ".json";
            String text;
            try {
                text = read(path);
            } catch (IOException missing) {
                return null;
            }
            CuboidModel parsed;
            try (Reader reader = new java.io.StringReader(text)) {
                parsed = CuboidModel.fromStream(reader);
            } catch (IOException impossible) {
                throw new IllegalStateException(impossible);
            }
            if (parsed.geometry() != null) {
                return parsed.geometry();
            }
            current = parsed.parent() == null ? null : parsed.parent().toString();
        }
        return null;
    }

    private static JsonObject bake(final Entry entry, final UnbakedCuboidGeometry cuboid) throws Exception {
        BlockModelRotation rotation = BlockModelRotation.get(Quadrant.fromXYZAngles(
            Quadrant.parseJson(entry.x), Quadrant.parseJson(entry.y), Quadrant.parseJson(entry.z)
        ));
        ModelState state = entry.uvlock ? rotation.withUvLock() : rotation;

        Map<TextureAtlasSprite, String> slots = new IdentityHashMap<>();
        Map<String, TextureAtlasSprite> sprites = new TreeMap<>();
        FakeMaterials materials = allocate(FakeMaterials.class);
        materials.resolver = id -> new Material.Baked(sprites.computeIfAbsent(id, key -> {
            TextureAtlasSprite sprite = wholeAtlasSprite();
            slots.put(sprite, key);
            return sprite;
        }), true);
        ModelBaker.Interner interner = new ModelBaker.Interner() {
            @Override
            public Vector3fc vector(final Vector3fc vector) {
                return vector;
            }

            @Override
            public BakedQuad.MaterialInfo materialInfo(final BakedQuad.MaterialInfo material) {
                return material;
            }
        };
        InvocationHandler handler = (proxy, method, arguments) -> switch (method.getName()) {
            case "materials" -> materials;
            case "interner" -> interner;
            default -> throw new UnsupportedOperationException(method.getName());
        };
        ModelBaker baker = (ModelBaker) Proxy.newProxyInstance(
            ModelBaker.class.getClassLoader(), new Class<?>[]{ModelBaker.class}, handler
        );
        ModelDebugName name = () -> entry.model;
        QuadCollection quads = UnbakedCuboidGeometry.bake(cuboid.elements(), null, baker, state, name);

        Map<BakedQuad, Direction> cull = new IdentityHashMap<>();
        for (Direction direction : Direction.values()) {
            for (BakedQuad quad : quads.getQuads(direction)) {
                cull.put(quad, direction);
            }
        }

        JsonArray faces = new JsonArray();
        for (BakedQuad quad : quads.getAll()) {
            JsonObject face = new JsonObject();
            face.addProperty("texture", slots.get(quad.materialInfo().sprite()));
            face.addProperty("direction", quad.direction().getSerializedName());
            Direction culled = cull.get(quad);
            face.addProperty("cull", culled == null ? null : culled.getSerializedName());
            face.addProperty("tint", quad.materialInfo().tintIndex());
            Direction shade = quad.materialInfo().shadeDirectionOverride();
            face.addProperty("shade_face", shade == null ? null : shade.getSerializedName());
            face.addProperty("light_emission", quad.materialInfo().lightEmission());
            JsonArray positions = new JsonArray();
            JsonArray uvs = new JsonArray();
            for (int vertex = 0; vertex < 4; vertex++) {
                Vector3fc p = quad.position(vertex);
                positions.add(triple(p.x(), p.y(), p.z()));
                long packed = quad.packedUV(vertex);
                JsonArray uv = new JsonArray();
                uv.add(number(UVPair.unpackU(packed)));
                uv.add(number(UVPair.unpackV(packed)));
                uvs.add(uv);
            }
            face.add("positions", positions);
            face.add("uv", uvs);
            faces.add(face);
        }

        JsonObject row = new JsonObject();
        row.addProperty("model", entry.model);
        row.addProperty("x", entry.x);
        row.addProperty("y", entry.y);
        row.addProperty("z", entry.z);
        row.addProperty("uvlock", entry.uvlock);
        row.add("faces", faces);
        return row;
    }

    private static JsonArray triple(final float x, final float y, final float z) {
        JsonArray array = new JsonArray();
        array.add(number(x));
        array.add(number(y));
        array.add(number(z));
        return array;
    }

    private static Number number(final float value) {
        float clean = value == 0.0F ? 0.0F : value;
        if (clean == Math.rint(clean) && Math.abs(clean) < 1.0E6F) {
            return (int) clean;
        }
        return Double.parseDouble(Float.toString(clean));
    }

    @SuppressWarnings("unchecked")
    private static <T> T allocate(final Class<T> type) throws Exception {
        Field field = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        sun.misc.Unsafe unsafe = (sun.misc.Unsafe) field.get(null);
        return (T) unsafe.allocateInstance(type);
    }

    private static TextureAtlasSprite wholeAtlasSprite() {
        try {
            TextureAtlasSprite sprite = allocate(TextureAtlasSprite.class);
            set(sprite, "atlasLocation", TextureAtlas.LOCATION_BLOCKS);
            set(sprite, "u0", 0.0F);
            set(sprite, "u1", 1.0F);
            set(sprite, "v0", 0.0F);
            set(sprite, "v1", 1.0F);
            return sprite;
        } catch (Exception failure) {
            throw new IllegalStateException(failure);
        }
    }

    private static void set(final Object target, final String field, final Object value) throws ReflectiveOperationException {
        Field f = TextureAtlasSprite.class.getDeclaredField(field);
        f.setAccessible(true);
        f.set(target, value);
    }

    public static class FakeMaterials extends MaterialBaker {
        java.util.function.Function<String, Material.Baked> resolver;

        public FakeMaterials() {
            super(null, null);
        }

        @Override
        public Material.Baked resolveSlot(final TextureSlots slots, final String id, final ModelDebugName name) {
            return this.resolver.apply(id);
        }
    }

    private static String read(final String name) throws IOException {
        InputStream stream = BlockFaceOracle.class.getClassLoader().getResourceAsStream(name);
        if (stream == null) {
            throw new IOException("missing " + name);
        }
        try (stream) {
            return new String(stream.readAllBytes(), StandardCharsets.UTF_8);
        }
    }

    private static List<String> list(final String prefix) throws Exception {
        URL url = BlockFaceOracle.class.getClassLoader().getResource(prefix + "stone.json");
        if (url == null) {
            throw new IllegalStateException("cannot locate " + prefix);
        }
        List<String> names = new ArrayList<>();
        if (url.openConnection() instanceof JarURLConnection connection) {
            try (JarFile jar = connection.getJarFile()) {
                jar.stream().map(e -> e.getName())
                    .filter(n -> n.startsWith(prefix) && n.endsWith(".json") && n.indexOf('/', prefix.length()) < 0)
                    .forEach(names::add);
            }
        } else {
            Path directory = Path.of(URI.create(url.toString())).getParent();
            try (Stream<Path> listed = Files.list(directory)) {
                listed.map(p -> prefix + p.getFileName()).filter(n -> n.endsWith(".json")).forEach(names::add);
            }
        }
        names.sort(String::compareTo);
        return names;
    }

    private static void write(final Path path, final List<JsonObject> rows) throws IOException {
        JsonObject units = new JsonObject();
        units.addProperty("position", "blocks; the block occupies 0..1 on each axis; +Y up, -Z north, +X east");
        units.addProperty("uv", "local to the sprite: 0..1 covers the sprite; u to the right, v down");
        units.addProperty("faces", "in output order; texture is the texture reference of the face; cull is the rotated cull face");
        StringBuilder text = new StringBuilder("{\n");
        text.append("  \"format\": ").append(FORMAT).append(",\n");
        text.append("  \"kind\": \"block_faces\",\n");
        text.append("  \"game_version\": ").append(LINE.toJson(SharedConstants.getCurrentVersion().id())).append(",\n");
        text.append("  \"units\": ").append(LINE.toJson(units)).append(",\n");
        text.append("  \"cases\": [\n");
        for (int index = 0; index < rows.size(); index++) {
            text.append("    ").append(LINE.toJson(rows.get(index)));
            text.append(index + 1 < rows.size() ? ",\n" : "\n");
        }
        text.append("  ]\n}\n");
        Files.writeString(path, text.toString(), StandardCharsets.UTF_8);
        System.out.println("wrote " + path + " (" + rows.size() + " cases)");
    }
}
