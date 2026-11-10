package mcrs.capture;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.UUID;
import net.fabricmc.fabric.api.client.gametest.v1.FabricClientGameTest;
import net.fabricmc.fabric.api.client.gametest.v1.context.ClientGameTestContext;
import net.fabricmc.fabric.api.client.gametest.v1.context.TestSingleplayerContext;
import net.fabricmc.fabric.api.client.gametest.v1.screenshot.TestScreenshotOptions;
import net.minecraft.client.Camera;
import net.minecraft.client.Minecraft;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.phys.Vec3;

/// Runs every scene of `tools/scenes` in a vanilla client and writes its frames and records.
public final class SceneCapture implements FabricClientGameTest {
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create();
	private static final String PLAYER = "Capture";
	private static final int SETTLE_TICKS = 10;

	private record Pose3(double x, double y, double z, float yaw, float pitch) {
		static Pose3 of(JsonObject camera) {
			JsonArray p = camera.getAsJsonArray("position");
			return new Pose3(p.get(0).getAsDouble(), p.get(1).getAsDouble(), p.get(2).getAsDouble(),
				camera.get("yaw").getAsFloat(), camera.get("pitch").getAsFloat());
		}
	}

	private record Shot(String name, Pose3 camera, float partialTick) {}

	@Override
	public void runTest(ClientGameTestContext context) {
		Path scenes = Path.of(System.getProperty("mcrs.capture.scenes"));
		Path out = Path.of(System.getProperty("mcrs.capture.out"));
		String filter = System.getProperty("mcrs.capture.filter", "");
		try {
			Files.createDirectories(out);
			List<Path> files;
			try (var stream = Files.list(scenes)) {
				files = stream.filter(p -> p.toString().endsWith(".json")).sorted().toList();
			}
			for (Path file : files) {
				JsonObject scene = JsonParser.parseString(Files.readString(file)).getAsJsonObject();
				String name = scene.get("name").getAsString();
				if (!filter.isEmpty() && !filter.equals(name)) {
					continue;
				}
				runScene(context, scene, out);
			}
		} catch (IOException e) {
			throw new RuntimeException(e);
		}
	}

	private static void runScene(ClientGameTestContext context, JsonObject scene, Path out) throws IOException {
		String name = scene.get("name").getAsString();
		JsonArray size = scene.getAsJsonArray("resolution");
		int width = size.get(0).getAsInt();
		int height = size.get(1).getAsInt();
		context.restoreDefaultGameOptions();
		applyOptions(context, scene.getAsJsonObject("options"));
		context.runOnClient(client -> {
			if (!client.gui.hud.isHidden()) {
				client.gui.hud.toggle();
			}
		});

		List<Shot> shots = new ArrayList<>();
		JsonArray frames = new JsonArray();
		try (TestSingleplayerContext world = context.worldBuilder().create()) {
			settle(context, world);
			world.getServer().runCommand("gamemode spectator " + PLAYER);
			context.runOnClient(client -> EntityLog.start());
			for (JsonElement element : scene.getAsJsonArray("script")) {
				JsonObject step = element.getAsJsonObject();
				if (step.has("command")) {
					world.getServer().runCommand(step.get("command").getAsString());
				} else if (step.has("commands")) {
					for (JsonElement command : step.getAsJsonArray("commands")) {
						world.getServer().runCommand(command.getAsString());
					}
				} else if (step.has("wait")) {
					context.waitTicks(step.get("wait").getAsInt());
				} else if (step.has("step")) {
					stepTicks(context, world, step.get("step").getAsInt());
				} else if (step.has("pose")) {
					setPose(world, step.getAsJsonObject("pose"));
					context.waitTicks(SETTLE_TICKS);
				} else if (step.has("camera")) {
					placeCamera(context, world, Pose3.of(step.getAsJsonObject("camera")));
				} else if (step.has("capture")) {
					float pt = step.has("partial_tick") ? step.get("partial_tick").getAsFloat() : 1.0F;
					Pose3 camera = Pose3.of(step.getAsJsonObject("at"));
					placeCamera(context, world, camera);
					String frame = step.get("capture").getAsString();
					frames.add(capture(context, out, name + "." + frame, width, height, pt));
					shots.add(new Shot(frame, camera, pt));
				} else {
					throw new IllegalArgumentException(name + ": unknown step " + step);
				}
			}
			context.runOnClient(client -> EntityLog.stop());
			world.getServer().runCommand("tp @e[type=!player] 0 -60 5000");
			context.waitTicks(SETTLE_TICKS * 2);
			for (Shot shot : shots) {
				placeCamera(context, world, shot.camera());
				frames.add(capture(context, out, name + "." + shot.name() + ".background", width, height, shot.partialTick()));
			}
		}

		JsonObject records = new JsonObject();
		records.addProperty("scene", name);
		records.addProperty("version", System.getProperty("mcrs.capture.version"));
		records.add("frames", frames);
		Files.writeString(out.resolve(name + ".records.json"), GSON.toJson(records) + "\n");

		JsonObject packets = new JsonObject();
		packets.addProperty("scene", name);
		packets.addProperty("version", System.getProperty("mcrs.capture.version"));
		packets.add("packets", EntityLog.packets());
		Files.writeString(out.resolve(name + ".packets.json"), GSON.toJson(packets) + "\n");

		JsonObject ticks = new JsonObject();
		ticks.addProperty("scene", name);
		ticks.addProperty("version", System.getProperty("mcrs.capture.version"));
		ticks.add("updates", EntityLog.ticks());
		Files.writeString(out.resolve(name + ".entity_ticks.json"), GSON.toJson(ticks) + "\n");
	}

