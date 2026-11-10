package mcrs.data;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.blaze3d.vertex.PoseStack;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.Font;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.blockentity.AbstractSignRenderer;
import net.minecraft.client.renderer.blockentity.BlockEntityRendererProvider;
import net.minecraft.client.renderer.blockentity.HangingSignRenderer;
import net.minecraft.client.renderer.blockentity.StandingSignRenderer;
import net.minecraft.client.renderer.blockentity.state.HangingSignRenderState;
import net.minecraft.client.renderer.blockentity.state.SignRenderState;
import net.minecraft.client.renderer.blockentity.state.StandingSignRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.locale.Language;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.network.chat.FormattedText;
import net.minecraft.network.chat.Style;
import net.minecraft.network.chat.TextColor;
import net.minecraft.resources.Identifier;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.util.LightCoordsUtil;
import net.minecraft.world.item.DyeColor;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.entity.HangingSignBlockEntity;
import net.minecraft.world.level.block.entity.SignBlockEntity;
import net.minecraft.world.level.block.entity.SignText;
import org.joml.Matrix4fc;

/**
 * Runs the sign text renderers of the client against a recording collector and writes what they submit.
 *
 * <p>The font is replaced by a stub that measures every code point as {@link #STUB_ADVANCE} and does not wrap, so the file
 * records the placement, the colours and the light of the renderers, not the metrics of the font.
 */
final class SignTextOracle {
    private static final int STUB_ADVANCE = 3;
    private static final int LIGHT = LightCoordsUtil.pack(5, 3);
    private static final SignBlockEntity SIGN = new SignBlockEntity(BlockPos.ZERO, Blocks.OAK_SIGN.defaultBlockState());
    private static final HangingSignBlockEntity HANGING =
        new HangingSignBlockEntity(BlockPos.ZERO, Blocks.OAK_HANGING_SIGN.defaultBlockState());
    private static final Direction[] HORIZONTAL = {Direction.NORTH, Direction.EAST, Direction.SOUTH, Direction.WEST};

    private SignTextOracle() {}

    /** One text line as the collector received it. */
    private record Submitted(float x, float y, FormattedCharSequence text, boolean shadow, String mode, int light, int color, int background, int outline, Matrix4fc pose) {}

    private static final class Rig {
        final StandingSignRenderer standing;
        final HangingSignRenderer hanging;
        final List<Submitted> seen = new ArrayList<>();
        final SubmitNodeCollector collector;

        Rig() {
            Font font = new Font(null) {
                @Override
                public int width(final FormattedCharSequence text) {
                    int[] count = {0};
                    text.accept((index, style, point) -> {
                        count[0]++;
                        return true;
                    });
                    return count[0] * STUB_ADVANCE;
                }

                @Override
                public List<FormattedCharSequence> split(final FormattedText text, final int width) {
                    return List.of(Language.getInstance().getVisualOrder(text));
                }
            };
            BlockEntityRendererProvider.Context context =
                new BlockEntityRendererProvider.Context(null, null, null, null, null, font, null, null);
            standing = new StandingSignRenderer(context);
            hanging = new HangingSignRenderer(context);
            collector = (SubmitNodeCollector) Proxy.newProxyInstance(
                SubmitNodeCollector.class.getClassLoader(),
                new Class<?>[] {SubmitNodeCollector.class},
                (proxy, method, args) -> {
                    if (method.getName().equals("submitText")) {
                        PoseStack pose = (PoseStack) args[0];
                        seen.add(new Submitted(
                            (float) args[1], (float) args[2], (FormattedCharSequence) args[3], (boolean) args[4],
                            args[5].toString(), (int) args[6], (int) args[7], (int) args[8], (int) args[9],
                            new org.joml.Matrix4f(pose.last().pose())
                        ));
                    }
                    return switch (method.getName()) {
                        case "hashCode" -> System.identityHashCode(proxy);
                        case "equals" -> proxy == args[0];
                        case "toString" -> "recording collector";
                        default -> defaultFor(method.getReturnType(), proxy);
                    };
                }
            );
        }

