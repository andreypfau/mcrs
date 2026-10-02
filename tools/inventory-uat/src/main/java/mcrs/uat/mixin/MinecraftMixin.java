package mcrs.uat.mixin;

import mcrs.uat.InventoryUat;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

@Mixin(Minecraft.class)
public class MinecraftMixin {
    @Inject(method = "tick", at = @At("RETURN"))
    private void uat$tick(CallbackInfo ci) {
        InventoryUat.onClientTick(Minecraft.getInstance());
    }
}
