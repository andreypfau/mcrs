package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.math.Quadrant;
import com.mojang.serialization.JsonOps;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.Reader;
import java.lang.reflect.Field;
import java.net.JarURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.jar.JarFile;
import java.util.stream.Collectors;
import net.minecraft.SharedConstants;
import net.minecraft.client.renderer.block.dispatch.BlockStateModel;
import net.minecraft.client.renderer.block.dispatch.BlockStateModelDispatcher;
import net.minecraft.client.renderer.block.dispatch.SingleVariant;
import net.minecraft.client.renderer.block.dispatch.Variant;
import net.minecraft.client.renderer.block.dispatch.WeightedVariants;
import net.minecraft.client.renderer.block.dispatch.multipart.MultiPartModel;
import net.minecraft.client.resources.model.ModelDiscovery;
import net.minecraft.client.resources.model.ResolvedModel;
import net.minecraft.client.resources.model.UnbakedModel;
import net.minecraft.client.resources.model.cuboid.CuboidModel;
import net.minecraft.client.resources.model.cuboid.ItemModelGenerator;
import net.minecraft.client.resources.model.cuboid.ItemTransform;
import net.minecraft.client.resources.model.cuboid.ItemTransforms;
import net.minecraft.client.resources.model.cuboid.MissingCuboidModel;
import net.minecraft.client.resources.model.sprite.Material;
import net.minecraft.client.resources.model.sprite.TextureSlots;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.RandomSource;
import net.minecraft.util.StrictJsonParser;
import net.minecraft.util.random.Weighted;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Property;
import org.joml.Vector3fc;

/**
 * Resolves the blockstate files and block model files of the client resource pack, as the client
 * does at load, and writes the result as JSON. Nothing is baked or drawn.
 *
 * <p>Arguments: the output directory.
 */
public final class BlockModelOracle {
    private static final int FORMAT = 1;
    private static final Gson LINE = new GsonBuilder().disableHtmlEscaping().create();
    private static final String ASSETS = "assets/";
    private static final int[][] POSITIONS = {
        {0, 0, 0}, {1, 64, -3}, {-17, 5, 1000}, {123456, -60, -987654}, {-1, -1, -1}, {29999999, 319, -29999999}
    };
    private static final Set<String> SEED_BLOCKS = Set.of(
        "minecraft:oak_door", "minecraft:red_bed", "minecraft:tall_grass", "minecraft:sunflower", "minecraft:stone"
    );

    private BlockModelOracle() {}

    public static void main(final String[] args) {
        int status;
        try {
            SharedConstants.tryDetectVersion();
            Bootstrap.bootStrap();
            Path out = Path.of(args[0]);
            Files.createDirectories(out);
            run(out.resolve("block_models.json"));
            status = 0;
        } catch (Throwable failure) {
            failure.printStackTrace();
            status = 1;
        }
        System.exit(status);
    }

