package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import com.mojang.serialization.JsonOps;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.TreeSet;
import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.core.particles.ParticleTypes;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.EasingType;
import net.minecraft.util.KeyframeTrackSampler;
import net.minecraft.util.StringRepresentable;
import net.minecraft.world.attribute.AmbientParticle;
import net.minecraft.world.attribute.AttributeType;
import net.minecraft.world.attribute.AttributeTypes;
import net.minecraft.world.attribute.EnvironmentAttribute;
import net.minecraft.world.attribute.EnvironmentAttributeMap;
import net.minecraft.world.attribute.EnvironmentAttributeSystem;
import net.minecraft.world.attribute.GaussianSampler;
import net.minecraft.world.attribute.LerpFunction;
import net.minecraft.world.attribute.SpatialAttributeInterpolator;
import net.minecraft.world.attribute.WeatherAttributes;
import net.minecraft.world.attribute.modifier.AttributeModifier;
import net.minecraft.world.attribute.modifier.ColorModifier;
import net.minecraft.world.attribute.modifier.FloatModifier;
import net.minecraft.world.attribute.modifier.FloatWithAlpha;
import net.minecraft.world.clock.ClockNetworkState;
import net.minecraft.world.clock.ClockState;
import net.minecraft.world.clock.ClockTimeMarker;
import net.minecraft.world.clock.ServerClockManager;
import net.minecraft.world.clock.WorldClock;
import net.minecraft.world.entity.schedule.Activity;
import net.minecraft.world.level.MoonPhase;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.timeline.AttributeTrack;
import net.minecraft.client.ClientClockManager;
import org.joml.Vector3f;
import org.joml.Vector3fc;
import org.joml.Vector4f;
import org.joml.Vector4fc;

/** Samples the environment attribute arithmetic, the timeline tracks and the clocks of the game. */
@SuppressWarnings({"unchecked", "rawtypes"})
public final class EnvironmentOracle {
    private static final String[] TIMELINES = {"day", "moon", "villager_schedule", "early_game"};
    private static final String[] EASINGS = {
        "constant", "linear", "in_back", "in_bounce", "in_circ", "in_cubic", "in_elastic", "in_expo", "in_quad",
        "in_quart", "in_quint", "in_sine", "in_out_back", "in_out_bounce", "in_out_circ", "in_out_cubic",
        "in_out_elastic", "in_out_expo", "in_out_quad", "in_out_quart", "in_out_quint", "in_out_sine", "out_back",
        "out_bounce", "out_circ", "out_cubic", "out_elastic", "out_expo", "out_quad", "out_quart", "out_quint",
        "out_sine"
    };
    private static final float[] ALPHAS = {0.0F, 0.1F, 0.25F, 0.5F, 0.75F, 0.9F, 1.0F};

