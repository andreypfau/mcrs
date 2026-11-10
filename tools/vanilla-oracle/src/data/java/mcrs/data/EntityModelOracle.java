package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.net.JarURLConnection;
import java.net.URI;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.function.Function;
import java.util.jar.JarFile;
import java.util.stream.Stream;
import net.minecraft.SharedConstants;
import net.minecraft.client.animation.AnimationChannel;
import net.minecraft.client.animation.AnimationDefinition;
import net.minecraft.client.animation.Keyframe;
import net.minecraft.client.model.EntityModel;
import net.minecraft.client.model.animal.frog.FrogModel;
import net.minecraft.client.model.animal.pig.BabyPigModel;
import net.minecraft.client.model.animal.pig.ColdPigModel;
import net.minecraft.client.model.animal.pig.PigModel;
import net.minecraft.client.model.geom.LayerDefinitions;
import net.minecraft.client.model.geom.ModelLayerLocation;
import net.minecraft.client.model.geom.ModelLayers;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.model.geom.PartPose;
import net.minecraft.client.model.geom.builders.CubeDefinition;
import net.minecraft.client.model.geom.builders.CubeDeformation;
import net.minecraft.client.model.geom.builders.LayerDefinition;
import net.minecraft.client.model.geom.builders.MaterialDefinition;
import net.minecraft.client.model.geom.builders.MeshDefinition;
import net.minecraft.client.model.geom.builders.PartDefinition;
import net.minecraft.client.model.geom.builders.UVPair;
import net.minecraft.client.model.object.banner.BannerFlagModel;
import net.minecraft.client.model.object.book.BookModel;
import net.minecraft.client.model.object.skull.DragonHeadModel;
import net.minecraft.client.model.object.skull.PiglinHeadModel;
import net.minecraft.client.model.object.skull.SkullModelBase;
import net.minecraft.client.renderer.blockentity.BannerRenderer;
import net.minecraft.client.renderer.blockentity.ChestRenderer;
import net.minecraft.client.renderer.blockentity.DecoratedPotRenderer;
import net.minecraft.client.renderer.blockentity.LecternRenderer;
import net.minecraft.client.renderer.blockentity.ShulkerBoxRenderer;
import net.minecraft.client.renderer.blockentity.SkullBlockRenderer;
import com.mojang.math.Transformation;
import org.joml.Matrix4fc;
import net.minecraft.client.renderer.entity.state.FrogRenderState;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.syncher.EntityDataAccessor;
import net.minecraft.network.syncher.EntityDataSerializers;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.entity.AnimationState;
import net.minecraft.world.entity.EntityDimensions;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;
import org.joml.Vector3fc;

/**
 * Builds the entity model layers and keyframe clips of the client, and poses selected models,
 * then writes them as JSON. Nothing is drawn, so no graphics context is needed.
 *
 * <p>Arguments: the output directory, then one of {@code models}, {@code animations}, {@code poses}, {@code types}, {@code block_entities} or {@code sign_text}.
 */
public final class EntityModelOracle {
    private static final int FORMAT = 1;
    private static final Gson LINE = new GsonBuilder().disableHtmlEscaping().serializeNulls().serializeSpecialFloatingPointValues().create();

    private EntityModelOracle() {}

    public static void main(final String[] args) {
        int status;
        try {
            SharedConstants.tryDetectVersion();
            Bootstrap.bootStrap();
            Path out = Path.of(args[0]);
            Files.createDirectories(out);
            switch (args[1]) {
                case "models" -> write(out.resolve("entity_models.json"), header("models"), "layers", models());
                case "animations" -> write(out.resolve("entity_animations.json"), header("animations"), "clips", animations());
                case "poses" -> write(out.resolve("entity_poses.json"), header("poses"), "cases", poses());
                case "types" -> write(out.resolve("entity_types.json"), header("types"), "types", types());
                case "block_entities" -> write(out.resolve("block_entity_poses.json"), header("block_entities"), "cases", blockEntities());
                case "sign_text" -> write(out.resolve("sign_text.json"), header("sign_text"), "cases", SignTextOracle.rows());
                default -> throw new IllegalArgumentException("unknown mode " + args[1]);
            }
            status = 0;
        } catch (Throwable failure) {
            failure.printStackTrace();
            status = 1;
        }
        System.exit(status);
    }

