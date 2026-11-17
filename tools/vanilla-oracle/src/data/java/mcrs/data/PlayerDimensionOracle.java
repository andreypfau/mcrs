package mcrs.data;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import net.minecraft.SharedConstants;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.RemotePlayer;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.entity.EntityDimensions;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.entity.ai.attributes.AttributeMap;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.entity.ai.attributes.DefaultAttributes;

public final class PlayerDimensionOracle {
    private static final Gson COMPACT = new GsonBuilder().disableHtmlEscaping().create();
    private static final double[] SCALES = {1.0, 2.0};

    private PlayerDimensionOracle() {}

    private static final class ScaledPlayer extends RemotePlayer {
        private AttributeMap map;

        private ScaledPlayer() {
            super((ClientLevel) null, null);
        }

        @Override
        public AttributeMap getAttributes() {
            return map;
        }
    }

    public static void main(final String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Path out = Path.of(args[0]);
        Files.createDirectories(out);
        StringBuilder text = new StringBuilder();
        text.append("{\n");
        text.append("  \"game_version\": ").append(COMPACT.toJson(SharedConstants.getCurrentVersion().id())).append(",\n");
        text.append("  \"entity_type\": ").append(COMPACT.toJson(BuiltInRegistries.ENTITY_TYPE.getKey(EntityTypes.PLAYER).toString())).append(",\n");
        text.append("  \"units\": \"blocks; scale is the base value of minecraft:scale; width, height and eye_height are f32\",\n");
        text.append("  \"columns\": [\"pose_id\", \"pose\", \"scale\", \"width\", \"height\", \"eye_height\", \"fixed\"],\n");
        text.append("  \"rows\": [\n");
        Pose[] poses = Pose.values();
        for (int s = 0; s < SCALES.length; s++) {
            ScaledPlayer player = allocate();
            player.map = new AttributeMap(DefaultAttributes.getSupplier(EntityTypes.PLAYER));
            player.map.getInstance(Attributes.SCALE).setBaseValue(SCALES[s]);
            for (int p = 0; p < poses.length; p++) {
                Pose pose = poses[p];
                EntityDimensions size = player.getDimensions(pose);
                if (size.eyeHeight() != player.getEyeHeight(pose)) {
                    throw new IllegalStateException("eye height disagrees for " + pose.getSerializedName());
                }
                JsonArray row = new JsonArray();
                row.add(pose.id());
                row.add(pose.getSerializedName());
                row.add(SCALES[s]);
                row.add(size.width());
                row.add(size.height());
                row.add(size.eyeHeight());
                row.add(size.fixed());
                boolean last = s + 1 == SCALES.length && p + 1 == poses.length;
                text.append("    ").append(COMPACT.toJson(row)).append(last ? "\n" : ",\n");
            }
        }
        text.append("  ]\n}\n");
        Files.writeString(out.resolve("player_dimensions.json"), text.toString(), StandardCharsets.UTF_8);
    }

    private static ScaledPlayer allocate() {
        try {
            java.lang.reflect.Field theUnsafe = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
            theUnsafe.setAccessible(true);
            sun.misc.Unsafe unsafe = (sun.misc.Unsafe) theUnsafe.get(null);
            return (ScaledPlayer) unsafe.allocateInstance(ScaledPlayer.class);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException("cannot allocate a player", e);
        }
    }
}