    private EnvironmentOracle() {}

    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Path out = Path.of(args[0]);
        Files.createDirectories(out);
        JsonObject root = new JsonObject();
        root.addProperty("game_version", SharedConstants.getCurrentVersion().id());
        root.add("modifiers", modifiers());
        root.add("type_lerps", typeLerps());
        root.add("easings", easings());
        root.add("timelines", timelines());
        root.add("spatial", spatial());
        root.add("gaussian", gaussian());
        root.add("weather", weather());
        root.add("range_clamp", rangeClamp());
        root.add("server_clock", serverClock());
        root.add("client_clock", clientClock());
        root.add("time_markers", timeMarkers());
        Gson gson = new GsonBuilder().disableHtmlEscaping().serializeSpecialFloatingPointValues().setPrettyPrinting().create();
        Files.writeString(out.resolve("environment.json"), gson.toJson(root) + "\n", StandardCharsets.UTF_8);
    }

    private static EnvironmentAttribute attribute(final String id) {
        EnvironmentAttribute attribute = BuiltInRegistries.ENVIRONMENT_ATTRIBUTE.getValue(Identifier.parse(id));
        if (attribute == null) {
            throw new IllegalStateException("no attribute " + id);
        }
        return attribute;
    }

    private static JsonElement value(final Object value) {
        if (value instanceof Float f) {
            return new JsonPrimitive(f);
        } else if (value instanceof Integer i) {
            return new JsonPrimitive(i);
        } else if (value instanceof Long l) {
            return new JsonPrimitive(l);
        } else if (value instanceof Boolean b) {
            return new JsonPrimitive(b);
        } else if (value instanceof Vector4fc v) {
            JsonArray array = new JsonArray();
            array.add(v.x());
            array.add(v.y());
            array.add(v.z());
            array.add(v.w());
            return array;
        } else if (value instanceof Vector3fc v) {
            JsonArray array = new JsonArray();
            array.add(v.x());
            array.add(v.y());
            array.add(v.z());
            return array;
        } else if (value instanceof FloatWithAlpha f) {
            JsonObject object = new JsonObject();
            object.addProperty("value", f.value());
            object.addProperty("alpha", f.alpha());
            return object;
        } else if (value instanceof ColorModifier.BlendToGray g) {
            JsonObject object = new JsonObject();
            object.addProperty("brightness", g.brightness());
            object.addProperty("factor", g.factor());
            return object;
        } else if (value instanceof Activity activity) {
            return new JsonPrimitive(BuiltInRegistries.ACTIVITY.getKey(activity).toString());
        } else if (value instanceof StringRepresentable s) {
            return new JsonPrimitive(s.getSerializedName());
        } else if (value instanceof AmbientParticle p) {
            JsonObject object = new JsonObject();
            object.addProperty("particle", BuiltInRegistries.PARTICLE_TYPE.getKey(p.particle().getType()).toString());
            object.addProperty("probability", p.probability());
            return object;
        } else if (value instanceof List<?> list) {
            JsonArray array = new JsonArray();
            for (Object element : list) {
                array.add(value(element));
            }
            return array;
        } else if (value instanceof ParticleOptions p) {
            return new JsonPrimitive(BuiltInRegistries.PARTICLE_TYPE.getKey(p.getType()).toString());
        }
        throw new IllegalStateException("cannot write " + value);
    }

    private static JsonObject modifierCase(final Object subject, final Object argument, final Object result) {
        JsonObject object = new JsonObject();
        object.add("subject", value(subject));
        object.add("argument", value(argument));
        object.add("result", value(result));
        return object;
    }

    private static JsonArray library(final Map library, final AttributeModifier.OperationId op, final List subjects, final List arguments) {
        AttributeModifier modifier = (AttributeModifier)library.get(op);
        JsonArray cases = new JsonArray();
        for (Object subject : subjects) {
            for (Object argument : arguments) {
                cases.add(modifierCase(subject, argument, modifier.apply(subject, argument)));
            }
        }
        return cases;
    }

    private static List<Vector3fc> rgbValues() {
        return List.of(
            new Vector3f(0.0F, 0.0F, 0.0F),
            new Vector3f(1.0F, 1.0F, 1.0F),
            new Vector3f(0x78 / 255.0F, 0xA7 / 255.0F, 0xFF / 255.0F),
            new Vector3f(0.2F, 0.5F, 0.9F),
            new Vector3f(1.5F, -0.25F, 0.5F)
        );
    }

    private static List<Vector4fc> argbValues() {
        return List.of(
            new Vector4f(0.0F, 0.0F, 0.0F, 1.0F),
            new Vector4f(1.0F, 1.0F, 1.0F, 0.8F),
            new Vector4f(0.2F, 0.5F, 0.9F, 0.0F),
            new Vector4f(0.48F, 0.48F, 1.0F, 0.3125F),
            new Vector4f(0.1F, 0.1F, 0.15F, 0.5F),
            new Vector4f(1.0F, 0.5F, 0.5F, 0.6F)
        );
    }

    private static JsonObject modifiers() {
        JsonObject root = new JsonObject();
        List<Float> floats = List.of(0.0F, -0.0F, 0.25F, 1.0F, -3.5F, 1.0E6F, 0.1F);
        JsonObject floatCases = new JsonObject();
        for (AttributeModifier.OperationId op : List.of(
            AttributeModifier.OperationId.ADD,
            AttributeModifier.OperationId.SUBTRACT,
            AttributeModifier.OperationId.MULTIPLY,
            AttributeModifier.OperationId.MINIMUM,
            AttributeModifier.OperationId.MAXIMUM
        )) {
            floatCases.add(op.getSerializedName(), library(AttributeModifier.FLOAT_LIBRARY, op, floats, floats));
        }
        List<FloatWithAlpha> withAlpha = List.of(
            new FloatWithAlpha(4.0F, 0.3125F), new FloatWithAlpha(0.24F, 0.52734375F), new FloatWithAlpha(1.0F, 0.0F),
            new FloatWithAlpha(-2.0F, 1.0F), new FloatWithAlpha(10.0F, 0.5F)
        );
        floatCases.add("alpha_blend", library(AttributeModifier.FLOAT_LIBRARY, AttributeModifier.OperationId.ALPHA_BLEND, floats, withAlpha));
        root.add("float", floatCases);

        JsonObject integerCases = new JsonObject();
        List<Integer> integers = List.of(0, 1, -7, Integer.MAX_VALUE, Integer.MIN_VALUE, 46341);
        for (AttributeModifier.OperationId op : List.of(
            AttributeModifier.OperationId.ADD,
            AttributeModifier.OperationId.SUBTRACT,
            AttributeModifier.OperationId.MULTIPLY,
            AttributeModifier.OperationId.MINIMUM,
            AttributeModifier.OperationId.MAXIMUM
        )) {
            integerCases.add(op.getSerializedName(), library(AttributeModifier.INTEGER_LIBRARY, op, integers, integers));
        }
        root.add("integer", integerCases);

        JsonObject booleanCases = new JsonObject();
        List<Boolean> booleans = List.of(false, true);
        for (AttributeModifier.OperationId op : List.of(
            AttributeModifier.OperationId.AND,
            AttributeModifier.OperationId.NAND,
            AttributeModifier.OperationId.OR,
            AttributeModifier.OperationId.NOR,
            AttributeModifier.OperationId.XOR,
            AttributeModifier.OperationId.XNOR
        )) {
            booleanCases.add(op.getSerializedName(), library(AttributeModifier.BOOLEAN_LIBRARY, op, booleans, booleans));
        }
        root.add("boolean", booleanCases);

        List<ColorModifier.BlendToGray> grays = List.of(
            new ColorModifier.BlendToGray(0.6F, 0.75F),
            new ColorModifier.BlendToGray(0.24F, 0.5F),
            new ColorModifier.BlendToGray(0.24F, 0.94F),
            new ColorModifier.BlendToGray(0.095F, 0.94F),
            new ColorModifier.BlendToGray(1.0F, 0.0F),
            new ColorModifier.BlendToGray(0.0F, 1.0F)
        );
        JsonObject rgbCases = new JsonObject();
        rgbCases.add("add", library(AttributeModifier.RGB_COLOR_LIBRARY, AttributeModifier.OperationId.ADD, rgbValues(), rgbValues()));
        rgbCases.add("subtract", library(AttributeModifier.RGB_COLOR_LIBRARY, AttributeModifier.OperationId.SUBTRACT, rgbValues(), rgbValues()));
        rgbCases.add("multiply", library(AttributeModifier.RGB_COLOR_LIBRARY, AttributeModifier.OperationId.MULTIPLY, rgbValues(), rgbValues()));
        rgbCases.add("alpha_blend", library(AttributeModifier.RGB_COLOR_LIBRARY, AttributeModifier.OperationId.ALPHA_BLEND, rgbValues(), argbValues()));
        rgbCases.add("blend_to_gray", library(AttributeModifier.RGB_COLOR_LIBRARY, AttributeModifier.OperationId.BLEND_TO_GRAY, rgbValues(), grays));
        root.add("rgb_color", rgbCases);

        JsonObject argbCases = new JsonObject();
        argbCases.add("add", library(AttributeModifier.ARGB_COLOR_LIBRARY, AttributeModifier.OperationId.ADD, argbValues(), rgbValues()));
        argbCases.add("subtract", library(AttributeModifier.ARGB_COLOR_LIBRARY, AttributeModifier.OperationId.SUBTRACT, argbValues(), rgbValues()));
        argbCases.add("multiply", library(AttributeModifier.ARGB_COLOR_LIBRARY, AttributeModifier.OperationId.MULTIPLY, argbValues(), argbValues()));
        argbCases.add("alpha_blend", library(AttributeModifier.ARGB_COLOR_LIBRARY, AttributeModifier.OperationId.ALPHA_BLEND, argbValues(), argbValues()));
        argbCases.add("blend_to_gray", library(AttributeModifier.ARGB_COLOR_LIBRARY, AttributeModifier.OperationId.BLEND_TO_GRAY, argbValues(), grays));
        root.add("argb_color", argbCases);
        return root;
    }

    private static JsonArray lerpCases(final LerpFunction lerp, final List<Object[]> pairs) {
        JsonArray cases = new JsonArray();
        for (Object[] pair : pairs) {
            for (float alpha : ALPHAS) {
                JsonObject object = new JsonObject();
                object.addProperty("alpha", alpha);
                object.add("from", value(pair[0]));
                object.add("to", value(pair[1]));
                object.add("result", value(lerp.apply(alpha, pair[0], pair[1])));
                cases.add(object);
            }
        }
        return cases;
    }

    private static JsonObject typeLerp(final AttributeType type, final List<Object[]> pairs) {
        JsonObject object = new JsonObject();
        object.add("keyframe", lerpCases(type.keyframeLerp(), pairs));
        object.add("weather", lerpCases(type.stateChangeLerp(), pairs));
        object.add("spatial", lerpCases(type.spatialLerp(), pairs));
        object.add("partial_tick", lerpCases(type.partialTickLerp(), pairs));
        return object;
    }

    private static JsonObject typeLerps() {
        JsonObject root = new JsonObject();
        root.add("float", typeLerp(AttributeTypes.FLOAT, List.of(new Object[]{0.0F, 1.0F}, new Object[]{15.0F, 4.0F}, new Object[]{-8.0F, 96.0F})));
        root.add("angle_degrees", typeLerp(AttributeTypes.ANGLE_DEGREES, List.of(
            new Object[]{350.0F, 10.0F}, new Object[]{10.0F, 350.0F}, new Object[]{0.0F, 89.0F}, new Object[]{0.0F, 90.0F},
            new Object[]{0.0F, -90.0F}, new Object[]{180.0F, -179.0F}, new Object[]{359.9F, 0.0F}, new Object[]{720.0F, 0.5F}
        )));
        root.add("rgb_color", typeLerp(AttributeTypes.RGB_COLOR, List.of(new Object[]{rgbValues().get(0), rgbValues().get(2)}, new Object[]{rgbValues().get(4), rgbValues().get(1)})));
        root.add("argb_color", typeLerp(AttributeTypes.ARGB_COLOR, List.of(new Object[]{argbValues().get(0), argbValues().get(1)}, new Object[]{argbValues().get(3), argbValues().get(5)})));
        root.add("integer", typeLerp(AttributeTypes.INTEGER, List.of(new Object[]{0, 10}, new Object[]{10, 0}, new Object[]{-3, 4})));
        root.add("boolean", typeLerp(AttributeTypes.BOOLEAN, List.of(new Object[]{false, true}, new Object[]{true, false})));
        root.add("moon_phase", typeLerp(AttributeTypes.MOON_PHASE, List.<Object[]>of(new Object[]{MoonPhase.FULL_MOON, MoonPhase.NEW_MOON})));
        List<AmbientParticle> one = List.of(new AmbientParticle(ParticleTypes.ASH, 0.5F));
        List<AmbientParticle> two = List.of(new AmbientParticle(ParticleTypes.WHITE_ASH, 0.25F), new AmbientParticle(ParticleTypes.SPORE_BLOSSOM_AIR, 1.0F));
        root.add("ambient_particles", typeLerp(AttributeTypes.AMBIENT_PARTICLES, List.<Object[]>of(new Object[]{one, two})));
        return root;
    }

    private static JsonObject easings() {
        JsonObject root = new JsonObject();
        float[] xs = {0.0F, 0.05F, 0.1F, 0.25F, 0.3333F, 0.5F, 0.6F, 0.75F, 0.9F, 0.99F, 1.0F};
        JsonObject named = new JsonObject();
        for (String name : EASINGS) {
            EasingType easing = EasingType.CODEC.parse(JsonOps.INSTANCE, new JsonPrimitive(name)).getOrThrow();
            JsonArray samples = new JsonArray();
            for (float x : xs) {
                JsonArray sample = new JsonArray();
                sample.add(x);
                sample.add(easing.apply(x));
                samples.add(sample);
            }
            named.add(name, samples);
        }
        root.add("named", named);
        JsonArray beziers = new JsonArray();
        float[][] controls = {
            {0.362F, 0.241F, 0.638F, 0.759F}, {0.0F, 0.0F, 1.0F, 1.0F}, {0.42F, 0.0F, 0.58F, 1.0F},
            {0.25F, 0.1F, 0.25F, 1.0F}, {0.0F, 1.5F, 1.0F, -0.5F}, {1.0F, 0.0F, 0.0F, 1.0F}
        };
        for (float[] c : controls) {
            EasingType easing = EasingType.cubicBezier(c[0], c[1], c[2], c[3]);
            JsonObject object = new JsonObject();
            JsonArray cs = new JsonArray();
            for (float v : c) {
                cs.add(v);
            }
            object.add("controls", cs);
            JsonArray samples = new JsonArray();
            for (int i = 0; i <= 40; i++) {
                float x = i / 40.0F;
                JsonArray sample = new JsonArray();
                sample.add(x);
                sample.add(easing.apply(x));
                samples.add(sample);
            }
            object.add("samples", samples);
            beziers.add(object);
        }
        root.add("cubic_bezier", beziers);
        return root;
    }

    private static JsonObject readTimeline(final String name) throws Exception {
        String resource = "data/minecraft/timeline/" + name + ".json";
        try (InputStream in = EnvironmentOracle.class.getClassLoader().getResourceAsStream(resource)) {
            if (in == null) {
                throw new IllegalStateException("no resource " + resource);
            }
            return JsonParser.parseReader(new InputStreamReader(in, StandardCharsets.UTF_8)).getAsJsonObject();
        }
    }

    private static JsonObject timelines() throws Exception {
        JsonObject root = new JsonObject();
        for (String name : TIMELINES) {
            JsonObject file = readTimeline(name);
            Optional<Integer> period = file.has("period_ticks") ? Optional.of(file.get("period_ticks").getAsInt()) : Optional.empty();
            JsonObject timeline = new JsonObject();
            period.ifPresent(p -> timeline.addProperty("period_ticks", p));
            JsonObject tracks = new JsonObject();
            for (Map.Entry<String, JsonElement> entry : file.getAsJsonObject("tracks").entrySet()) {
                EnvironmentAttribute attribute = attribute(entry.getKey());
                AttributeTrack track = (AttributeTrack)AttributeTrack.createCodec(attribute)
                    .parse(JsonOps.INSTANCE, entry.getValue())
                    .getOrThrow();
                KeyframeTrackSampler sampler = track.argumentTrack()
                    .bakeSampler(period, track.modifier().argumentKeyframeLerp(attribute));
                TreeSet<Long> ticks = new TreeSet<>();
                int span = period.orElse(240000);
                for (JsonElement keyframe : entry.getValue().getAsJsonObject().getAsJsonArray("keyframes")) {
                    long tick = keyframe.getAsJsonObject().get("ticks").getAsLong();
                    ticks.add(tick - 1);
                    ticks.add(tick);
                    ticks.add(tick + 1);
                }
                for (long t = 0; t <= span; t += Math.max(1, span / 96)) {
                    ticks.add(t);
                }
                ticks.add(-1L);
                ticks.add(-(long)span - 1);
                ticks.add((long)span + 1);
                ticks.add(2L * span + 7);
                JsonArray samples = new JsonArray();
                for (long tick : ticks) {
                    JsonArray sample = new JsonArray();
                    sample.add(tick);
                    sample.add(value(sampler.sample(tick)));
                    samples.add(sample);
                }
                JsonObject out = new JsonObject();
                JsonObject source = entry.getValue().getAsJsonObject();
                out.addProperty("modifier", source.has("modifier") ? source.get("modifier").getAsString() : "override");
                out.add("samples", samples);
                tracks.add(entry.getKey(), out);
            }
            timeline.add("tracks", tracks);
            root.add("minecraft:" + name, timeline);
        }
        return root;
    }

    private static JsonArray spatial() {
        EnvironmentAttribute sky = attribute("minecraft:visual/sky_color");
        EnvironmentAttribute fogEnd = attribute("minecraft:visual/fog_end_distance");
        EnvironmentAttribute moon = attribute("minecraft:visual/moon_phase");
        EnvironmentAttribute occluder = attribute("minecraft:visual/has_sky_occluder");
        EnvironmentAttributeMap a = EnvironmentAttributeMap.builder()
            .set(sky, new Vector3f(1.0F, 0.0F, 0.0F))
            .set(fogEnd, 100.0F)
            .set(moon, MoonPhase.NEW_MOON)
            .set(occluder, true)
            .build();
        EnvironmentAttributeMap b = EnvironmentAttributeMap.builder()
            .set(sky, new Vector3f(0.0F, 0.0F, 1.0F))
            .modify(fogEnd, FloatModifier.MULTIPLY, 0.5F)
            .set(moon, MoonPhase.FIRST_QUARTER)
            .build();
        EnvironmentAttributeMap c = EnvironmentAttributeMap.builder().build();
        Object[][] bases = {
            {sky, new Vector3f(0.5F, 0.5F, 0.5F)}, {fogEnd, 1024.0F}, {moon, MoonPhase.FULL_MOON}, {occluder, false}
        };
        double[][] weights = {{1.0, 0.0, 0.0}, {1.0, 1.0, 0.0}, {1.0, 3.0, 0.0}, {2.0, 1.0, 1.0}, {0.0, 1.0, 1.0}, {1.0, 2.0, 5.0}};
        EnvironmentAttributeMap[] maps = {a, b, c};
        JsonArray cases = new JsonArray();
        for (Object[] base : bases) {
            for (double[] w : weights) {
                SpatialAttributeInterpolator interpolator = new SpatialAttributeInterpolator();
                JsonArray sources = new JsonArray();
                for (int i = 0; i < 3; i++) {
                    if (w[i] > 0.0) {
                        interpolator.accumulate(w[i], maps[i]);
                        JsonArray source = new JsonArray();
                        source.add("abc".substring(i, i + 1));
                        source.add(w[i]);
                        sources.add(source);
                    }
                }
                EnvironmentAttribute attribute = (EnvironmentAttribute)base[0];
                JsonObject object = new JsonObject();
                object.addProperty("attribute", BuiltInRegistries.ENVIRONMENT_ATTRIBUTE.getKey(attribute).toString());
                object.add("base", value(base[1]));
                object.add("sources", sources);
                object.add("result", value(interpolator.applyAttributeLayer(attribute, base[1])));
                cases.add(object);
            }
        }
        return cases;
    }

    private static JsonArray gaussian() {
        JsonArray cases = new JsonArray();
        Vec3[] positions = {new Vec3(0.0, 0.0, 0.0), new Vec3(0.3, 64.7, -12.2), new Vec3(1.5, 1.5, 1.5), new Vec3(-0.75, 3.25, 0.5)};
        for (Vec3 position : positions) {
            JsonObject object = new JsonObject();
            JsonArray p = new JsonArray();
            p.add(position.x);
            p.add(position.y);
            p.add(position.z);
            object.add("position", p);
            JsonArray samples = new JsonArray();
            GaussianSampler.sample(position, (x, y, z, weight) -> {
                JsonArray sample = new JsonArray();
                sample.add(x);
                sample.add(y);
                sample.add(z);
                sample.add(weight);
                samples.add(sample);
            });
            object.add("samples", samples);
            cases.add(object);
        }
        return cases;
    }

    private static JsonArray weather() {
        String[] ids = {
            "minecraft:visual/sky_color", "minecraft:visual/fog_color", "minecraft:visual/cloud_color",
            "minecraft:gameplay/sky_light_level", "minecraft:visual/sky_light_color", "minecraft:visual/sky_light_factor",
            "minecraft:visual/star_brightness", "minecraft:visual/sunrise_sunset_color", "minecraft:gameplay/bees_stay_in_hive"
        };
        Object[] bases = {
            new Vector3f(0x78 / 255.0F, 0xA7 / 255.0F, 0xFF / 255.0F), new Vector3f(0xC0 / 255.0F, 0xD8 / 255.0F, 1.0F),
            new Vector4f(1.0F, 1.0F, 1.0F, 0.8F), 15.0F, new Vector3f(1.0F, 1.0F, 1.0F), 1.0F, 0.5F,
            new Vector4f(1.0F, 0.6F, 0.2F, 0.7F), false
        };
        float[][] levels = {{0.0F, 0.0F}, {0.5F, 0.0F}, {1.0F, 0.0F}, {1.0F, 0.25F}, {1.0F, 1.0F}, {0.3F, 0.3F}, {0.01F, 0.0F}};
        EnvironmentAttributeMap.Builder baseMap = EnvironmentAttributeMap.builder();
        for (int i = 0; i < ids.length; i++) {
            baseMap.set(attribute(ids[i]), bases[i]);
        }
        EnvironmentAttributeMap base = baseMap.build();
        JsonArray cases = new JsonArray();
        for (float[] level : levels) {
            EnvironmentAttributeSystem.Builder builder = EnvironmentAttributeSystem.builder();
            builder.addConstantLayer(base);
            WeatherAttributes.addBuiltinLayers(builder, new WeatherAttributes.WeatherAccess() {
                @Override
                public float rainLevel() {
                    return level[0];
                }

                @Override
                public float thunderLevel() {
                    return level[1];
                }
            });
            EnvironmentAttributeSystem system = builder.build();
            JsonObject object = new JsonObject();
            object.addProperty("rain_level", level[0]);
            object.addProperty("thunder_level", level[1]);
            JsonObject results = new JsonObject();
            for (int i = 0; i < ids.length; i++) {
                results.add(ids[i], value(system.getValue(attribute(ids[i]), Vec3.ZERO)));
            }
            object.add("results", results);
            cases.add(object);
        }
        JsonObject baseValues = new JsonObject();
        for (int i = 0; i < ids.length; i++) {
            baseValues.add(ids[i], value(bases[i]));
        }
        JsonObject wrapper = new JsonObject();
        wrapper.add("base", baseValues);
        cases.add(wrapper);
        return cases;
    }

    private static JsonArray rangeClamp() {
        Object[][] cases = {
            {"minecraft:visual/fog_end_distance", 100.0F, -500.0F},
            {"minecraft:visual/star_brightness", 0.5F, 2.0F},
            {"minecraft:visual/star_brightness", 0.5F, -2.0F},
            {"minecraft:gameplay/sky_light_level", 15.0F, 10.0F},
            {"minecraft:visual/fog_start_distance", 0.0F, -1000.0F}
        };
        JsonArray out = new JsonArray();
        for (Object[] c : cases) {
            EnvironmentAttribute attribute = attribute((String)c[0]);
            EnvironmentAttributeMap map = EnvironmentAttributeMap.builder()
                .set(attribute, c[1])
                .build();
            EnvironmentAttributeMap add = EnvironmentAttributeMap.builder()
                .modify(attribute, (AttributeModifier) FloatModifier.ADD, (Object) c[2])
                .build();
            EnvironmentAttributeSystem system = EnvironmentAttributeSystem.builder().addConstantLayer(map).addConstantLayer(add).build();
            JsonObject object = new JsonObject();
            object.addProperty("attribute", (String)c[0]);
            object.addProperty("value", (Float)c[1]);
            object.addProperty("add", (Float)c[2]);
            object.add("result", value(system.getValue(attribute, Vec3.ZERO)));
            out.add(object);
        }
        return out;
    }

    private static JsonArray serverClock() {
        float[] rates = {1.0F, 0.25F, 0.1F, 2.5F, 1.0F / 3.0F, 0.7F};
        JsonArray cases = new JsonArray();
        for (float rate : rates) {
            ServerClockManager.ServerClockInstance clock = new ServerClockManager.ServerClockInstance();
            clock.loadFrom(new ClockState(100L, 0.0F, rate, false));
            JsonArray steps = new JsonArray();
            for (int tick = 1; tick <= 1000; tick++) {
                clock.tick();
                if (tick <= 12 || tick % 100 == 0) {
                    JsonArray step = new JsonArray();
                    step.add(tick);
                    step.add(clock.totalTicks());
                    step.add(clock.partialTick());
                    steps.add(step);
                }
            }
            JsonObject object = new JsonObject();
            object.addProperty("rate", rate);
            object.addProperty("start_total_ticks", 100L);
            object.add("steps", steps);
            cases.add(object);
        }
        return cases;
    }

    private static JsonArray clientClock() {
        float[] rates = {1.0F, 0.75F, 0.1F, 2.5F, 0.0F};
        long[] gameTimes = {1L, 2L, 3L, 10L, 11L, 11L, 50L};
        JsonArray cases = new JsonArray();
        for (float rate : rates) {
            ClientClockManager manager = new ClientClockManager();
            Holder<WorldClock> holder = Holder.direct(new WorldClock());
            manager.handleUpdates(0L, Map.of(holder, new ClockNetworkState(100L, 0.5F, rate)));
            JsonArray steps = new JsonArray();
            for (long gameTime : gameTimes) {
                manager.tick(gameTime);
                ClientClockManager.ClientClockInstance clock = manager.getInstance(holder);
                JsonArray step = new JsonArray();
                step.add(gameTime);
                step.add(clock.totalTicks());
                step.add(clock.partialTick());
                steps.add(step);
            }
            JsonObject object = new JsonObject();
            object.addProperty("rate", rate);
            object.addProperty("update_game_time", 0L);
            object.addProperty("update_total_ticks", 100L);
            object.addProperty("update_partial_tick", 0.5F);
            object.add("steps", steps);
            cases.add(object);
        }
        return cases;
    }

    private static JsonArray timeMarkers() {
        Holder<WorldClock> holder = Holder.direct(new WorldClock());
        Object[][] markers = {{1000, 24000}, {0, 24000}, {13000, 24000}, {100, null}, {0, 192000}};
        long[] totals = {0L, 1L, 500L, 999L, 1000L, 1001L, 23999L, 24000L, 49000L, 50000L, 191999L, 192000L};
        JsonArray cases = new JsonArray();
        for (Object[] m : markers) {
            int ticks = (Integer)m[0];
            Optional<Integer> period = m[1] == null ? Optional.empty() : Optional.of((Integer)m[1]);
            ClockTimeMarker marker = new ClockTimeMarker(holder, ticks, period, false);
            JsonArray samples = new JsonArray();
            for (long t : totals) {
                JsonArray sample = new JsonArray();
                sample.add(t);
                sample.add(marker.occursAt(t));
                sample.add(marker.resolveTimeToMoveTo(t));
                sample.add(marker.getRepetitionCount(t));
                samples.add(sample);
            }
            JsonObject object = new JsonObject();
            object.addProperty("ticks", ticks);
            period.ifPresent(p -> object.addProperty("period_ticks", p));
            object.add("samples", samples);
            cases.add(object);
        }
        return cases;
    }
}