    private static JsonObject header(final String kind) {
        JsonObject header = new JsonObject();
        header.addProperty("format", FORMAT);
        header.addProperty("kind", kind);
        header.addProperty("game_version", SharedConstants.getCurrentVersion().id());
        JsonObject units = new JsonObject();
        units.addProperty("length", "1/16 block (pixel)");
        units.addProperty("axes", "model space: +Y down, -Z is the front of the entity, -X is its right side");
        units.addProperty("rotation", "radians; a part applies Z, then Y, then X to its parent frame (a vertex is turned about X first)");
        switch (kind) {
            case "models" -> units.addProperty(
                "uv",
                "cube uv in texels; uv_scale is a factor of the texture size of the cube; face vertices: [x, y, z] in pixels of the part frame, [u, v] in texels; "
                    + "normal in model space; internal_direction names the face in the internal space (+Y up, +X the right side of the entity, -Z the front)"
            );
            case "animations" -> {
                units.addProperty("position", "pixels in model space; the authored up value is stored negated (Y down)");
                units.addProperty("rotation_keyframe", "radians; authored in degrees");
                units.addProperty("scale", "stored as authored factor minus 1; channels add this value to the part scale");
            }
            case "types" -> {
                units.addProperty("size", "blocks; width, height and eye_height of the standing adult; a baby scales them by its age scale");
                units.addProperty("update_interval", "ticks between two position updates of the server");
                units.addProperty("data", "entity data entries in index order; wire_type is the type id of the entity data codec");
            }
            case "block_entities" -> {
                units.addProperty(
                    "pose",
                    "position in pixels of the parent frame; rotation in radians; scale as a factor; values after the pose setup of the static block"
                );
                units.addProperty(
                    "placement",
                    "4x4 matrix, rows of 4, that maps a point of the layer in model space, in blocks (pixels / 16), to block space (+Y up, the block from 0 to 1)"
                );
            }
            case "sign_text" -> {
                units.addProperty(
                    "matrix",
                    "4x4 matrix, rows of 4, applied to the text of one side: maps a point of the text plane, in text units "
                        + "(+X right, +Y down, one unit is one pixel of the text), to block space (+Y up, the block from 0 to 1)"
                );
                units.addProperty("x_y", "position of the first glyph of a line in text units, before the matrix");
                units.addProperty("colors", "ARGB integers, signed 32 bit");
                units.addProperty("font", "stub: every code point has the advance stub_advance, lines are not wrapped; the file records placement, colour and light only");
            }
            default -> units.addProperty(
                "pose",
                "position in pixels of the parent frame; rotation in radians; scale as a factor; values after the pose setup"
            );
        }
        header.add("units", units);
        return header;
    }

    private static void write(final Path path, final JsonObject header, final String listKey, final List<JsonObject> rows) throws IOException {
        StringBuilder text = new StringBuilder("{\n");
        for (Map.Entry<String, JsonElement> entry : header.entrySet()) {
            text.append("  ").append(LINE.toJson(entry.getKey())).append(": ").append(LINE.toJson(entry.getValue())).append(",\n");
        }
        text.append("  ").append(LINE.toJson(listKey)).append(": [\n");
        for (int index = 0; index < rows.size(); index++) {
            text.append("    ").append(LINE.toJson(rows.get(index)));
            text.append(index + 1 < rows.size() ? ",\n" : "\n");
        }
        text.append("  ]\n}\n");
        Files.writeString(path, text.toString(), StandardCharsets.UTF_8);
        System.out.println("wrote " + path + " (" + rows.size() + " " + listKey + ")");
    }