        List<Submitted> run(final boolean hangingFamily, final SignRenderState state) {
            seen.clear();
            PoseStack pose = new PoseStack();
            CameraRenderState camera = new CameraRenderState();
            if (hangingFamily) {
                hanging.submit((HangingSignRenderState) state, pose, collector, camera);
            } else {
                standing.submit((StandingSignRenderState) state, pose, collector, camera);
            }
            return new ArrayList<>(seen);
        }
    }

    private static Object defaultFor(final Class<?> type, final Object proxy) {
        if (type == boolean.class) {
            return false;
        }
        if (type == char.class) {
            return (char) 0;
        }
        if (type == byte.class) {
            return (byte) 0;
        }
        if (type == short.class) {
            return (short) 0;
        }
        if (type == int.class) {
            return 0;
        }
        if (type == long.class) {
            return 0L;
        }
        if (type == float.class) {
            return 0.0F;
        }
        if (type == double.class) {
            return 0.0D;
        }
        return type.isInstance(proxy) ? proxy : null;
    }

    static List<JsonObject> rows() throws ReflectiveOperationException {
        Rig rig = new Rig();
        List<JsonObject> rows = new ArrayList<>();
        rows.add(constants());
        for (DyeColor dye : DyeColor.values()) {
            JsonObject row = new JsonObject();
            row.addProperty("kind", "dye_colors");
            row.addProperty("dye", dye.getSerializedName());
            row.addProperty("id", dye.getId());
            row.addProperty("bright", AbstractSignRenderer.BRIGHT_TEXT_COLORS.pick(dye));
            row.addProperty("dark", AbstractSignRenderer.DARK_TEXT_COLORS.pick(dye));
            rows.add(row);
        }
        transforms(rig, rows);
        layout(rig, rows);
        paint(rig, rows);
        filtering(rig, rows);
        styles(rig, rows);
        return rows;
    }

    private static JsonObject constants() throws ReflectiveOperationException {
        JsonObject row = new JsonObject();
        row.addProperty("kind", "constants");
        row.addProperty("sign_line_height", SIGN.getTextLineHeight());
        row.addProperty("sign_max_line_width", SIGN.getMaxTextLineWidth());
        row.addProperty("hanging_line_height", HANGING.getTextLineHeight());
        row.addProperty("hanging_max_line_width", HANGING.getMaxTextLineWidth());
        row.addProperty("lines", SignText.LINES);
        row.addProperty("full_bright_light", LightCoordsUtil.FULL_BRIGHT);
        row.addProperty("black_glow_outline_color", privateInt("BLACK_TEXT_OUTLINE_COLOR"));
        row.addProperty("outline_distance_squared", privateInt("OUTLINE_RENDER_DISTANCE"));
        row.addProperty("stub_advance", STUB_ADVANCE);
        return row;
    }

    private static int privateInt(final String name) throws ReflectiveOperationException {
        Field field = AbstractSignRenderer.class.getDeclaredField(name);
        field.setAccessible(true);
        return field.getInt(null);
    }

    private static SignText text(final DyeColor dye, final boolean glowing, final String... lines) {
        List<Component> messages = new ArrayList<>();
        for (String line : lines) {
            messages.add(Component.literal(line));
        }
        return new SignText(messages, messages, dye, glowing);
    }

    private static SignRenderState state(final boolean hangingFamily, final SignText front, final SignText back) {
        SignRenderState state = hangingFamily ? new HangingSignRenderState() : new StandingSignRenderState();
        state.frontText = front;
        state.backText = back;
        state.textLineHeight = lineHeight(hangingFamily);
        state.maxTextLineWidth = lineWidth(hangingFamily);
        state.lightCoords = LIGHT;
        return state;
    }

    private static int lineHeight(final boolean hangingFamily) {
        return (hangingFamily ? HANGING : SIGN).getTextLineHeight();
    }

    private static int lineWidth(final boolean hangingFamily) {
        return (hangingFamily ? HANGING : SIGN).getMaxTextLineWidth();
    }