	private static void applyOptions(ClientGameTestContext context, JsonObject options) throws IOException {
		context.<IOException>runOnClient(client -> {
			client.options.save();
			Path file = client.gameDirectory.toPath().resolve("options.txt");
			Map<String, String> lines = new LinkedHashMap<>();
			for (String line : Files.readAllLines(file)) {
				int colon = line.indexOf(':');
				if (colon > 0) {
					lines.put(line.substring(0, colon), line.substring(colon + 1));
				}
			}
			for (Map.Entry<String, JsonElement> option : options.entrySet()) {
				lines.put(option.getKey(), option.getValue().getAsString());
			}
			List<String> text = new ArrayList<>();
			lines.forEach((key, value) -> text.add(key + ":" + value));
			Files.write(file, text);
			client.options.load();
			client.options.save();
		});
	}

	private static void settle(ClientGameTestContext context, TestSingleplayerContext world) {
		world.getConnection().waitForChunksDownload();
		context.waitTicks(SETTLE_TICKS * 4);
	}

	private static void stepTicks(ClientGameTestContext context, TestSingleplayerContext world, int ticks) {
		world.getServer().runCommand("tick step " + ticks);
		world.getServer().waitFor(server -> !server.tickRateManager().isSteppingForward());
		context.waitTicks(SETTLE_TICKS);
	}

	private static void setPose(TestSingleplayerContext world, JsonObject pose) {
		UUID id = UUID.fromString(pose.get("entity").getAsString());
		Pose value = Pose.valueOf(pose.get("pose").getAsString().toUpperCase(Locale.ROOT));
		world.getServer().runOnServer(server -> {
			for (ServerLevel level : server.getAllLevels()) {
				Entity entity = level.getEntity(id);
				if (entity != null) {
					entity.setPose(value);
					return;
				}
			}
			throw new IllegalStateException("no entity " + id);
		});
	}

	private static void placeCamera(ClientGameTestContext context, TestSingleplayerContext world, Pose3 camera) {
		double eye = world.getServer().computeOnServer(server -> (double) server.getPlayerList().getPlayerByName(PLAYER).getEyeHeight());
		world.getServer().runCommand(String.format(Locale.ROOT, "tp %s %.4f %.4f %.4f %.4f %.4f",
			PLAYER, camera.x(), camera.y() - eye, camera.z(), camera.yaw(), camera.pitch()));
		context.waitTicks(SETTLE_TICKS);
		settle(context, world);
	}

	private static JsonObject capture(ClientGameTestContext context, Path out, String file, int width, int height, float pt) {
		JsonObject frame = context.computeOnClient(client -> record(client, file, pt));
		context.takeScreenshot(TestScreenshotOptions.of(file)
			.disableCounterPrefix()
			.withSize(width, height)
			.withDeltaTicks(pt)
			.withDestinationDir(out));
		return frame;
	}

	private static JsonObject record(Minecraft client, String file, float pt) {
		JsonObject frame = new JsonObject();
		frame.addProperty("image", file + ".png");
		Camera camera = client.gameRenderer.mainCamera();
		JsonObject view = new JsonObject();
		view.add("position", vector(camera.position()));
		view.addProperty("yaw", camera.yRot());
		view.addProperty("pitch", camera.xRot());
		view.addProperty("fov", camera.getFov());
		view.addProperty("partial_tick", pt);
		frame.add("camera", view);

		JsonArray entities = new JsonArray();
		for (Entity entity : client.level.entitiesForRendering()) {
			if (entity == client.player) {
				continue;
			}
			boolean frozen = client.level.tickRateManager().isEntityFrozen(entity);
			float t = frozen ? 1.0F : pt;
			JsonObject e = new JsonObject();
			e.addProperty("id", entity.getUUID().toString());
			e.addProperty("type", BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString());
			e.addProperty("frozen", frozen);
			e.addProperty("partial_tick", t);
			e.add("position", vector(entity.getPosition(t)));
			e.addProperty("age", entity.tickCount);
			e.addProperty("pose", entity.getPose().name().toLowerCase(Locale.ROOT));
			e.addProperty("invisible", entity.isInvisible());
			e.addProperty("head_pitch", entity.getXRot(t));
			if (entity instanceof LivingEntity living) {
				e.addProperty("body_yaw", lerpDegrees(t, living.yBodyRotO, living.yBodyRot));
				e.addProperty("head_yaw", lerpDegrees(t, living.yHeadRotO, living.yHeadRot));
				e.addProperty("walk_position", living.walkAnimation.position(t));
				e.addProperty("walk_speed", living.walkAnimation.speed(t));
				e.addProperty("hurt_time", living.hurtTime);
				e.addProperty("death_time", living.deathTime);
				e.addProperty("baby", living.isBaby());
			}
			Facts.add(entity, entity.tickCount + t, e);
			entities.add(e);
		}
		frame.add("entities", entities);
		return frame;
	}

	private static float lerpDegrees(float t, float from, float to) {
		float delta = to - from;
		while (delta < -180.0F) {
			delta += 360.0F;
		}
		while (delta >= 180.0F) {
			delta -= 360.0F;
		}
		return from + t * delta;
	}

	private static JsonArray vector(Vec3 v) {
		JsonArray array = new JsonArray();
		array.add(v.x);
		array.add(v.y);
		array.add(v.z);
		return array;
	}
}
