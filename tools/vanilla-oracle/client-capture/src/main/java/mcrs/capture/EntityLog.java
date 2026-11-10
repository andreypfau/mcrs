package mcrs.capture;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.datafixers.util.Pair;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.game.ClientboundAddEntityPacket;
import net.minecraft.network.protocol.game.ClientboundDamageEventPacket;
import net.minecraft.network.protocol.game.ClientboundEntityEventPacket;
import net.minecraft.network.protocol.game.ClientboundEntityPositionSyncPacket;
import net.minecraft.network.protocol.game.ClientboundHurtAnimationPacket;
import net.minecraft.network.protocol.game.ClientboundMoveEntityPacket;
import net.minecraft.network.protocol.game.ClientboundRemoveEntitiesPacket;
import net.minecraft.network.protocol.game.ClientboundRotateHeadPacket;
import net.minecraft.network.protocol.game.ClientboundSetEntityDataPacket;
import net.minecraft.network.protocol.game.ClientboundSetEntityMotionPacket;
import net.minecraft.network.protocol.game.ClientboundSetEquipmentPacket;
import net.minecraft.network.protocol.game.ClientboundTeleportEntityPacket;
import net.minecraft.network.protocol.game.ClientboundTickingStatePacket;
import net.minecraft.network.protocol.game.ClientboundTickingStepPacket;
import net.minecraft.network.protocol.game.VecDelta;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.PositionPath;
import net.minecraft.world.entity.PositionStep;
import net.minecraft.world.entity.Relative;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.phys.Vec3;

/// Records, on the client thread, every entity packet the client applies and the entity state after each entity update.
public final class EntityLog {
	private static boolean active;
	private static int updates;
	private static JsonArray packets = new JsonArray();
	private static JsonArray ticks = new JsonArray();
	private static final Map<Integer, Boolean> FROZEN = new HashMap<>();
	private static final Map<Integer, Boolean> INTERPOLATING = new HashMap<>();

	private EntityLog() {
	}

	public static void start() {
		active = true;
		updates = 0;
		packets = new JsonArray();
		ticks = new JsonArray();
	}

	public static void stop() {
		active = false;
	}

	public static JsonArray packets() {
		return packets;
	}

	public static JsonArray ticks() {
		return ticks;
	}

	public static void beforeEntityUpdate(ClientLevel level) {
		if (!active) {
			return;
		}
		FROZEN.clear();
		for (Entity entity : level.entitiesForRendering()) {
			FROZEN.put(entity.getId(), level.tickRateManager().isEntityFrozen(entity));
			INTERPOLATING.put(entity.getId(), entity.isInterpolating());
		}
	}

	public static void afterEntityUpdate(ClientLevel level) {
		if (!active) {
			return;
		}
		updates++;
		JsonArray rows = new JsonArray();
		for (Entity entity : level.entitiesForRendering()) {
			if (entity instanceof net.minecraft.world.entity.player.Player) {
				continue;
			}
			if (FROZEN.getOrDefault(entity.getId(), true)) {
				continue;
			}
			JsonObject e = new JsonObject();
			e.addProperty("entity", entity.getId());
			e.addProperty("age", entity.tickCount);
			e.add("position", vector(entity.position()));
			e.add("previous_position", vector(new Vec3(entity.xo, entity.yo, entity.zo)));
			e.addProperty("on_ground", entity.onGround());
			e.addProperty("path_before_update", INTERPOLATING.getOrDefault(entity.getId(), false));
			e.addProperty("path_after_update", entity.isInterpolating());
			e.addProperty("pitch", entity.getXRot());
			if (entity instanceof LivingEntity living) {
				e.addProperty("body_yaw", living.yBodyRot);
				e.addProperty("previous_body_yaw", living.yBodyRotO);
				e.addProperty("head_yaw", living.yHeadRot);
				e.addProperty("previous_head_yaw", living.yHeadRotO);
				e.addProperty("walk_position", living.walkAnimation.position());
				e.addProperty("walk_speed", living.walkAnimation.speed());
				e.addProperty("hurt_time", living.hurtTime);
				e.addProperty("death_time", living.deathTime);
			}
			rows.add(e);
		}
		if (!rows.isEmpty()) {
			JsonObject row = new JsonObject();
			row.addProperty("update", updates);
			row.add("entities", rows);
			ticks.add(row);
		}
	}

