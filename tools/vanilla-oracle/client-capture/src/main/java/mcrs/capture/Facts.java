package mcrs.capture;

import com.google.gson.JsonObject;
import net.minecraft.core.Holder;
import net.minecraft.world.entity.AnimationState;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.animal.frog.Frog;
import net.minecraft.world.entity.animal.pig.Pig;

/// The type-specific render inputs of a frame record.
final class Facts {
	private Facts() {}

	static void add(Entity entity, JsonObject out) {
		if (entity instanceof Mob mob) {
			out.addProperty("saddled", mob.isSaddled());
		}
		if (entity instanceof Pig pig) {
			out.addProperty("variant", key(pig.getVariant()));
		}
		if (entity instanceof Frog frog) {
			out.addProperty("variant", key(frog.getVariant()));
			out.add("croak", animation(frog.croakAnimationState, frog.tickCount));
			out.add("tongue", animation(frog.tongueAnimationState, frog.tickCount));
		}
	}

	private static String key(Holder<?> holder) {
		return holder.unwrapKey().map(k -> k.identifier().toString()).orElse("");
	}

	private static JsonObject animation(AnimationState state, int age) {
		JsonObject out = new JsonObject();
		out.addProperty("started", state.isStarted());
		out.addProperty("elapsed_ticks", state.isStarted() ? state.getTimeInMillis(age) / 50.0F : 0.0F);
		return out;
	}
}
