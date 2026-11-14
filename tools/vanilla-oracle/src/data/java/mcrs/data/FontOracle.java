package mcrs.data;

import com.google.common.collect.Lists;
import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.blaze3d.font.GlyphBitmap;
import com.mojang.blaze3d.font.GlyphInfo;
import com.mojang.blaze3d.font.GlyphProvider;
import com.mojang.blaze3d.font.SpaceProvider;
import com.mojang.blaze3d.font.UnbakedGlyph;
import com.mojang.blaze3d.platform.NativeImage;
import com.mojang.serialization.JsonOps;
import it.unimi.dsi.fastutil.ints.IntIterator;
import it.unimi.dsi.fastutil.ints.IntSet;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.HexFormat;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.concurrent.CompletableFuture;
import net.minecraft.SharedConstants;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GlyphSource;
import net.minecraft.client.gui.font.FontManager;
import net.minecraft.client.gui.font.FontOption;
import net.minecraft.client.gui.font.FontSet;
import net.minecraft.client.gui.font.GlyphStitcher;
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
import net.minecraft.client.gui.font.glyphs.BakedSheetGlyph;
import net.minecraft.client.gui.font.glyphs.EffectGlyph;
import net.minecraft.client.gui.font.glyphs.SpecialGlyphs;
import net.minecraft.client.gui.font.providers.UnihexProvider;
import net.minecraft.client.resources.IndexedAssetSource;
import net.minecraft.network.chat.FontDescription;
import net.minecraft.network.chat.FormattedText;
import net.minecraft.network.chat.Style;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.VanillaPackResources;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.server.packs.resources.ResourceManager;

/**
 * Dumps the glyphs of the fonts of the client: which provider draws a code point, its advance, its
 * bitmap and the offsets of its shadow and its bold strike, and the widths of sample strings.
 *
 * <p>The client jar holds an empty placeholder of the definition of the Unifont provider. The
 * real definition and the hex archives are objects of the asset index, which this oracle fetches
 * and checks against their SHA-1 into a cache directory. Nothing in the output depends on the cache.
 */
@SuppressWarnings({"unchecked", "rawtypes"})
public final class FontOracle {
    private static final String[] HEX_ARCHIVES = {"unifont", "unifont_jp", "unifont_pua"};
    /** The winners are listed for the basic plane only: the other planes hold only the holes of the Unifont. */
    private static final int LAST_WINNER = 0xFFFF;
    private static final Gson COMPACT = new GsonBuilder().disableHtmlEscaping().create();

    private FontOracle() {}

    /** The geometry of a stitched glyph, as the atlas of the font would keep it. */
    private record Stitched(int pixelWidth, int pixelHeight, float oversample, float left, float right, float top, float bottom) {}

    /** Takes the place of the glyph atlas: keeps the geometry of each glyph and draws nothing. */
    private static final class RecordingStitcher extends GlyphStitcher {
        final Map<BakedGlyph, Stitched> stitched = new IdentityHashMap<>();

        RecordingStitcher() {
            super(null, Identifier.withDefaultNamespace("oracle"));
        }

        @Override
        public BakedSheetGlyph stitch(final GlyphInfo info, final GlyphBitmap bitmap) {
            BakedSheetGlyph glyph = new BakedSheetGlyph(
                info, null, null, 0.0F, 0.0F, 0.0F, 0.0F, bitmap.getLeft(), bitmap.getRight(), bitmap.getTop(), bitmap.getBottom()
            );
            stitched.put(
                glyph,
                new Stitched(
                    bitmap.getPixelWidth(), bitmap.getPixelHeight(), bitmap.getOversample(), bitmap.getLeft(), bitmap.getRight(),
                    bitmap.getTop(), bitmap.getBottom()
                )
            );
            return glyph;
        }

        @Override
        public void reset() {}

        @Override
        public void close() {}
    }