    private static void run(final Path path) throws Exception {
        Map<String, byte[]> files = jarAssets();
        Map<Identifier, UnbakedModel> models = new HashMap<>();
        for (Map.Entry<String, byte[]> file : files.entrySet()) {
            Identifier id = idOf(file.getKey(), "models");
            if (id == null) {
                continue;
            }
            try (Reader reader = reader(file.getValue())) {
                models.put(id, CuboidModel.fromStream(reader));
            }
        }

        List<JsonObject> blocks = new ArrayList<>();
        List<JsonObject> picks = new ArrayList<>();
        Set<Identifier> referenced = new TreeSet<>();
        ModelDiscovery discovery = new ModelDiscovery(models, MissingCuboidModel.missingModel());
        discovery.addSpecialModel(ItemModelGenerator.GENERATED_ITEM_MODEL_ID, new ItemModelGenerator());
        discovery.addRoot(resolver -> models.keySet().forEach(resolver::markDependency));

        List<Block> sorted = new ArrayList<>();
        BuiltInRegistries.BLOCK.forEach(sorted::add);
        sorted.sort((a, b) -> key(a).compareTo(key(b)));
        for (Block block : sorted) {
            Identifier id = BuiltInRegistries.BLOCK.getKey(block);
            byte[] bytes = files.get(ASSETS + id.getNamespace() + "/blockstates/" + id.getPath() + ".json");
            JsonObject row = new JsonObject();
            row.addProperty("block", id.toString());
            Map<BlockState, BlockStateModel.UnbakedRoot> roots = Map.of();
            if (bytes == null) {
                row.addProperty("definition", "none");
            } else {
                BlockStateModelDispatcher dispatcher;
                try (Reader reader = reader(bytes)) {
                    JsonElement element = StrictJsonParser.parse(reader);
                    dispatcher = BlockStateModelDispatcher.CODEC.parse(JsonOps.INSTANCE, element).getOrThrow();
                }
                String kind = dispatcher.simpleModels().isPresent()
                    ? (dispatcher.multiPart().isPresent() ? "both" : "variants")
                    : "multipart";
                row.addProperty("definition", kind);
                roots = dispatcher.instantiate(block.getStateDefinition(), id::toString);
            }
            List<String> table = new ArrayList<>();
            Map<String, Integer> index = new HashMap<>();
            JsonArray states = new JsonArray();
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                JsonObject entry = new JsonObject();
                entry.addProperty("state", stateKey(state));
                BlockStateModel.UnbakedRoot root = roots.get(state);
                if (root == null) {
                    entry.addProperty("missing", true);
                } else {
                    JsonArray parts = new JsonArray();
                    for (BlockStateModel.Unbaked part : selectedParts(root, state)) {
                        JsonArray alternatives = alternatives(part, referenced);
                        String text = LINE.toJson(alternatives);
                        Integer at = index.get(text);
                        if (at == null) {
                            at = table.size();
                            table.add(text);
                            index.put(text, at);
                        }
                        parts.add(at);
                    }
                    entry.add("parts", parts);
                }
                states.add(entry);
                if (root != null && (hasWeights(root, state) || SEED_BLOCKS.contains(id.toString()))) {
                    boolean everyState = SEED_BLOCKS.contains(id.toString());
                    if (everyState || state == block.defaultBlockState()) {
                        picks.addAll(picks(id, state, root));
                    }
                }
            }
            JsonArray partTable = new JsonArray();
            for (String text : table) {
                partTable.add(LINE.fromJson(text, JsonArray.class));
            }
            row.add("part_table", partTable);
            String rule = seedRule(block);
            if (!rule.equals("position")) {
                row.addProperty("seed_rule", rule);
            }
            row.add("states", states);
            blocks.add(row);
        }

        Map<Identifier, ResolvedModel> resolved = discovery.resolve();
        List<JsonObject> modelRows = new ArrayList<>();
        List<String> displayTable = new ArrayList<>();
        Map<String, Integer> displayIndex = new HashMap<>();
        for (Identifier id : new TreeSet<>(resolved.keySet())) {
            modelRows.add(model(id, resolved.get(id), displayTable, displayIndex));
        }
        List<JsonObject> displays = new ArrayList<>();
        for (String entry : displayTable) {
            displays.add(LINE.fromJson(entry, JsonObject.class));
        }
        JsonArray absent = new JsonArray();
        for (Identifier id : referenced) {
            if (!models.containsKey(id)) {
                absent.add(id.toString());
            }
        }