    // ---- models -------------------------------------------------------------------------------

    private static List<JsonObject> models() throws ReflectiveOperationException {
        Map<ModelLayerLocation, LayerDefinition> roots = LayerDefinitions.createRoots();
        List<ModelLayerLocation> keys = new ArrayList<>(roots.keySet());
        keys.sort(Comparator.comparing((ModelLayerLocation key) -> key.model().toString()).thenComparing(ModelLayerLocation::layer));
        List<JsonObject> rows = new ArrayList<>();
        for (ModelLayerLocation key : keys) {
            LayerDefinition layer = roots.get(key);
            MaterialDefinition material = (MaterialDefinition) read(layer, "material");
            int width = (int) read(material, "xTexSize");
            int height = (int) read(material, "yTexSize");
            MeshDefinition mesh = (MeshDefinition) read(layer, "mesh");
            JsonObject row = new JsonObject();
            row.addProperty("model", key.model().toString());
            row.addProperty("layer", key.layer());
            row.addProperty("texture_width", width);
            row.addProperty("texture_height", height);
            row.add("root", part("root", mesh.getRoot(), width, height));
            rows.add(row);
        }
        return rows;
    }

    @SuppressWarnings("unchecked")
    private static JsonObject part(final String name, final PartDefinition definition, final int width, final int height)
        throws ReflectiveOperationException {
        PartPose pose = (PartPose) read(definition, "partPose");
        JsonObject part = new JsonObject();
        part.addProperty("name", name);
        part.add("pivot", vector(pose.x(), pose.y(), pose.z()));
        part.add("rotation", vector(pose.xRot(), pose.yRot(), pose.zRot()));
        part.add("scale", vector(pose.xScale(), pose.yScale(), pose.zScale()));
        JsonArray cubes = new JsonArray();
        for (CubeDefinition cube : (List<CubeDefinition>) read(definition, "cubes")) {
            cubes.add(cube(cube, width, height));
        }
        part.add("cubes", cubes);
        JsonArray children = new JsonArray();
        Map<String, PartDefinition> sorted = new TreeMap<>((Map<String, PartDefinition>) read(definition, "children"));
        for (Map.Entry<String, PartDefinition> child : sorted.entrySet()) {
            children.add(part(child.getKey(), child.getValue(), width, height));
        }
        part.add("children", children);
        return part;
    }

    @SuppressWarnings("unchecked")
    private static JsonObject cube(final CubeDefinition definition, final int width, final int height) throws ReflectiveOperationException {
        Vector3fc corner = (Vector3fc) read(definition, "origin");
        Vector3fc size = (Vector3fc) read(definition, "dimensions");
        CubeDeformation grow = (CubeDeformation) read(definition, "grow");
        UVPair uv = (UVPair) read(definition, "texCoord");
        UVPair uvScale = (UVPair) read(definition, "texScale");
        JsonObject cube = new JsonObject();
        cube.add("corner", vector(corner.x(), corner.y(), corner.z()));
        cube.add("size", vector(size.x(), size.y(), size.z()));
        cube.add("inflate", vector((float) read(grow, "growX"), (float) read(grow, "growY"), (float) read(grow, "growZ")));
        cube.add("uv", pair(uv.u(), uv.v()));
        cube.add("uv_scale", pair(uvScale.u(), uvScale.v()));
        cube.addProperty("mirror", (boolean) read(definition, "mirror"));
        JsonArray declared = new JsonArray();
        new TreeSet<>(((Set<Direction>) read(definition, "visibleFaces")).stream().map(Direction::getSerializedName).toList())
            .forEach(declared::add);
        cube.add("visible_faces", declared);
        JsonArray faces = new JsonArray();
        for (ModelPart.Polygon polygon : definition.bake(width, height).polygons) {
            JsonObject face = new JsonObject();
            // Artefact space turned 180 degrees about Z is the internal space of the entity pack.
            face.addProperty(
                "internal_direction",
                Direction.getApproximateNearest(-polygon.normal().x(), -polygon.normal().y(), polygon.normal().z()).getSerializedName()
            );
            face.add("normal", vector(polygon.normal().x(), polygon.normal().y(), polygon.normal().z()));
            JsonArray vertices = new JsonArray();
            for (ModelPart.Vertex vertex : polygon.vertices()) {
                JsonArray row = new JsonArray();
                row.add(vertex.x());
                row.add(vertex.y());
                row.add(vertex.z());
                row.add(vertex.u() * width);
                row.add(vertex.v() * height);
                vertices.add(row);
            }
            face.add("vertices", vertices);
            faces.add(face);
        }
        cube.add("faces", faces);
        return cube;
    }