	public static void packet(ClientPacketListener listener, Packet<?> packet) {
		if (!active) {
			return;
		}
		JsonObject fields = describe(listener.getLevel(), packet);
		if (fields == null) {
			return;
		}
		JsonObject p = new JsonObject();
		p.addProperty("updates_before", updates);
		p.addProperty("packet", packet.type().id().toString());
		for (Map.Entry<String, com.google.gson.JsonElement> field : fields.entrySet()) {
			p.add(field.getKey(), field.getValue());
		}
		packets.add(p);
	}

	private static JsonObject describe(ClientLevel level, Packet<?> packet) {
		JsonObject f = new JsonObject();
		switch (packet) {
			case ClientboundAddEntityPacket add -> {
				f.addProperty("entity", add.getId());
				f.addProperty("uuid", add.getUUID().toString());
				f.addProperty("type", BuiltInRegistries.ENTITY_TYPE.getKey(add.getType()).toString());
				f.add("position", vector(new Vec3(add.getX(), add.getY(), add.getZ())));
				f.add("velocity", vector(add.getMovement()));
				f.addProperty("yaw", add.getYRot());
				f.addProperty("pitch", add.getXRot());
				f.addProperty("head_yaw", add.getYHeadRot());
				f.addProperty("data", add.getData());
			}
			case ClientboundRemoveEntitiesPacket remove -> {
				JsonArray ids = new JsonArray();
				remove.entityIds().forEach(ids::add);
				f.add("entities", ids);
			}
			case ClientboundMoveEntityPacket move -> {
				Entity entity = move.getEntity(level);
				if (entity == null) {
					return null;
				}
				f.addProperty("entity", entity.getId());
				if (move.hasPosition()) {
					VecDelta delta = move.getPositionDelta();
					f.add("delta", delta(delta));
					f.add("path", path(delta.decode(entity.getPositionCodec())));
				}
				if (move.hasRotation()) {
					f.addProperty("yaw", move.getYRot());
					f.addProperty("pitch", move.getXRot());
				}
				f.addProperty("on_ground", move.isOnGround());
			}
			case ClientboundEntityPositionSyncPacket sync -> {
				f.addProperty("entity", sync.id());
				f.add("path", path(sync.position()));
				f.addProperty("yaw", sync.yRot());
				f.addProperty("pitch", sync.xRot());
				f.addProperty("on_ground", sync.onGround());
			}
			case ClientboundTeleportEntityPacket teleport -> {
				f.addProperty("entity", teleport.id());
				f.add("position", vector(teleport.change().position()));
				f.add("velocity", vector(teleport.change().deltaMovement()));
				f.addProperty("yaw", teleport.change().yRot());
				f.addProperty("pitch", teleport.change().xRot());
				f.add("relative", relative(teleport.relatives()));
				f.addProperty("on_ground", teleport.onGround());
			}
			case ClientboundRotateHeadPacket head -> {
				Entity entity = head.getEntity(level);
				if (entity == null) {
					return null;
				}
				f.addProperty("entity", entity.getId());
				f.addProperty("head_yaw", head.getYHeadRot());
			}
			case ClientboundSetEntityDataPacket data -> {
				f.addProperty("entity", data.id());
				JsonArray items = new JsonArray();
				for (SynchedEntityData.DataValue<?> value : data.packedItems()) {
					JsonObject item = new JsonObject();
					item.addProperty("index", value.id());
					value(item, value.value());
					items.add(item);
				}
				f.add("values", items);
			}
			case ClientboundSetEquipmentPacket equipment -> {
				f.addProperty("entity", equipment.getEntity());
				JsonArray slots = new JsonArray();
				for (Pair<EquipmentSlot, ItemStack> slot : equipment.getSlots()) {
					JsonObject s = new JsonObject();
					s.addProperty("slot", slot.getFirst().getSerializedName());
					s.addProperty("item", BuiltInRegistries.ITEM.getKey(slot.getSecond().getItem()).toString());
					s.addProperty("count", slot.getSecond().getCount());
					slots.add(s);
				}
				f.add("slots", slots);
			}
			case ClientboundEntityEventPacket event -> {
				Entity entity = event.getEntity(level);
				if (entity == null) {
					return null;
				}
				f.addProperty("entity", entity.getId());
				f.addProperty("event", event.getEventId());
			}
			case ClientboundDamageEventPacket damage -> {
				f.addProperty("entity", damage.entityId());
				f.addProperty("damage_type", damage.sourceType().getRegisteredName());
				f.addProperty("cause", damage.sourceCauseId());
				f.addProperty("direct_cause", damage.sourceDirectId());
				damage.sourcePosition().ifPresent(position -> f.add("source_position", vector(position)));
			}
			case ClientboundHurtAnimationPacket hurt -> {
				f.addProperty("entity", hurt.id());
				f.addProperty("yaw", hurt.yaw());
			}
			case ClientboundSetEntityMotionPacket motion -> {
				f.addProperty("entity", motion.id());
				f.add("velocity", vector(motion.movement()));
			}
			case ClientboundTickingStatePacket state -> {
				f.addProperty("tick_rate", state.tickRate());
				f.addProperty("frozen", state.isFrozen());
			}
			case ClientboundTickingStepPacket step -> f.addProperty("steps", step.tickSteps());
			default -> {
				return null;
			}
		}
		return f;
	}

