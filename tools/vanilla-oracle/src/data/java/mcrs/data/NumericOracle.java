package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.Random;
import net.minecraft.SharedConstants;
import net.minecraft.util.Mth;

/** Samples the numeric primitives of the game. Floats are written as their raw bits in hex. */
public final class NumericOracle {
    private static final double TABLE_SCALE = 10430.378350470453;

    private NumericOracle() {}

    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Path out = Path.of(args[0]);
        Files.createDirectories(out);
        Random random = new Random(20261107L);
        JsonObject root = new JsonObject();
        root.addProperty("game_version", SharedConstants.getCurrentVersion().id());
        root.add("sine_table", sineTable());
        root.add("sin_cos", sinCos(random));
        root.add("wrap_degrees", wrapDegrees(random));
        root.add("atan2_tables", atan2Tables());
        root.add("atan2", atan2(random));
        root.add("inverse_sqrt_estimate", inverseSqrt(random));
        root.add("hsv_to_rgb", hsv(random));
        root.add("floor", floor(random));
        root.add("clamp", clamp(random));
        root.add("min_max_signum", minMaxSignum(random));
        root.add("log_pow", logPow(random));
        root.add("lerp", lerp(random));
        root.add("lerp_int", lerpInt(random));
        root.add("map", map(random));
        root.add("floor_div_mod", floorDivMod(random));
        StringBuilder text = new StringBuilder();
        write(text, root, "");
        Files.writeString(out.resolve("numeric.json"), text.append('\n').toString(), StandardCharsets.UTF_8);
    }

    private static final Gson COMPACT = new GsonBuilder().disableHtmlEscaping().create();

    private static void write(final StringBuilder text, final JsonElement element, final String indent) {
        String inner = indent + "  ";
        if (element instanceof JsonObject object) {
            text.append("{\n");
            int i = 0;
            for (var entry : object.entrySet()) {
                text.append(inner).append(COMPACT.toJson(entry.getKey())).append(": ");
                write(text, entry.getValue(), inner);
                text.append(++i < object.size() ? ",\n" : "\n");
            }
            text.append(indent).append('}');
        } else if (element instanceof JsonArray array && !array.isEmpty() && array.get(0).isJsonArray()) {
            text.append("[\n");
            for (int i = 0; i < array.size(); i++) {
                text.append(inner).append(COMPACT.toJson(array.get(i))).append(i + 1 < array.size() ? ",\n" : "\n");
            }
            text.append(indent).append(']');
        } else {
            text.append(COMPACT.toJson(element));
        }
    }

    private static String f(final float v) {
        return String.format("%08x", Float.floatToRawIntBits(v));
    }

    private static String d(final double v) {
        return String.format("%016x", Double.doubleToRawLongBits(v));
    }

    private static JsonArray row(final String... cells) {
        JsonArray row = new JsonArray();
        for (String cell : cells) {
            row.add(cell);
        }
        return row;
    }

    private static JsonObject sineTable() throws Exception {
        ByteBuffer bytes = ByteBuffer.allocate(65536 * 4).order(ByteOrder.LITTLE_ENDIAN);
        JsonArray probes = new JsonArray();
        int[] probeIndices = {0, 1, 2, 4096, 8191, 8192, 16383, 16384, 16385, 24576, 32767, 32768, 32769, 40960, 49151, 49152, 49153, 65534, 65535};
        int probe = 0;
        for (int index = 0; index < 65536; index++) {
            float entry = Mth.sin(index / TABLE_SCALE);
            bytes.putInt(Float.floatToRawIntBits(entry));
            if (probe < probeIndices.length && probeIndices[probe] == index) {
                JsonArray cell = new JsonArray();
                cell.add(index);
                cell.add(f(entry));
                probes.add(cell);
                probe++;
            }
        }
        JsonObject table = new JsonObject();
        table.addProperty("entries", 65536);
        table.addProperty("sha256_of_le_bits", HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes.array())));
        table.add("probes", probes);
        return table;
    }

    private static JsonArray sinCos(final Random random) {
        List<Double> inputs = new ArrayList<>(List.of(
            0.0, -0.0, Double.NaN, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY,
            Math.PI, -Math.PI, Math.PI / 2, -Math.PI / 2, 2 * Math.PI, 3 * Math.PI / 2, -3 * Math.PI / 2,
            (double) (float) Math.PI, (double) (float) (Math.PI / 2), 1.0E-300, -1.0E-300,
            1.0E10, -1.0E10, 1.0E300, -1.0E300, 9.0E14, 1.0E15, 1.0E16
        ));
        for (int k : new int[] {1, 7, 8191, 16383, 32767, 32768, 49151, 65535, 65536, 65537, 100000}) {
            for (double half : new double[] {-0.5, 0.0, 0.5}) {
                double x = (k + half) / TABLE_SCALE;
                inputs.add(x);
                inputs.add(Math.nextUp(x));
                inputs.add(Math.nextDown(x));
                inputs.add(-x);
            }
        }
        for (int i = 0; i < 1500; i++) {
            inputs.add((random.nextDouble() * 2.0 - 1.0) * 100.0);
        }
        for (int i = 0; i < 500; i++) {
            inputs.add((double) ((random.nextFloat() * 2.0F - 1.0F) * 10.0F));
        }
        JsonArray rows = new JsonArray();
        for (double x : inputs) {
            rows.add(row(d(x), f(Mth.sin(x)), f(Mth.cos(x))));
        }
        return rows;
    }

    private static JsonArray wrapDegrees(final Random random) {
        List<Float> inputs = new ArrayList<>(List.of(
            0.0F, -0.0F, 180.0F, -180.0F, Math.nextDown(180.0F), Math.nextUp(180.0F), Math.nextDown(-180.0F), Math.nextUp(-180.0F),
            360.0F, -360.0F, 540.0F, -540.0F, 720.0F, 1.0E-45F, -1.0E-45F, 1.0E10F, -1.0E10F, Float.MAX_VALUE,
            Float.NaN, Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY, 359.99997F, -359.99997F
        ));
        for (int i = 0; i < 1000; i++) {
            inputs.add((random.nextFloat() * 2.0F - 1.0F) * 2000.0F);
        }
        JsonArray rows = new JsonArray();
        for (float x : inputs) {
            rows.add(row(f(x), f(Mth.wrapDegrees(x))));
        }
        return rows;
    }

    private static JsonArray atan2Tables() {
        JsonArray rows = new JsonArray();
        for (int index = 0; index <= 256; index++) {
            double angle = Math.asin(index / 256.0);
            rows.add(row(Integer.toString(index), d(angle), d(Math.cos(angle))));
        }
        return rows;
    }

    private static JsonArray atan2(final Random random) {
        double[][] edges = {
            {0.0, 0.0}, {-0.0, 0.0}, {0.0, -0.0}, {-0.0, -0.0}, {1.0, 0.0}, {0.0, 1.0}, {-1.0, 0.0}, {0.0, -1.0},
            {1.0, 1.0}, {-1.0, -1.0}, {1.0, -1.0}, {Double.NaN, 1.0}, {1.0, Double.NaN},
            {1.0, Double.POSITIVE_INFINITY}, {Double.POSITIVE_INFINITY, Double.POSITIVE_INFINITY},
            {1.0E-300, 1.0E-300}, {1.0E200, 1.0E200}, {1.0E-5, 1.0}, {1.0, 1.0E-5}
        };
        JsonArray rows = new JsonArray();
        for (double[] edge : edges) {
            rows.add(row(d(edge[0]), d(edge[1]), d(Mth.atan2(edge[0], edge[1]))));
        }
        for (int i = 0; i < 2000; i++) {
            double scale = Math.pow(10.0, random.nextInt(9) - 4);
            double y = (random.nextDouble() * 2.0 - 1.0) * scale;
            double x = (random.nextDouble() * 2.0 - 1.0) * scale;
            rows.add(row(d(y), d(x), d(Mth.atan2(y, x))));
        }
        return rows;
    }

    @SuppressWarnings("deprecation")
    private static JsonArray inverseSqrt(final Random random) {
        JsonArray rows = new JsonArray();
        double[] edges = {1.0, 0.25, 2.0, 4.0, 1.0E-10, 1.0E10, 0.5, 3.0};
        for (double x : edges) {
            rows.add(row(d(x), d(Mth.fastInvSqrt(x))));
        }
        for (int i = 0; i < 500; i++) {
            double x = random.nextDouble() * Math.pow(10.0, random.nextInt(13) - 6);
            rows.add(row(d(x), d(Mth.fastInvSqrt(x))));
        }
        return rows;
    }

    private static JsonArray hsv(final Random random) {
        JsonArray rows = new JsonArray();
        float[][] edges = {
            {0.0F, 0.0F, 0.0F}, {0.0F, 1.0F, 1.0F}, {1.0F, 1.0F, 1.0F}, {0.99999994F, 1.0F, 1.0F},
            {1.0F / 6.0F, 1.0F, 1.0F}, {0.5F, 0.5F, 0.5F}, {2.5F, 1.0F, 1.0F}, {-0.1F, 1.0F, 1.0F},
            {0.3F, 1.5F, 1.0F}, {0.3F, 1.0F, 1.5F}, {0.62222224F, 0.5F, 1.0F}, {Float.NaN, 1.0F, 1.0F}
        };
        for (float[] edge : edges) {
            rows.add(row(f(edge[0]), f(edge[1]), f(edge[2]), Integer.toString(Mth.hsvToRgb(edge[0], edge[1], edge[2]))));
        }
        for (int i = 0; i < 1000; i++) {
            float h = random.nextFloat();
            float s = random.nextFloat();
            float v = random.nextFloat();
            rows.add(row(f(h), f(s), f(v), Integer.toString(Mth.hsvToRgb(h, s, v))));
        }
        return rows;
    }

    private static JsonArray floor(final Random random) {
        List<Float> inputs = new ArrayList<>(List.of(
            0.0F, -0.0F, 0.5F, -0.5F, 1.0F, -1.0F, 7.5F, -7.5F, 1.0E30F, -1.0E30F, 2.14748365E9F, -2.14748365E9F,
            Float.NaN, Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY, Math.nextDown(1.0F), Math.nextUp(-1.0F)
        ));
        for (int i = 0; i < 500; i++) {
            inputs.add((random.nextFloat() * 2.0F - 1.0F) * 1.0E6F);
        }
        JsonArray rows = new JsonArray();
        for (float x : inputs) {
            rows.add(row(f(x), Integer.toString(Mth.floor(x)), Integer.toString(Mth.ceil(x)), Integer.toString((int) x)));
        }
        return rows;
    }

    private static JsonArray clamp(final Random random) {
        float[][] edges = {
            {Float.NaN, 0.0F, 1.0F}, {-0.0F, 0.0F, 1.0F}, {0.0F, -1.0F, -0.0F}, {-0.0F, -1.0F, 0.0F},
            {2.0F, 0.0F, 1.0F}, {-2.0F, 0.0F, 1.0F}, {0.5F, 0.0F, 1.0F}, {Float.POSITIVE_INFINITY, -1.0F, 1.0F},
            {Float.NEGATIVE_INFINITY, -1.0F, 1.0F}, {1.0F, 1.0F, 1.0F}
        };
        JsonArray rows = new JsonArray();
        for (float[] edge : edges) {
            rows.add(row(f(edge[0]), f(edge[1]), f(edge[2]), f(Math.clamp(edge[0], edge[1], edge[2]))));
        }
        for (int i = 0; i < 300; i++) {
            float x = (random.nextFloat() * 2.0F - 1.0F) * 4.0F;
            rows.add(row(f(x), f(-1.0F), f(1.0F), f(Math.clamp(x, -1.0F, 1.0F))));
        }
        return rows;
    }

    private static JsonArray minMaxSignum(final Random random) {
        float[] values = {0.0F, -0.0F, 1.0F, -1.0F, Float.NaN, Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY, 1.0E-45F, -1.0E-45F, 3.5F};
        JsonArray rows = new JsonArray();
        for (float a : values) {
            for (float b : values) {
                rows.add(row(f(a), f(b), f(Math.min(a, b)), f(Math.max(a, b)), f(Math.signum(a))));
            }
        }
        return rows;
    }

    private static JsonArray logPow(final Random random) {
        JsonArray rows = new JsonArray();
        float[][] edges = {
            {0.0F, 2.0F}, {-0.0F, 0.5F}, {-1.0F, 2.0F}, {1.0F, Float.NaN}, {Float.NaN, 0.0F}, {2.0F, 0.5F},
            {-8.0F, 1.0F / 3.0F}, {Float.POSITIVE_INFINITY, -1.0F}, {10.0F, 38.0F}, {10.0F, 39.0F}
        };
        for (float[] edge : edges) {
            rows.add(row(f(edge[0]), f(edge[1]), f((float) Math.log(edge[0])), f((float) Math.pow(edge[0], edge[1]))));
        }
        for (int i = 0; i < 1000; i++) {
            float a = random.nextFloat() * 100.0F;
            float b = (random.nextFloat() * 2.0F - 1.0F) * 8.0F;
            rows.add(row(f(a), f(b), f((float) Math.log(a)), f((float) Math.pow(a, b))));
        }
        return rows;
    }

    private static JsonObject lerp(final Random random) {
        JsonArray singles = new JsonArray();
        JsonArray doubles = new JsonArray();
        for (int i = 0; i < 500; i++) {
            float a = random.nextFloat() * 3.0F - 1.0F;
            float p0 = (random.nextFloat() * 2.0F - 1.0F) * 1000.0F;
            float p1 = (random.nextFloat() * 2.0F - 1.0F) * 1000.0F;
            singles.add(row(f(a), f(p0), f(p1), f(Mth.lerp(a, p0, p1))));
            double da = random.nextDouble() * 3.0 - 1.0;
            double d0 = (random.nextDouble() * 2.0 - 1.0) * 1000.0;
            double d1 = (random.nextDouble() * 2.0 - 1.0) * 1000.0;
            doubles.add(row(d(da), d(d0), d(d1), d(Mth.lerp(da, d0, d1))));
        }
        JsonObject lerp = new JsonObject();
        lerp.add("f32", singles);
        lerp.add("f64", doubles);
        return lerp;
    }

    private static JsonArray lerpInt(final Random random) {
        JsonArray rows = new JsonArray();
        int[][] edges = {{0, 10}, {10, 0}, {Integer.MIN_VALUE, Integer.MAX_VALUE}, {-5, 5}};
        float[] alphas = {0.0F, 0.09F, 0.1F, 0.5F, 0.99F, 1.0F, -0.1F, 1.5F, Float.NaN};
        for (int[] edge : edges) {
            for (float a : alphas) {
                rows.add(row(f(a), Integer.toString(edge[0]), Integer.toString(edge[1]), Integer.toString(Mth.lerpInt(a, edge[0], edge[1]))));
            }
        }
        for (int i = 0; i < 300; i++) {
            float a = random.nextFloat();
            int p0 = random.nextInt(2001) - 1000;
            int p1 = random.nextInt(2001) - 1000;
            rows.add(row(f(a), Integer.toString(p0), Integer.toString(p1), Integer.toString(Mth.lerpInt(a, p0, p1))));
        }
        return rows;
    }

    private static JsonObject map(final Random random) {
        JsonArray clampedSingle = new JsonArray();
        JsonArray clampedDouble = new JsonArray();
        JsonArray unclamped = new JsonArray();
        for (int i = 0; i < 500; i++) {
            float v = (random.nextFloat() * 2.0F - 1.0F) * 50.0F;
            float a = (random.nextFloat() * 2.0F - 1.0F) * 30.0F;
            float b = a + random.nextFloat() * 30.0F + 0.5F;
            float c = (random.nextFloat() * 2.0F - 1.0F) * 10.0F;
            float e = (random.nextFloat() * 2.0F - 1.0F) * 10.0F;
            clampedSingle.add(row(f(v), f(a), f(b), f(c), f(e), f(Mth.clampedMap(v, a, b, c, e))));
            double dv = (random.nextDouble() * 2.0 - 1.0) * 50.0;
            double da = (random.nextDouble() * 2.0 - 1.0) * 30.0;
            double db = da + random.nextDouble() * 30.0 + 0.5;
            double dc = (random.nextDouble() * 2.0 - 1.0) * 10.0;
            double de = (random.nextDouble() * 2.0 - 1.0) * 10.0;
            clampedDouble.add(row(d(dv), d(da), d(db), d(dc), d(de), d(Mth.clampedMap(dv, da, db, dc, de))));
            unclamped.add(row(d(dv), d(da), d(db), d(dc), d(de), d(Mth.map(dv, da, db, dc, de))));
        }
        clampedDouble.add(row(d(1.0), d(2.0), d(2.0), d(0.0), d(1.0), d(Mth.clampedMap(1.0, 2.0, 2.0, 0.0, 1.0))));
        clampedDouble.add(row(d(2.0), d(2.0), d(2.0), d(0.0), d(1.0), d(Mth.clampedMap(2.0, 2.0, 2.0, 0.0, 1.0))));
        JsonObject map = new JsonObject();
        map.add("clamped_f32", clampedSingle);
        map.add("clamped_f64", clampedDouble);
        map.add("unclamped_f64", unclamped);
        return map;
    }

    private static JsonArray floorDivMod(final Random random) {
        JsonArray rows = new JsonArray();
        int[][] edges = {{-1, 4}, {-1, -4}, {7, -4}, {-7, -4}, {-7, 4}, {0, -3}, {Integer.MIN_VALUE, -1}, {Integer.MIN_VALUE, 16}, {Integer.MAX_VALUE, -16}};
        for (int[] edge : edges) {
            rows.add(row(Integer.toString(edge[0]), Integer.toString(edge[1]),
                Integer.toString(Math.floorDiv(edge[0], edge[1])), Integer.toString(Math.floorMod(edge[0], edge[1]))));
        }
        for (int i = 0; i < 300; i++) {
            int a = random.nextInt();
            int b = random.nextInt(65) - 32;
            if (b == 0) {
                b = 1;
            }
            rows.add(row(Integer.toString(a), Integer.toString(b), Integer.toString(Math.floorDiv(a, b)), Integer.toString(Math.floorMod(a, b))));
        }
        return rows;
    }
}