    // ---- animations ---------------------------------------------------------------------------

    private static List<JsonObject> animations() throws Exception {
        List<JsonObject> rows = new ArrayList<>();
        for (Class<?> holder : definitionHolders()) {
            String owner = snake(holder.getSimpleName().replaceFirst("Animation$", ""));
            List<Field> fields = new ArrayList<>(List.of(holder.getDeclaredFields()));
            fields.sort(Comparator.comparing(Field::getName));
            for (Field field : fields) {
                if (!Modifier.isStatic(field.getModifiers()) || field.getType() != AnimationDefinition.class) {
                    continue;
                }
                field.setAccessible(true);
                AnimationDefinition definition = (AnimationDefinition) field.get(null);
                String clip = field.getName().toLowerCase();
                if (clip.startsWith(owner + "_")) {
                    clip = clip.substring(owner.length() + 1);
                }
                JsonObject row = new JsonObject();
                row.addProperty("id", owner + "/" + clip);
                row.addProperty("length", definition.lengthInSeconds());
                row.addProperty("looping", definition.looping());
                JsonObject bones = new JsonObject();
                for (Map.Entry<String, List<AnimationChannel>> bone : new TreeMap<>(definition.boneAnimations()).entrySet()) {
                    JsonArray channels = new JsonArray();
                    for (AnimationChannel channel : bone.getValue()) {
                        JsonObject json = new JsonObject();
                        json.addProperty("target", target(channel.target()));
                        JsonArray keyframes = new JsonArray();
                        for (Keyframe keyframe : channel.keyframes()) {
                            JsonObject key = new JsonObject();
                            key.addProperty("time", keyframe.timestamp());
                            key.add("pre", vector(keyframe.preTarget()));
                            key.add("post", vector(keyframe.postTarget()));
                            key.addProperty("interpolation", interpolation(keyframe.interpolation()));
                            keyframes.add(key);
                        }
                        json.add("keyframes", keyframes);
                        channels.add(json);
                    }
                    bones.add(bone.getKey(), channels);
                }
                row.add("bones", bones);
                rows.add(row);
            }
        }
        rows.sort(Comparator.comparing(row -> row.get("id").getAsString()));
        return rows;
    }

    private static List<Class<?>> definitionHolders() throws Exception {
        String anchor = "net/minecraft/client/animation/definitions/FrogAnimation.class";
        URL url = EntityModelOracle.class.getClassLoader().getResource(anchor);
        if (url == null) {
            throw new IllegalStateException("cannot locate " + anchor);
        }
        String prefix = "net/minecraft/client/animation/definitions/";
        List<String> names = new ArrayList<>();
        if (url.openConnection() instanceof JarURLConnection connection) {
            try (JarFile jar = connection.getJarFile()) {
                jar.stream().map(entry -> entry.getName())
                    .filter(name -> name.startsWith(prefix) && name.endsWith(".class") && name.indexOf('/', prefix.length()) < 0)
                    .forEach(names::add);
            }
        } else {
            Path directory = Path.of(URI.create(url.toString())).getParent();
            try (Stream<Path> listed = Files.list(directory)) {
                listed.map(path -> prefix + path.getFileName()).filter(name -> name.endsWith(".class")).forEach(names::add);
            }
        }
        List<Class<?>> holders = new ArrayList<>();
        for (String name : names) {
            if (name.contains("$") || name.endsWith("package-info.class")) {
                continue;
            }
            holders.add(Class.forName(name.substring(0, name.length() - ".class".length()).replace('/', '.')));
        }
        holders.sort(Comparator.comparing(Class::getName));
        return holders;
    }