    // Each side with one line: the pose of the collector is the map from block space to text space.
    private static void transforms(final Rig rig, final List<JsonObject> rows) {
        for (boolean hangingFamily : new boolean[] {false, true}) {
            var table = hangingFamily ? HangingSignRenderer.TRANSFORMATIONS : StandingSignRenderer.TRANSFORMATIONS;
            for (int index = 0; index < 20; index++) {
                SignRenderState state = state(
                    hangingFamily, text(DyeColor.BLACK, false, "x", "", "", ""), text(DyeColor.BLACK, false, "y", "", "", "")
                );
                String attachment;
                String position;
                if (index < 16) {
                    state.transformations = table.freeTransformations(index);
                    attachment = hangingFamily ? "ceiling" : "ground";
                    position = Integer.toString(index);
                } else {
                    Direction facing = HORIZONTAL[index - 16];
                    state.transformations = table.wallTransformation(facing);
                    attachment = "wall";
                    position = facing.getSerializedName();
                }
                List<Submitted> seen = rig.run(hangingFamily, state);
                String[] sides = {"front", "back"};
                for (int side = 0; side < 2; side++) {
                    Submitted line = firstOfText(seen, side == 0 ? "x" : "y");
                    JsonObject row = new JsonObject();
                    row.addProperty("kind", "transform");
                    row.addProperty("family", hangingFamily ? "hanging" : "standing");
                    row.addProperty("attachment", attachment);
                    row.addProperty("state", position);
                    row.addProperty("side", sides[side]);
                    row.add("matrix", matrix(line.pose()));
                    rows.add(row);
                }
            }
        }
    }

    private static Submitted firstOfText(final List<Submitted> seen, final String text) {
        for (Submitted line : seen) {
            if (plain(line.text()).equals(text)) {
                return line;
            }
        }
        throw new IllegalStateException("no line " + text);
    }

    // Four lines of several widths: where the collector puts each line, with an identity transform.
    private static void layout(final Rig rig, final List<JsonObject> rows) {
        for (boolean hangingFamily : new boolean[] {false, true}) {
            SignRenderState state = state(
                hangingFamily, text(DyeColor.BLACK, false, "", "a", "ab", "abc"), null
            );
            addLines(rows, hangingFamily, rig.run(hangingFamily, state));
            SignRenderState wide = state(
                hangingFamily, text(DyeColor.BLACK, false, "abcd", "abcde", "abcdef", "abcdefg"), null
            );
            addLines(rows, hangingFamily, rig.run(hangingFamily, wide));
        }
    }

    private static void addLines(final List<JsonObject> rows, final boolean hangingFamily, final List<Submitted> lines) {
        JsonObject row = new JsonObject();
        row.addProperty("kind", "layout");
        row.addProperty("family", hangingFamily ? "hanging" : "standing");
        JsonArray array = new JsonArray();
        for (Submitted line : lines) {
            String text = plain(line.text());
            JsonObject json = new JsonObject();
            json.addProperty("text", text);
            json.addProperty("advance_total", text.codePointCount(0, text.length()) * STUB_ADVANCE);
            json.addProperty("x", line.x());
            json.addProperty("y", line.y());
            array.add(json);
        }
        row.add("lines", array);
        rows.add(row);
    }

    // Colour, outline and light of every dye in both families, with and without glow, with and without the outline distance.
    private static void paint(final Rig rig, final List<JsonObject> rows) {
        for (boolean hangingFamily : new boolean[] {false, true}) {
        for (DyeColor dye : DyeColor.values()) {
            for (boolean glowing : new boolean[] {false, true}) {
                for (boolean outline : new boolean[] {false, true}) {
                    SignRenderState state = state(hangingFamily, text(dye, glowing, "p", "", "", ""), null);
                    state.drawOutline = outline;
                    Submitted line = firstOfText(rig.run(hangingFamily, state), "p");
                    JsonObject row = new JsonObject();
                    row.addProperty("kind", "paint");
                    row.addProperty("family", hangingFamily ? "hanging" : "standing");
                    row.addProperty("dye", dye.getSerializedName());
                    row.addProperty("glowing", glowing);
                    row.addProperty("outline_in_range", outline);
                    row.addProperty("color", line.color());
                    row.addProperty("outline_color", line.outline());
                    row.addProperty("light", line.light());
                    row.addProperty("input_light", LIGHT);
                    row.addProperty("shadow", line.shadow());
                    row.addProperty("background", line.background());
                    row.addProperty("mode", line.mode());
                    rows.add(row);
                }
            }
        }
        }
    }

