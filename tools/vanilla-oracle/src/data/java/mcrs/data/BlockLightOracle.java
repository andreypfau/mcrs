package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.blaze3d.vertex.QuadInstance;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;
import net.minecraft.SharedConstants;
import net.minecraft.client.renderer.FaceInfo;
import net.minecraft.client.renderer.block.BlockAndTintGetter;
import net.minecraft.client.renderer.block.BlockModelLighter;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.CardinalLighting;
import net.minecraft.world.level.ColorResolver;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.material.FluidState;
import org.joml.Vector3f;

/**
 * Golden values for the smooth and the flat lighting of block faces, from synthetic
 * neighbourhoods, and the list of block states whose occlusion shade does not follow the
 * collision rule.
 */
public final class BlockLightOracle {
    private static final Gson LINE = new GsonBuilder().disableHtmlEscaping().create();
    private static final int FORMAT = 1;

    private static final String[] PALETTE = {
        "minecraft:air",
        "minecraft:stone",
        "minecraft:glass",
        "minecraft:oak_slab[type=bottom]",
        "minecraft:oak_slab[type=top]",
        "minecraft:mud",
        "minecraft:snow[layers=8]",
        "minecraft:glowstone",
        "minecraft:magma_block",
        "minecraft:oak_leaves",
        "minecraft:barrier",
        "minecraft:soul_sand",
        "minecraft:oak_fence",
    };

    private BlockLightOracle() {
    }

    public static void main(final String[] args) {
        int status;
        try {
            SharedConstants.tryDetectVersion();
            Bootstrap.bootStrap();
            Path out = Path.of(args[0]);
            Files.createDirectories(out);
            switch (args[1]) {
                case "smooth_light" -> smoothLight(out.resolve("block_smooth_light.json"));
                case "occlusion_shade" -> occlusionShade(out.resolve("block_occlusion_shade.json"));
                default -> throw new IllegalArgumentException("unknown mode " + args[1]);
            }
            status = 0;
        } catch (Throwable failure) {
            failure.printStackTrace();
            status = 1;
        }
        System.exit(status);
    }