    private static String target(final AnimationChannel.Target target) {
        if (target == AnimationChannel.Targets.POSITION) {
            return "position";
        }
        if (target == AnimationChannel.Targets.ROTATION) {
            return "rotation";
        }
        if (target == AnimationChannel.Targets.SCALE) {
            return "scale";
        }
        throw new IllegalStateException("unknown channel target");
    }

    private static String interpolation(final AnimationChannel.Interpolation interpolation) {
        if (interpolation == AnimationChannel.Interpolations.LINEAR) {
            return "linear";
        }
        if (interpolation == AnimationChannel.Interpolations.CATMULLROM) {
            return "catmullrom";
        }
        throw new IllegalStateException("unknown interpolation");
    }

    private static String snake(final String camel) {
        return camel.replaceAll("([a-z0-9])([A-Z])", "$1_$2").toLowerCase();
    }

    // ---- types --------------------------------------------------------------------------------

    private static List<JsonObject> types() throws ReflectiveOperationException {
        Map<EntityType<?>, Class<?>> classes = typeClasses();
        List<JsonObject> rows = new ArrayList<>();
        List<EntityType<?>> types = new ArrayList<>(BuiltInRegistries.ENTITY_TYPE.stream().toList());
        types.sort(Comparator.comparing(type -> BuiltInRegistries.ENTITY_TYPE.getKey(type).toString()));
        for (EntityType<?> type : types) {
            JsonObject row = new JsonObject();
            row.addProperty("id", BuiltInRegistries.ENTITY_TYPE.getKey(type).toString());
            EntityDimensions size = type.getDimensions();
            row.addProperty("width", size.width());
            row.addProperty("height", size.height());
            row.addProperty("eye_height", size.eyeHeight());
            row.addProperty("update_interval", type.updateInterval());
            row.addProperty("tracking_range_chunks", type.clientTrackingRange());
            row.addProperty("fire_immune", type.fireImmune());
            Class<?> leaf = classes.get(type);
            if (leaf == null) {
                throw new IllegalStateException("no class for " + BuiltInRegistries.ENTITY_TYPE.getKey(type));
            }
            row.add("data", data(leaf));
            rows.add(row);
        }
        return rows;
    }

    // The type argument of each registered type constant names the class whose static
    // definitions assign the entity data indices.
    private static Map<EntityType<?>, Class<?>> typeClasses() throws ReflectiveOperationException {
        Map<EntityType<?>, Class<?>> classes = new java.util.HashMap<>();
        for (Field field : EntityTypes.class.getDeclaredFields()) {
            if (!Modifier.isStatic(field.getModifiers()) || field.getType() != EntityType.class) {
                continue;
            }
            if (field.getGenericType() instanceof java.lang.reflect.ParameterizedType generic
                && generic.getActualTypeArguments()[0] instanceof Class<?> argument) {
                field.setAccessible(true);
                classes.put((EntityType<?>) field.get(null), argument);
            }
        }
        return classes;
    }

    private static JsonArray data(final Class<?> leaf) throws ReflectiveOperationException {
        Map<Integer, Integer> entries = new TreeMap<>();
        for (Class<?> type = leaf; type != null; type = type.getSuperclass()) {
            Class.forName(type.getName(), true, type.getClassLoader());
            for (Field field : type.getDeclaredFields()) {
                if (!Modifier.isStatic(field.getModifiers()) || field.getType() != EntityDataAccessor.class) {
                    continue;
                }
                field.setAccessible(true);
                EntityDataAccessor<?> accessor = (EntityDataAccessor<?>) field.get(null);
                entries.put(accessor.id(), EntityDataSerializers.getSerializedId(accessor.serializer()));
            }
        }
        JsonArray data = new JsonArray();
        for (Map.Entry<Integer, Integer> entry : entries.entrySet()) {
            JsonObject json = new JsonObject();
            json.addProperty("index", entry.getKey());
            json.addProperty("wire_type", entry.getValue());
            data.add(json);
        }
        return data;
    }

