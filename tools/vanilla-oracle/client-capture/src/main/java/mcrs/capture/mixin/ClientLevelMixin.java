package mcrs.capture.mixin;

import mcrs.capture.EntityLog;
import net.minecraft.client.multiplayer.ClientLevel;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(ClientLevel.class)
abstract class ClientLevelMixin {
	@Inject(method = "tickEntities", at = @At("HEAD"))
	private void mcrs$beforeEntities(CallbackInfo ci) {
		EntityLog.beforeEntityUpdate((ClientLevel) (Object) this);
	}

	@Inject(method = "tickEntities", at = @At("RETURN"))
	private void mcrs$afterEntities(CallbackInfo ci) {
		EntityLog.afterEntityUpdate((ClientLevel) (Object) this);
	}
}