    /** One font set with its recording stitcher. */
    private record Loaded(FontSet set, RecordingStitcher stitcher, List<GlyphProvider> providers) {}

    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Path out = Path.of(args[0]);
        Path cache = Path.of(args[1]);
        Path release = Path.of(args[2]);
        Files.createDirectories(out);
        String index = fetchAssets(cache, release);
        try (MultiPackResourceManager manager = resourceManager(cache.resolve("assets"), index)) {
            Map<Identifier, List<GlyphProvider.Conditional>> prepared = prepare(manager);
            Map<String, IntSet> archives = archives(manager);
            JsonObject root = new JsonObject();
            root.addProperty("game_version", SharedConstants.getCurrentVersion().id());
            root.addProperty("asset_index", index);
            root.add("special_glyphs", specialGlyphs());
            JsonArray configs = new JsonArray();
            configs.add(config(archives, prepared, "minecraft:default", EnumSet.noneOf(FontOption.class), true));
            configs.add(config(archives, prepared, "minecraft:default", EnumSet.of(FontOption.UNIFORM), false));
            configs.add(config(archives, prepared, "minecraft:default", EnumSet.of(FontOption.JAPANESE_VARIANTS), false));
            configs.add(config(archives, prepared, "minecraft:uniform", EnumSet.noneOf(FontOption.class), false));
            root.add("configs", configs);
            StringBuilder text = new StringBuilder();
            write(text, root, 0, 2);
            Files.writeString(out.resolve("font.json"), text.append('\n').toString(), StandardCharsets.UTF_8);
        }
    }

    // ----- assets -----

    private static String fetchAssets(final Path cache, final Path release) throws Exception {
        JsonObject pin = JsonParser.parseString(Files.readString(release)).getAsJsonObject();
        JsonObject json = pin.getAsJsonObject("json");
        try (HttpClient http = HttpClient.newBuilder().followRedirects(HttpClient.Redirect.NORMAL).build()) {
            return fetchAssets(http, cache, json);
        }
    }

    private static String fetchAssets(final HttpClient http, final Path cache, final JsonObject json) throws Exception {
        byte[] version = fetch(http, cache.resolve("version.json"), json.get("url").getAsString(), json.get("sha1").getAsString());
        JsonObject assetIndex = JsonParser.parseString(new String(version, StandardCharsets.UTF_8))
            .getAsJsonObject()
            .getAsJsonObject("assetIndex");
        String id = assetIndex.get("id").getAsString();
        byte[] indexBytes = fetch(
            http, cache.resolve("assets/indexes/" + id + ".json"), assetIndex.get("url").getAsString(), assetIndex.get("sha1").getAsString()
        );
        JsonObject objects = JsonParser.parseString(new String(indexBytes, StandardCharsets.UTF_8)).getAsJsonObject().getAsJsonObject("objects");
        for (Map.Entry<String, JsonElement> entry : objects.entrySet()) {
            if (entry.getKey().startsWith("minecraft/font/")) {
                String hash = entry.getValue().getAsJsonObject().get("hash").getAsString();
                fetch(http, cache.resolve("assets/objects/" + hash.substring(0, 2) + "/" + hash), "https://resources.download.minecraft.net/" + hash.substring(0, 2) + "/" + hash, hash);
            }
        }
        return id;
    }

    private static byte[] fetch(final HttpClient http, final Path file, final String url, final String sha1) throws Exception {
        if (Files.isRegularFile(file)) {
            byte[] cached = Files.readAllBytes(file);
            if (sha1(cached).equals(sha1)) {
                return cached;
            }
        }
        Files.createDirectories(file.getParent());
        HttpResponse<byte[]> response = http.send(HttpRequest.newBuilder(URI.create(url)).build(), HttpResponse.BodyHandlers.ofByteArray());
        if (response.statusCode() != 200 || !sha1(response.body()).equals(sha1)) {
            throw new IllegalStateException(url + ": status " + response.statusCode() + " or a wrong SHA-1");
        }
        Files.write(file, response.body());
        return response.body();
    }

    private static String sha1(final byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-1").digest(bytes));
    }

    /** The resources the client has: the layer of its jar, then the layer of the asset index above it. */
    private static MultiPackResourceManager resourceManager(final Path assets, final String index) throws Exception {
        Method create = Class.forName("net.minecraft.client.resources.ClientPackSource")
            .getDeclaredMethod("createVanillaPackSource", Path.class);
        create.setAccessible(true);
        VanillaPackResources vanilla = (VanillaPackResources)create.invoke(null, IndexedAssetSource.createIndexFs(assets, index));
        Field layers = VanillaPackResources.class.getDeclaredField("resourceLayers");
        layers.setAccessible(true);
        return new MultiPackResourceManager(PackType.CLIENT_RESOURCES, (List<PackResources>)layers.get(vanilla));
    }

    // ----- loading -----

    /** Runs the font loading of the client on the resources, without the window and its textures. */
    private static Map<Identifier, List<GlyphProvider.Conditional>> prepare(final ResourceManager manager) throws Exception {
        Field unsafeField = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
        unsafeField.setAccessible(true);
        Object unsafe = unsafeField.get(null);
        FontManager fonts = (FontManager)unsafe.getClass().getMethod("allocateInstance", Class.class).invoke(unsafe, FontManager.class);
        Method prepare = FontManager.class.getDeclaredMethod("prepare", ResourceManager.class, java.util.concurrent.Executor.class);
        prepare.setAccessible(true);
        Object preparation = ((CompletableFuture<?>)prepare.invoke(fonts, manager, (java.util.concurrent.Executor)Runnable::run)).join();
        Method sets = preparation.getClass().getDeclaredMethod("fontSets");
        sets.setAccessible(true);
        return (Map<Identifier, List<GlyphProvider.Conditional>>)sets.invoke(preparation);
    }

    private static Loaded load(final Map<Identifier, List<GlyphProvider.Conditional>> prepared, final String font, final Set<FontOption> options) throws Exception {
        RecordingStitcher stitcher = new RecordingStitcher();
        FontSet set = new FontSet(stitcher);
        set.reload(Lists.reverse(prepared.get(Identifier.parse(font))), options);
        Field active = FontSet.class.getDeclaredField("activeProviders");
        active.setAccessible(true);
        return new Loaded(set, stitcher, (List<GlyphProvider>)active.get(set));
    }

    // ----- the dump -----

    private static JsonObject config(
        final Map<String, IntSet> archives,
        final Map<Identifier, List<GlyphProvider.Conditional>> prepared,
        final String font,
        final Set<FontOption> options,
        final boolean full
    ) throws Exception {
        Loaded loaded = load(prepared, font, options);
        JsonObject config = new JsonObject();
        config.addProperty("font", font);
        JsonArray names = new JsonArray();
        options.forEach(option -> names.add(option.getSerializedName()));
        config.add("options", names);

        JsonArray providers = new JsonArray();
        for (int i = 0; i < loaded.providers.size(); i++) {
            providers.add(describe(archives, loaded.providers.get(i), i));
        }
        config.add("providers", providers);
        config.add("fallback_index", intNode(loaded.providers.size()));

        JsonArray glyphs = new JsonArray();
        for (int cp : probes(full)) {
            glyphs.add(glyph(loaded, cp));
        }
        config.add("glyphs", glyphs);
        config.add("ranges", ranges(loaded));
        if (full) {
            config.add("winners", winners(loaded));
            config.add("bitmap_advances", bitmapAdvances(loaded));
            config.add("unihex_widths", unihexWidths(loaded));
            config.add("strings", strings(loaded));
        }
        return config;
    }

    private static JsonElement intNode(final int value) {
        return COMPACT.toJsonTree(value);
    }

    /** The code points each Unifont archive supports, loaded once. */
    private static Map<String, IntSet> archives(final ResourceManager manager) throws Exception {
        Map<String, IntSet> result = new LinkedHashMap<>();
        for (String archive : HEX_ARCHIVES) {
            JsonObject definition = new JsonObject();
            definition.addProperty("hex_file", "minecraft:font/" + archive + ".zip");
            UnihexProvider.Definition parsed = UnihexProvider.Definition.CODEC.codec().parse(JsonOps.INSTANCE, definition).getOrThrow();
            result.put("minecraft:font/" + archive + ".zip", parsed.unpack().left().orElseThrow().load(manager).getSupportedGlyphs());
        }
        return result;
    }

    private static JsonObject describe(final Map<String, IntSet> archives, final GlyphProvider provider, final int index) throws Exception {
        JsonObject entry = new JsonObject();
        entry.addProperty("index", index);
        IntSet supported = provider.getSupportedGlyphs();
        if (provider instanceof SpaceProvider) {
            entry.addProperty("kind", "space");
        } else if (provider.getClass().getSimpleName().equals("BitmapProvider")) {
            entry.addProperty("kind", "bitmap");
            Field data = provider.getClass().getDeclaredField("imageData");
            data.setAccessible(true);
            Object holder = data.get(provider);
            Field identifier = holder.getClass().getDeclaredField("identifier");
            identifier.setAccessible(true);
            entry.addProperty("file", identifier.get(holder).toString());
        } else if (provider.getClass().getSimpleName().equals("UnihexProvider")) {
            entry.addProperty("kind", "unihex");
            List<String> matches = archives.entrySet().stream().filter(a -> a.getValue().equals(supported)).map(Map.Entry::getKey).toList();
            if (matches.size() != 1) {
                throw new IllegalStateException("unihex provider " + index + " matches the archives " + matches);
            }
            entry.addProperty("file", matches.get(0));
        } else {
            entry.addProperty("kind", "missing");
        }
        entry.addProperty("supported", supported.size());
        return entry;
    }

    private static JsonObject glyph(final Loaded loaded, final int cp) {
        JsonObject entry = new JsonObject();
        entry.addProperty("cp", String.format("U+%04X", cp));
        GlyphSource source = loaded.set.source(false);
        BakedGlyph baked = source.getGlyph(cp);
        entry.addProperty("provider", winner(loaded, cp));
        GlyphInfo info = baked.info();
        entry.addProperty("advance", info.getAdvance(false));
        entry.addProperty("advance_bold", info.getAdvance(true));
        entry.addProperty("bold_offset", info.getBoldOffset());
        entry.addProperty("shadow_offset", info.getShadowOffset());
        Stitched stitched = loaded.stitcher.stitched.get(baked);
        if (stitched != null) {
            JsonArray pixels = new JsonArray();
            pixels.add(stitched.pixelWidth);
            pixels.add(stitched.pixelHeight);
            entry.add("pixels", pixels);
            entry.addProperty("oversample", stitched.oversample);
            JsonArray box = new JsonArray();
            box.add(stitched.left);
            box.add(stitched.right);
            box.add(stitched.top);
            box.add(stitched.bottom);
            entry.add("box", box);
        }
        return entry;
    }

    /** For each provider, the range of the code points it supports and the extremes of its advances. */
    private static JsonArray ranges(final Loaded loaded) {
        JsonArray result = new JsonArray();
        for (int i = 0; i < loaded.providers.size(); i++) {
            GlyphProvider provider = loaded.providers.get(i);
            int min = Integer.MAX_VALUE;
            int max = -1;
            float low = Float.MAX_VALUE;
            float high = -Float.MAX_VALUE;
            IntIterator it = provider.getSupportedGlyphs().iterator();
            while (it.hasNext()) {
                int cp = it.nextInt();
                min = Math.min(min, cp);
                max = Math.max(max, cp);
                GlyphInfo info = provider.getGlyph(cp).info();
                low = Math.min(low, Math.min(info.getAdvance(false), info.getAdvance(true)));
                high = Math.max(high, Math.max(info.getAdvance(false), info.getAdvance(true)));
            }
            JsonObject entry = new JsonObject();
            entry.addProperty("provider", i);
            if (max < 0) {
                entry.addProperty("empty", true);
                result.add(entry);
                continue;
            }
            entry.addProperty("first", String.format("U+%04X", min));
            entry.addProperty("last", String.format("U+%04X", max));
            entry.addProperty("lowest_advance", low);
            entry.addProperty("highest_advance", high);
            result.add(entry);
        }
        return result;
    }

    /** The provider that draws each code point of the basic plane, as runs: `first-last:provider` in hexadecimal. */
    private static JsonArray winners(final Loaded loaded) {
        List<String> runs = new ArrayList<>();
        int start = 0;
        int current = winner(loaded, 0);
        for (int cp = 1; cp <= LAST_WINNER + 1; cp++) {
            int now = cp <= LAST_WINNER ? winner(loaded, cp) : -1;
            if (now != current) {
                runs.add(String.format("%04X-%04X:%d", start, cp - 1, current));
                start = cp;
                current = now;
            }
        }
        return chunks(runs, 6);
    }

    private static int winner(final Loaded loaded, final int cp) {
        for (int i = 0; i < loaded.providers.size(); i++) {
            if (loaded.providers.get(i).getGlyph(cp) != null) {
                return i;
            }
        }
        return loaded.providers.size();
    }

    /** The advance of every glyph of every bitmap provider, in the order of the code points. */
    private static JsonObject bitmapAdvances(final Loaded loaded) {
        JsonObject result = new JsonObject();
        for (int i = 0; i < loaded.providers.size(); i++) {
            GlyphProvider provider = loaded.providers.get(i);
            if (!provider.getClass().getSimpleName().equals("BitmapProvider")) {
                continue;
            }
            TreeMap<Integer, Float> advances = new TreeMap<>();
            IntIterator it = provider.getSupportedGlyphs().iterator();
            while (it.hasNext()) {
                int cp = it.nextInt();
                advances.put(cp, provider.getGlyph(cp).info().getAdvance(false));
            }
            List<String> items = new ArrayList<>();
            advances.forEach((cp, advance) -> items.add(String.format("%04X:%d", cp, (int)(float)advance)));
            result.add(String.valueOf(i), chunks(items, 12));
        }
        return result;
    }

    private static JsonArray chunks(final List<String> items, final int size) {
        JsonArray result = new JsonArray();
        for (int from = 0; from < items.size(); from += size) {
            result.add(String.join(" ", items.subList(from, Math.min(items.size(), from + size))));
        }
        return result;
    }

    /** For each width of a Unifont glyph in pixels, how many glyphs have it and the first of them. */
    private static JsonArray unihexWidths(final Loaded loaded) {
        JsonArray result = new JsonArray();
        for (int i = 0; i < loaded.providers.size(); i++) {
            GlyphProvider provider = loaded.providers.get(i);
            if (!provider.getClass().getSimpleName().equals("UnihexProvider")) {
                continue;
            }
            TreeMap<Integer, int[]> widths = new TreeMap<>();
            int[] sorted = provider.getSupportedGlyphs().toIntArray();
            Arrays.sort(sorted);
            for (int cp : sorted) {
                int[] width = new int[1];
                provider.getGlyph(cp).bake(new UnbakedGlyph.Stitcher() {
                    @Override
                    public BakedGlyph stitch(final GlyphInfo info, final GlyphBitmap bitmap) {
                        width[0] = bitmap.getPixelWidth();
                        return null;
                    }

                    @Override
                    public BakedGlyph getMissing() {
                        throw new IllegalStateException();
                    }
                });
                int[] slot = widths.computeIfAbsent(width[0], w -> new int[] {cp, 0});
                slot[1]++;
            }
            JsonArray rows = new JsonArray();
            widths.forEach((width, slot) -> {
                JsonObject row = new JsonObject();
                row.addProperty("pixel_width", width);
                row.addProperty("count", slot[1]);
                row.addProperty("first", String.format("U+%04X", slot[0]));
                rows.add(row);
            });
            JsonObject entry = new JsonObject();
            entry.addProperty("provider", i);
            entry.add("widths", rows);
            result.add(entry);
        }
        return result;
    }

    private static JsonArray strings(final Loaded loaded) {
        Font font = new Font(new Font.Provider() {
            public GlyphSource glyphs(final FontDescription description) {
                return loaded.set.source(false);
            }

            public EffectGlyph effect() {
                return loaded.set.whiteGlyph();
            }
        });
        Style bold = Style.EMPTY.withBold(true);
        String[] plain = {
            "", " ", "Hello, World!", "The quick brown fox jumps over the lazy dog", "iiii", "WWWW", "@@@@", "0123456789",
            "ÀÉÎÕÜçñß", "Привет, мир!",
            "Ёё Йй Її Єє Іі Ґґ", "ΑΒΓ αβγ",
            "日本語", "가한", "あア", "ＡＢ", "a‌b", "a　b", "☃❤", "😀",
            "a\uD800b", "", "§lBold", "§lb§rc", "a\nb", "a\tb"
        };
        JsonArray result = new JsonArray();
        for (String text : plain) {
            result.add(measured(font, "plain", text, FormattedText.of(text), text));
        }
        String[] boldSamples = {"Hello", "Привет", "日", "日本", "日本語", "Éé", "a日", "☃"};
        for (String text : boldSamples) {
            result.add(measured(font, "bold", text, FormattedText.of(text, bold), null));
        }
        result.add(mixed(font, bold, new String[] {"ab", "日", "本"}, new boolean[] {false, true, false}));
        result.add(mixed(font, bold, new String[] {"日", "日", "日"}, new boolean[] {true, true, true}));
        return result;
    }

    /** A text of consecutive runs: `text` is the measured text, `runs` the code points and the style of each run. */
    private static JsonObject mixed(final Font font, final Style bold, final String[] texts, final boolean[] bolds) {
        FormattedText[] parts = new FormattedText[texts.length];
        JsonArray runs = new JsonArray();
        for (int i = 0; i < texts.length; i++) {
            parts[i] = bolds[i] ? FormattedText.of(texts[i], bold) : FormattedText.of(texts[i]);
            JsonObject run = new JsonObject();
            run.addProperty("text", codes(texts[i]));
            run.addProperty("bold", bolds[i]);
            runs.add(run);
        }
        JsonObject entry = measured(font, "mixed", String.join("", texts), FormattedText.composite(parts), null);
        entry.add("runs", runs);
        return entry;
    }

    private static JsonObject measured(final Font font, final String style, final String text, final FormattedText formatted, final String plain) {
        JsonObject entry = new JsonObject();
        entry.addProperty("style", style);
        entry.addProperty("text", codes(text));
        entry.addProperty("sum", font.getSplitter().stringWidth(formatted));
        entry.addProperty("width", font.width(formatted));
        if (plain != null) {
            entry.addProperty("string_width", font.width(plain));
        }
        return entry;
    }

    /** The code points of a text as hexadecimal numbers, so that a lone surrogate survives the file. */
    private static String codes(final String text) {
        StringBuilder out = new StringBuilder();
        text.codePoints().forEach(cp -> out.append(out.length() == 0 ? "" : " ").append(String.format("%04X", cp)));
        return out.toString();
    }

    private static JsonObject specialGlyphs() throws Exception {
        JsonObject result = new JsonObject();
        for (SpecialGlyphs special : new SpecialGlyphs[] {SpecialGlyphs.MISSING}) {
            Field image = SpecialGlyphs.class.getDeclaredField("image");
            image.setAccessible(true);
            NativeImage pixels = (NativeImage)image.get(special);
            JsonArray rows = new JsonArray();
            for (int y = 0; y < pixels.getHeight(); y++) {
                StringBuilder row = new StringBuilder();
                for (int x = 0; x < pixels.getWidth(); x++) {
                    row.append(pixels.getPixel(x, y) == 0 ? '.' : '#');
                }
                rows.add(row.toString());
            }
            JsonObject entry = new JsonObject();
            entry.addProperty("advance", special.getAdvance());
            entry.addProperty("bold_offset", special.getBoldOffset());
            entry.addProperty("shadow_offset", special.getShadowOffset());
            entry.add("rows", rows);
            result.add(special.name().toLowerCase(java.util.Locale.ROOT), entry);
        }
        return result;
    }

    // ----- probes -----

    private static List<Integer> probes(final boolean full) {
        List<Integer> list = new ArrayList<>();
        if (full) {
            range(list, 0x00, 0xFF);
            range(list, 0x400, 0x45F);
            add(list, 0x100, 0x101, 0x110, 0x141, 0x142, 0x152, 0x153, 0x160, 0x178, 0x17D, 0x17E, 0x1A0, 0x1B0, 0x2BB, 0x300, 0x301);
            add(list, 0x386, 0x391, 0x392, 0x3A3, 0x3A9, 0x3B1, 0x3B2, 0x3C2, 0x3C9, 0x490, 0x491, 0x4AE, 0x4AF, 0x4E8, 0x4E9);
            add(list, 0x531, 0x5D0, 0x627, 0x644, 0x905, 0xE01, 0x10D0, 0x1100, 0x1E9E);
            range(list, 0x2000, 0x200F);
            add(list, 0x2010, 0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2026, 0x2028, 0x202F, 0x2030, 0x205F, 0x2060);
            add(list, 0x20AC, 0x20BD, 0x20B4, 0x2122, 0x2190, 0x2192, 0x2260, 0x221E, 0x2264, 0x2500, 0x2588, 0x2600, 0x2665, 0x2764, 0x2B50);
            add(list, 0x26A1, 0x2714, 0x274C, 0x3000, 0x3001, 0x3042, 0x30A2, 0x3105, 0x3200, 0x4E00, 0x4E2D, 0x65E5, 0x672C, 0x8A9E, 0x9FFF);
            add(list, 0xA960, 0xAC00, 0xD7A3, 0xD7B0, 0xD800, 0xDFFF, 0xE000, 0xF8FF, 0xF900, 0xFEFF, 0xFF01, 0xFF21, 0xFF5E, 0xFF61);
            add(list, 0xFFFD, 0xFFFE, 0xFFFF, 0x378, 0x5FF, 0x10000, 0x1D400, 0x1F1E6, 0x1F4A9, 0x1F600, 0x20000, 0xF0000, 0x10FFFF);
        } else {
            add(list, 0x20, 0x41, 0x61, 0x7E, 0xC9, 0xE9, 0x401, 0x419, 0x451, 0x3B1, 0x20AC, 0x200C, 0x3000, 0x3042, 0x4E00, 0x65E5);
            add(list, 0xAC00, 0xD7B0, 0xE000, 0xF900, 0xFF21, 0x1F600, 0x2603);
        }
        return list;
    }

    private static void range(final List<Integer> list, final int from, final int to) {
        for (int cp = from; cp <= to; cp++) {
            list.add(cp);
        }
    }

    private static void add(final List<Integer> list, final int... code) {
        for (int cp : code) {
            list.add(cp);
        }
    }

    // ----- text -----

    /** Writes a tree with an element of an array of objects on each line. */
    private static void write(final StringBuilder out, final JsonElement node, final int depth, final int step) {
        String indent = " ".repeat(depth * step);
        if (node.isJsonObject() && depth < 4) {
            out.append("{\n");
            boolean first = true;
            for (Map.Entry<String, JsonElement> member : node.getAsJsonObject().entrySet()) {
                if (!first) {
                    out.append(",\n");
                }
                first = false;
                out.append(indent).append(" ".repeat(step)).append(COMPACT.toJson(member.getKey())).append(": ");
                write(out, member.getValue(), depth + 1, step);
            }
            out.append("\n").append(indent).append("}");
        } else if (node.isJsonArray() && node.getAsJsonArray().size() > 0 && node.getAsJsonArray().get(0).isJsonObject() && depth < 4) {
            out.append("[\n");
            JsonArray array = node.getAsJsonArray();
            for (int i = 0; i < array.size(); i++) {
                out.append(indent).append(" ".repeat(step));
                if (depth < 2) {
                    write(out, array.get(i), depth + 1, step);
                } else {
                    out.append(COMPACT.toJson(array.get(i)));
                }
                out.append(i + 1 < array.size() ? ",\n" : "\n");
            }
            out.append(indent).append("]");
        } else if (node.isJsonArray() && node.getAsJsonArray().size() > 8 && node.getAsJsonArray().get(0).isJsonPrimitive()
            && node.getAsJsonArray().get(0).getAsJsonPrimitive().isString()) {
            out.append("[\n");
            JsonArray array = node.getAsJsonArray();
            for (int i = 0; i < array.size(); i++) {
                out.append(indent).append(" ".repeat(step)).append(COMPACT.toJson(array.get(i)));
                out.append(i + 1 < array.size() ? ",\n" : "\n");
            }
            out.append(indent).append("]");
        } else {
            out.append(COMPACT.toJson(node));
        }
    }
}