    // ---- poses --------------------------------------------------------------------------------

    private static final float[] WALK_POSITIONS = {0.0F, 0.5F, 1.7F, 10.3F};
    private static final float[] WALK_SPEEDS = {0.0F, 0.25F, 1.0F};
    private static final float[] HEAD_YAWS = {-60.0F, 0.0F, 35.0F};
    private static final float[] HEAD_PITCHES = {-30.0F, 0.0F, 45.0F};
    private static final float[] CLIP_AGES = {0.0F, 2.5F, 5.0F, 9.25F, 17.0F, 40.0F, 61.5F};

    private static List<JsonObject> poses() {
        Map<ModelLayerLocation, LayerDefinition> roots = LayerDefinitions.createRoots();
        List<JsonObject> rows = new ArrayList<>();
        quadruped(rows, roots, ModelLayers.PIG, PigModel::new);
        quadruped(rows, roots, ModelLayers.PIG_BABY, BabyPigModel::new);
        quadruped(rows, roots, ModelLayers.COLD_PIG, ColdPigModel::new);
        quadruped(rows, roots, ModelLayers.PIG_SADDLE, PigModel::new);
        frog(rows, roots);
        return rows;
    }

    private static void quadruped(
        final List<JsonObject> rows,
        final Map<ModelLayerLocation, LayerDefinition> roots,
        final ModelLayerLocation key,
        final Function<ModelPart, ? extends EntityModel<LivingEntityRenderState>> factory
    ) {
        ModelPart root = roots.get(key).bakeRoot();
        EntityModel<LivingEntityRenderState> model = factory.apply(root);
        for (float position : WALK_POSITIONS) {
            for (float speed : WALK_SPEEDS) {
                for (float yaw : HEAD_YAWS) {
                    for (float pitch : HEAD_PITCHES) {
                        LivingEntityRenderState state = new LivingEntityRenderState();
                        state.walkAnimationPos = position;
                        state.walkAnimationSpeed = speed;
                        state.yRot = yaw;
                        state.xRot = pitch;
                        state.ageInTicks = 13.37F;
                        model.setupAnim(state);
                        JsonObject inputs = livingInputs(state);
                        rows.add(poseRow(key, inputs, root));
                    }
                }
            }
        }
    }

    private static void frog(final List<JsonObject> rows, final Map<ModelLayerLocation, LayerDefinition> roots) {
        ModelPart root = roots.get(ModelLayers.FROG).bakeRoot();
        FrogModel model = new FrogModel(root);
        for (boolean swimming : new boolean[] {false, true}) {
            for (float position : WALK_POSITIONS) {
                for (float speed : WALK_SPEEDS) {
                    FrogRenderState state = new FrogRenderState();
                    state.isSwimming = swimming;
                    state.walkAnimationPos = position;
                    state.walkAnimationSpeed = speed;
                    state.ageInTicks = 13.37F;
                    rows.add(frogRow(model, root, state));
                }
            }
        }
        String[] clips = {"jump", "croak", "tongue", "idle_water"};
        for (String clip : clips) {
            for (float age : CLIP_AGES) {
                FrogRenderState state = new FrogRenderState();
                state.ageInTicks = 100.0F + age;
                clipState(state, clip).start(100);
                rows.add(frogRow(model, root, state));
            }
        }
        for (float age : CLIP_AGES) {
            FrogRenderState state = new FrogRenderState();
            state.ageInTicks = 100.0F + age;
            state.walkAnimationPos = 3.1F;
            state.walkAnimationSpeed = 0.6F;
            state.croakAnimationState.start(100);
            rows.add(frogRow(model, root, state));
        }
    }