	private static JsonObject delta(VecDelta delta) {
		JsonObject d = new JsonObject();
		switch (delta) {
			case VecDelta.Linear linear -> {
				d.addProperty("form", "single");
				d.add("units", units(linear.xa(), linear.ya(), linear.za()));
			}
			case VecDelta.Stepped stepped -> {
				d.addProperty("form", "stepped");
				JsonArray steps = new JsonArray();
				for (VecDelta.Stepped.DeltaStep step : stepped.steps()) {
					JsonObject s = new JsonObject();
					s.add("units", units(step.xa(), step.ya(), step.za()));
					s.addProperty("ticks", step.ticks());
					steps.add(s);
				}
				d.add("steps", steps);
				d.addProperty("start_immediately", stepped.startImmediately());
			}
		}
		return d;
	}

	private static JsonObject path(PositionPath path) {
		JsonObject p = new JsonObject();
		switch (path) {
			case PositionPath.Linear linear -> {
				p.addProperty("form", "single");
				p.add("position", vector(linear.endPosition()));
			}
			case PositionPath.Stepped stepped -> {
				p.addProperty("form", "stepped");
				JsonArray steps = new JsonArray();
				for (PositionStep step : stepped.steps()) {
					JsonObject s = new JsonObject();
					s.add("position", vector(step.position()));
					s.addProperty("ticks", step.tickOffset());
					steps.add(s);
				}
				p.add("steps", steps);
				p.addProperty("start_immediately", stepped.startImmediately());
			}
		}
		return p;
	}

	private static JsonArray relative(Set<Relative> relatives) {
		JsonArray names = new JsonArray();
		for (Relative r : Relative.values()) {
			if (!relatives.contains(r)) {
				continue;
			}
			names.add(switch (r.ordinal()) {
				case 0 -> "x";
				case 1 -> "y";
				case 2 -> "z";
				case 3 -> "yaw";
				case 4 -> "pitch";
				case 5 -> "velocity_x";
				case 6 -> "velocity_y";
				case 7 -> "velocity_z";
				default -> "rotate_velocity";
			});
		}
		return names;
	}

	private static void value(JsonObject item, Object value) {
		switch (value) {
			case Number n -> item.addProperty("value", n);
			case Boolean b -> item.addProperty("value", b);
			case String s -> item.addProperty("value", s);
			case Holder<?> h -> item.addProperty("value", h.getRegisteredName());
			default -> item.addProperty("kind", "other");
		}
	}

	private static JsonArray units(short x, short y, short z) {
		JsonArray a = new JsonArray();
		a.add(x);
		a.add(y);
		a.add(z);
		return a;
	}

	private static JsonArray vector(Vec3 v) {
		JsonArray array = new JsonArray();
		array.add(v.x);
		array.add(v.y);
		array.add(v.z);
		return array;
	}
}