    // The unfiltered and the filtered messages differ; the flag of the state selects one list.
    private static void filtering(final Rig rig, final List<JsonObject> rows) {
        for (boolean filtering : new boolean[] {false, true}) {
            List<Component> raw = List.of(Component.literal("raw1"), Component.literal("raw2"), Component.literal("raw3"), Component.literal("raw4"));
            List<Component> clean = List.of(Component.literal("cln1"), Component.literal("cln2"), Component.literal("cln3"), Component.literal("cln4"));
            SignRenderState state = state(false, new SignText(raw, clean, DyeColor.BLACK, false), null);
            state.isTextFilteringEnabled = filtering;
            JsonObject row = new JsonObject();
            row.addProperty("kind", "filtering");
            row.addProperty("filtering_enabled", filtering);
            JsonArray texts = new JsonArray();
            rig.run(false, state).forEach(line -> texts.add(plain(line.text())));
            row.add("lines", texts);
            rows.add(row);
        }
    }

    // Formatting of the components reaches the collector as the style of each code point.
    private static void styles(final Rig rig, final List<JsonObject> rows) {
        styleRow(rig, rows, "styles", List.of(
            Component.literal("red").withColor(0xFF0000),
            Component.literal("b").append(Component.literal("i").withStyle(style -> style.withBold(true).withItalic(true))),
            Component.literal("u").withStyle(style -> style.withUnderlined(true).withStrikethrough(true).withObfuscated(true)),
            Component.translatable("sign.test.missing.key")
        ));
        // Inheritance from a parent, a font override, and a translation key with a fallback.
        styleRow(rig, rows, "styles_inherited", List.of(
            Component.literal("p").withColor(0xFF0000).append(Component.literal("c")),
            Component.literal("p").withStyle(style -> style.withBold(true)).append(Component.literal("c").withStyle(style -> style.withItalic(true))),
            Component.literal("f").withStyle(style -> style.withFont(new FontDescription.Resource(Identifier.withDefaultNamespace("alt"))))
                .append(Component.literal("g")),
            Component.translatableWithFallback("sign.test.missing.key", "fallback text")
        ));
    }

    private static void styleRow(final Rig rig, final List<JsonObject> rows, final String kind, final List<Component> messages) {
        SignRenderState state = state(false, new SignText(messages, messages, DyeColor.BLACK, false), null);
        JsonObject row = new JsonObject();
        row.addProperty("kind", kind);
        JsonArray lines = new JsonArray();
        for (Submitted line : rig.run(false, state)) {
            JsonArray runs = new JsonArray();
            JsonObject[] current = {null};
            Style[] last = {null};
            line.text().accept((index, style, point) -> {
                if (current[0] == null || !style.equals(last[0])) {
                    current[0] = new JsonObject();
                    current[0].addProperty("text", "");
                    TextColor color = style.getColor();
                    if (color != null) {
                        current[0].addProperty("color", color.getValue());
                    }
                    current[0].addProperty("bold", style.isBold());
                    current[0].addProperty("italic", style.isItalic());
                    current[0].addProperty("underlined", style.isUnderlined());
                    current[0].addProperty("strikethrough", style.isStrikethrough());
                    current[0].addProperty("obfuscated", style.isObfuscated());
                    if (style.getFont() != null) {
                        current[0].addProperty("font", style.getFont().toString());
                    }
                    runs.add(current[0]);
                    last[0] = style;
                }
                current[0].addProperty("text", current[0].get("text").getAsString() + new String(Character.toChars(point)));
                return true;
            });
            lines.add(runs);
        }
        row.add("lines", lines);
        rows.add(row);
    }

    private static String plain(final FormattedCharSequence text) {
        StringBuilder builder = new StringBuilder();
        text.accept((index, style, point) -> {
            builder.appendCodePoint(point);
            return true;
        });
        return builder.toString();
    }

    private static JsonArray matrix(final Matrix4fc matrix) {
        JsonArray rows = new JsonArray();
        for (int row = 0; row < 4; row++) {
            JsonArray values = new JsonArray();
            for (int column = 0; column < 4; column++) {
                values.add(matrix.get(column, row));
            }
            rows.add(values);
        }
        return rows;
    }
}