    private static AnimationState clipState(final FrogRenderState state, final String clip) {
        return switch (clip) {
            case "jump" -> state.jumpAnimationState;
            case "croak" -> state.croakAnimationState;
            case "tongue" -> state.tongueAnimationState;
            default -> state.swimIdleAnimationState;
        };
    }

    private static JsonObject frogRow(final FrogModel model, final ModelPart root, final FrogRenderState state) {
        model.setupAnim(state);
        JsonObject inputs = livingInputs(state);
        inputs.addProperty("swimming", state.isSwimming);
        JsonObject clips = new JsonObject();
        clips.add("jump", startTick(state.jumpAnimationState));
        clips.add("croak", startTick(state.croakAnimationState));
        clips.add("tongue", startTick(state.tongueAnimationState));
        clips.add("idle_water", startTick(state.swimIdleAnimationState));
        inputs.add("clip_start_ticks", clips);
        return poseRow(ModelLayers.FROG, inputs, root);
    }

    private static JsonElement startTick(final AnimationState state) {
        if (!state.isStarted()) {
            return JsonNull.INSTANCE;
        }
        // The state keeps only its start; the time at age 0 gives that start back.
        return new JsonPrimitive(-state.getTimeInMillis(0.0F) / 50L);
    }

    private static JsonObject livingInputs(final LivingEntityRenderState state) {
        JsonObject inputs = new JsonObject();
        inputs.addProperty("walk_position", state.walkAnimationPos);
        inputs.addProperty("walk_speed", state.walkAnimationSpeed);
        inputs.addProperty("head_yaw_degrees", state.yRot);
        inputs.addProperty("head_pitch_degrees", state.xRot);
        inputs.addProperty("age_ticks", state.ageInTicks);
        return inputs;
    }

    private static JsonObject poseRow(final ModelLayerLocation key, final JsonObject inputs, final ModelPart root) {
        JsonObject row = new JsonObject();
        row.addProperty("model", key.model().toString());
        row.addProperty("layer", key.layer());
        row.add("inputs", inputs);
        JsonObject parts = new JsonObject();
        collect(parts, "root", root);
        row.add("parts", parts);
        return row;
    }

    @SuppressWarnings("unchecked")
    private static void collect(final JsonObject parts, final String path, final ModelPart part) {
        JsonObject pose = new JsonObject();
        pose.add("position", vector(part.x, part.y, part.z));
        pose.add("rotation", vector(part.xRot, part.yRot, part.zRot));
        pose.add("scale", vector(part.xScale, part.yScale, part.zScale));
        pose.addProperty("visible", part.visible);
        parts.add(path, pose);
        Map<String, ModelPart> children;
        try {
            children = new TreeMap<>((Map<String, ModelPart>) read(part, "children"));
        } catch (ReflectiveOperationException failure) {
            throw new IllegalStateException(failure);
        }
        for (Map.Entry<String, ModelPart> child : children.entrySet()) {
            collect(parts, path + "/" + child.getKey(), child.getValue());
        }
    }

    // ---- block entities ------------------------------------------------------------------------

    private static final Direction[] HORIZONTAL = {Direction.NORTH, Direction.EAST, Direction.SOUTH, Direction.WEST};