    private static BlockState parse(final String text) throws Exception {
        return BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, text, false).blockState();
    }

    private static String describe(final BlockState state) {
        StringBuilder text = new StringBuilder(BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString());
        if (!state.getProperties().isEmpty()) {
            text.append('[');
            boolean first = true;
            for (Property<?> property : state.getProperties()) {
                if (!first) {
                    text.append(',');
                }
                first = false;
                text.append(property.getName()).append('=').append(valueName(state, property));
            }
            text.append(']');
        }
        return text.toString();
    }

    private static <T extends Comparable<T>> String valueName(final BlockState state, final Property<T> property) {
        return property.getName(state.getValue(property));
    }

    // ---- occlusion shade ----------------------------------------------------------------------

    private static void occlusionShade(final Path path) throws Exception {
        List<String> rows = new ArrayList<>();
        for (Block block : BuiltInRegistries.BLOCK) {
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                float shade = state.getShadeBrightness(EmptyBlockGetter.INSTANCE, BlockPos.ZERO);
                boolean full = state.isCollisionShapeFullBlock(EmptyBlockGetter.INSTANCE, BlockPos.ZERO);
                float byRule = full ? 0.2F : 1.0F;
                if (shade != byRule) {
                    JsonObject row = new JsonObject();
                    row.addProperty("state", describe(state));
                    row.addProperty("full_collision", full);
                    row.addProperty("occlusion_shade", shade);
                    rows.add(LINE.toJson(row));
                }
            }
        }
        StringBuilder text = new StringBuilder("{\n");
        text.append("  \"format\": ").append(FORMAT).append(",\n");
        text.append("  \"kind\": \"occlusion_shade\",\n");
        text.append("  \"game_version\": ").append(LINE.toJson(SharedConstants.getCurrentVersion().id())).append(",\n");
        text.append("  \"rule\": \"a state not listed has the occlusion shade 0.2 when its collision shape is a full block, else 1.0\",\n");
        text.append("  \"exceptions\": [\n");
        for (int index = 0; index < rows.size(); index++) {
            text.append("    ").append(rows.get(index)).append(index + 1 < rows.size() ? ",\n" : "\n");
        }
        text.append("  ]\n}\n");
        Files.writeString(path, text.toString(), StandardCharsets.UTF_8);
        System.out.println("wrote " + path + " (" + rows.size() + " exceptions)");
    }

    // ---- smooth light -------------------------------------------------------------------------

    private record Shape(String name, String own, float[] from, float[] to) {
    }

    private static final Shape[] SHAPES = {
        new Shape("cube", "minecraft:stone", new float[] {0, 0, 0}, new float[] {1, 1, 1}),
        new Shape("glass_cube", "minecraft:glass", new float[] {0, 0, 0}, new float[] {1, 1, 1}),
        new Shape("bottom_slab", "minecraft:oak_slab[type=bottom]", new float[] {0, 0, 0}, new float[] {1, 0.5F, 1}),
        new Shape("inset_in_full_block", "minecraft:stone", new float[] {0.25F, 0.25F, 0.25F}, new float[] {0.75F, 0.75F, 0.75F}),
        new Shape("post", "minecraft:oak_fence", new float[] {0.375F, 0, 0.375F}, new float[] {0.625F, 1, 0.625F}),
        new Shape("thin_layer", "minecraft:oak_fence", new float[] {0, 0, 0}, new float[] {1, 0.125F, 1}),
    };

    private static final class World implements BlockAndTintGetter {
        final Map<BlockPos, BlockState> states = new HashMap<>();
        final Map<BlockPos, int[]> light = new HashMap<>();
        final int[] fallback;

        World(final int[] fallback) {
            this.fallback = fallback;
        }

        @Override
        public CardinalLighting cardinalLighting() {
            return CardinalLighting.DEFAULT;
        }

        @Override
        public int getBlockTint(final BlockPos pos, final ColorResolver color) {
            return -1;
        }

        @Override
        public LevelLightEngine getLightEngine() {
            return LevelLightEngine.EMPTY;
        }

        @Override
        public int getBrightness(final LightLayer layer, final BlockPos pos) {
            int[] value = this.light.getOrDefault(pos, this.fallback);
            return layer == LightLayer.BLOCK ? value[0] : value[1];
        }

        @Override
        public BlockEntity getBlockEntity(final BlockPos pos) {
            return null;
        }

        @Override
        public BlockState getBlockState(final BlockPos pos) {
            return this.states.getOrDefault(pos, Blocks.AIR.defaultBlockState());
        }

        @Override
        public FluidState getFluidState(final BlockPos pos) {
            return this.getBlockState(pos).getFluidState();
        }

        @Override
        public int getHeight() {
            return 384;
        }

        @Override
        public int getMinY() {
            return -64;
        }
    }

    private static Vector3f[] boxFace(final Direction direction, final float[] from, final float[] to) {
        Vector3f min = new Vector3f(from[0], from[1], from[2]);
        Vector3f max = new Vector3f(to[0], to[1], to[2]);
        FaceInfo info = FaceInfo.fromFacing(direction);
        Vector3f[] positions = new Vector3f[4];
        for (int vertex = 0; vertex < 4; vertex++) {
            positions[vertex] = info.getVertexInfo(vertex).select(min, max);
        }
        return positions;
    }

    private static BakedQuad quad(final Direction direction, final Vector3f[] p) {
        BakedQuad.MaterialInfo material = new BakedQuad.MaterialInfo(null, null, null, null, null, -1, null, 0);
        return new BakedQuad(p[0], p[1], p[2], p[3], 0L, 0L, 0L, 0L, direction, material);
    }

    private static int[] unpack(final int coords) {
        return new int[] {coords & 0xFF, coords >> 16 & 0xFF};
    }

    private static JsonArray ints(final int... values) {
        JsonArray array = new JsonArray();
        for (int value : values) {
            array.add(value);
        }
        return array;
    }

    private static JsonObject lightCase(
        final String name,
        final Direction direction,
        final String own,
        final Vector3f[] positions,
        final World world,
        final Map<BlockState, Integer> index
    ) {
        BlockState ownState = world.getBlockState(BlockPos.ZERO);
        BakedQuad quad = quad(direction, positions);
        BlockModelLighter lighter = new BlockModelLighter();

        QuadInstance smooth = new QuadInstance();
        lighter.prepareQuadAmbientOcclusion(world, ownState, BlockPos.ZERO, quad, smooth);
        QuadInstance flatFree = new QuadInstance();
        lighter.prepareQuadFlat(world, ownState, BlockPos.ZERO, BlockModelLighter.CHECK_LIGHT, quad, flatFree);
        int culledCoords = lighter.getLightCoords(ownState, world, BlockPos.ZERO.relative(direction));

        JsonObject row = new JsonObject();
        row.addProperty("name", name);
        row.addProperty("direction", direction.getSerializedName());
        row.addProperty("own", own);
        JsonArray quadPositions = new JsonArray();
        for (Vector3f p : positions) {
            JsonArray xyz = new JsonArray();
            xyz.add(p.x());
            xyz.add(p.y());
            xyz.add(p.z());
            quadPositions.add(xyz);
        }
        row.add("positions", quadPositions);
        row.add("default_light", ints(world.fallback));
        JsonArray cells = new JsonArray();
        for (int y = -2; y <= 2; y++) {
            for (int z = -2; z <= 2; z++) {
                for (int x = -2; x <= 2; x++) {
                    BlockPos pos = new BlockPos(x, y, z);
                    if (pos.equals(BlockPos.ZERO)) {
                        continue;
                    }
                    BlockState state = world.getBlockState(pos);
                    int[] level = world.light.getOrDefault(pos, world.fallback);
                    boolean plain = state.isAir() && level == world.fallback;
                    if (!plain) {
                        cells.add(ints(x, y, z, index.get(state), level[0], level[1]));
                    }
                }
            }
        }
        row.add("cells", cells);
        int[] ownLight = world.light.getOrDefault(BlockPos.ZERO, world.fallback);
        row.add("own_light", ints(ownLight));

        JsonArray shade = new JsonArray();
        JsonArray light = new JsonArray();
        for (int vertex = 0; vertex < 4; vertex++) {
            shade.add(smooth.getColor(vertex) & 0xFF);
            light.add(ints(unpack(smooth.getLightCoords(vertex))));
        }
        row.add("smooth_shade", shade);
        row.add("smooth_light", light);
        row.addProperty("flat_shade", flatFree.getColor(0) & 0xFF);
        row.add("flat_light_without_cull_face", ints(unpack(flatFree.getLightCoords(0))));
        row.add("flat_light_with_cull_face", ints(unpack(culledCoords)));
        return row;
    }

    private static void smoothLight(final Path path) throws Exception {
        BlockState[] palette = new BlockState[PALETTE.length];
        Map<BlockState, Integer> index = new HashMap<>();
        for (int i = 0; i < PALETTE.length; i++) {
            palette[i] = parse(PALETTE[i]);
            index.put(palette[i], i);
        }
        Map<String, BlockState> owns = new HashMap<>();
        for (Shape shape : SHAPES) {
            BlockState state = parse(shape.own());
            owns.put(shape.own(), state);
            index.putIfAbsent(state, index.size());
        }
        List<BlockState> byIndex = new ArrayList<>(index.keySet());
        byIndex.sort((a, b) -> index.get(a) - index.get(b));

        Random random = new Random(0x5EED1E57L);
        List<JsonObject> rows = new ArrayList<>();
        int[][] fallbacks = {{0, 15}, {0, 0}, {7, 4}};
        for (Shape shape : SHAPES) {
            for (Direction direction : Direction.values()) {
                Vector3f[] positions = boxFace(direction, shape.from(), shape.to());
                for (int sample = 0; sample < 10; sample++) {
                    World world = new World(fallbacks[sample % fallbacks.length]);
                    world.states.put(BlockPos.ZERO, owns.get(shape.own()));
                    for (int y = -2; y <= 2; y++) {
                        for (int z = -2; z <= 2; z++) {
                            for (int x = -2; x <= 2; x++) {
                                BlockPos pos = new BlockPos(x, y, z);
                                if (pos.equals(BlockPos.ZERO)) {
                                    continue;
                                }
                                if (random.nextFloat() < 0.4F) {
                                    world.states.put(pos, palette[1 + random.nextInt(palette.length - 1)]);
                                }
                                float roll = random.nextFloat();
                                if (roll < 0.25F) {
                                    world.light.put(pos, new int[] {0, 0});
                                } else if (roll < 0.75F) {
                                    world.light.put(pos, new int[] {random.nextInt(16), random.nextInt(16)});
                                }
                            }
                        }
                    }
                    if (sample == 0) {
                        world.states.keySet().removeIf(pos -> !pos.equals(BlockPos.ZERO));
                        world.light.clear();
                    }
                    rows.add(lightCase(shape.name() + "/" + sample, direction, shape.own(), positions, world, index));
                }
            }
        }

        Direction[] diagonal = {Direction.NORTH, Direction.SOUTH, Direction.WEST, Direction.EAST};
        for (Direction direction : diagonal) {
            for (int sample = 0; sample < 6; sample++) {
                World world = new World(fallbacks[sample % fallbacks.length]);
                BlockState own = owns.get("minecraft:oak_fence");
                world.states.put(BlockPos.ZERO, own);
                for (int y = -2; y <= 2; y++) {
                    for (int z = -2; z <= 2; z++) {
                        for (int x = -2; x <= 2; x++) {
                            BlockPos pos = new BlockPos(x, y, z);
                            if (!pos.equals(BlockPos.ZERO) && random.nextFloat() < 0.4F) {
                                world.states.put(pos, palette[1 + random.nextInt(palette.length - 1)]);
                            }
                            if (!pos.equals(BlockPos.ZERO) && random.nextFloat() < 0.6F) {
                                world.light.put(pos, new int[] {random.nextInt(16), random.nextInt(16)});
                            }
                        }
                    }
                }
                Vector3f[] positions = {
                    new Vector3f(0.85F, 1, 0.15F),
                    new Vector3f(0.85F, 0, 0.15F),
                    new Vector3f(0.15F, 0, 0.85F),
                    new Vector3f(0.15F, 1, 0.85F),
                };
                rows.add(lightCase("diagonal/" + sample, direction, "minecraft:oak_fence", positions, world, index));
            }
        }

        StringBuilder text = new StringBuilder("{\n");
        text.append("  \"format\": ").append(FORMAT).append(",\n");
        text.append("  \"kind\": \"smooth_light\",\n");
        text.append("  \"game_version\": ").append(LINE.toJson(SharedConstants.getCurrentVersion().id())).append(",\n");
        text.append("  \"units\": {\"light\": \"[block, sky] in sixteenths of a level\", \"shade\": \"byte after the directional face shade of the overworld\", \"cell\": \"[x, y, z, palette index, block level, sky level]\"},\n");
        JsonArray names = new JsonArray();
        for (BlockState state : byIndex) {
            JsonObject entry = new JsonObject();
            entry.addProperty("state", describe(state));
            entry.addProperty("full_collision", state.isCollisionShapeFullBlock(EmptyBlockGetter.INSTANCE, BlockPos.ZERO));
            entry.addProperty("solid_render", state.isSolidRender());
            entry.addProperty("light_dampening", state.getLightDampening());
            entry.addProperty("light_emission", state.getLightEmission());
            entry.addProperty("emissive_rendering", state.emissiveRendering());
            entry.addProperty("occlusion_shade", state.getShadeBrightness(EmptyBlockGetter.INSTANCE, BlockPos.ZERO));
            names.add(entry);
        }
        text.append("  \"palette\": ").append(LINE.toJson(names)).append(",\n");
        text.append("  \"cases\": [\n");
        for (int i = 0; i < rows.size(); i++) {
            text.append("    ").append(LINE.toJson(rows.get(i))).append(i + 1 < rows.size() ? ",\n" : "\n");
        }
        text.append("  ]\n}\n");
        Files.writeString(path, text.toString(), StandardCharsets.UTF_8);
        System.out.println("wrote " + path + " (" + rows.size() + " cases)");
    }
}