        StringBuilder text = new StringBuilder("{\n");
        text.append("  \"format\": ").append(FORMAT).append(",\n");
        text.append("  \"kind\": \"block_models\",\n");
        text.append("  \"game_version\": ").append(LINE.toJson(SharedConstants.getCurrentVersion().id())).append(",\n");
        JsonObject units = new JsonObject();
        units.addProperty(
            "states",
            "blocks sorted by id; every state of the block; state is name=value pairs sorted by property name; "
                + "parts lists, in file order, the indices into part_table of the parts that the state draws; missing marks a state with no model"
        );
        units.addProperty(
            "display_table",
            "each entry maps a display context to its transform: the nearest model of the chain that declares the context supplies it; "
                + "rotation in degrees, translation in blocks after the clamp, scale after the clamp"
        );
        units.addProperty(
            "seed_rule",
            "absent: the position seed uses the position of the block; lower_half: the upper half uses the position one block below; "
                + "bed_head: the foot uses the position of the head, one block in the facing direction, at its own y"
        );
        units.addProperty("part_table", "each part is a weighted list of variants; x, y, z in degrees; weight as written, default 1");
        units.addProperty(
            "models",
            "every model file plus the built-in models; geometry_from is the model that supplies the elements, absent for none; "
                + "textures holds the resolved slots; unresolved lists the slots of the chain that resolve to no texture; "
                + "display is an index into display_table, absent when no model of the chain declares a display context"
        );
        units.addProperty(
            "picks",
            "seed is the 64-bit position seed of the state at the position; chosen lists, per part of the state, the index of the variant in its weighted list"
        );
        text.append("  \"units\": ").append(LINE.toJson(units)).append(",\n");
        text.append("  \"absent_models\": ").append(LINE.toJson(absent)).append(",\n");
        appendList(text, "blocks", blocks, false);
        appendList(text, "display_table", displays, false);
        appendList(text, "models", modelRows, false);
        appendList(text, "picks", picks, true);
        text.append("}\n");
        Files.writeString(path, text.toString(), StandardCharsets.UTF_8);
        System.out.println("wrote " + path + " (" + blocks.size() + " blocks, " + modelRows.size() + " models, " + picks.size() + " picks)");
    }

    private static String key(final Block block) {
        return BuiltInRegistries.BLOCK.getKey(block).toString();
    }

    private static void appendList(final StringBuilder text, final String key, final List<JsonObject> rows, final boolean last) {
        text.append("  ").append(LINE.toJson(key)).append(": [\n");
        for (int i = 0; i < rows.size(); i++) {
            text.append("    ").append(LINE.toJson(rows.get(i))).append(i + 1 < rows.size() ? ",\n" : "\n");
        }
        text.append(last ? "  ]\n" : "  ],\n");
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private static String stateKey(final BlockState state) {
        List<String> pairs = new ArrayList<>();
        for (Property property : state.getProperties()) {
            pairs.add(property.getName() + "=" + property.getName(state.getValue(property)));
        }
        return String.join(",", pairs);
    }

    @SuppressWarnings("unchecked")
    private static List<BlockStateModel.Unbaked> selectedParts(final BlockStateModel.UnbakedRoot root, final BlockState state) throws Exception {
        if (root instanceof MultiPartModel.Unbaked multipart) {
            List<MultiPartModel.Selector<BlockStateModel.Unbaked>> selectors =
                (List<MultiPartModel.Selector<BlockStateModel.Unbaked>>) read(multipart, "selectors");
            List<BlockStateModel.Unbaked> out = new ArrayList<>();
            for (MultiPartModel.Selector<BlockStateModel.Unbaked> selector : selectors) {
                if (selector.condition().test(state)) {
                    out.add(selector.model());
                }
            }
            return out;
        }
        return List.of((BlockStateModel.Unbaked) read(root, "contents"));
    }

    private static boolean hasWeights(final BlockStateModel.UnbakedRoot root, final BlockState state) throws Exception {
        for (BlockStateModel.Unbaked part : selectedParts(root, state)) {
            if (part instanceof WeightedVariants.Unbaked) {
                return true;
            }
        }
        return false;
    }

    private static JsonArray alternatives(final BlockStateModel.Unbaked part, final Set<Identifier> referenced) {
        JsonArray out = new JsonArray();
        if (part instanceof SingleVariant.Unbaked single) {
            out.add(variant(single.variant(), 1, referenced));
        } else if (part instanceof WeightedVariants.Unbaked weighted) {
            for (Weighted<BlockStateModel.Unbaked> entry : weighted.entries().unwrap()) {
                out.add(variant(((SingleVariant.Unbaked) entry.value()).variant(), entry.weight(), referenced));
            }
        } else {
            throw new IllegalStateException("unknown part " + part);
        }
        return out;
    }

    private static JsonObject variant(final Variant variant, final int weight, final Set<Identifier> referenced) {
        referenced.add(variant.modelLocation());
        JsonObject out = new JsonObject();
        out.addProperty("model", variant.modelLocation().toString());
        out.addProperty("x", degrees(variant.modelState().x()));
        out.addProperty("y", degrees(variant.modelState().y()));
        out.addProperty("z", degrees(variant.modelState().z()));
        out.addProperty("uvlock", variant.modelState().uvLock());
        out.addProperty("weight", weight);
        return out;
    }

    private static int degrees(final Quadrant quadrant) {
        return quadrant.shift * 90;
    }

    private static List<JsonObject> picks(final Identifier id, final BlockState state, final BlockStateModel.UnbakedRoot root) throws Exception {
        List<JsonObject> out = new ArrayList<>();
        List<BlockStateModel.Unbaked> parts = selectedParts(root, state);
        for (int[] p : POSITIONS) {
            BlockPos pos = new BlockPos(p[0], p[1], p[2]);
            long seed = state.getSeed(pos);
            RandomSource random = RandomSource.createThreadLocalInstance(0L);
            random.setSeed(seed);
            JsonArray chosen = new JsonArray();
            if (root instanceof MultiPartModel.Unbaked) {
                long partSeed = random.nextLong();
                for (BlockStateModel.Unbaked part : parts) {
                    random.setSeed(partSeed);
                    chosen.add(choose(part, random));
                }
            } else {
                chosen.add(choose(parts.getFirst(), random));
            }
            JsonObject row = new JsonObject();
            row.addProperty("block", id.toString());
            row.addProperty("state", stateKey(state));
            JsonArray position = new JsonArray();
            for (int c : p) {
                position.add(c);
            }
            row.add("position", position);
            row.addProperty("seed", seed);
            row.add("chosen", chosen);
            out.add(row);
        }
        return out;
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    private static String seedRule(final Block block) {
        BlockPos pos = new BlockPos(5, 70, -9);
        String rule = "position";
        for (BlockState state : block.getStateDefinition().getPossibleStates()) {
            long seed = state.getSeed(pos);
            if (seed == net.minecraft.util.Mth.getSeed(pos)) {
                continue;
            }
            String found = null;
            for (Property property : state.getProperties()) {
                if (property.getName().equals("half")
                    && seed == net.minecraft.util.Mth.getSeed(pos.getX(), pos.getY() - 1, pos.getZ())) {
                    found = "lower_half";
                }
            }
            for (net.minecraft.core.Direction direction : net.minecraft.core.Direction.Plane.HORIZONTAL) {
                BlockPos other = pos.relative(direction);
                if (seed == net.minecraft.util.Mth.getSeed(other.getX(), pos.getY(), other.getZ())) {
                    found = "bed_head";
                }
            }
            if (found == null) {
                throw new IllegalStateException("unknown seed rule of " + block);
            }
            rule = found;
        }
        return rule;
    }

    private static int choose(final BlockStateModel.Unbaked part, final RandomSource random) {
        if (part instanceof WeightedVariants.Unbaked weighted) {
            BlockStateModel.Unbaked picked = weighted.entries().getRandomOrThrow(random);
            List<Weighted<BlockStateModel.Unbaked>> entries = weighted.entries().unwrap();
            for (int i = 0; i < entries.size(); i++) {
                if (entries.get(i).value() == picked) {
                    return i;
                }
            }
            throw new IllegalStateException("picked entry is not in the list");
        }
        return 0;
    }

    private static JsonObject model(
        final Identifier id, final ResolvedModel model, final List<String> displayTable, final Map<String, Integer> displayIndex
    ) {
        JsonObject out = new JsonObject();
        out.addProperty("id", id.toString());
        Identifier parent = model.wrapped().parent();
        if (parent != null) {
            out.addProperty("parent", parent.toString());
        }
        String geometryFrom = null;
        for (ResolvedModel at = model; at != null; at = at.parent()) {
            if (at.wrapped().geometry() != null) {
                geometryFrom = at.debugName();
                break;
            }
        }
        out.addProperty("geometry_from", geometryFrom);
        out.addProperty("ambient_occlusion", model.getTopAmbientOcclusion());
        out.addProperty("gui_light", model.getTopGuiLight() == UnbakedModel.GuiLight.FRONT ? "front" : "side");

        Set<String> slots = new TreeSet<>();
        for (ResolvedModel at = model; at != null; at = at.parent()) {
            slots.addAll(at.wrapped().textureSlots().values().keySet());
        }
        TextureSlots textures = model.getTopTextureSlots();
        Map<String, JsonElement> resolvedSlots = new TreeMap<>();
        JsonArray unresolved = new JsonArray();
        for (String slot : slots) {
            Material material = textures.getMaterial(slot);
            if (material == null) {
                unresolved.add(slot);
                continue;
            }
            JsonObject value = new JsonObject();
            value.addProperty("sprite", material.sprite().toString());
            if (material.forceTranslucent()) {
                value.addProperty("force_translucent", true);
            }
            resolvedSlots.put(slot, value);
        }
        JsonObject texturesJson = new JsonObject();
        resolvedSlots.forEach(texturesJson::add);
        out.add("textures", texturesJson);
        if (!unresolved.isEmpty()) {
            out.add("unresolved", unresolved);
        }

        ItemTransforms transforms = model.getTopTransforms();
        JsonObject display = new JsonObject();
        for (ItemDisplayContext context : ItemDisplayContext.values()) {
            if (context == ItemDisplayContext.NONE) {
                continue;
            }
            ItemTransform transform = transforms.getTransform(context);
            if (transform == ItemTransform.NO_TRANSFORM) {
                continue;
            }
            JsonObject t = new JsonObject();
            t.add("rotation", vector(transform.rotation()));
            t.add("translation", vector(transform.translation()));
            t.add("scale", vector(transform.scale()));
            display.add(context.getSerializedName(), t);
        }
        if (!display.isEmpty()) {
            String key = LINE.toJson(display);
            Integer at = displayIndex.get(key);
            if (at == null) {
                at = displayTable.size();
                displayTable.add(key);
                displayIndex.put(key, at);
            }
            out.addProperty("display", at);
        }
        return out;
    }

    private static JsonArray vector(final Vector3fc v) {
        JsonArray out = new JsonArray();
        out.add(v.x());
        out.add(v.y());
        out.add(v.z());
        return out;
    }

    private static Identifier idOf(final String path, final String folder) {
        if (!path.startsWith(ASSETS) || !path.endsWith(".json")) {
            return null;
        }
        String rest = path.substring(ASSETS.length());
        int slash = rest.indexOf('/');
        if (slash < 0) {
            return null;
        }
        String namespace = rest.substring(0, slash);
        String tail = rest.substring(slash + 1);
        if (!tail.startsWith(folder + "/")) {
            return null;
        }
        return Identifier.fromNamespaceAndPath(namespace, tail.substring(folder.length() + 1, tail.length() - ".json".length()));
    }

    private static Reader reader(final byte[] bytes) {
        return new InputStreamReader(new java.io.ByteArrayInputStream(bytes), StandardCharsets.UTF_8);
    }

    private static Map<String, byte[]> jarAssets() throws IOException {
        String anchor = "assets/minecraft/blockstates/stone.json";
        URL url = BlockModelOracle.class.getClassLoader().getResource(anchor);
        if (url == null) {
            throw new IllegalStateException("cannot locate " + anchor + " on the class path");
        }
        Map<String, byte[]> out = new HashMap<>();
        if (!(url.openConnection() instanceof JarURLConnection connection)) {
            throw new IllegalStateException("the client assets are not in a jar: " + url);
        }
        try (JarFile jar = connection.getJarFile()) {
            for (var entry : jar.stream().collect(Collectors.toList())) {
                String name = entry.getName();
                if (name.startsWith(ASSETS) && (name.contains("/blockstates/") || name.contains("/models/")) && name.endsWith(".json")) {
                    try (var in = jar.getInputStream(entry)) {
                        out.put(name, in.readAllBytes());
                    }
                }
            }
        }
        return out;
    }

    private static Object read(final Object owner, final String name) throws ReflectiveOperationException {
        Class<?> type = owner.getClass();
        while (type != null) {
            try {
                Field field = type.getDeclaredField(name);
                field.setAccessible(true);
                return field.get(owner);
            } catch (NoSuchFieldException missing) {
                type = type.getSuperclass();
            }
        }
        throw new NoSuchFieldException(owner.getClass().getName() + "." + name);
    }
}