    private static List<JsonObject> blockEntities() throws ReflectiveOperationException {
        Map<ModelLayerLocation, LayerDefinition> roots = LayerDefinitions.createRoots();
        List<JsonObject> rows = new ArrayList<>();

        ModelPart book = roots.get(ModelLayers.BOOK).bakeRoot();
        BookModel bookModel = new BookModel(book);
        bookModel.setupAnim(BookModel.State.forAnimation(0.0F, 0.1F, 0.9F, 0.0F));
        rows.add(staticPose("enchanting_table_book", ModelLayers.BOOK, book));
        Field lectern = LecternRenderer.class.getDeclaredField("BOOK_STATE");
        lectern.setAccessible(true);
        bookModel.setupAnim((BookModel.State) lectern.get(null));
        rows.add(staticPose("lectern_book", ModelLayers.BOOK, book));

        for (ModelLayerLocation key : new ModelLayerLocation[] {ModelLayers.STANDING_BANNER_FLAG, ModelLayers.WALL_BANNER_FLAG}) {
            ModelPart flag = roots.get(key).bakeRoot();
            new BannerFlagModel(flag).setupAnim(0.0F);
            rows.add(staticPose("banner_flag", key, flag));
        }
        ModelPart piglin = roots.get(ModelLayers.PIGLIN_HEAD).bakeRoot();
        new PiglinHeadModel(piglin).setupAnim(new SkullModelBase.State());
        rows.add(staticPose("piglin_head", ModelLayers.PIGLIN_HEAD, piglin));
        ModelPart dragon = roots.get(ModelLayers.DRAGON_SKULL).bakeRoot();
        new DragonHeadModel(dragon).setupAnim(new SkullModelBase.State());
        rows.add(staticPose("dragon_head", ModelLayers.DRAGON_SKULL, dragon));

        for (Direction facing : HORIZONTAL) {
            rows.add(placement("chest", facing.getSerializedName(), ChestRenderer.modelTransformation(facing)));
            rows.add(placement("decorated_pot", facing.getSerializedName(), DecoratedPotRenderer.modelTransformation(facing)));
            rows.add(placement("wall_banner", facing.getSerializedName(), BannerRenderer.TRANSFORMATIONS.wallTransformation(facing)));
            rows.add(placement("wall_head", facing.getSerializedName(), SkullBlockRenderer.TRANSFORMATIONS.wallTransformation(facing)));
        }
        for (Direction facing : Direction.values()) {
            rows.add(placement("shulker_box", facing.getSerializedName(), ShulkerBoxRenderer.modelTransform(facing)));
        }
        for (int rotation = 0; rotation < 16; rotation++) {
            rows.add(placement("standing_banner", Integer.toString(rotation), BannerRenderer.TRANSFORMATIONS.freeTransformations(rotation)));
            rows.add(placement("standing_head", Integer.toString(rotation), SkullBlockRenderer.TRANSFORMATIONS.freeTransformations(rotation)));
        }
        return rows;
    }

    private static JsonObject staticPose(final String name, final ModelLayerLocation key, final ModelPart root) {
        JsonObject row = new JsonObject();
        row.addProperty("kind", "pose");
        row.addProperty("case", name);
        row.addProperty("model", key.model().toString());
        row.addProperty("layer", key.layer());
        JsonObject parts = new JsonObject();
        collect(parts, "root", root);
        row.add("parts", parts);
        return row;
    }

    private static JsonObject placement(final String shape, final String state, final Transformation transformation) {
        Matrix4fc matrix = transformation.getMatrix();
        JsonArray rows = new JsonArray();
        for (int row = 0; row < 4; row++) {
            JsonArray values = new JsonArray();
            for (int column = 0; column < 4; column++) {
                values.add(matrix.get(column, row));
            }
            rows.add(values);
        }
        JsonObject out = new JsonObject();
        out.addProperty("kind", "placement");
        out.addProperty("shape", shape);
        out.addProperty("state", state);
        out.add("matrix", rows);
        return out;
    }

    // ---- helpers ------------------------------------------------------------------------------

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

    private static JsonArray vector(final float x, final float y, final float z) {
        JsonArray array = new JsonArray();
        array.add(x);
        array.add(y);
        array.add(z);
        return array;
    }

    private static JsonArray vector(final Vector3fc value) {
        return vector(value.x(), value.y(), value.z());
    }

    private static JsonArray pair(final float u, final float v) {
        JsonArray array = new JsonArray();
        array.add(u);
        array.add(v);
        return array;
    }
}
